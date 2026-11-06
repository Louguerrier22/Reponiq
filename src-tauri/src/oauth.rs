use super::{background, gh_client, gh_get, save_account, Account, GithubUser, State};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

pub(super) static ACCOUNT_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
const DEVICE_URL: &str = "https://github.com/login/device/code";
const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const VERIFY_URL: &str = "https://github.com/login/device";

#[derive(Default)]
pub struct LoginState(Mutex<Flows>);

#[derive(Default)]
struct Flows {
    sequence: u64,
    active: Option<Session>,
}
struct Session {
    id: u64,
    client_id: String,
    device_code: Option<String>,
    expires: Instant,
    next_poll: Instant,
    interval: u64,
    polling: bool,
}
impl Flows {
    fn reserve(&mut self, client_id: String) -> Result<u64, String> {
        if self
            .active
            .as_ref()
            .is_some_and(|s| s.expires > Instant::now())
        {
            return Err("A GitHub sign-in is already in progress.".into());
        }
        self.sequence += 1;
        let id = self.sequence;
        self.active = Some(Session {
            id,
            client_id,
            device_code: None,
            expires: Instant::now() + Duration::from_secs(900),
            next_poll: Instant::now(),
            interval: 5,
            polling: true,
        });
        Ok(id)
    }
    fn session(&mut self, id: u64) -> Result<&mut Session, String> {
        if self
            .active
            .as_ref()
            .is_some_and(|s| s.expires <= Instant::now())
        {
            self.active = None;
        }
        self.active
            .as_mut()
            .filter(|s| s.id == id)
            .ok_or_else(|| "GitHub sign-in expired or was canceled. Try again.".into())
    }
    fn cancel(&mut self, id: u64) {
        if self.active.as_ref().is_some_and(|s| s.id == id) {
            self.active = None;
        }
    }
}
fn lock_error<T>(_: T) -> String {
    "GitHub sign-in is unavailable. Restart Reponiq and try again.".into()
}
fn client_id() -> Result<String, String> {
    #[derive(Deserialize)]
    struct Config {
        client_id: String,
    }
    let config: Config =
        serde_json::from_str(include_str!("../github-oauth.json")).map_err(lock_error)?;
    let id = if config.client_id.is_empty() {
        std::env::var("REPONIQ_GITHUB_CLIENT_ID").unwrap_or_default()
    } else {
        config.client_id
    };
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    {
        return Err(
            "Browser sign-in is not configured in this build. Use a personal access token.".into(),
        );
    }
    Ok(id)
}
#[tauri::command]
pub fn browser_login_available() -> bool {
    client_id().is_ok()
}

#[derive(Deserialize)]
struct DeviceCode {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}
#[derive(Serialize)]
pub struct LoginCode {
    id: u64,
    user_code: String,
    expires_in: u64,
    interval: u64,
}

