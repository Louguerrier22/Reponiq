use super::{background, branch_name, err, now, repo_path, State};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

pub(super) fn initialize(db: &Connection) -> Result<(), String> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS workflow_registrations(
            repo_id INTEGER PRIMARY KEY REFERENCES repos(id) ON DELETE CASCADE,
            token TEXT NOT NULL UNIQUE
        );
        CREATE TRIGGER IF NOT EXISTS workflow_register_repo AFTER INSERT ON repos BEGIN
            INSERT INTO workflow_registrations VALUES(NEW.id,lower(hex(randomblob(16))));
        END;
        INSERT OR IGNORE INTO workflow_registrations SELECT id,lower(hex(randomblob(16))) FROM repos;
        CREATE TABLE IF NOT EXISTS repo_workflow(
            repo_id INTEGER PRIMARY KEY REFERENCES repos(id) ON DELETE CASCADE,
            fetch_mode TEXT NOT NULL DEFAULT 'inherit',
            warn_default TEXT NOT NULL DEFAULT 'inherit',
            editor TEXT, review_paths TEXT NOT NULL DEFAULT '[]',
            preferred_base TEXT, pr_login TEXT, pin_position INTEGER,
            opened_at INTEGER, fetched_at INTEGER
        );
        CREATE TABLE IF NOT EXISTS workflow_branches(
            repo_id INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
            branch TEXT NOT NULL, visited_at INTEGER NOT NULL,
            PRIMARY KEY(repo_id,branch)
        );
        CREATE TABLE IF NOT EXISTS workflow_drafts(
            repo_id INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
            branch TEXT NOT NULL, account TEXT NOT NULL, base TEXT NOT NULL,
            title TEXT NOT NULL, body TEXT NOT NULL, updated_at INTEGER NOT NULL,
            PRIMARY KEY(repo_id,branch,account,base)
        );
        CREATE TABLE IF NOT EXISTS workflow_worktrees(
            repo_id INTEGER PRIMARY KEY REFERENCES repos(id) ON DELETE CASCADE,
            main_path TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS workflow_published(
            repo_id INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
            sha TEXT NOT NULL,
            PRIMARY KEY(repo_id,sha)
        );
        CREATE TABLE IF NOT EXISTS workflow_status(
            repo_id INTEGER PRIMARY KEY REFERENCES repos(id) ON DELETE CASCADE,
            path TEXT NOT NULL, branch TEXT NOT NULL, dirty INTEGER NOT NULL,
            ahead INTEGER NOT NULL, behind INTEGER NOT NULL, conflicts INTEGER NOT NULL
        );",
    )
    .map_err(err)
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Override {
    #[default]
    Inherit,
    On,
    Off,
}

