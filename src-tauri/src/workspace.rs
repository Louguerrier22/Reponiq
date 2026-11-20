use super::{background, err, State};
use cap_fs_ext::DirExt;
use cap_std::fs::Dir;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
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
    let folder = match input.default_folder.as_deref() {
        Some(path) if !path.trim().is_empty() => {
            if path.len() > 32768 || !local_path(Path::new(path)) {
                return Err("Choose an absolute repository folder with Browse.".into());
            }
            Some(
                run_isolated(
                    Probe::ValidateFolder(path.to_owned()),
                    Duration::from_secs(5),
                )
                .await?,
            )
        }
        _ => None,
    };
    background(state,move|s|{
 let saved=Settings{auto_detect:input.auto_detect&&folder.is_some(),default_folder:folder,setup_complete:input.setup_complete};let mut db=s.database.lock().map_err(err)?;let tx=db.transaction().map_err(err)?;
 for (key,value) in [("default-repository-folder",saved.default_folder.clone().unwrap_or_default()),("repository-auto-detect",saved.auto_detect.to_string()),("workspace-setup-complete",saved.setup_complete.to_string())]{tx.execute("INSERT INTO preferences(key,value)VALUES(?1,?2) ON CONFLICT(key)DO UPDATE SET value=excluded.value",params![key,value]).map_err(err)?;}tx.commit().map_err(err)?;Ok(saved)
}).await
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    DefaultFolder,
    Folder,
    GithubDesktop,
    VsCode,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct Candidate {
    pub name: String,
    pub path: String,
    pub source: String,
}
#[derive(Serialize, Deserialize)]
pub struct Discovery {
    pub items: Vec<Candidate>,
    pub limited: bool,
    pub notes: Vec<String>,
}
struct Scan {
    items: Vec<Candidate>,
    seen: HashSet<String>,
    entries: usize,
    bytes: usize,
    limited: bool,
    start: Instant,
}
const DISCOVERY_BYTES: usize = 3 * 1024 * 1024;

fn path_key(path: &str) -> String {
    if cfg!(windows) {
        path.to_lowercase()
    } else {
        path.to_owned()
    }
}

