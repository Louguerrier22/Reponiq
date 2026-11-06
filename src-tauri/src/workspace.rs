use super::{background, err, State};
use cap_fs_ext::DirExt;
use cap_std::fs::Dir;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    time::Instant,
};
#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    pub default_folder: Option<String>,
    pub auto_detect: bool,
    pub setup_complete: bool,
}
fn settings(db: &rusqlite::Connection) -> Result<Settings, String> {
    let get = |key: &str| -> Result<Option<String>, String> {
        db.query_row("SELECT value FROM preferences WHERE key=?1", [key], |r| {
            r.get(0)
        })
        .optional()
        .map_err(err)
    };
    let boolean = |key: &str| -> Result<bool, String> {
        match get(key)?.as_deref(){None|Some("false")=>Ok(false),Some("true")=>Ok(true),_=>Err("Saved workspace settings are not supported. Your existing data has been preserved.".into())}
    };
    Ok(Settings {
        default_folder: get("default-repository-folder")?.filter(|s| !s.is_empty()),
        auto_detect: boolean("repository-auto-detect")?,
        setup_complete: boolean("workspace-setup-complete")?,
    })
}
#[tauri::command]
pub async fn workspace_settings(state: tauri::State<'_, State>) -> Result<Settings, String> {
    background(state, |s| settings(&*s.database.lock().map_err(err)?)).await
}
#[tauri::command]
pub async fn save_workspace_settings(
    input: Settings,
    state: tauri::State<'_, State>,
) -> Result<Settings, String> {
    background(state,move|s|{
 let folder=match input.default_folder{Some(ref path)if !path.trim().is_empty()=>{if path.len()>32768||!local_path(Path::new(path)){return Err("Choose an absolute repository folder with Browse.".into());}let root=std::fs::canonicalize(path).map_err(|_|"The chosen folder is unavailable.")?;if !root.is_dir(){return Err("Choose an existing folder.".into());}Some(root.to_str().ok_or("The folder name is not UTF-8.")?.to_owned())},_=>None};
 let saved=Settings{auto_detect:input.auto_detect&&folder.is_some(),default_folder:folder,setup_complete:input.setup_complete};let mut db=s.database.lock().map_err(err)?;let tx=db.transaction().map_err(err)?;
 for (key,value) in [("default-repository-folder",saved.default_folder.clone().unwrap_or_default()),("repository-auto-detect",saved.auto_detect.to_string()),("workspace-setup-complete",saved.setup_complete.to_string())]{tx.execute("INSERT INTO preferences(key,value)VALUES(?1,?2) ON CONFLICT(key)DO UPDATE SET value=excluded.value",params![key,value]).map_err(err)?;}tx.commit().map_err(err)?;Ok(saved)
}).await
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    DefaultFolder,
    Folder,
    GithubDesktop,
    VsCode,
}
#[derive(Serialize, Clone)]
pub struct Candidate {
    pub name: String,
    pub path: String,
    pub source: String,
}
#[derive(Serialize)]
pub struct Discovery {
    pub items: Vec<Candidate>,
    pub limited: bool,
    pub notes: Vec<String>,
}
struct Scan {
    items: Vec<Candidate>,
    seen: HashSet<String>,
    entries: usize,
    limited: bool,
    start: Instant,
}
impl Scan {
    fn room(&mut self) -> bool {
        if self.items.len() >= 200 || self.entries >= 4000 || self.start.elapsed().as_secs() >= 12 {
            self.limited = true;
            false
        } else {
            true
        }
    }
    fn candidate(&mut self, path: PathBuf, source: &str) {
        if !self.room() || !local_path(&path) {
            return;
        }
        let Ok(root) = std::fs::canonicalize(path) else {
            return;
        };
        let Ok(meta) = std::fs::symlink_metadata(root.join(".git")) else {
            return;
        };
        if meta.file_type().is_symlink() || (!meta.is_file() && !meta.is_dir()) {
            return;
        }
        self.record(&root, source);
    }
    fn record(&mut self, root: &Path, source: &str) {
        let Some(value) = root.to_str() else { return };
        let key = if cfg!(windows) {
            value.to_lowercase()
        } else {
            value.to_owned()
        };
        if self.seen.insert(key) {
            self.items.push(Candidate {
                name: root
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "repository".into()),
                path: value.to_owned(),
                source: source.into(),
            });
        }
    }
    fn walk(&mut self, dir: &Dir, absolute: &Path, depth: usize, source: &str) {
        if !self.room() {
            return;
        }
        if let Ok(marker) = dir.symlink_metadata(".git") {
            if marker.is_dir() || marker.is_file() {
                self.record(absolute, source);
                return;
            }
        }
        if depth >= 3 {
            return;
        }
        let Ok(entries) = dir.entries() else {
            self.limited = true;
            return;
        };
        for entry in entries {
            if !self.room() {
                break;
            }
            self.entries += 1;
            let Ok(entry) = entry else { continue };
            let name = entry.file_name();
            let Some(text) = name.to_str() else { continue };
            if text.starts_with('.')
                || matches!(
                    text,
                    "node_modules" | "target" | "dist" | "vendor" | "outputs"
                )
            {
                continue;
            }
            if !entry.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            if let Ok(child) = dir.open_dir_nofollow(&name) {
                self.walk(&child, &absolute.join(name), depth + 1, source);
            }
        }
    }
    fn folder(&mut self, path: &Path, source: &str) -> Result<(), String> {
        if !local_path(path) {
            return Err("Choose a local folder on this computer.".into());
        }
        let root =
            std::fs::canonicalize(path).map_err(|_| "The repository folder is unavailable.")?;
        let dir = Dir::open_ambient_dir(&root, cap_std::ambient_authority())
            .map_err(|_| "Could not read the repository folder.")?;
        self.walk(&dir, &root, 0, source);
        Ok(())
    }
}
fn local_path(path: &Path) -> bool {
    if !path.is_absolute() {
        return false;
    }
    #[cfg(windows)]
    {
        matches!(path.components().next(),Some(std::path::Component::Prefix(p))if matches!(p.kind(),std::path::Prefix::Disk(_)|std::path::Prefix::VerbatimDisk(_)))
    }
    #[cfg(not(windows))]
    {
        true
    }
}
fn user_home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}
fn app_config() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| user_home().map(|p| p.join(".config")))
    }
}
fn desktop_log_path(line: &str) -> Option<&str> {
    let (_, path) = line.split_once("[AppStore] adding repository at ")?;
    let path = path.strip_suffix(" to store")?;
    if path.len() > 32768 || path.contains('\0') {
        None
    } else {
        Some(path)
    }
}
fn desktop(scan: &mut Scan, notes: &mut Vec<String>) {
    let Some(config) = app_config() else { return };
    let folder = config.join("GitHub Desktop/logs");
    let Ok(entries) = std::fs::read_dir(folder) else {
        notes.push(
            "GitHub Desktop history was not found. You can scan its repository folder instead."
                .into(),
        );
        return;
    };
    let mut files = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|s| s == "log"))
        .take(100)
        .collect::<Vec<_>>();
    files.sort_by_key(|e| e.file_name());
    let mut total = 0usize;
    for entry in files.into_iter().rev().take(20) {
        if !scan.room() {
            break;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if meta.len() > 2 * 1024 * 1024 || total + meta.len() as usize > 32 * 1024 * 1024 {
            scan.limited = true;
            continue;
        }
        let Ok(file) = std::fs::File::open(entry.path()) else {
            continue;
        };
        total += meta.len() as usize;
        let mut reader = BufReader::new(file.take(2 * 1024 * 1024));
        let mut line = Vec::new();
        loop {
            line.clear();
            let Ok(n) = reader.by_ref().take(65537).read_until(b'\n', &mut line) else {
                break;
            };
            if n == 0 {
                break;
            }
            if n > 65536 {
                scan.limited = true;
                break;
            }
            let Ok(text) = std::str::from_utf8(&line) else {
                continue;
            };
            if let Some(path) = desktop_log_path(text.trim_end_matches(['\r', '\n'])) {
                scan.candidate(PathBuf::from(path), "GitHub Desktop");
            }
            if !scan.room() {
                break;
            }
        }
    }
    notes.push("GitHub Desktop import uses locally recorded repository paths. Older or removed entries may be missing; review the results or scan a folder.".into());
}
fn saved_folder_fallback(scan: &mut Scan, notes: &mut Vec<String>, state: &State) {
    let folder = state
        .database
        .lock()
        .map_err(err)
        .and_then(|db| settings(&db));
    match folder {
        Ok(Settings{default_folder:Some(folder),..}) if scan.room()=>{
            if scan.folder(Path::new(&folder),"Default repository folder").is_ok(){notes.push("Also checked your saved repository folder. Review each source before importing.".into());}
            else{notes.push("Your saved repository folder could not be checked. Choose a folder to scan instead.".into());}
        },
        Err(_)=>notes.push("Saved folder settings could not be read; they were preserved. Choose a folder to scan instead.".into()),
        _=>{}
    }
}
fn vscode(scan: &mut Scan, notes: &mut Vec<String>) {
    let Some(config) = app_config() else { return };
    let file = config.join("Code/User/globalStorage/state.vscdb");
    let Ok(db) = rusqlite::Connection::open_with_flags(
        file,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        notes.push(
            "Visual Studio Code history was not found. You can scan a repository folder instead."
                .into(),
        );
        return;
    };
    let text=db.query_row("SELECT value FROM ItemTable WHERE key='history.recentlyOpenedPathsList' AND length(value)<=8388608 LIMIT 1",[],|r|r.get::<_,String>(0)).optional();
    if let Ok(Some(text)) = text {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(entries) = value.get("entries").and_then(|v| v.as_array()) {
                for item in entries.iter().take(2000) {
                    if !scan.room() {
                        break;
                    }
                    if let Some(raw) = item.get("folderUri").and_then(|v| v.as_str()) {
                        if let Ok(url) = url::Url::parse(raw) {
                            if url.scheme() == "file"
                                && url.host_str().is_none_or(|h| h == "localhost")
                            {
                                if let Ok(path) = url.to_file_path() {
                                    scan.candidate(path, "Visual Studio Code");
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    notes.push("Only recent local folders that are Git repositories are included. Other clients can be imported by scanning their repository folder.".into());
}
#[tauri::command]
pub async fn discover_repositories(
    source: Source,
    folder: Option<String>,
    state: tauri::State<'_, State>,
) -> Result<Discovery, String> {
    background(state, move |s| {
        let mut scan = Scan {
            items: vec![],
            seen: HashSet::new(),
            entries: 0,
            limited: false,
            start: Instant::now(),
        };
        {
            let db = s.database.lock().map_err(err)?;
            let mut query = db.prepare("SELECT path FROM repos").map_err(err)?;
            for row in query
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(err)?
            {
                if let Ok(path) = row {
                    scan.seen.insert(if cfg!(windows) {
                        path.to_lowercase()
                    } else {
                        path
                    });
                }
            }
        }
        let mut notes = vec![];
        match source {
            Source::DefaultFolder => {
                let folder = settings(&*s.database.lock().map_err(err)?)?
                    .default_folder
                    .ok_or("Choose a default repository folder in Settings.")?;
                scan.folder(Path::new(&folder), "Repository folder")?;
            }
            Source::Folder => {
                let folder = folder
                    .filter(|p| p.len() <= 32768)
                    .ok_or("Choose a folder to scan.")?;
                scan.folder(Path::new(&folder), "Folder scan")?;
            }
            Source::GithubDesktop => {
                desktop(&mut scan, &mut notes);
                saved_folder_fallback(&mut scan, &mut notes, s);
            }
            Source::VsCode => vscode(&mut scan, &mut notes),
        }
        scan.items
            .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(Discovery {
            items: scan.items,
            limited: scan.limited,
            notes,
        })
    })
    .await
}
#[derive(Serialize)]
pub struct Imported {
    pub added: usize,
    pub remaining: Vec<String>,
    pub failures: Vec<ImportFailure>,
}
#[derive(Serialize)]
pub struct ImportFailure {
    pub path: String,
    pub message: String,
}
fn import_paths(paths: Vec<String>, s: &State) -> Result<Imported, String> {
    let _guard = super::gitflow::MUTATIONS.lock().map_err(err)?;
    let start = Instant::now();
    let mut added = 0;
    let mut remaining = vec![];
    let mut failures = vec![];
    for path in paths {
        let result = if start.elapsed().as_secs() >= 20 {
            Err(
                "This batch reached its time limit. Import the remaining folders again."
                    .to_string(),
            )
        } else {
            super::add_repo_impl(path.clone(), s).map(|_| ())
        };
        match result {
            Ok(()) => added += 1,
            Err(error) => {
                let message = if error.starts_with("Git ")
                    || error.starts_with("Could not start Git")
                    || error.starts_with("This batch")
                {
                    error
                } else {
                    "Could not read or register this folder. Check that it exists, is accessible and contains a Git repository.".into()
                };
                failures.push(ImportFailure {
                    path: path.clone(),
                    message,
                });
                remaining.push(path);
            }
        }
    }
    Ok(Imported {
        added,
        remaining,
        failures,
    })
}
#[tauri::command]
pub async fn import_repositories(
    paths: Vec<String>,
    state: tauri::State<'_, State>,
) -> Result<Imported, String> {
    if paths.is_empty()
        || paths.len() > 200
        || paths
            .iter()
            .any(|p| p.len() > 32768 || !local_path(Path::new(p)))
    {
        return Err("Select up to 200 local repositories.".into());
    }
    background(state, move |s| import_paths(paths, s)).await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_folder_fallback_finds_repos_missing_from_desktop_logs() {
        let folder = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(folder.path().join("project/.git")).unwrap();
        let db = super::super::initialize(&folder.path().join("test.sqlite")).unwrap();
        db.execute(
            "INSERT INTO preferences(key,value) VALUES('default-repository-folder',?1)",
            [folder.path().to_str().unwrap()],
        )
        .unwrap();
        let state = State {
            database: std::sync::Arc::new(std::sync::Mutex::new(db)),
            credential_service: "reponiq-test",
        };
        let mut scan = Scan {
            items: vec![],
            seen: HashSet::new(),
            entries: 0,
            limited: false,
            start: Instant::now(),
        };
        let mut notes = vec![];
        saved_folder_fallback(&mut scan, &mut notes, &state);
        assert_eq!(scan.items.len(), 1);
        assert_eq!(scan.items[0].name, "project");
        assert_eq!(scan.items[0].source, "Default repository folder");
        assert!(!notes.is_empty());
    }
    #[test]
    fn batch_import_preserves_success_and_explains_each_failure() {
        let folder = tempfile::tempdir().unwrap();
        let repo = folder.path().join("project");
        std::fs::create_dir(&repo).unwrap();
        super::super::git(&repo, &["init", "--initial-branch=main", "--template="]).unwrap();
        let db = super::super::initialize(&folder.path().join("test.sqlite")).unwrap();
        let state = State {
            database: std::sync::Arc::new(std::sync::Mutex::new(db)),
            credential_service: "reponiq-test",
        };
        let valid = repo.to_str().unwrap().to_string();
        let invalid = folder.path().join("missing").to_str().unwrap().to_string();
        let result = import_paths(vec![valid.clone(), invalid.clone()], &state).unwrap();
        assert_eq!(result.added, 1);
        assert_eq!(result.remaining, vec![invalid.clone()]);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].path, invalid);
        assert!(!result.failures[0].message.is_empty());
        let retry = import_paths(vec![valid], &state).unwrap();
        assert_eq!(retry.added, 1);
        assert!(retry.remaining.is_empty());
        assert_eq!(super::super::list_repos_impl(&state).unwrap().len(), 1);
    }
    #[test]
    fn desktop_parser_only_accepts_known_repository_metadata() {
        assert_eq!(
            desktop_log_path("info: [AppStore] adding repository at /example/repo to store"),
            Some("/example/repo")
        );
        assert!(desktop_log_path("token: secret-fixture").is_none());
        assert!(desktop_log_path("[AppStore] adding repository at /example/repo").is_none());
    }
    #[test]
    fn default_preferences_do_not_rewrite_unknown_data() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE preferences(key TEXT PRIMARY KEY,value TEXT NOT NULL);INSERT INTO preferences VALUES('future','keep');").unwrap();
        let value = settings(&db).unwrap();
        assert!(!value.setup_complete);
        assert_eq!(
            db.query_row(
                "SELECT value FROM preferences WHERE key='future'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "keep"
        );
    }
    #[test]
    fn discovery_finds_markers_without_running_git() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("one/.git")).unwrap();
        std::fs::create_dir_all(root.path().join("node_modules/ignore/.git")).unwrap();
        let mut scan = Scan {
            items: vec![],
            seen: HashSet::new(),
            entries: 0,
            limited: false,
            start: Instant::now(),
        };
        scan.folder(root.path(), "test").unwrap();
        assert_eq!(scan.items.len(), 1);
        assert_eq!(scan.items[0].name, "one");
    }
}