impl Override {
    fn as_str(self) -> &'static str {
        match self {
            Self::Inherit => "inherit",
            Self::On => "on",
            Self::Off => "off",
        }
    }
    pub(super) fn resolve(self, default: bool) -> bool {
        match self {
            Self::Inherit => default,
            Self::On => true,
            Self::Off => false,
        }
    }
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "inherit" => Ok(Self::Inherit),
            "on" => Ok(Self::On),
            "off" => Ok(Self::Off),
            _ => Err("The saved repository override is unsupported.".into()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RepoPreferences {
    pub fetch_mode: Override,
    pub warn_default: Override,
    pub editor: Option<String>,
    pub review_paths: Vec<String>,
    pub preferred_base: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RepoWorkflow {
    id: i64,
    path: String,
    #[serde(flatten)]
    preferences: RepoPreferences,
    pr_login: Option<String>,
    pin_position: Option<i64>,
    opened_at: Option<i64>,
    pub fetched_at: Option<i64>,
    main_worktree: Option<String>,
    cached_status: Option<CachedStatus>,
}
#[derive(Clone, Debug, Serialize)]
struct CachedStatus {
    branch: String,
    dirty: usize,
    ahead: i64,
    behind: i64,
    conflicts: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AppPreferences {
    pub save_drafts: bool,
    pub warn_default: bool,
    pub editor: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Target {
    pub id: i64,
    pub path: String,
    pub registration: Option<String>,
}

impl Target {
    pub(super) fn verify(&self, db: &Connection) -> Result<std::path::PathBuf, String> {
        let path = repo_path(db, self.id)?;
        if path != std::path::Path::new(&self.path) {
            return Err("The repository location changed. Reopen it and try again.".into());
        }
        if let Some(expected) = &self.registration {
            if registration(db, self.id)?.as_ref() != Some(expected) {
                return Err("The repository registration changed. Reopen it and try again.".into());
            }
        }
        Ok(path)
    }
}

pub(super) fn registration(db: &Connection, id: i64) -> Result<Option<String>, String> {
    db.query_row(
        "SELECT token FROM workflow_registrations WHERE repo_id=?1",
        [id],
        |row| row.get(0),
    )
    .optional()
    .map_err(err)
}

fn ensure(db: &Connection, id: i64) -> Result<(), String> {
    db.execute(
        "INSERT OR IGNORE INTO repo_workflow(repo_id) VALUES(?1)",
        [id],
    )
    .map_err(err)?;
    Ok(())
}

pub(super) fn fetch_mode(db: &Connection, id: i64) -> Result<Override, String> {
    let value: Option<String> = db
        .query_row(
            "SELECT fetch_mode FROM repo_workflow WHERE repo_id=?1",
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(err)?;
    Override::parse(value.as_deref().unwrap_or("inherit"))
}

pub(super) fn fetched(db: &Connection, id: i64, path: &std::path::Path) -> Result<(), String> {
    if repo_path(db, id)? == path {
        db.execute("INSERT INTO repo_workflow(repo_id,fetched_at) VALUES(?1,?2) ON CONFLICT(repo_id) DO UPDATE SET fetched_at=excluded.fetched_at", params![id,now()]).map_err(err)?;
    }
    Ok(())
}

pub(super) fn cache_status(db: &Connection, status: &super::RepoSummary) -> Result<(), String> {
    if status.status_error.is_some()
        || repo_path(db, status.id)? != std::path::Path::new(&status.path)
        || registration(db, status.id)? != status.registration
    {
        return Ok(());
    }
    db.execute("INSERT INTO workflow_status VALUES(?1,?2,?3,?4,?5,?6,?7)
        ON CONFLICT(repo_id) DO UPDATE SET path=excluded.path,branch=excluded.branch,dirty=excluded.dirty,
        ahead=excluded.ahead,behind=excluded.behind,conflicts=excluded.conflicts
        WHERE (path,branch,dirty,ahead,behind,conflicts)<>(excluded.path,excluded.branch,excluded.dirty,excluded.ahead,excluded.behind,excluded.conflicts)",
        params![status.id,status.path,status.branch,status.dirty,status.ahead,status.behind,status.conflicts]).map_err(err)?;
    Ok(())
}

pub(super) fn app_preferences(db: &Connection) -> Result<AppPreferences, String> {
    let get = |key: &str| {
        db.query_row("SELECT value FROM preferences WHERE key=?1", [key], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .map_err(err)
    };
    let enabled = |key: &str| match get(key)?.as_deref() {
        None | Some("true") => Ok(true),
        Some("false") => Ok(false),
        _ => Err::<bool, String>("The saved workflow setting is unsupported.".into()),
    };
    Ok(AppPreferences {
        save_drafts: enabled("workflow-save-drafts")?,
        warn_default: enabled("workflow-warn-default")?,
        editor: get("workflow-editor")?.filter(|text| !text.is_empty()),
    })
}

pub(super) fn editor_path(path: Option<String>) -> Result<Option<String>, String> {
    path.map(|path| {
        let path = std::path::Path::new(&path);
        if !path.is_absolute() || !path.is_file() {
            return Err("Choose an installed editor executable.".into());
        }
        #[cfg(windows)]
        if !path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
        {
            return Err("Choose the editor's .exe file.".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if std::fs::metadata(path).map_err(err)?.permissions().mode() & 0o111 == 0 {
                return Err("Choose an executable editor file.".into());
            }
        }
        std::fs::canonicalize(path)
            .map_err(err)?
            .into_os_string()
            .into_string()
            .map_err(|_| "The editor path is not UTF-8.".into())
    })
    .transpose()
}

#[tauri::command]
pub async fn workflow_app(state: tauri::State<'_, State>) -> Result<AppPreferences, String> {
    background(state, |s| {
        app_preferences(&*s.database.lock().map_err(err)?)
    })
    .await
}

#[tauri::command]
pub async fn save_workflow_app(
    mut input: AppPreferences,
    state: tauri::State<'_, State>,
) -> Result<AppPreferences, String> {
    background(state, move |s| {
        let mut db = s.database.lock().map_err(err)?;
        let tx = db.transaction().map_err(err)?;
        let previous = app_preferences(&tx)?;
        if input.editor != previous.editor { input.editor = editor_path(input.editor)?; }
        for (key, value) in [
            ("workflow-save-drafts", input.save_drafts.to_string()),
            ("workflow-warn-default", input.warn_default.to_string()),
            ("workflow-editor", input.editor.clone().unwrap_or_default()),
        ] {
            tx.execute("INSERT INTO preferences(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,value]).map_err(err)?;
        }
        if !input.save_drafts { tx.execute("DELETE FROM workflow_drafts", []).map_err(err)?; }
        tx.commit().map_err(err)?;
        Ok(input)
    }).await
}

#[tauri::command]
pub async fn list_repo_workflow(
    state: tauri::State<'_, State>,
) -> Result<Vec<RepoWorkflow>, String> {
    background(state, |s| {
        let db = s.database.lock().map_err(err)?;
        let mut statement = db.prepare(
            "SELECT r.id,r.path,COALESCE(w.fetch_mode,'inherit'),COALESCE(w.warn_default,'inherit'),w.editor,
                    COALESCE(w.review_paths,'[]'),w.preferred_base,w.pr_login,w.pin_position,w.opened_at,w.fetched_at,t.main_path,
                    c.branch,c.dirty,c.ahead,c.behind,c.conflicts
             FROM repos r LEFT JOIN repo_workflow w ON r.id=w.repo_id
             LEFT JOIN workflow_worktrees t ON t.repo_id=r.id
             LEFT JOIN workflow_status c ON c.repo_id=r.id AND c.path=r.path"
        ).map_err(err)?;
        let mut rows = statement.query([]).map_err(err)?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().map_err(err)? {
            let cached_status=if let Some(branch)=row.get::<_,Option<String>>(12).map_err(err)? {
                Some(CachedStatus {branch,dirty:row.get(13).map_err(err)?,ahead:row.get(14).map_err(err)?,
                    behind:row.get(15).map_err(err)?,conflicts:row.get(16).map_err(err)?})
            } else {None};
            result.push(RepoWorkflow {
                id: row.get(0).map_err(err)?, path: row.get(1).map_err(err)?,
                preferences: RepoPreferences {
                    fetch_mode: Override::parse(&row.get::<_,String>(2).map_err(err)?)?,
                    warn_default: Override::parse(&row.get::<_,String>(3).map_err(err)?)?,
                    editor: row.get(4).map_err(err)?,
                    review_paths: serde_json::from_str(&row.get::<_,String>(5).map_err(err)?)
                        .map_err(|_| "The saved review filters are unsupported.")?,
                    preferred_base: row.get(6).map_err(err)?,
                },
                pr_login: row.get(7).map_err(err)?, pin_position: row.get(8).map_err(err)?,
                opened_at: row.get(9).map_err(err)?, fetched_at: row.get(10).map_err(err)?,
                main_worktree: row.get(11).map_err(err)?,
                cached_status,
            });
        }
        Ok(result)
    }).await
}

#[tauri::command]
pub async fn save_repo_workflow(
    target: Target,
    mut input: RepoPreferences,
    state: tauri::State<'_, State>,
) -> Result<RepoPreferences, String> {
    if input.review_paths.len() > 30
        || input.review_paths.iter().any(|path| {
            path.len() > 512
                || path.trim().is_empty()
                || path.starts_with('/')
                || path.contains(['\\', ':'])
                || path.chars().any(char::is_control)
                || path
                    .trim_end_matches('/')
                    .split('/')
                    .any(|component| matches!(component, "." | ".." | ""))
        })
    {
        return Err(
            "Enter up to 30 literal file or folder paths relative to this repository.".into(),
        );
    }
    if let Some(base) = &input.preferred_base {
        branch_name(base)?;
    }
    background(state, move |s| {
        let db = s.database.lock().map_err(err)?;
        target.verify(&db)?;
        ensure(&db,target.id)?;
        let existing: Option<String> = db.query_row("SELECT editor FROM repo_workflow WHERE repo_id=?1", [target.id], |row| row.get(0)).map_err(err)?;
        if input.editor != existing { input.editor = editor_path(input.editor)?; }
        db.execute("UPDATE repo_workflow SET fetch_mode=?2,warn_default=?3,editor=?4,review_paths=?5,preferred_base=?6 WHERE repo_id=?1",
            params![target.id,input.fetch_mode.as_str(),input.warn_default.as_str(),input.editor,serde_json::to_string(&input.review_paths).map_err(err)?,input.preferred_base]).map_err(err)?;
        Ok(input)
    }).await
}

#[tauri::command]
pub async fn visit_repo(
    target: Target,
    branch: Option<String>,
    state: tauri::State<'_, State>,
) -> Result<Vec<String>, String> {
    if let Some(branch) = &branch {
        branch_name(branch)?;
    }
    background(state, move |s| {
        let path=target.verify(&*s.database.lock().map_err(err)?)?;
        let main=super::gitflow::common_directory(&path).ok().and_then(|common|{
            let folder=common.parent()?;
            (folder!=path&&folder.join(".git")==common&&folder.is_dir()).then(||folder.to_string_lossy().into_owned())
        });
        let mut db = s.database.lock().map_err(err)?;
        let tx = db.transaction().map_err(err)?;
        target.verify(&tx)?;
        let timestamp = chrono::Utc::now().timestamp_millis();
        tx.execute("INSERT INTO repo_workflow(repo_id,opened_at) VALUES(?1,?2) ON CONFLICT(repo_id) DO UPDATE SET opened_at=excluded.opened_at", params![target.id,timestamp]).map_err(err)?;
        if let Some(main)=main {
            tx.execute("INSERT INTO workflow_worktrees VALUES(?1,?2) ON CONFLICT(repo_id) DO UPDATE SET main_path=excluded.main_path",params![target.id,main]).map_err(err)?;
        } else {
            tx.execute("DELETE FROM workflow_worktrees WHERE repo_id=?1",[target.id]).map_err(err)?;
        }
        if let Some(branch) = branch {
            tx.execute("INSERT INTO workflow_branches VALUES(?1,?2,?3) ON CONFLICT(repo_id,branch) DO UPDATE SET visited_at=excluded.visited_at",
                params![target.id,branch,timestamp]).map_err(err)?;
            tx.execute("DELETE FROM workflow_branches WHERE repo_id=?1 AND branch NOT IN (SELECT branch FROM workflow_branches WHERE repo_id=?1 ORDER BY visited_at DESC,branch LIMIT 12)", [target.id]).map_err(err)?;
        }
        let branches = {
            let mut query = tx.prepare("SELECT branch FROM workflow_branches WHERE repo_id=?1 ORDER BY visited_at DESC,branch").map_err(err)?;
            let rows = query.query_map([target.id], |row| row.get(0)).map_err(err)?;
            rows.collect::<Result<Vec<String>,_>>().map_err(err)?
        };
        tx.commit().map_err(err)?;
        Ok(branches)
    }).await
}

pub(super) fn pin(db: &mut Connection, id: i64, pinned: bool) -> Result<(), String> {
    let tx = db.transaction().map_err(err)?;
    if pinned {
        // Materialize the prior visual order before appending this pin.
        let mut ids = pinned_ids(&tx)?;
        if !ids.contains(&id) {
            ids.push(id);
        }
        write_order(&tx, &ids)?;
    } else {
        tx.execute(
            "UPDATE repo_workflow SET pin_position=NULL WHERE repo_id=?1",
            [id],
        )
        .map_err(err)?;
    }
    tx.execute(
        "UPDATE repos SET pinned=?2 WHERE id=?1",
        params![id, pinned],
    )
    .map_err(err)?;
    tx.commit().map_err(err)
}

fn pinned_ids(db: &Connection) -> Result<Vec<i64>, String> {
    let mut query = db.prepare("SELECT r.id FROM repos r LEFT JOIN repo_workflow w ON r.id=w.repo_id WHERE r.pinned=1 ORDER BY w.pin_position IS NULL,w.pin_position,r.name COLLATE NOCASE,r.id").map_err(err)?;
    let rows = query.query_map([], |row| row.get(0)).map_err(err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(err)
}

fn write_order(db: &Connection, ids: &[i64]) -> Result<(), String> {
    for (position, id) in ids.iter().enumerate() {
        db.execute("INSERT INTO repo_workflow(repo_id,pin_position) VALUES(?1,?2) ON CONFLICT(repo_id) DO UPDATE SET pin_position=excluded.pin_position",
            params![id,position as i64]).map_err(err)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn move_pin(
    target: Target,
    earlier: bool,
    state: tauri::State<'_, State>,
) -> Result<(), String> {
    background(state, move |s| {
        let mut db = s.database.lock().map_err(err)?;
        let tx = db.transaction().map_err(err)?;
        target.verify(&tx)?;
        let mut ids = pinned_ids(&tx)?;
        let index = ids
            .iter()
            .position(|id| *id == target.id)
            .ok_or("Pin this repository before arranging it.")?;
        if let Some(next) = (if earlier {
            index.checked_sub(1)
        } else {
            index.checked_add(1)
        })
        .filter(|next| *next < ids.len())
        {
            ids.swap(index, next);
            write_order(&tx, &ids)?;
        }
        tx.commit().map_err(err)
    })
    .await
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DraftScope {
    branch: String,
    account: String,
    base: String,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Draft {
    title: String,
    body: String,
}

fn validate_draft(scope: &DraftScope, value: Option<&Draft>) -> Result<(), String> {
    branch_name(&scope.branch)?;
    if scope.branch.len() > 512
        || scope.base.len() > 512
        || scope.account.len() > 100
        || scope
            .account
            .bytes()
            .any(|ch| !ch.is_ascii_alphanumeric() && ch != b'-')
        || (scope.account.is_empty() != scope.base.is_empty())
    {
        return Err("Choose a valid draft branch and account.".into());
    }
    if !scope.base.is_empty() {
        branch_name(&scope.base)?;
    }
    if let Some(value) = value {
        if value.title.encode_utf16().count() > if scope.account.is_empty() { 500 } else { 256 }
            || value.body.encode_utf16().count() > if scope.account.is_empty() { 0 } else { 20000 }
        {
            return Err("The draft exceeds the message length limit.".into());
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn read_workflow_draft(
    target: Target,
    scope: DraftScope,
    state: tauri::State<'_, State>,
) -> Result<Option<Draft>, String> {
    validate_draft(&scope, None)?;
    background(state, move |s| {
        let db = s.database.lock().map_err(err)?;
        target.verify(&db)?;
        if !app_preferences(&db)?.save_drafts { return Ok(None); }
        db.query_row("SELECT title,body FROM workflow_drafts WHERE repo_id=?1 AND branch=?2 AND account=?3 AND base=?4",
            params![target.id,scope.branch,scope.account,scope.base],
            |row| Ok(Draft {title:row.get(0)?,body:row.get(1)?})).optional().map_err(err)
    }).await
}

#[tauri::command]
pub async fn save_workflow_draft(
    target: Target,
    scope: DraftScope,
    input: Option<Draft>,
    state: tauri::State<'_, State>,
) -> Result<(), String> {
    validate_draft(&scope, input.as_ref())?;
    background(state, move |s| {
        let db = s.database.lock().map_err(err)?;
        target.verify(&db)?;
        if !app_preferences(&db)?.save_drafts { return Ok(()); }
        if let Some(input) = input.filter(|draft| !draft.title.is_empty() || !draft.body.is_empty()) {
            db.execute("INSERT INTO workflow_drafts VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(repo_id,branch,account,base) DO UPDATE SET title=excluded.title,body=excluded.body,updated_at=excluded.updated_at",
                params![target.id,scope.branch,scope.account,scope.base,input.title,input.body,now()]).map_err(err)?;
        } else {
            db.execute("DELETE FROM workflow_drafts WHERE repo_id=?1 AND branch=?2 AND account=?3 AND base=?4",
                params![target.id,scope.branch,scope.account,scope.base]).map_err(err)?;
        }
        Ok(())
    }).await
}

pub(super) fn used_pr_account(db: &Connection, id: i64, login: &str) -> Result<(), String> {
    db.execute("INSERT INTO repo_workflow(repo_id,pr_login) VALUES(?1,?2) ON CONFLICT(repo_id) DO UPDATE SET pr_login=excluded.pr_login", params![id,login]).map_err(err)?;
    Ok(())
}

pub(super) fn record_publication(db: &Connection, id: i64, sha: &str) -> Result<(), String> {
    // Keep the history of tips observed by Reponiq, even if the remote later prunes a branch.
    db.execute(
        "INSERT OR IGNORE INTO workflow_published VALUES(?1,?2)",
        params![id, sha],
    )
    .map_err(err)?;
    Ok(())
}

pub(super) fn published_tips(db: &Connection, id: i64) -> Result<Vec<String>, String> {
    let mut query = db
        .prepare("SELECT sha FROM workflow_published WHERE repo_id=?1 ORDER BY sha LIMIT 501")
        .map_err(err)?;
    let rows = query.query_map([id], |row| row.get(0)).map_err(err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_repo_removal_cannot_transfer_preferences_or_drafts_to_a_reused_id() {
        let dir = tempfile::tempdir().unwrap();
        let db = super::super::initialize(&dir.path().join("fixture.sqlite")).unwrap();
        db.execute_batch(
            "INSERT INTO repos(id,path,name) VALUES(1,'/first','First');
            INSERT INTO repo_workflow(repo_id,fetch_mode) VALUES(1,'off');
            INSERT INTO workflow_drafts VALUES(1,'main','','','local draft','',0);
            UPDATE repos SET pinned=1 WHERE id=1;",
        )
        .unwrap();
        assert_eq!(fetch_mode(&db, 1).unwrap(), Override::Off);
        let prior = Target {
            id: 1,
            path: "/first".into(),
            registration: registration(&db, 1).unwrap(),
        };
        db.execute_batch("DELETE FROM repos WHERE id=1; INSERT INTO repos(id,path,name) VALUES(1,'/first','Second');").unwrap();
        assert!(prior.verify(&db).is_err());
        assert_eq!(fetch_mode(&db, 1).unwrap(), Override::Inherit);
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM workflow_drafts", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}