impl Scan {
    fn room(&mut self) -> bool {
        if self.items.len() >= 4000
            || self.entries >= 4000
            || self.bytes >= DISCOVERY_BYTES
            || self.start.elapsed().as_secs() >= 12
        {
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
        if value.len() > 32768 {
            self.limited = true;
            return;
        }
        if self.seen.insert(path_key(value)) {
            let item = Candidate {
                name: root
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "repository".into()),
                path: value.to_owned(),
                source: source.into(),
            };
            let size = serde_json::to_vec(&item).map_or(DISCOVERY_BYTES, |bytes| bytes.len() + 1);
            self.bytes += size;
            if self.bytes <= DISCOVERY_BYTES {
                self.items.push(item);
            } else {
                self.limited = true;
            }
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
        .take(4000)
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
        if !meta.is_file() {
            continue;
        }
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
fn saved_folder_fallback(scan: &mut Scan, notes: &mut Vec<String>, folder: Option<String>) {
    match folder {
        Some(folder) if scan.room() => {
            if scan
                .folder(Path::new(&folder), "Default repository folder")
                .is_ok()
            {
                notes.push("Also checked your saved repository folder. Review each source before importing.".into());
            } else {
                notes.push("Your saved repository folder could not be checked. Choose a folder to scan instead.".into());
            }
        }
        _ => {}
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
    let mut settings_unavailable = false;
    let default = if matches!(source, Source::DefaultFolder | Source::GithubDesktop) {
        let saved = background(state.clone(), |s| {
            Ok(settings(&*s.database.lock().map_err(err)?)?.default_folder)
        })
        .await;
        match saved {
            Ok(folder) => folder,
            Err(_) if matches!(source, Source::GithubDesktop) => {
                settings_unavailable = true;
                None
            }
            Err(error) => return Err(error),
        }
    } else {
        None
    };
    if folder.as_ref().is_some_and(|path| path.len() > 32768) {
        return Err("Choose a folder to scan.".into());
    }
    let mut result: Discovery = run_isolated(
        Probe::Discover {
            source,
            folder,
            default,
        },
        Duration::from_secs(13),
    )
    .await?;
    if settings_unavailable {
        result.notes.push("Saved folder settings could not be read and were preserved. Choose a folder to scan instead.".into());
    }
    background(state, move |s| {
        let db = s.database.lock().map_err(err)?;
        if !result.items.is_empty() {
            // Keep only the candidate set in memory, while preserving the
            // existing Windows case-insensitive matching.
            let mut unknown: HashSet<_> = result
                .items
                .iter()
                .map(|item| path_key(&item.path))
                .collect();
            let mut query = db.prepare_cached("SELECT path FROM repos").map_err(err)?;
            for stored in query
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(err)?
            {
                unknown.remove(&path_key(&stored.map_err(err)?));
                if unknown.is_empty() {
                    break;
                }
            }
            result
                .items
                .retain(|item| unknown.contains(&path_key(&item.path)));
        }
        result.limited |= result.items.len() > 200;
        result.items.truncate(200);
        Ok(result)
    })
    .await
}

fn discover(
    source: Source,
    folder: Option<String>,
    default: Option<String>,
) -> Result<Discovery, String> {
    let mut scan = Scan {
        items: vec![],
        seen: HashSet::new(),
        entries: 0,
        bytes: 0,
        limited: false,
        start: Instant::now(),
    };
    let mut notes = vec![];
    match source {
        Source::DefaultFolder => {
            let folder = default.ok_or("Choose a default repository folder in Settings.")?;
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
            saved_folder_fallback(&mut scan, &mut notes, default);
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
}

const WORKER_MODE: &str = "--internal-workspace-probe";
const PROBE_INPUT_LIMIT: u64 = 512 * 1024;

#[cfg(target_os = "linux")]
pub(super) fn is_probe(command: &Command) -> bool {
    command.get_program() == "/proc/self/exe"
        && command.get_args().next() == Some(std::ffi::OsStr::new(WORKER_MODE))
}

#[derive(Serialize, Deserialize)]
enum Probe {
    Discover {
        source: Source,
        folder: Option<String>,
        default: Option<String>,
    },
    ValidateFolder(String),
    PrepareImport(String),
}

fn execute_probe(probe: Probe) -> Result<serde_json::Value, String> {
    match probe {
        Probe::Discover {
            source,
            folder,
            default,
        } => serde_json::to_value(discover(source, folder, default)?).map_err(err),
        Probe::ValidateFolder(path) => {
            let root =
                std::fs::canonicalize(path).map_err(|_| "The chosen folder is unavailable.")?;
            if !root.is_dir() {
                return Err("Choose an existing folder.".into());
            }
            let path = root.to_str().ok_or("The folder name is not UTF-8.")?;
            Ok(serde_json::Value::String(path.to_owned()))
        }
        Probe::PrepareImport(path) => {
            serde_json::to_value(super::prepare_repo(&path)?).map_err(err)
        }
    }
}

/// Enter the read-only workspace probe before initializing app state.
pub fn run_workspace_worker() {
    if std::env::args_os().nth(1).as_deref() != Some(std::ffi::OsStr::new(WORKER_MODE)) {
        return;
    }
    let mut request = Vec::new();
    let response = (|| {
        std::io::stdin()
            .take(PROBE_INPUT_LIMIT + 1)
            .read_to_end(&mut request)
            .map_err(err)?;
        if request.len() as u64 > PROBE_INPUT_LIMIT {
            return Err("Workspace request is too large.".into());
        }
        execute_probe(serde_json::from_slice(&request).map_err(err)?)
    })();
    let written = serde_json::to_writer(std::io::stdout().lock(), &response).is_ok();
    std::process::exit(if written { 0 } else { 1 });
}

const IO_TIMEOUT: &str =
    "The folder check took too long. Confirm the drive is reachable and try again.";

async fn run_isolated<T: serde::de::DeserializeOwned + Send + 'static>(
    probe: Probe,
    budget: Duration,
) -> Result<T, String> {
    static IO_SLOT: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);
    let permit = IO_SLOT
        .try_acquire()
        .map_err(|_| "Another folder check is still finishing. Try again shortly.")?;
    let job = tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        let payload = serde_json::to_vec(&probe).map_err(err)?;
        #[cfg(target_os = "linux")]
        let executable = "/proc/self/exe";
        #[cfg(not(target_os = "linux"))]
        let executable = std::env::current_exe().map_err(err)?;
        let mut command = Command::new(executable);
        command.arg(WORKER_MODE);
        let output =
            super::git_process::capture_workspace(command, &payload, budget).map_err(|error| {
                match error {
                    super::git_process::Error::Timeout => IO_TIMEOUT,
                    _ => "Could not complete the folder check.",
                }
            })?;
        if !output.status.success() {
            return Err("Could not complete the folder check.".into());
        }
        serde_json::from_slice::<Result<T, String>>(&output.stdout).map_err(err)?
    });
    tokio::time::timeout(budget + Duration::from_millis(250), job)
        .await
        .map_err(|_| IO_TIMEOUT)?
        .map_err(err)?
}

pub(super) async fn prepare_import(path: String) -> Result<super::PreparedRepo, String> {
    run_isolated(Probe::PrepareImport(path), Duration::from_secs(20)).await
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
fn record_import(imported: &mut Imported, path: String, result: Result<(), String>) {
    match result {
        Ok(()) => imported.added += 1,
        Err(error) => {
            let message = if error.starts_with("Git ")
                || error.starts_with("Could not start Git")
                || error.starts_with("This batch")
                || error.starts_with("Another folder check")
                || error == IO_TIMEOUT
            {
                error
            } else {
                "Could not read or register this folder. Check that it exists, is accessible and contains a Git repository.".into()
            };
            imported.failures.push(ImportFailure {
                path: path.clone(),
                message,
            });
            imported.remaining.push(path);
        }
    }
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
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut imported = Imported {
        added: 0,
        remaining: vec![],
        failures: vec![],
    };
    for path in paths {
        let budget = deadline.saturating_duration_since(Instant::now());
        let result = if budget.is_zero() {
            Err("This batch reached its time limit. Import the remaining folders again.".into())
        } else {
            match run_isolated(Probe::PrepareImport(path.clone()), budget).await {
                Ok(prepared) => {
                    background(state.clone(), move |s| {
                        super::register_repo(prepared, s).map(|_| ())
                    })
                    .await
                }
                Err(error) => Err(error),
            }
        };
        record_import(&mut imported, path, result);
    }
    Ok(imported)
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
        let default = settings(&db).unwrap().default_folder;
        let mut scan = Scan {
            items: vec![],
            seen: HashSet::new(),
            entries: 0,
            bytes: 0,
            limited: false,
            start: Instant::now(),
        };
        let mut notes = vec![];
        saved_folder_fallback(&mut scan, &mut notes, default);
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
        let import = |paths: Vec<String>| {
            let mut result = Imported {
                added: 0,
                remaining: vec![],
                failures: vec![],
            };
            for path in paths {
                let registered = super::super::prepare_repo(&path)
                    .and_then(|repo| super::super::register_repo(repo, &state))
                    .map(|_| ());
                record_import(&mut result, path, registered);
            }
            result
        };
        let result = import(vec![valid.clone(), invalid.clone()]);
        assert_eq!(result.added, 1);
        assert_eq!(result.remaining, vec![invalid.clone()]);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].path, invalid);
        assert!(!result.failures[0].message.is_empty());
        let retry = import(vec![valid]);
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
            bytes: 0,
            limited: false,
            start: Instant::now(),
        };
        scan.folder(root.path(), "test").unwrap();
        assert_eq!(scan.items.len(), 1);
        assert_eq!(scan.items[0].name, "one");
    }
}