async fn json_body<T: DeserializeOwned>(mut response: reqwest::Response) -> Result<T, String> {
    if !response.status().is_success() {
        return Err(
            "GitHub could not complete sign-in. Check your connection and try again.".into(),
        );
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Could not read GitHub's response. Try again.")?
    {
        if bytes.len() + chunk.len() > 65536 {
            return Err("GitHub returned an unexpected response. Try again.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| "GitHub returned an unexpected response. Try again.".into())
}

#[tauri::command]
pub async fn start_github_login(
    private_repos: bool,
    login: tauri::State<'_, LoginState>,
) -> Result<LoginCode, String> {
    let client_id = client_id()?;
    let id = login
        .0
        .lock()
        .map_err(lock_error)?
        .reserve(client_id.clone())?;
    let result = async {
        let client = gh_client()?;
        let response = client
            .post(DEVICE_URL)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", client_id.as_str()),
                (
                    "scope",
                    if private_repos {
                        "repo read:org"
                    } else {
                        "public_repo read:org"
                    },
                ),
            ])
            .send()
            .await
            .map_err(|_| "Could not reach GitHub. Check your connection and try again.")?;
        let code: DeviceCode = json_body(response).await?;
        if code.verification_uri != VERIFY_URL
            || code.device_code.is_empty()
            || code.device_code.len() > 4096
            || code.user_code.is_empty()
            || code.user_code.len() > 32
            || !code
                .user_code
                .bytes()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'-')
            || !(1..=900).contains(&code.expires_in)
            || !(1..=60).contains(&code.interval)
        {
            return Err("GitHub returned an unexpected sign-in code. Try again.".into());
        }
        let mut flows = login.0.lock().map_err(lock_error)?;
        let session = flows.session(id)?;
        session.device_code = Some(code.device_code);
        session.expires = Instant::now() + Duration::from_secs(code.expires_in);
        session.interval = code.interval;
        session.next_poll = Instant::now() + Duration::from_secs(code.interval);
        session.polling = false;
        Ok(LoginCode {
            id,
            user_code: code.user_code,
            expires_in: code.expires_in,
            interval: code.interval,
        })
    }
    .await;
    if result.is_err() {
        login.0.lock().map_err(lock_error)?.cancel(id);
    }
    result
}

#[tauri::command]
pub async fn open_github_login(
    id: u64,
    app: tauri::AppHandle,
    login: tauri::State<'_, LoginState>,
) -> Result<(), String> {
    login.0.lock().map_err(lock_error)?.session(id)?;
    tauri::async_runtime::spawn_blocking(move || app.opener().open_url(VERIFY_URL, None::<&str>))
        .await
        .map_err(lock_error)?
        .map_err(|_| {
            "Could not open your browser. Open github.com/login/device and enter the code.".into()
        })
}
#[tauri::command]
pub fn cancel_github_login(id: u64, login: tauri::State<'_, LoginState>) -> Result<(), String> {
    login.0.lock().map_err(lock_error)?.cancel(id);
    Ok(())
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    token_type: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    refresh_token_expires_in: Option<u64>,
    error: Option<String>,
    interval: Option<u64>,
}
#[derive(Serialize, Deserialize, Clone)]
pub(super) struct Credential {
    version: u8,
    client_id: String,
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<i64>,
    refresh_expires_at: Option<i64>,
    #[serde(default)]
    account_login: Option<String>,
    #[serde(default)]
    account_created_at: Option<i64>,
    #[serde(default)]
    previous_access_token: Option<String>,
}
impl Credential {
    pub(super) fn bind_account(&mut self, login: &str, created_at: i64) {
        self.account_login = Some(login.to_owned());
        self.account_created_at = Some(created_at);
    }
    fn validate_binding(
        &self,
        login: &str,
        created_at: i64,
        raw: Option<&str>,
    ) -> Result<(), String> {
        let matches = self.version == 2
            && self
                .account_login
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case(login))
            && self.account_created_at == Some(created_at)
            && raw.is_some_and(|token| {
                token == self.access_token || self.previous_access_token.as_deref() == Some(token)
            });
        if matches {
            Ok(())
        } else {
            Err(
                "Saved sign-in does not match this account. Reconnect the selected GitHub account."
                    .into(),
            )
        }
    }

    fn from_response(response: TokenResponse, client_id: String) -> Result<Self, String> {
        let token = response
            .access_token
            .filter(|t| !t.is_empty() && t.len() <= 4096 && !t.chars().any(char::is_whitespace))
            .ok_or("GitHub sign-in was not completed. Try again.")?;
        if response.token_type.as_deref() != Some("bearer") {
            return Err("GitHub returned an unsupported token. Try again.".into());
        }
        let expires_at = response
            .expires_in
            .map(|s| super::now() + s.min(31_536_000) as i64);
        let refresh_expires_at = response
            .refresh_token_expires_in
            .map(|s| super::now() + s.min(31_536_000) as i64);
        if response
            .refresh_token
            .as_ref()
            .is_some_and(|t| t.is_empty() || t.len() > 4096 || t.chars().any(char::is_whitespace))
        {
            return Err("GitHub returned an unexpected token. Try again.".into());
        }
        Ok(Self {
            version: 2,
            client_id,
            access_token: token,
            refresh_token: response.refresh_token,
            expires_at,
            refresh_expires_at,
            account_login: None,
            account_created_at: None,
            previous_access_token: None,
        })
    }
}
#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LoginStatus {
    Pending {
        retry_after: u64,
        message: Option<&'static str>,
    },
    Connected {
        account: Account,
    },
}

