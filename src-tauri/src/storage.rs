use std::path::{Path, PathBuf};

pub struct Storage {
    pub database: PathBuf,
    pub credential_service: &'static str,
}

// Keep an existing installation on its original database and keyring namespace.
// This avoids copying live SQLite/WAL files or stranding credentials on upgrade.
pub fn resolve(root: &Path, windows: bool) -> Result<Storage, String> {
    let legacy = root
        .join(if windows { "ForgeDesk" } else { "forgedesk" })
        .join("forgedesk.sqlite");
    if legacy.try_exists().map_err(|e| e.to_string())? {
        return Ok(Storage {
            database: legacy,
            credential_service: "forgedesk",
        });
    }
    Ok(Storage {
        database: root
            .join(if windows { "Reponiq" } else { "reponiq" })
            .join("reponiq.sqlite"),
        credential_service: "reponiq",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_existing_data_and_rollback_on_both_platform_layouts() {
        let root =
            std::env::temp_dir().join(format!("reponiq-storage-test-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        for windows in [false, true] {
            let fresh = resolve(&root, windows).unwrap();
            assert_eq!(fresh.credential_service, "reponiq");
            assert_eq!(fresh.database.file_name().unwrap(), "reponiq.sqlite");
            let old = root
                .join(if windows { "ForgeDesk" } else { "forgedesk" })
                .join("forgedesk.sqlite");
            std::fs::create_dir_all(old.parent().unwrap()).unwrap();
            std::fs::write(&old, b"existing data").unwrap();
            // An existing legacy store remains authoritative even if both exist.
            std::fs::create_dir_all(fresh.database.parent().unwrap()).unwrap();
            std::fs::write(&fresh.database, b"separate new store").unwrap();
            let upgraded = resolve(&root, windows).unwrap();
            assert_eq!(upgraded.database, old);
            assert_eq!(upgraded.credential_service, "forgedesk");
            std::fs::write(&upgraded.database, b"new legitimate write").unwrap();
            assert_eq!(std::fs::read(&old).unwrap(), b"new legitimate write");
            assert_eq!(resolve(&root, windows).unwrap().database, old);
            std::fs::remove_file(&old).unwrap();
            std::fs::remove_file(&fresh.database).unwrap();
        }
        // Only remove the isolated test directory, never any application data.
        std::fs::remove_dir_all(&root).unwrap();
    }
}
