use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex, Once},
};

const CAPACITY: usize = 2000;
const FILE_LIMIT: u64 = 2 * 1024 * 1024;
const MESSAGE_LIMIT: usize = 8192;

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Entry {
    pub id: u64,
    pub timestamp: String,
    pub level: Level,
    pub source: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct Snapshot {
    entries: Vec<Entry>,
    cursor: u64,
    capacity: usize,
    log_file: Option<String>,
    persistence_error: Option<String>,
}

#[derive(Default)]
struct Journal {
    entries: VecDeque<Entry>,
    next: u64,
    path: Option<PathBuf>,
    writer: Option<File>,
    size: u64,
    persistence_error: Option<String>,
}

impl Journal {
    fn snapshot(&self, after: u64) -> Snapshot {
        Snapshot {
            entries: self
                .entries
                .iter()
                .filter(|e| e.id > after)
                .cloned()
                .collect(),
            cursor: self.next,
            capacity: CAPACITY,
            log_file: self.path.as_ref().map(|p| p.to_string_lossy().into_owned()),
            persistence_error: self.persistence_error.clone(),
        }
    }

    fn push(&mut self, level: Level, source: String, message: String, persist: bool) {
        self.next += 1;
        let entry = Entry {
            id: self.next,
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            level,
            source,
            message,
        };
        if persist {
            if let Err(error) = self.write(&entry) {
                self.persistence_error = Some(format!("Could not save logs: {error}"));
                self.writer = None;
            }
        }
        self.entries.push_back(entry);
        while self.entries.len() > CAPACITY {
            self.entries.pop_front();
        }
    }

    fn write(&mut self, entry: &Entry) -> std::io::Result<()> {
        if self.writer.is_none() {
            return Ok(());
        }
        let mut bytes = serde_json::to_vec(entry)?;
        bytes.push(b'\n');
        if self.size + bytes.len() as u64 > FILE_LIMIT {
            self.writer.take();
            let path = self.path.as_ref().expect("log writer has a path");
            let previous = path.with_file_name("reponiq.previous.log");
            if previous.exists() {
                std::fs::remove_file(&previous)?;
            }
            std::fs::rename(path, previous)?;
            self.writer = Some(open_log(path, false)?);
            self.size = 0;
        }
        if let Some(writer) = &mut self.writer {
            writer.write_all(&bytes)?;
            self.size += bytes.len() as u64;
        }
        Ok(())
    }

    fn attach(&mut self, directory: &Path) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(directory)?;
        }
        #[cfg(not(unix))]
        std::fs::create_dir_all(directory)?;
        let path = directory.join("reponiq.log");
        let writer = open_log(&path, false)?;
        let pending = std::mem::take(&mut self.entries);
        for file in [directory.join("reponiq.previous.log"), path.clone()] {
            if let Ok(input) = File::open(file) {
                let mut text = String::new();
                let _ = input
                    .take(FILE_LIMIT + MESSAGE_LIMIT as u64 * 2)
                    .read_to_string(&mut text);
                for line in text.lines() {
                    if let Ok(mut entry) = serde_json::from_str::<Entry>(line) {
                        self.next += 1;
                        entry.id = self.next;
                        entry.source = redact(&entry.source, 160);
                        entry.message = redact(&entry.message, MESSAGE_LIMIT);
                        self.entries.push_back(entry);
                        if self.entries.len() > CAPACITY {
                            self.entries.pop_front();
                        }
                    }
                }
            }
        }
        self.size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        self.writer = Some(writer);
        self.path = Some(path);
        for entry in pending {
            self.push(entry.level, entry.source, entry.message, true);
        }
        Ok(())
    }

    fn clear(&mut self) -> std::io::Result<()> {
        self.writer.take();
        if let Some(path) = &self.path {
            let previous = path.with_file_name("reponiq.previous.log");
            if previous.exists() {
                std::fs::remove_file(previous)?;
            }
            self.writer = Some(open_log(path, true)?);
        }
        self.entries.clear();
        self.size = 0;
        self.persistence_error = None;
        Ok(())
    }
}