#[tauri::command]
pub async fn poll_github_login(
    id: u64,
    app: tauri::AppHandle,
    login: tauri::State<'_, LoginState>,
    state: tauri::State<'_, State>,
) -> Result<LoginStatus, String> {
    let (device_code, client_id) = {
        let mut flows = login.0.lock().map_err(lock_error)?;
        let session = flows.session(id)?;
        if session.polling || session.next_poll > Instant::now() {
            return Ok(LoginStatus::Pending {
                retry_after: session
                    .next_poll
                    .saturating_duration_since(Instant::now())
                    .as_secs()
                    .saturating_add(1)
                    .max(1),
                message: None,
            });
        }
        session.polling = true;
        (
            session
                .device_code
                .clone()
                .ok_or("GitHub sign-in is still starting.")?,
            session.client_id.clone(),
        )
    };
    let client = gh_client()?;
    let response = client
        .post(TOKEN_URL)
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id.as_str()),
            ("device_code", device_code.as_str()),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await;
    let response = match response {
        Ok(r) => json_body::<TokenResponse>(r).await,
        Err(_) => Err("Connection failed".into()),
    };
    let token = match response {
        Ok(response) if response.access_token.is_some() => response,
        response => {
            let mut flows = login.0.lock().map_err(lock_error)?;
            let session = flows.session(id)?;
            session.polling = false;
            let mut message = None;
            match response {
                Ok(response) => match response.error.as_deref() {
                    Some("authorization_pending") => {}
                    Some("slow_down") => {
                        session.interval = response
                            .interval
                            .unwrap_or(0)
                            .max(session.interval.saturating_add(5))
                    }
                    other => {
                        let text = match other {
                            Some("access_denied") => "GitHub sign-in was declined.",
                            Some("expired_token") | Some("token_expired") => {
                                "The sign-in code expired. Try again."
                            }
                            Some("device_flow_disabled") => {
                                "Device sign-in is not enabled for this GitHub app."
                            }
                            _ => "GitHub sign-in could not be completed. Try again.",
                        };
                        flows.cancel(id);
                        return Err(text.into());
                    }
                },
                Err(_) => message = Some("Could not reach GitHub. Retrying…"),
            }
            session.next_poll = Instant::now() + Duration::from_secs(session.interval.min(900));
            return Ok(LoginStatus::Pending {
                retry_after: session.interval.min(900),
                message,
            });
        }
    };
    let result = async {
        let credential = Credential::from_response(token, client_id)?;
        let user: GithubUser = json_body(
            gh_get(
                &client,
                "https://api.github.com/user",
                &credential.access_token,
            )
            .await?,
        )
        .await?;
        let _account_guard = ACCOUNT_LOCK.lock().await;
        // Hold the flow lock only for the final blocking save. Cancellation that wins
        // this lock prevents persistence; a completed save wins before cancellation.
        background(state, move |state| {
            let login = app.state::<LoginState>();
            let mut flows = login.0.lock().map_err(lock_error)?;
            flows.session(id)?;
            let account = save_account(
                user.login,
                "https://api.github.com".into(),
                &credential.access_token,
                Some(&credential),
                state,
            );
            flows.cancel(id);
            account
        })
        .await
        .map(|account| LoginStatus::Connected { account })
    }
    .await;
    if result.is_err() {
        login.0.lock().map_err(lock_error)?.cancel(id);
    }
    result
}

