use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::utils::{config::BundleType, platform::bundle_type};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
use url::Url;

const FEED: &str = "https://github.com/Louguerrier22/Reponiq/releases/latest/download/latest.json";
const RELEASES: &str = "/Louguerrier22/Reponiq/releases/";
const FAILURE: &str = "The update could not be prepared. Try again in App updates.";

#[derive(Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(super) enum Status {
    Unavailable,
    Current,
    Checking,
    Ready { version: String },
}

struct Staged {
    update: Update,
    bytes: Vec<u8>,
}

#[derive(Default)]
struct Pending {
    checking: bool,
    installing: bool,
    install_failed: bool,
    staged: Option<Staged>,
}

#[derive(Default)]
pub(super) struct Updates(Mutex<Pending>);

#[derive(Default)]
struct Activity {
    jobs: usize,
    installing: bool,
}

static ACTIVITY: Mutex<Activity> = Mutex::new(Activity {
    jobs: 0,
    installing: false,
});

pub(super) struct WorkspaceJob;

pub(super) fn workspace_job() -> Result<WorkspaceJob, String> {
    let mut activity = ACTIVITY.lock().map_err(super::err)?;
    if activity.installing {
        return Err("Reponiq is installing an update.".into());
    }
    activity.jobs += 1;
    Ok(WorkspaceJob)
}

impl Drop for WorkspaceJob {
    fn drop(&mut self) {
        if let Ok(mut activity) = ACTIVITY.lock() {
            activity.jobs -= 1;
        }
    }
}

struct Installation;

impl Installation {
    fn start() -> Result<Self, String> {
        let mut activity = ACTIVITY.lock().map_err(super::err)?;
        if activity.installing || activity.jobs != 0 {
            return Err("Finish the current workspace activity, then try the update again.".into());
        }
        activity.installing = true;
        Ok(Self)
    }
}

impl Drop for Installation {
    fn drop(&mut self) {
        if let Ok(mut activity) = ACTIVITY.lock() {
            activity.installing = false;
        }
    }
}

fn package() -> Option<(&'static str, &'static str, usize)> {
    if !cfg!(target_arch = "x86_64") {
        return None;
    }
    match bundle_type() {
        Some(BundleType::Nsis) if cfg!(windows) => {
            Some(("windows-x86_64", "windows-x64.exe", 64 << 20))
        }
        Some(BundleType::AppImage) if cfg!(target_os = "linux") => {
            Some(("linux-x86_64", "linux-x64.AppImage", 256 << 20))
        }
        _ => None,
    }
}

fn allow_download_host(url: &Url) -> bool {
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return false;
    }
    match url.host_str() {
        Some("github.com") => url.path().starts_with(RELEASES),
        Some("release-assets.githubusercontent.com" | "objects.githubusercontent.com") => true,
        _ => false,
    }
}

async fn fetch(
    client: &reqwest::Client,
    url: Url,
    limit: usize,
    missing_is_empty: bool,
) -> Result<Option<Vec<u8>>, String> {
    if !allow_download_host(&url) {
        return Err("Update URL is outside the Reponiq release source.".into());
    }
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|_| FAILURE.to_string())?;
    if missing_is_empty && response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success()
        || response.content_length().is_some_and(|n| n > limit as u64)
    {
        return Err(FAILURE.into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| FAILURE.to_string())? {
        if chunk.len() > limit - bytes.len() {
            return Err("The update exceeded the download size limit.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(Some(bytes))
}

fn package_url(version: &str, suffix: &str) -> Result<Url, String> {
    format!("https://github.com{RELEASES}download/v{version}/Reponiq-{version}-{suffix}")
        .parse()
        .map_err(|_| "Invalid Reponiq update version.".into())
}

async fn prepare(
    app: &AppHandle,
    target: &str,
    suffix: &str,
    limit: usize,
) -> Result<Option<Staged>, String> {
    let client = reqwest::Client::builder()
        .user_agent("Reponiq updater")
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::custom(|redirect| {
            if redirect.previous().len() >= 5 || !allow_download_host(redirect.url()) {
                redirect.error("Update redirect outside approved release source")
            } else {
                redirect.follow()
            }
        }))
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|_| FAILURE.to_string())?;
    let Some(manifest) = fetch(
        &client,
        FEED.parse().expect("fixed release URL"),
        64 << 10,
        true,
    )
    .await?
    else {
        return Ok(None);
    };
    let metadata =
        serde_json::from_slice(&manifest).map_err(|_| "The update manifest is invalid.")?;
    let updater = app
        .updater_builder()
        .target(target)
        .build()
        .map_err(|_| FAILURE.to_string())?;
    let Some(update) = updater
        .restore_update(metadata)
        .map_err(|_| "The update manifest is incompatible.")?
    else {
        return Ok(None);
    };
    if package_url(&update.version, suffix)? != update.download_url {
        return Err("The update does not point to its matching Reponiq release package.".into());
    }
    let bytes = fetch(&client, update.download_url.clone(), limit, false)
        .await?
        .ok_or(FAILURE)?;
    tauri::async_runtime::spawn_blocking(move || {
        update
            .verify(&bytes)
            .map_err(|_| "The update signature could not be verified.")?;
        Ok(Some(Staged { update, bytes }))
    })
    .await
    .map_err(|_| FAILURE.to_string())?
}

#[tauri::command]
pub(super) async fn check_app_update(app: AppHandle) -> Result<Status, String> {
    let Some((target, suffix, limit)) = package() else {
        return Ok(Status::Unavailable);
    };
    let updates = app.state::<Updates>();
    {
        let mut pending = updates.0.lock().map_err(super::err)?;
        if let Some(staged) = &pending.staged {
            return Ok(Status::Ready {
                version: staged.update.version.clone(),
            });
        }
        if pending.checking || pending.installing {
            return Ok(Status::Checking);
        }
        pending.checking = true;
    }
    let result = prepare(&app, target, suffix, limit).await;
    let mut pending = updates.0.lock().map_err(super::err)?;
    pending.checking = false;
    if let Some(staged) = result? {
        let version = staged.update.version.clone();
        pending.staged = Some(staged);
        Ok(Status::Ready { version })
    } else {
        Ok(Status::Current)
    }
}

fn begin_install(app: &AppHandle, restart: bool) -> Result<bool, String> {
    let updates = app.state::<Updates>();
    let mut pending = updates.0.lock().map_err(super::err)?;
    if pending.installing {
        return Ok(true);
    }
    if pending.staged.is_none() || (!restart && pending.install_failed) {
        return Ok(false);
    }
    let permit = Installation::start()?;
    let staged = pending.staged.take().expect("checked above");
    pending.installing = true;
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let update = staged.update.clone().restart_after_install(restart);
        match update.install(&staged.bytes) {
            Ok(()) if restart => app.restart(),
            Ok(()) => app.exit(0),
            Err(error) => {
                log::error!("Could not install signed application update: {error}");
                if let Ok(mut pending) = app.state::<Updates>().0.lock() {
                    pending.staged = Some(staged);
                    pending.installing = false;
                    pending.install_failed = true;
                }
                let _ = app.emit(
                    "app-update-error",
                    "The update could not be installed. Retry in App updates or close Reponiq to leave it for the next launch.",
                );
            }
        }
        drop(permit);
    });
    Ok(true)
}