fn open_log(path: &Path, truncate: bool) -> std::io::Result<File> {
    if std::fs::symlink_metadata(path).is_ok_and(|m| !m.is_file()) {
        return Err(std::io::Error::other("The log path is not a regular file."));
    }
    let mut options = OpenOptions::new();
    options
        .create(true)
        .write(truncate)
        .append(!truncate)
        .truncate(truncate);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

static JOURNAL: LazyLock<Mutex<Journal>> = LazyLock::new(|| Mutex::new(Journal::default()));
static SECRETS: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

pub fn remember_secret(secret: &str) {
    if secret.len() >= 4 && secret.len() <= 16384 {
        if let Ok(mut secrets) = SECRETS.lock() {
            if !secrets.iter().any(|s| s == secret) {
                secrets.push_back(secret.to_owned());
                if secrets.len() > 128 {
                    secrets.pop_front();
                }
            }
        }
    }
}

pub fn redact(text: &str, limit: usize) -> String {
    static URL: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r#"(?i)\b(?:https?|ssh|git)://[^\s<>\"'`]+"#).unwrap());
    static FIELD: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r#"(?i)((?:authorization|proxy-authorization|password|passwd|token|access_token|refresh_token|client_secret|user_code|device_code)[\"']?\s*[:=]\s*[\"']?)(?:bearer\s+|basic\s+)?[^\s\"',;]+"#).unwrap()
    });
    static TOKEN: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
            r"\b(?:gh[pousr]_[A-Za-z0-9_]+|github_pat_[A-Za-z0-9_]+|glpat-[A-Za-z0-9_-]+)\b",
        )
        .unwrap()
    });
    static ANSI: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]").unwrap());
    let mut text = text.to_owned();
    if let Ok(secrets) = SECRETS.lock() {
        for secret in secrets.iter() {
            text = text.replace(secret, "[REDACTED]");
        }
    }
    text = URL
        .replace_all(&text, |captures: &regex::Captures<'_>| {
            if url::Url::parse(&captures[0]).is_ok() {
                crate::safe_remote(&captures[0])
            } else {
                "[redacted URL]".into()
            }
        })
        .into_owned();
    text = FIELD.replace_all(&text, "${1}[REDACTED]").into_owned();
    text = TOKEN.replace_all(&text, "[REDACTED]").into_owned();
    text = ANSI.replace_all(&text, "").into_owned();
    let mut clean: String = text
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .take(limit)
        .collect();
    if text.chars().count() > limit {
        clean.push_str("\n[truncated]");
    }
    clean
}

pub fn record(level: Level, source: &str, message: &str) {
    let source = redact(source, 160);
    let message = redact(message, MESSAGE_LIMIT);
    if let Ok(mut journal) = JOURNAL.lock() {
        journal.push(level, source, message, true);
    }
}

pub fn attach(directory: &Path) {
    let error = JOURNAL.lock().ok().and_then(|mut journal| {
        journal.attach(directory).err().map(|e| {
            let message = format!("Could not open the log file: {e}");
            journal.persistence_error = Some(message.clone());
            message
        })
    });
    if let Some(error) = error {
        record(Level::Warn, "diagnostics", &error);
    }
}

struct Logger;
impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn || metadata.target().starts_with("reponiq")
    }
    fn log(&self, entry: &log::Record<'_>) {
        if self.enabled(entry.metadata()) {
            let level = match entry.level() {
                log::Level::Error => Level::Error,
                log::Level::Warn => Level::Warn,
                log::Level::Info => Level::Info,
                _ => Level::Debug,
            };
            let source = entry
                .line()
                .map(|line| format!("{}:{line}", entry.target()))
                .unwrap_or_else(|| entry.target().into());
            record(level, &source, &entry.args().to_string());
        }
    }
    fn flush(&self) {}
}

