use std::path::{Path, PathBuf};

const UPDATE_MARKER: &str = "update-pending";
const BACKUP_SUFFIX: &str = ".bak";

/// Check for a staged update at startup (before Iced runs).
/// Returns true if an update was applied successfully.
pub fn check_and_apply_staged_update() -> bool {
    let data_dir = crate::infra::platform::data_path(None);
    let staging_dir = data_dir.join("updates").join("staged");

    if !staging_dir.exists() {
        return false;
    }

    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Cannot determine current exe path: {e}");
            return false;
        }
    };

    let exe_dir = match current_exe.parent() {
        Some(p) => p,
        None => return false,
    };

    match apply_staged_update(&staging_dir, exe_dir, &current_exe) {
        Ok(()) => {
            log::info!("Staged update applied successfully");
            let _ = std::fs::write(data_dir.join(UPDATE_MARKER), "");
            let _ = std::fs::remove_dir_all(&staging_dir);
            true
        }
        Err(e) => {
            log::error!("Failed to apply staged update: {e}");
            eprintln!("Update failed: {e}");
            false
        }
    }
}

/// Apply a staged update by swapping binaries and assets.
fn apply_staged_update(
    staging_dir: &Path,
    exe_dir: &Path,
    current_exe: &Path,
) -> Result<(), String> {
    let binary_name = if cfg!(windows) {
        "kairos.exe"
    } else {
        "kairos"
    };

    let staged_binary = find_file_recursive(staging_dir, binary_name)
        .ok_or("Staged binary not found in extracted archive")?;

    let staged_root = staged_binary.parent().unwrap_or(staging_dir);

    // 1. Backup current binary and swap
    let exe_name = current_exe
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let bak = current_exe.with_file_name(format!("{exe_name}{BACKUP_SUFFIX}"));

    std::fs::rename(current_exe, &bak)
        .map_err(|e| format!("Failed to backup current binary: {e}"))?;

    if let Err(e) = std::fs::copy(&staged_binary, current_exe) {
        // Rollback: restore backup
        let _ = std::fs::rename(&bak, current_exe);
        return Err(format!("Failed to install new binary: {e}"));
    }

    // Restore execute permission on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        let _ = std::fs::set_permissions(current_exe, perms);
    }

    // 2. Update assets directory if present in staged
    let staged_assets = staged_root.join("assets");
    if staged_assets.exists() {
        let target_assets = exe_dir.join("assets");
        if target_assets.exists() {
            let assets_bak = exe_dir.join(format!("assets{BACKUP_SUFFIX}"));
            let _ = std::fs::rename(&target_assets, &assets_bak);
        }
        copy_dir_recursive(&staged_assets, &target_assets)
            .map_err(|e| format!("Failed to update assets: {e}"))?;
    }

    Ok(())
}

/// Clean up backup files from a previous successful update.
pub fn cleanup_after_successful_launch() {
    let data_dir = crate::infra::platform::data_path(None);
    let marker = data_dir.join(UPDATE_MARKER);

    if !marker.exists() {
        return;
    }

    let _ = std::fs::remove_file(&marker);
    log::info!("Update marker cleared — update verified successful");

    if let Ok(exe) = std::env::current_exe() {
        let exe_name = exe.file_name().unwrap().to_string_lossy().to_string();
        let bak = exe.with_file_name(format!("{exe_name}{BACKUP_SUFFIX}"));
        if bak.exists() {
            let _ = std::fs::remove_file(&bak);
            log::info!("Removed backup binary");
        }
        if let Some(dir) = exe.parent() {
            let assets_bak = dir.join(format!("assets{BACKUP_SUFFIX}"));
            if assets_bak.exists() {
                let _ = std::fs::remove_dir_all(&assets_bak);
                log::info!("Removed backup assets");
            }
        }
    }

    let updates_dir = data_dir.join("updates");
    if updates_dir.exists() {
        let _ = std::fs::remove_dir_all(&updates_dir);
    }
}

fn find_file_recursive(dir: &Path, name: &str) -> Option<PathBuf> {
    let direct = dir.join(name);
    if direct.exists() {
        return Some(direct);
    }
    // Check one level of subdirectory (archive may extract to kairos-vX.Y.Z-xxx/)
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let candidate = entry.path().join(name);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
