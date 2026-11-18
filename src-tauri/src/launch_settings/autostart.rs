use std::path::{Path, PathBuf};

fn executable() -> Result<PathBuf, String> {
    if cfg!(debug_assertions) {
        return Err("Start at sign-in is available in packaged builds.".into());
    }
    #[cfg(target_os = "linux")]
    if std::env::var_os("APPDIR").is_some() || std::env::var_os("APPIMAGE").is_some() {
        return std::env::var_os("APPIMAGE")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute() && p.is_file())
            .ok_or("Open Reponiq from its original AppImage to set up sign-in startup.".into());
    }
    std::env::current_exe().map_err(|_| "Could not find the Reponiq executable.".into())
}

fn quoted_executable(path: &Path, desktop_entry: bool) -> Result<String, String> {
    let path = path
        .to_str()
        .filter(|path| {
            !path.chars().any(|ch| ch.is_control() || ch == '"')
                && (!desktop_entry || !path.contains(['%', '\\', '`', '$']))
        })
        .ok_or("Reponiq's executable path contains characters that sign-in startup cannot use.")?;
    Ok(format!("\"{path}\""))
}

#[cfg(windows)]
mod platform {
    use std::io::{self, ErrorKind::NotFound};
    use winreg::{
        enums::{HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_BINARY},
        RegKey, RegValue,
    };

    const ENTRY: &str = "Reponiq";
    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const APPROVED: &str =
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

    pub fn status(_: &tauri::AppHandle, command: &str) -> io::Result<bool> {
        let user = RegKey::predef(HKEY_CURRENT_USER);
        match user
            .open_subkey_with_flags(RUN, KEY_QUERY_VALUE)
            .and_then(|key| key.get_value::<String, _>(ENTRY))
        {
            Ok(value) if value == command => {}
            Ok(_) => return Ok(false),
            Err(error) if error.kind() == NotFound => return Ok(false),
            Err(error) => return Err(error),
        }
        match user
            .open_subkey_with_flags(APPROVED, KEY_QUERY_VALUE)
            .and_then(|key| key.get_raw_value(ENTRY))
        {
            Ok(value) => Ok(value.bytes.first().is_some_and(|value| value % 2 == 0)),
            Err(error) if error.kind() == NotFound => Ok(true),
            Err(error) => Err(error),
        }
    }

    pub fn set(_: &tauri::AppHandle, command: Option<&str>) -> io::Result<()> {
        let user = RegKey::predef(HKEY_CURRENT_USER);
        if let Some(command) = command {
            let (run, _) = user.create_subkey(RUN)?;
            run.set_value(ENTRY, &command)?;
            match user.open_subkey_with_flags(APPROVED, KEY_SET_VALUE) {
                Ok(key) => key.set_raw_value(
                    ENTRY,
                    &RegValue {
                        vtype: REG_BINARY,
                        bytes: (&[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0][..]).into(),
                    },
                )?,
                Err(error) if error.kind() == NotFound => {}
                Err(error) => return Err(error),
            }
            return Ok(());
        }
        match user
            .open_subkey_with_flags(RUN, KEY_SET_VALUE)
            .and_then(|key| key.delete_value(ENTRY))
        {
            Err(error) if error.kind() == NotFound => Ok(()),
            result => result,
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::{
        fs,
        io::{self, ErrorKind::NotFound, Write},
        path::PathBuf,
    };
    use tauri::Manager;

    fn entry(app: &tauri::AppHandle) -> io::Result<PathBuf> {
        Ok(app
            .path()
            .config_dir()
            .map_err(io::Error::other)?
            .join("autostart/dev.reponiq.desktop"))
    }

    pub fn status(app: &tauri::AppHandle, command: &str) -> io::Result<bool> {
        let exec = format!("Exec={command}");
        match fs::read_to_string(entry(app)?) {
            Ok(data) => Ok(data.lines().any(|line| line == exec)
                && !data
                    .lines()
                    .any(|line| matches!(line, "Hidden=true" | "X-GNOME-Autostart-enabled=false"))),
            Err(error) if error.kind() == NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    pub fn set(app: &tauri::AppHandle, command: Option<&str>) -> io::Result<()> {
        let path = entry(app)?;
        if let Some(command) = command {
            let folder = path
                .parent()
                .ok_or_else(|| io::Error::other("Missing autostart directory"))?;
            fs::create_dir_all(folder)?;
            let mut file = tempfile::NamedTempFile::new_in(folder)?;
            writeln!(
                file,
                "[Desktop Entry]\nType=Application\nName=Reponiq\nExec={command}\nTerminal=false"
            )?;
            file.persist(path).map_err(|error| error.error)?;
            return Ok(());
        }
        match fs::remove_file(path) {
            Err(error) if error.kind() == NotFound => Ok(()),
            result => result,
        }
    }
}

pub(super) fn status(app: &tauri::AppHandle) -> Result<bool, String> {
    let command = quoted_executable(&executable()?, cfg!(target_os = "linux"))?;
    platform::status(app, &command)
        .map_err(|_| "Could not read the sign-in startup setting.".into())
}

pub(super) fn set(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let executable = executable()?;
    let command = enabled
        .then(|| quoted_executable(&executable, cfg!(target_os = "linux")))
        .transpose()?;
    platform::set(app, command.as_deref())
        .map_err(|_| "Could not update sign-in startup. Check your system settings.".into())
}

#[cfg(not(any(windows, target_os = "linux")))]
mod platform {
    use std::io;

    pub fn status(_: &tauri::AppHandle, _: &str) -> io::Result<bool> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn set(_: &tauri::AppHandle, _: Option<&str>) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_paths_with_spaces_stay_one_argument() {
        assert_eq!(
            quoted_executable(Path::new("C:/Program Files/Reponiq/Reponiq.exe"), false).unwrap(),
            "\"C:/Program Files/Reponiq/Reponiq.exe\""
        );
        assert_eq!(
            quoted_executable(Path::new("/home/user/My Apps/Reponiq.AppImage"), true).unwrap(),
            "\"/home/user/My Apps/Reponiq.AppImage\""
        );
        assert!(quoted_executable(Path::new("/home/user/%f.AppImage"), true).is_err());
    }
}