pub fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = log::set_logger(&Logger);
        log::set_max_level(log::LevelFilter::Debug);
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic| {
            record(
                Level::Error,
                "panic",
                &format!("{panic}\n{}", std::backtrace::Backtrace::force_capture()),
            );
            previous(panic);
        }));
        record(
            Level::Info,
            "app",
            &format!(
                "Reponiq {} starting · {} {} · {}",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                std::env::consts::ARCH,
                if cfg!(debug_assertions) {
                    "development"
                } else {
                    "release"
                }
            ),
        );
    });
}

#[derive(Deserialize)]
pub struct ClientEntry {
    level: Level,
    source: String,
    message: String,
}

#[tauri::command]
pub async fn diagnostics_snapshot(after: u64) -> Result<Snapshot, String> {
    JOURNAL
        .lock()
        .map(|j| j.snapshot(after))
        .map_err(|_| "Logs are temporarily unavailable.".into())
}

#[tauri::command]
pub async fn diagnostics_write(entries: Vec<ClientEntry>) -> Result<(), String> {
    if entries.len() > 100
        || entries
            .iter()
            .any(|e| e.message.len() > 32768 || e.source.len() > 512)
    {
        return Err("Log batch is too large.".into());
    }
    for entry in entries {
        record(
            entry.level,
            &format!("frontend.{}", entry.source),
            &entry.message,
        );
    }
    Ok(())
}

#[tauri::command]
pub async fn diagnostics_clear() -> Result<Snapshot, String> {
    let mut journal = JOURNAL
        .lock()
        .map_err(|_| "Logs are temporarily unavailable.")?;
    if let Err(error) = journal.clear() {
        let error = format!("Could not clear saved logs: {error}");
        journal.persistence_error = Some(error.clone());
        return Err(error);
    }
    Ok(journal.snapshot(0))
}