pub(super) fn entry(service: &str, id: i64, oauth: bool) -> Result<keyring::Entry, String> {
    keyring::Entry::new(
        service,
        &format!("github-{}-{id}", if oauth { "oauth" } else { "account" }),
    )
    .map_err(|_| "Could not access the OS credential store.".into())
}
pub(super) fn read_secret(entry: &keyring::Entry) -> Result<Option<String>, String> {
    match entry.get_password() {
        Ok(s) => Ok(Some(s)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err("Could not read the OS credential store. Unlock it and try again.".into()),
    }
}
pub(super) fn write_secret(entry: &keyring::Entry, value: Option<&str>) -> Result<(), String> {
    match value {
        Some(value)=>entry.set_password(value).map_err(|_|"Could not save to the OS credential store. Unlock it and try again.".into()),
        None=>match entry.delete_credential() { Ok(())|Err(keyring::Error::NoEntry)=>Ok(()),Err(_)=>Err("Could not remove the saved credential. Unlock the OS credential store and try again.".into()) }
    }
}

pub(super) async fn account_token(
    id: i64,
    state: tauri::State<'_, State>,
) -> Result<String, String> {
    let (raw, metadata, login, created_at) = background(state.clone(), move |state| {
        let (login, created_at) = state
            .database
            .lock()
            .map_err(lock_error)?
            .query_row(
                "SELECT login,created_at FROM accounts WHERE id=?1",
                [id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
            )
            .map_err(|_| "Select a connected GitHub account.".to_string())?;
        Ok((
            read_secret(&entry(state.credential_service, id, false)?)?,
            read_secret(&entry(state.credential_service, id, true)?)?,
            login,
            created_at,
        ))
    })
    .await?;
    let Some(metadata) = metadata else {
        return raw.ok_or("No saved token. Reconnect this GitHub account.".into());
    };
    let credential: Credential = serde_json::from_str(&metadata)
        .map_err(|_| "Saved sign-in data is not supported. Reconnect this GitHub account.")?;
    credential.validate_binding(&login, created_at, raw.as_deref())?;
    if !credential
        .expires_at
        .is_some_and(|expiry| expiry <= super::now() + 60)
    {
        return Ok(credential.access_token);
    }
    let refresh = credential
        .refresh_token
        .as_deref()
        .filter(|_| {
            !credential
                .refresh_expires_at
                .is_some_and(|expiry| expiry <= super::now())
        })
        .ok_or("GitHub sign-in expired. Reconnect this account.")?;
    let response = gh_client()?
        .post(TOKEN_URL)
        .header("Accept", "application/json")
        .form(&[
            ("client_id", credential.client_id.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh),
        ])
        .send()
        .await
        .map_err(|_| "Could not refresh GitHub sign-in. Check your connection and try again.")?;
    let response: TokenResponse = json_body(response).await?;
    let mut credential = Credential::from_response(response, credential.client_id)?;
    credential.bind_account(&login, created_at);
    credential.previous_access_token = raw;
    let token = credential.access_token.clone();
    background(state, move |state| {
        // Persist the rotated refresh token first: it is the recovery authority if
        // updating the legacy access-token mirror subsequently fails.
        write_secret(
            &entry(state.credential_service, id, true)?,
            Some(&serde_json::to_string(&credential).map_err(lock_error)?),
        )?;
        write_secret(
            &entry(state.credential_service, id, false)?,
            Some(&credential.access_token),
        )
    })
    .await?;
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_writer_account_id_reuse_cannot_select_an_orphaned_oauth_identity() {
        let raw =
            keyring::Entry::new_with_credential(Box::new(keyring::mock::MockCredential::default()));
        let response: TokenResponse =
            serde_json::from_str(r#"{"access_token":"oauth-a","token_type":"bearer"}"#).unwrap();
        let mut credential = Credential::from_response(response, "client".into()).unwrap();
        credential.bind_account("account-a", 100);
        write_secret(&raw, Some("oauth-a")).unwrap();
        assert!(credential
            .validate_binding("account-a", 100, read_secret(&raw).unwrap().as_deref())
            .is_ok());
        // Previous app disconnects A but leaves the new, unknown sidecar.
        write_secret(&raw, None).unwrap();
        assert!(credential.validate_binding("account-a", 100, None).is_err());
        // Its next PAT account reuses the numeric database ID.
        write_secret(&raw, Some("pat-b")).unwrap();
        assert!(credential
            .validate_binding("account-b", 101, read_secret(&raw).unwrap().as_deref())
            .is_err());
        assert!(credential
            .validate_binding("account-a", 100, Some("pat-b"))
            .is_err());
        // A failed mirror update after rotation remains recoverable for A only.
        credential.previous_access_token = Some("oauth-a".into());
        credential.access_token = "rotated-a".into();
        assert!(credential
            .validate_binding("account-a", 100, Some("oauth-a"))
            .is_ok());
        assert!(credential
            .validate_binding("account-b", 101, Some("oauth-a"))
            .is_err());
    }
    #[test]
    fn credential_store_errors_are_not_treated_as_missing_or_exposed() {
        let entry =
            keyring::Entry::new_with_credential(Box::new(keyring::mock::MockCredential::default()));
        assert!(read_secret(&entry).unwrap().is_none());
        write_secret(&entry, Some("fixture-secret")).unwrap();
        assert_eq!(
            read_secret(&entry).unwrap().as_deref(),
            Some("fixture-secret")
        );
        let mock = entry
            .get_credential()
            .downcast_ref::<keyring::mock::MockCredential>()
            .unwrap();
        mock.set_error(keyring::Error::Invalid(
            "private-fixture-detail".into(),
            "private-error".into(),
        ));
        let error = read_secret(&entry).unwrap_err();
        assert!(!error.contains("private-fixture"));
        assert!(error.contains("credential store"));
        write_secret(&entry, None).unwrap();
        assert!(read_secret(&entry).unwrap().is_none());
        write_secret(&entry, None).unwrap();
    }
    #[test]
    fn cancellation_and_expiry_cannot_complete_another_flow() {
        let mut flows = Flows::default();
        let first = flows.reserve("client".into()).unwrap();
        assert!(flows.reserve("client".into()).is_err());
        flows.cancel(first);
        let second = flows.reserve("client".into()).unwrap();
        flows.cancel(first);
        assert!(flows.session(second).is_ok());
        assert!(flows.session(first).is_err());
        flows.active.as_mut().unwrap().expires = Instant::now();
        assert!(flows.session(second).is_err());
        assert!(flows.reserve("client".into()).is_ok());
    }
    #[test]
    fn expiring_credentials_keep_refresh_and_legacy_token_separate_from_public_dto() {
        let token: TokenResponse=serde_json::from_str(r#"{"access_token":"access-fixture","token_type":"bearer","refresh_token":"refresh-fixture","expires_in":28800,"refresh_token_expires_in":15552000}"#).unwrap();
        let credential = Credential::from_response(token, "client".into()).unwrap();
        assert!(credential.expires_at.unwrap() > super::super::now() + 28000);
        assert_eq!(credential.refresh_token.as_deref(), Some("refresh-fixture"));
        let json = serde_json::to_string(&LoginCode {
            id: 1,
            user_code: "ABCD-1234".into(),
            expires_in: 900,
            interval: 5,
        })
        .unwrap();
        assert!(!json.contains("access"));
        assert!(!json.contains("refresh"));
        assert!(!json.contains("device_code"));
    }
}
