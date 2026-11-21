use super::workflow::{self, Override, Target};
use super::{
    background, background_job, checked_repo_path, err, git, git_startup_fetch, gitflow, State,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
mod autostart;

const SELECTED_KEY: &str = "launch-fetch-selected-v1";

#[derive(Deserialize)]
pub struct RepoFetchChange {
    target: Target,
    mode: Override,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FetchScope {
    #[default]
    Pinned,
    All,
    Selected,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct FetchSettings {
    enabled: bool,
    scope: FetchScope,
}

impl FetchSettings {
    fn read(db: &Connection) -> Result<Self, String> {
        let get = |key: &str| -> Result<Option<String>, String> {
            db.query_row("SELECT value FROM preferences WHERE key=?1", [key], |row| {
                row.get(0)
            })
            .optional()
            .map_err(err)
        };
        let enabled = match get("launch-fetch-enabled")?.as_deref() {
            None | Some("false") => false,
            Some("true") => true,
            _ => return Err("The saved launch fetch setting is unsupported.".into()),
        };
        let scope = match get("launch-fetch-scope")?.as_deref() {
            None | Some("pinned") => FetchScope::Pinned,
            Some("all") => FetchScope::All,
            _ => return Err("The saved launch fetch scope is unsupported.".into()),
        };
        if !enabled && scope == FetchScope::Pinned {
            if let Some(selected) = get(SELECTED_KEY)? {
                let enabled = selected.parse().map_err(err)?;
                return Ok(Self {
                    enabled,
                    scope: FetchScope::Selected,
                });
            }
        }
        Ok(Self { enabled, scope })
    }

    fn write(self, db: &mut Connection, changes: &[RepoFetchChange]) -> Result<Self, String> {
        let tx = db.transaction().map_err(err)?;
        let selected = self.scope == FetchScope::Selected;
        tx.execute("DELETE FROM preferences WHERE key=?1", [SELECTED_KEY])
            .map_err(err)?;
        if selected {
            tx.execute(
                "INSERT INTO preferences(key,value) VALUES(?1,?2)",
                params![SELECTED_KEY, self.enabled.to_string()],
            )
            .map_err(err)?;
        }
        for (key, value) in [
            (
                "launch-fetch-enabled",
                if self.enabled && !selected {
                    "true"
                } else {
                    "false"
                },
            ),
            (
                "launch-fetch-scope",
                match self.scope {
                    FetchScope::Pinned | FetchScope::Selected => "pinned",
                    FetchScope::All => "all",
                },
            ),
        ] {
            tx.execute("INSERT INTO preferences(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,value]).map_err(err)?;
        }
        for change in changes {
            workflow::set_fetch_mode(&tx, &change.target, change.mode)?;
        }
        tx.commit().map_err(err)?;
        Ok(self)
    }
}

#[tauri::command]
pub async fn fetch_settings(state: tauri::State<'_, State>) -> Result<FetchSettings, String> {
    background(state, |state| {
        FetchSettings::read(&*state.database.lock().map_err(err)?)
    })
    .await
}

#[tauri::command]
pub async fn save_fetch_settings(
    input: FetchSettings,
    changes: Option<Vec<RepoFetchChange>>,
    state: tauri::State<'_, State>,
) -> Result<FetchSettings, String> {
    background(state, move |state| {
        input.write(
            &mut *state.database.lock().map_err(err)?,
            changes.as_deref().unwrap_or_default(),
        )
    })
    .await
}

#[tauri::command]
pub async fn save_repo_fetch_mode(
    target: Target,
    mode: Override,
    state: tauri::State<'_, State>,
) -> Result<(), String> {
    background(state, move |state| {
        let mut db = state.database.lock().map_err(err)?;
        let tx = db.transaction().map_err(err)?;
        workflow::set_fetch_mode(&tx, &target, mode)?;
        tx.commit().map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn launch_fetch(
    id: i64,
    batch: Option<u64>,
    state: tauri::State<'_, State>,
) -> Result<bool, String> {
    background_job(state, move |state| {
        let path = {
            let db = state.database.lock().map_err(err)?;
            let settings = FetchSettings::read(&db)?;
            let pinned = db.query_row(
                "SELECT pinned FROM repos WHERE id=?1", [id], |row| row.get::<_, bool>(0),
            ).optional().map_err(err)?.unwrap_or(false);
            if !settings.enabled || !super::workflow::fetch_mode(&db,id)?.resolve(settings.scope == FetchScope::All || (settings.scope == FetchScope::Pinned && pinned)) {
                return Ok(false);
            }
            checked_repo_path(&db, id)?
        };
        if let Some(batch)=batch {
            use std::{collections::{BTreeMap,HashSet},path::PathBuf,sync::{LazyLock,Mutex}};
            type Batches = BTreeMap<u64,HashSet<PathBuf>>;
            static SEEN: LazyLock<Mutex<Batches>>=LazyLock::new(Default::default);
            let common=gitflow::common_directory(&path)?;
            let mut seen=SEEN.lock().map_err(err)?;
            if !seen.entry(batch).or_default().insert(common) {return Ok(false);}
            while seen.len()>8 {seen.pop_first();}
        }
        let lock = gitflow::repository_lock(&path)?;
        let _guard = lock.try_lock().map_err(|_| "This repository is busy. Use Fetch when the current operation finishes.")?;
        let remotes = git(&path, &["remote"])?;
        if remotes.is_empty() { return Ok(false); }
        // Respect custom fetch configuration, but only update remote-tracking refs
        // automatically. Unusual setups can still use an explicit manual fetch.
        for remote in remotes.lines() {
            let refspecs = git(&path, &["config", "--get-all", &format!("remote.{remote}.fetch")])
                .map_err(|_| "Startup fetch needs a configured remote-tracking refspec. Use manual Fetch for this repository.")?;
            let destination = format!("refs/remotes/{remote}/");
            if !refspecs.lines().all(|spec| safe_fetch_refspec(spec, &destination)) {
                return Err("Startup fetch was skipped because this repository has a custom fetch destination. Use manual Fetch.".into());
            }
        }
        git_startup_fetch(&path)?;
        if let Ok(db)=state.database.lock() {let _=super::workflow::fetched(&db,id,&path);}
        Ok(true)
    },true).await
}

fn safe_fetch_refspec(spec: &str, destination: &str) -> bool {
    if let Some(source) = spec.strip_prefix('^') {
        return source.starts_with("refs/heads/") && !source.contains(':');
    }
    spec.trim_start_matches('+')
        .split_once(':')
        .is_some_and(|(source, target)| {
            source.starts_with("refs/heads/") && target.starts_with(destination)
        })
}

#[tauri::command]
pub fn autostart_status(app: tauri::AppHandle) -> Result<bool, String> {
    autostart::status(&app)
}

#[tauri::command]
pub fn set_autostart(enabled: bool, app: tauri::AppHandle) -> Result<bool, String> {
    autostart::set(&app, enabled)?;
    autostart::status(&app)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_workspace_writes_preserve_launch_preferences() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE preferences(key TEXT PRIMARY KEY,value TEXT NOT NULL); INSERT INTO preferences VALUES('future-key','keep');").unwrap();
        assert_eq!(FetchSettings::read(&db).unwrap(), FetchSettings::default());
        let enabled_all = FetchSettings {
            enabled: true,
            scope: FetchScope::All,
        };
        enabled_all.write(&mut db, &[]).unwrap();
        db.execute_batch("INSERT INTO preferences VALUES('workspace-setup-complete','true') ON CONFLICT(key) DO UPDATE SET value=excluded.value;").unwrap();
        assert_eq!(FetchSettings::read(&db).unwrap(), enabled_all);
        assert_eq!(
            db.query_row(
                "SELECT value FROM preferences WHERE key='future-key'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "keep"
        );
    }

    #[test]
    fn selected_scope_round_trips_and_disables_fetch_for_older_builds() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = super::super::initialize(&dir.path().join("fixture.sqlite")).unwrap();
        db.execute_batch(
            "INSERT INTO repos(id,path,name) VALUES(1,'/one','One');
             INSERT INTO repos(id,path,name) VALUES(2,'/two','Two');",
        )
        .unwrap();
        let selected = FetchSettings {
            enabled: true,
            scope: FetchScope::Selected,
        };
        let change = RepoFetchChange {
            target: Target {
                id: 1,
                path: "/one".into(),
                registration: workflow::registration(&db, 1).unwrap(),
            },
            mode: Override::On,
        };
        let invalid = RepoFetchChange {
            target: Target {
                id: 2,
                path: "/old-path".into(),
                registration: None,
            },
            mode: Override::Off,
        };
        assert!(selected
            .write(
                &mut db,
                &[
                    RepoFetchChange {
                        target: change.target.clone(),
                        mode: change.mode
                    },
                    invalid
                ]
            )
            .is_err());
        assert_eq!(FetchSettings::read(&db).unwrap(), FetchSettings::default());
        assert_eq!(workflow::fetch_mode(&db, 1).unwrap(), Override::Inherit);

        selected.write(&mut db, &[change]).unwrap();
        assert_eq!(FetchSettings::read(&db).unwrap(), selected);
        assert_eq!(workflow::fetch_mode(&db, 1).unwrap(), Override::On);
        let legacy_enabled: String = db
            .query_row(
                "SELECT value FROM preferences WHERE key='launch-fetch-enabled'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(legacy_enabled, "false");
        FetchSettings::default().write(&mut db, &[]).unwrap();
        assert_eq!(FetchSettings::read(&db).unwrap(), FetchSettings::default());
    }

    #[test]
    fn automatic_fetch_refspecs_are_scoped_to_remote_refs() {
        let destination = "refs/remotes/origin/";
        assert!(safe_fetch_refspec(
            "+refs/heads/*:refs/remotes/origin/*",
            destination
        ));
        assert!(safe_fetch_refspec("^refs/heads/archive/*", destination));
        assert!(!safe_fetch_refspec(
            "+refs/heads/*:refs/heads/*",
            destination
        ));
        assert!(!safe_fetch_refspec("refs/tags/*:refs/tags/*", destination));
        assert!(!safe_fetch_refspec(
            "refs/heads/main:refs/remotes/original/main",
            destination
        ));
    }
}