#[tauri::command]
pub async fn diagnostics_export(
    text: String,
    window: tauri::WebviewWindow,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    const EXPORT_LIMIT: usize = 16 * 1024 * 1024;
    if text.len() > EXPORT_LIMIT {
        return Err("Log export is too large. Narrow the filters and try again.".into());
    }
    // Export the displayed snapshot, including entries retained in a paused view
    // after the live journal has rotated. Reapply redaction at the native boundary.
    let text = redact(&text, EXPORT_LIMIT);
    let log_path = JOURNAL
        .lock()
        .map_err(|_| "Logs are temporarily unavailable.")?
        .path
        .clone();
    let (send, receive) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Save diagnostic logs")
        .set_file_name(format!(
            "Reponiq-logs-{}.log",
            chrono::Utc::now().format("%Y%m%d-%H%M%S")
        ))
        .add_filter("Log files", &["log", "txt"])
        .save_file(move |path| {
            let _ = send.send(path);
        });
    let Some(path) = receive
        .await
        .map_err(|_| "The save dialog closed unexpectedly.")?
    else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|_| "Choose a local file.")?;
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(log_path) = log_path {
            let destination = path.canonicalize().unwrap_or_else(|_| path.clone());
            if [
                log_path.clone(),
                log_path.with_file_name("reponiq.previous.log"),
            ]
            .iter()
            .any(|p| p.canonicalize().unwrap_or_else(|_| p.clone()) == destination)
            {
                return Err("Choose a file outside the live log files.".into());
            }
        }
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options
            .open(&path)
            .and_then(|mut f| f.write_all(text.as_bytes()))
            .map_err(|e| format!("Could not save logs: {e}"))?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|_| "Could not finish saving logs.")?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_git_failures_reach_the_runtime_journal_with_stderr() {
        init();
        let directory = tempfile::tempdir().unwrap();
        assert!(crate::git(directory.path(), &["reponiq-fixture-missing-command"]).is_err());
        let journal = JOURNAL.lock().unwrap();
        assert!(journal
            .entries
            .iter()
            .any(|entry| entry.level == Level::Error
                && entry.source.contains("git")
                && entry.message.contains("reponiq-fixture-missing-command")
                && entry.message.contains("not a git command")));
    }

    #[test]
    fn credentials_are_removed_without_hiding_useful_git_errors() {
        remember_secret("opaque-fixture-credential");
        let output = redact("fatal: Authentication failed for https://person:password@github.com/team/repo.git?access_token=private#secret\nAuthorization: Bearer abc.def\npassword='hidden-password'\n{\"access_token\":\"json-secret\"}\nghp_dummycredential opaque-fixture-credential", MESSAGE_LIMIT);
        for secret in [
            "person:",
            "password@",
            "private",
            "abc.def",
            "hidden-password",
            "json-secret",
            "ghp_dummycredential",
            "opaque-fixture-credential",
        ] {
            assert!(!output.contains(secret), "{secret}: {output}");
        }
        assert!(output.contains("Authentication failed"));
        assert!(output.contains("https://github.com/team/repo.git"));
        assert_eq!(
            redact("\x1b[31mPermission denied\x1b[0m", MESSAGE_LIMIT),
            "Permission denied"
        );
    }

    #[test]
    fn journal_is_bounded_and_polling_does_not_duplicate_entries() {
        let mut journal = Journal::default();
        for n in 0..CAPACITY + 10 {
            journal.push(Level::Info, "fixture".into(), n.to_string(), false);
        }
        assert_eq!(journal.entries.len(), CAPACITY);
        assert_eq!(journal.entries.front().unwrap().message, "10");
        let snapshot = journal.snapshot(0);
        assert!(journal.snapshot(snapshot.cursor).entries.is_empty());
        journal.push(Level::Error, "git".into(), "clone failed".into(), false);
        assert_eq!(journal.snapshot(snapshot.cursor).entries.len(), 1);
        journal.clear().unwrap();
        assert!(journal.snapshot(0).entries.is_empty());
        journal.push(Level::Info, "fixture".into(), "after clear".into(), false);
        assert_eq!(journal.snapshot(snapshot.cursor).entries.len(), 1);
    }

    #[test]
    fn saved_logs_rotate_reload_and_clear_without_losing_new_errors() {
        let parent = tempfile::tempdir().unwrap();
        let directory = parent.path().join("logs");
        let mut journal = Journal::default();
        journal.push(Level::Info, "app".into(), "startup".into(), false);
        journal.attach(&directory).unwrap();
        for n in 0..300 {
            journal.push(
                Level::Debug,
                "fixture".into(),
                format!("{n}: {}", "x".repeat(MESSAGE_LIMIT)),
                true,
            );
        }
        assert!(directory.join("reponiq.previous.log").exists());
        assert!(
            std::fs::metadata(directory.join("reponiq.log"))
                .unwrap()
                .len()
                <= FILE_LIMIT
        );
        drop(journal);
        let mut restored = Journal::default();
        restored.attach(&directory).unwrap();
        assert!(restored.entries.back().unwrap().message.starts_with("299:"));
        restored.clear().unwrap();
        assert!(!directory.join("reponiq.previous.log").exists());
        assert_eq!(
            std::fs::metadata(directory.join("reponiq.log"))
                .unwrap()
                .len(),
            0
        );
        restored.push(Level::Error, "git".into(), "new failure".into(), true);
        assert!(std::fs::read_to_string(directory.join("reponiq.log"))
            .unwrap()
            .contains("new failure"));
    }

    #[test]
    fn messages_and_sources_cannot_inject_terminal_controls_or_grow_unbounded() {
        let output = redact(&"é".repeat(MESSAGE_LIMIT + 100), MESSAGE_LIMIT);
        assert!(output.ends_with("[truncated]"));
        assert!(output.chars().count() < MESSAGE_LIMIT + 20);
        assert_eq!(
            redact("hello\0world\r\nnext", MESSAGE_LIMIT),
            "helloworld\nnext"
        );
    }
}
