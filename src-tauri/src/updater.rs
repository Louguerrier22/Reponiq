use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Duration;
use tauri::utils::{config::BundleType, platform::bundle_type};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
use url::Url;

const CATALOG: &str = "https://api.github.com/repos/Louguerrier22/Reponiq/releases?per_page=50";
const PREFERENCE: &str = "app-update-selection";

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub(super) struct Selection {
    beta: bool,
    version: Option<String>,
}

#[derive(Deserialize)]
struct ReleaseEntry {
    tag_name: String,
    draft: bool,
    assets: Vec<ReleaseAsset>,
}

#[derive(Deserialize)]
struct ReleaseAsset {
    name: String,
}

fn version_tuple(version: &str) -> Option<[u32; 3]> {
    let mut result = [0; 3];
    let mut components = version.split('.');
    for value in &mut result {
        let part = components.next()?;
        if part.is_empty()
            || !part.bytes().all(|b| b.is_ascii_digit())
            || (part.len() > 1 && part.starts_with('0'))
        {
            return None;
        }
        *value = part.parse().ok()?;
    }
    components.next().is_none().then_some(result)
}

fn matches_channel(version: [u32; 3], beta: bool) -> bool {
    (version[2] != 0) == beta
}

async fn available_versions(
    app: &AppHandle,
    suffix: &str,
    beta: bool,
) -> Result<Vec<String>, String> {
    let client = reqwest::Client::builder()
        .user_agent("Reponiq updater")
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(super::err)?;
    let mut releases = Vec::new();
    let mut remaining_bytes = 16 * 1024 * 1024;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    for page in 1..=10 {
        let (entries, has_next) = tokio::time::timeout_at(deadline, async {
            let mut response = client
                .get(CATALOG)
                .query(&[("page", page)])
                .send()
                .await
                .map_err(|_| FAILURE.to_string())?
                .error_for_status()
                .map_err(|_| FAILURE.to_string())?;
            let has_next = response
                .headers()
                .get_all(reqwest::header::LINK)
                .iter()
                .filter_map(|value| value.to_str().ok())
                .flat_map(|value| value.split(','))
                .any(|link| {
                    link.split(';')
                        .skip(1)
                        .any(|parameter| parameter.trim() == "rel=\"next\"")
                });
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| FAILURE.to_string())? {
                if chunk.len() > remaining_bytes {
                    return Err(FAILURE.into());
                }
                remaining_bytes -= chunk.len();
                bytes.extend_from_slice(&chunk);
            }
            let entries: Vec<ReleaseEntry> = serde_json::from_slice(&bytes).map_err(super::err)?;
            Ok::<_, String>((entries, has_next))
        })
        .await
        .map_err(|_| FAILURE.to_string())??;
        releases.extend(entries);
        if !has_next {
            break;
        }
        if page == 10 {
            return Err("The release catalog exceeded the update check limit.".into());
        }
    }
    let current = version_tuple(&app.package_info().version.to_string()).ok_or(FAILURE)?;
    let mut versions: Vec<_> = releases
        .into_iter()
        .filter_map(|release| {
            let label = release.tag_name.strip_prefix('v')?;
            let version = version_tuple(label)?;
            let package = format!("Reponiq-{label}-{suffix}");
            let has_asset = |name: &str| release.assets.iter().any(|asset| asset.name == name);
            (!release.draft
                && version > current
                && matches_channel(version, beta)
                && has_asset("latest.json")
                && has_asset(&package)
                && has_asset(&format!("{package}.sig")))
            .then(|| (version, label.to_owned()))
        })
        .collect();
    versions.sort_unstable_by(|a, b| b.0.cmp(&a.0));
    versions.dedup_by(|a, b| a.0 == b.0);
    Ok(versions.into_iter().map(|(_, label)| label).collect())
}

fn read_selection(app: &AppHandle) -> Result<Selection, String> {
    let state = app.state::<super::State>();
    let db = state.database.lock().map_err(super::err)?;
    let saved: Option<String> = db
        .query_row(
            "SELECT value FROM preferences WHERE key=?1",
            [PREFERENCE],
            |row| row.get(0),
        )
        .optional()
        .map_err(super::err)?;
    saved.map_or_else(
        || Ok(Selection::default()),
        |value| serde_json::from_str(&value).map_err(super::err),
    )
}

#[tauri::command]
pub(super) fn app_update_selection(app: AppHandle) -> Result<Selection, String> {
    read_selection(&app)
}

#[tauri::command]
pub(super) async fn app_update_versions(app: AppHandle, beta: bool) -> Result<Vec<String>, String> {
    match package() {
        Some((_, suffix, _)) => available_versions(&app, suffix, beta).await,
        None => Ok(vec![]),
    }
}

#[tauri::command]
pub(super) fn set_app_update_selection(app: AppHandle, selection: Selection) -> Result<(), String> {
    if let Some(version) = selection.version.as_deref() {
        let selected = version_tuple(version).ok_or("Invalid version.")?;
        let current = version_tuple(&app.package_info().version.to_string()).ok_or(FAILURE)?;
        if !matches_channel(selected, selection.beta) || selected <= current {
            return Err("Choose a newer version from the selected channel.".into());
        }
    }
    let updates = app.state::<Updates>();
    let mut pending = updates.0.lock().map_err(super::err)?;
    if pending.checking || pending.installing {
        return Err("Wait for the current update operation to finish.".into());
    }
    let state = app.state::<super::State>();
    let db = state.database.lock().map_err(super::err)?;
    let value = serde_json::to_string(&selection).map_err(super::err)?;
    db.execute("INSERT INTO preferences(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [PREFERENCE, &value])
        .map_err(super::err)?;
    pending.staged = None;
    pending.install_failed = false;
    Ok(())
}

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
    let selection = read_selection(app)?;
    let versions = available_versions(app, suffix, selection.beta).await?;
    let requested = match selection.version {
        Some(version) if versions.contains(&version) => version,
        Some(_) => return Ok(None),
        None => match versions.into_iter().next() {
            Some(version) => version,
            None => return Ok(None),
        },
    };
    let manifest_url = format!("https://github.com{RELEASES}download/v{requested}/latest.json");
    let Some(manifest) = fetch(
        &client,
        manifest_url.parse().map_err(super::err)?,
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
    if update.version != requested || package_url(&update.version, suffix)? != update.download_url {
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
    fn patch_versions_belong_to_beta_and_channels_sort_numerically() {
        assert!(matches_channel(version_tuple("1.3.0").unwrap(), false));
        assert!(matches_channel(version_tuple("1.2.5").unwrap(), true));
        assert!(version_tuple("1.2.10") > version_tuple("1.2.5"));
        for invalid in ["1.2", "1.2.0-beta", "1.2.00", "1.2.0/other"] {
            assert!(version_tuple(invalid).is_none());
        }
    }

    #[test]
    fn update_downloads_only_follow_approved_https_release_locations() {
        assert!(allow_download_host(
            &package_url("1.2.5", "windows-x64.exe").unwrap()
        ));
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
