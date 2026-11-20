use super::{
    background, background_job, checked_repo_path, err, git, git_startup_fetch, gitflow, State,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
mod autostart;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FetchScope {
    #[default]
    Pinned,
    All,
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
        Ok(Self { enabled, scope })
    }

    fn write(self, db: &mut Connection) -> Result<Self, String> {
        let tx = db.transaction().map_err(err)?;
        for (key, value) in [
            (
                "launch-fetch-enabled",
                if self.enabled { "true" } else { "false" },
            ),
            (
                "launch-fetch-scope",
                match self.scope {
                    FetchScope::Pinned => "pinned",
                    FetchScope::All => "all",
                },
            ),
        ] {
            tx.execute("INSERT INTO preferences(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,value]).map_err(err)?;
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
    state: tauri::State<'_, State>,
) -> Result<FetchSettings, String> {
    background(state, move |state| {
        input.write(&mut *state.database.lock().map_err(err)?)
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
            if !settings.enabled || !super::workflow::fetch_mode(&db,id)?.resolve(settings.scope==FetchScope::All || pinned) {
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
        enabled_all.write(&mut db).unwrap();
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
