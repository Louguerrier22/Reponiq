use super::{background, err, State};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

const THEME_KEY: &str = "appearance-v1";
const REGISTRY_THEME_KEY: &str = "appearance-registry-v1";
const STEP_KEY: &str = "onboarding-step-v1";
const ACCOUNT_STEP_KEY: &str = "onboarding-accounts-pending-v1";
const LEGACY_SETUP_KEY: &str = "workspace-setup-complete";

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Preset {
    #[default]
    Reponiq,
    Nova,
    Lyra,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Light,
    #[default]
    Dark,
    System,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Palette {
    Graphite,
    Neutral,
    Slate,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Accent {
    Lavender,
    Indigo,
    Teal,
    Rose,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Radius {
    Square,
    Soft,
    Rounded,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Density {
    Compact,
    Comfortable,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum TextSize {
    Standard,
    Large,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Theme {
    preset: Preset,
    mode: Mode,
    palette: Option<Palette>,
    accent: Option<Accent>,
    radius: Option<Radius>,
    density: Option<Density>,
    text_size: Option<TextSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    imported: Option<super::registry_theme::RegistryTheme>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SetupStep {
    Appearance,
    Workspace,
    Accounts,
    Startup,
    Review,
    Complete,
}

impl SetupStep {
    fn next(self) -> Self {
        match self {
            Self::Appearance => Self::Workspace,
            Self::Workspace => Self::Accounts,
            Self::Accounts => Self::Startup,
            Self::Startup => Self::Review,
            Self::Review | Self::Complete => Self::Complete,
        }
    }
}

#[derive(Serialize)]
pub(super) struct AppearanceSettings {
    theme: Theme,
    setup_step: SetupStep,
}

fn read(db: &Connection, key: &str) -> Result<Option<String>, String> {
    db.query_row("SELECT value FROM preferences WHERE key=?1", [key], |row| {
        row.get(0)
    })
    .optional()
    .map_err(err)
}

fn write(db: &Connection, key: &str, value: &str) -> Result<(), String> {
    db.execute(
        "INSERT INTO preferences(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key, value],
    ).map(|_| ()).map_err(err)
}

fn setup_step(db: &Connection) -> Result<SetupStep, String> {
    match read(db, LEGACY_SETUP_KEY)?.as_deref() {
        Some("true") => return Ok(SetupStep::Complete),
        None | Some("false") => {}
        _ => {
            return Err(
                "Saved setup preferences are unsupported. Your settings were preserved.".into(),
            )
        }
    }
    let step = read(db, STEP_KEY)?
        .map(|value| {
            serde_json::from_str(&value).map_err(|_| {
                "Saved setup progress is unsupported. Your settings were preserved.".to_owned()
            })
        })
        .unwrap_or(Ok(SetupStep::Appearance))?;
    if step == SetupStep::Startup && read(db, ACCOUNT_STEP_KEY)?.as_deref() == Some("true") {
        return Ok(SetupStep::Accounts);
    }
    Ok(step)
}

fn theme(db: &Connection) -> Result<Theme, String> {
    let mut result: Theme = read(db, THEME_KEY)?
        .map(|value| {
            serde_json::from_str(&value).map_err(|_| {
                "Saved appearance is unsupported. Your settings were preserved.".to_owned()
            })
        })
        .unwrap_or_else(|| Ok(Theme::default()))?;
    if let Some(saved) = read(db, REGISTRY_THEME_KEY)? {
        result.imported = serde_json::from_str(&saved).map_err(|_| {
            "Saved theme data is unsupported. Your settings were preserved.".to_string()
        })?;
        if let Some(imported) = &mut result.imported {
            imported.validate()?;
        }
    }
    Ok(result)
}

fn advance(db: &mut Connection, expected: SetupStep) -> Result<SetupStep, String> {
    let tx = db.transaction().map_err(err)?;
    let current = setup_step(&tx)?;
    if current != expected || current == SetupStep::Complete {
        return Ok(current);
    }
    let next = current.next();
    // Older builds resume at Startup if setup stops at the GitHub step.
    let stored_step = if next == SetupStep::Accounts {
        write(&tx, ACCOUNT_STEP_KEY, "true")?;
        SetupStep::Startup
    } else {
        write(&tx, ACCOUNT_STEP_KEY, "false")?;
        next
    };
    write(
        &tx,
        STEP_KEY,
        &serde_json::to_string(&stored_step).map_err(err)?,
    )?;
    if next == SetupStep::Complete {
        write(&tx, LEGACY_SETUP_KEY, "true")?;
    }
    tx.commit().map_err(err)?;
    Ok(next)
}

#[tauri::command]
pub(super) async fn appearance_settings(
    state: tauri::State<'_, State>,
) -> Result<AppearanceSettings, String> {
    background(state, |s| {
        let db = s.database.lock().map_err(err)?;
        Ok(AppearanceSettings {
            theme: theme(&db)?,
            setup_step: setup_step(&db)?,
        })
    })
    .await
}

#[tauri::command]
pub(super) async fn save_appearance(
    mut input: Theme,
    state: tauri::State<'_, State>,
) -> Result<Theme, String> {
    background(state, |s| {
        let mut db = s.database.lock().map_err(err)?;
        let tx = db.transaction().map_err(err)?;
        theme(&tx)?;
        let mut imported = input.imported.take();
        if let Some(value) = &mut imported {
            value.validate()?;
        }
        write(&tx, THEME_KEY, &serde_json::to_string(&input).map_err(err)?)?;
        write(
            &tx,
            REGISTRY_THEME_KEY,
            &serde_json::to_string(&imported).map_err(err)?,
        )?;
        tx.commit().map_err(err)?;
        input.imported = imported;
        Ok(input)
    })
    .await
}

#[tauri::command]
pub(super) async fn advance_setup(
    expected: SetupStep,
    state: tauri::State<'_, State>,
) -> Result<SetupStep, String> {
    background(state, move |s| {
        advance(&mut *s.database.lock().map_err(err)?, expected)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute(
            "CREATE TABLE preferences(key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            [],
        )
        .unwrap();
        db
    }

    #[test]
    fn first_run_checkpoints_resume_and_finish_for_older_builds() {
        let mut db = database();
        assert_eq!(setup_step(&db).unwrap(), SetupStep::Appearance);
        assert_eq!(
            advance(&mut db, SetupStep::Appearance).unwrap(),
            SetupStep::Workspace
        );
        assert_eq!(
            advance(&mut db, SetupStep::Appearance).unwrap(),
            SetupStep::Workspace
        );
        assert_eq!(setup_step(&db).unwrap(), SetupStep::Workspace);
        assert_eq!(
            advance(&mut db, SetupStep::Workspace).unwrap(),
            SetupStep::Accounts
        );
        assert_eq!(setup_step(&db).unwrap(), SetupStep::Accounts);
        assert_eq!(read(&db, STEP_KEY).unwrap().as_deref(), Some("\"startup\""));
        assert_eq!(
            advance(&mut db, SetupStep::Accounts).unwrap(),
            SetupStep::Startup
        );
        assert_eq!(
            advance(&mut db, SetupStep::Startup).unwrap(),
            SetupStep::Review
        );
        assert_eq!(
            advance(&mut db, SetupStep::Review).unwrap(),
            SetupStep::Complete
        );
        assert_eq!(
            read(&db, LEGACY_SETUP_KEY).unwrap().as_deref(),
            Some("true")
        );
        assert_eq!(read(&db, "walkthrough-v1").unwrap(), None);
    }

    #[test]
    fn existing_setup_and_new_appearance_survive_legacy_preference_writes() {
        let db = database();
        write(&db, LEGACY_SETUP_KEY, "true").unwrap();
        let expected = Theme {
            preset: Preset::Lyra,
            mode: Mode::System,
            accent: Some(Accent::Teal),
            ..Theme::default()
        };
        write(&db, THEME_KEY, &serde_json::to_string(&expected).unwrap()).unwrap();
        write(&db, "default-repository-folder", "/repositories").unwrap();
        write(&db, "walkthrough-v1", "seen").unwrap();
        assert_eq!(setup_step(&db).unwrap(), SetupStep::Complete);
        assert_eq!(theme(&db).unwrap(), expected);
        write(&db, THEME_KEY, r#"{"preset":"future","mode":"system"}"#).unwrap();
        assert!(theme(&db).is_err());
        assert!(read(&db, THEME_KEY).unwrap().unwrap().contains("future"));
    }
}
