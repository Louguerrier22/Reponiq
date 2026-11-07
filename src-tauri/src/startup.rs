#[cfg(any(target_os = "linux", test))]
fn fontconfig_path(
    appdir: Option<&std::path::Path>,
    custom_config: bool,
) -> Option<std::path::PathBuf> {
    if custom_config {
        return None;
    }
    let path = appdir?.join("usr/share/reponiq/fontconfig.conf");
    path.is_file().then_some(path)
}

/// Run before GTK/WebKit or worker threads initialize. Only AppImages use the
/// bundled font stack; keep host font settings and Git's environment intact.
pub fn configure_font_cache() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "linux")]
    {
        let appdir = std::env::var_os("APPDIR");
        let path = fontconfig_path(
            appdir.as_deref().map(std::path::Path::new),
            std::env::var_os("FONTCONFIG_FILE").is_some(),
        )?;
        std::env::set_var("FONTCONFIG_FILE", &path);
        Some(path)
    }
    #[cfg(not(target_os = "linux"))]
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_cache_isolation_only_applies_to_a_packaged_appimage() {
        let appdir = tempfile::tempdir().unwrap();
        assert!(fontconfig_path(None, false).is_none());
        assert!(fontconfig_path(Some(appdir.path()), false).is_none());
        let config = appdir.path().join("usr/share/reponiq/fontconfig.conf");
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();
        std::fs::write(
            &config,
            include_str!("../../assets/fontconfig/reponiq.conf"),
        )
        .unwrap();
        assert_eq!(fontconfig_path(Some(appdir.path()), false), Some(config));
        assert!(fontconfig_path(Some(appdir.path()), true).is_none());
    }
}