#[tauri::command]
pub(super) fn restart_for_update(app: AppHandle) -> Result<(), String> {
    if begin_install(&app, true)? {
        Ok(())
    } else {
        Err("There is no downloaded update to install yet.".into())
    }
}

#[derive(Default, PartialEq, Eq)]
enum QuitState {
    #[default]
    Starting,
    Ready(u32),
    SavingDrafts(u32),
}

#[derive(Default)]
pub(super) struct QuitHandler(Mutex<QuitState>);

#[tauri::command]
pub(super) fn register_app_quit_handler(app: AppHandle) -> Result<(), String> {
    let handler = app.state::<QuitHandler>();
    let mut state = handler.0.lock().map_err(super::err)?;
    if *state == QuitState::Starting {
        *state = QuitState::Ready(0);
    }
    Ok(())
}

pub(super) fn request_quit(app: &AppHandle) {
    let handler = app.state::<QuitHandler>();
    let Ok(mut state) = handler.0.lock() else {
        return;
    };
    match *state {
        QuitState::Starting => {
            drop(state);
            quit_or_update(app);
        }
        QuitState::Ready(previous) => {
            let request = previous.wrapping_add(1);
            *state = QuitState::SavingDrafts(request);
            if let Err(error) = app.emit("app-quit-requested", request) {
                *state = QuitState::Ready(request);
                log::error!("Could not request a draft flush before quitting: {error}");
                return;
            }
            drop(state);
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_secs(15)).await;
                if complete_quit_request(&app, request).unwrap_or(false) {
                    let _ = app.emit("app-quit-timed-out", request);
                }
            });
        }
        QuitState::SavingDrafts(_) => {}
    }
}

#[tauri::command]
pub(super) fn finish_app_quit(
    app: AppHandle,
    request: u32,
    drafts_saved: bool,
) -> Result<bool, String> {
    let accepted = complete_quit_request(&app, request)?;
    if accepted && drafts_saved {
        quit_or_update(&app);
    }
    Ok(accepted)
}

fn complete_quit_request(app: &AppHandle, request: u32) -> Result<bool, String> {
    let handler = app.state::<QuitHandler>();
    let mut state = handler.0.lock().map_err(super::err)?;
    if *state != QuitState::SavingDrafts(request) {
        return Ok(false);
    }
    *state = QuitState::Ready(request);
    Ok(true)
}

fn quit_or_update(app: &AppHandle) {
    match begin_install(app, false) {
        Ok(false) => app.exit(0),
        Ok(true) => {}
        Err(error) => {
            let _ = app.emit("app-update-error", error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_downloads_only_follow_approved_https_release_locations() {
        assert!(allow_download_host(&FEED.parse().unwrap()));
        assert!(allow_download_host(
            &"https://release-assets.githubusercontent.com/test"
                .parse()
                .unwrap()
        ));
        for url in [
            "http://github.com/Louguerrier22/Reponiq/releases/latest",
            "https://github.com/Louguerrier22/Other/releases/latest",
            "https://github.com.evil.invalid/Louguerrier22/Reponiq/releases/latest",
            "https://user@github.com/Louguerrier22/Reponiq/releases/latest",
            "https://release-assets.githubusercontent.com:8080/test",
        ] {
            assert!(!allow_download_host(&url.parse().unwrap()), "{url}");
        }
        assert_eq!(
            package_url("1.2.0", "windows-x64.exe").unwrap().path(),
            "/Louguerrier22/Reponiq/releases/download/v1.2.0/Reponiq-1.2.0-windows-x64.exe"
        );
    }
}
