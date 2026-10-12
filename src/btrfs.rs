use crate::model::SnapshotInfo;
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Returns the base directory for d4c storage volumes: ~/.local/share/d4c/volumes
pub fn get_volumes_dir() -> Result<PathBuf> {
    let data_dir = dirs_or_env_var()?;
    let vol_dir = data_dir.join("d4c").join("volumes");
    fs::create_dir_all(&vol_dir).context("Failed to create base volume directory")?;
    Ok(vol_dir)
}

fn dirs_or_env_var() -> Result<PathBuf> {
    if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
        Ok(PathBuf::from(xdg_data))
    } else if let Ok(home) = std::env::var("HOME") {
        Ok(PathBuf::from(home).join(".local").join("share"))
    } else {
        bail!("Unable to determine the user's home directory");
    }
}

/// Checks if the path or its parent directory is on a Btrfs filesystem
pub fn is_btrfs(path: &Path) -> bool {
    let check_path = if path.exists() {
        path
    } else {
        path.parent().unwrap_or(path)
    };

    let status = Command::new("btrfs")
        .args(["filesystem", "df"])
        .arg(check_path)
        .output();

    match status {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

/// Returns the standard volume directory path for a service's active data
pub fn get_service_volume_path(service: &str) -> Result<PathBuf> {
    let base_dir = get_volumes_dir()?;
    Ok(base_dir.join(format!("d4c_vol_{}_data", service)))
}

/// Ensures that the service's primary data subvolume exists.
/// Creates a Btrfs subvolume if supported, or standard directory otherwise.
pub fn ensure_service_volume(service: &str) -> Result<PathBuf> {
    let target = get_service_volume_path(service)?;
    if target.exists() {
        return Ok(target);
    }

    let parent = target.parent().context("Target subvolume has no parent")?;
    fs::create_dir_all(parent)?;

    if is_btrfs(parent) {
        let status = Command::new("btrfs")
            .args(["subvolume", "create"])
            .arg(&target)
            .status();

        match status {
            Ok(s) if s.success() => return Ok(target),
            _ => {
                // Fall back to normal directory creation if btrfs subvolume create fails
                fs::create_dir_all(&target)?;
            }
        }
    } else {
        fs::create_dir_all(&target)?;
    }

    Ok(target)
}

/// Creates an atomic copy-on-write snapshot of a service volume
pub fn create_service_snapshot(
    service: &str,
    tag: Option<&str>,
    readonly: bool,
) -> Result<PathBuf> {
    let base_dir = get_volumes_dir()?;
    let source_subvol = base_dir.join(format!("d4c_vol_{}_data", service));

    if !source_subvol.exists() {
        bail!("Source subvolume not found in: {:?}", source_subvol);
    }

    let snap_tag = match tag {
        Some(t) => t.to_string(),
        None => {
            let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            format!("{}", ts)
        }
    };

    let snap_name = format!("d4c_vol_{}_snap_{}", service, snap_tag);
    let dest_subvol = base_dir.join(&snap_name);

    if dest_subvol.exists() {
        bail!("Snapshot with tag '{}' already exists for service '{}'", snap_tag, service);
    }

    let btrfs_available = is_btrfs(&source_subvol);

    if btrfs_available {
        let mut cmd = Command::new("btrfs");
        cmd.args(["subvolume", "snapshot"]);
        if readonly {
            cmd.arg("-r");
        }
        cmd.arg(&source_subvol).arg(&dest_subvol);

        let status = cmd
            .status()
            .context("Failed to execute command 'btrfs subvolume snapshot'")?;

        if !status.success() {
            // Attempt reflink copy fallback
            copy_dir_reflink(&source_subvol, &dest_subvol)
                .context("Failed to create Btrfs snapshot and fallback copy failed")?;
        }
    } else {
        // Fallback to cp --reflink=auto or recursive copy
        copy_dir_reflink(&source_subvol, &dest_subvol)?;
    }

    Ok(dest_subvol)
}

/// Clones a service volume into a new independent writable subvolume for a branch
pub fn branch_service_volume(
    source_service: &str,
    branch_service: &str,
) -> Result<PathBuf> {
    let base_dir = get_volumes_dir()?;
    let source_subvol = base_dir.join(format!("d4c_vol_{}_data", source_service));
    let branch_subvol = base_dir.join(format!("d4c_vol_{}_data", branch_service));

    if !source_subvol.exists() {
        bail!("Source service volume does not exist: {:?}", source_subvol);
    }

    if branch_subvol.exists() {
        bail!("Destination branch volume already exists: {:?}", branch_subvol);
    }

    if is_btrfs(&source_subvol) {
        // Create a writable snapshot
        let status = Command::new("btrfs")
            .args(["subvolume", "snapshot"])
            .arg(&source_subvol)
            .arg(&branch_subvol)
            .status();

        match status {
            Ok(s) if s.success() => Ok(branch_subvol),
            _ => {
                copy_dir_reflink(&source_subvol, &branch_subvol)?;
                Ok(branch_subvol)
            }
        }
    } else {
        copy_dir_reflink(&source_subvol, &branch_subvol)?;
        Ok(branch_subvol)
    }
}

/// Lists all snapshots created for a specific service
pub fn list_service_snapshots(service: &str) -> Result<Vec<SnapshotInfo>> {
    let base_dir = get_volumes_dir()?;
    let prefix = format!("d4c_vol_{}_snap_", service);
    let mut snapshots = Vec::new();

    if !base_dir.exists() {
        return Ok(snapshots);
    }

    for entry in fs::read_dir(&base_dir)? {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().to_string();

        if file_name.starts_with(&prefix) {
            let snap_tag = file_name.trim_start_matches(&prefix).to_string();
            let path = entry.path();
            let metadata = entry.metadata()?;

            let created_at = metadata
                .created()
                .or_else(|_| metadata.modified())
                .ok()
                .map(|t| {
                    let dt: DateTime<Utc> = t.into();
                    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
                });

            let readonly = check_is_readonly(&path);
            let size_bytes = calculate_dir_size(&path);

            snapshots.push(SnapshotInfo {
                name: file_name,
                service: service.to_string(),
                tag: snap_tag,
                path,
                readonly,
                size_bytes,
                created_at,
            });
        }
    }

    snapshots.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(snapshots)
}

/// Restores a service volume to a previous snapshot
pub fn restore_service_snapshot(service: &str, snapshot_tag: &str) -> Result<PathBuf> {
    let base_dir = get_volumes_dir()?;
    let target_subvol = base_dir.join(format!("d4c_vol_{}_data", service));

    // Snapshot may be specified by tag or full name
    let snap_dir = if snapshot_tag.starts_with("d4c_vol_") {
        base_dir.join(snapshot_tag)
    } else {
        base_dir.join(format!("d4c_vol_{}_snap_{}", service, snapshot_tag))
    };

    if !snap_dir.exists() {
        bail!("Snapshot '{}' not found at {:?}", snapshot_tag, snap_dir);
    }

    // Step 1: Create a safety backup snapshot of the current state before restoring
    let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let backup_tag = format!("pre_restore_{}", ts);
    let _ = create_service_snapshot(service, Some(&backup_tag), true);

    // Step 2: Remove current active volume
    delete_subvolume_or_dir(&target_subvol)?;

    // Step 3: Create a new writable snapshot from the chosen snapshot
    if is_btrfs(&snap_dir) {
        let status = Command::new("btrfs")
            .args(["subvolume", "snapshot"])
            .arg(&snap_dir)
            .arg(&target_subvol)
            .status();

        match status {
            Ok(s) if s.success() => {
                // Ensure the restored target is writable
                let _ = Command::new("btrfs")
                    .args(["property", "set", "-ts"])
                    .arg(&target_subvol)
                    .args(["ro", "false"])
                    .status();
                return Ok(target_subvol);
            }
            _ => {
                copy_dir_reflink(&snap_dir, &target_subvol)?;
            }
        }
    } else {
        copy_dir_reflink(&snap_dir, &target_subvol)?;
    }

    Ok(target_subvol)
}

/// Merges changes from a source branch volume into a target service volume
pub fn merge_service_volumes(source_service: &str, target_service: &str) -> Result<()> {
    let base_dir = get_volumes_dir()?;
    let source_subvol = base_dir.join(format!("d4c_vol_{}_data", source_service));
    let target_subvol = base_dir.join(format!("d4c_vol_{}_data", target_service));

    if !source_subvol.exists() {
        bail!("Source service volume not found: {:?}", source_subvol);
    }
    if !target_subvol.exists() {
        bail!("Target service volume not found: {:?}", target_subvol);
    }

    // Step 1: Create a safety backup snapshot of target before merging
    let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let safety_tag = format!("pre_merge_{}_{}", source_service, ts);
    let _ = create_service_snapshot(target_service, Some(&safety_tag), true);

    // Step 2: Synchronize data from source to target
    // Check if rsync is available for clean directory synchronization
    let rsync_status = Command::new("rsync")
        .args(["-a", "--delete"])
        .arg(format!("{}/", source_subvol.to_string_lossy()))
        .arg(format!("{}/", target_subvol.to_string_lossy()))
        .status();

    match rsync_status {
        Ok(s) if s.success() => Ok(()),
        _ => {
            // Fallback: replace target with new writable snapshot from source
            delete_subvolume_or_dir(&target_subvol)?;
            if is_btrfs(&source_subvol) {
                let status = Command::new("btrfs")
                    .args(["subvolume", "snapshot"])
                    .arg(&source_subvol)
                    .arg(&target_subvol)
                    .status();

                match status {
                    Ok(s) if s.success() => Ok(()),
                    _ => copy_dir_reflink(&source_subvol, &target_subvol),
                }
            } else {
                copy_dir_reflink(&source_subvol, &target_subvol)
            }
        }
    }
}

/// Creates an atomic snapshot and exports it as a compressed .tar.gz archive
pub fn export_service_snapshot(
    service: &str,
    destination: Option<&Path>,
) -> Result<PathBuf> {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let export_tag = format!("export_{}", ts);
    let snapshot_dir = create_service_snapshot(service, Some(&export_tag), true)?;

    let dest_dir = match destination {
        Some(d) => d.to_path_buf(),
        None => std::env::current_dir()?,
    };
    fs::create_dir_all(&dest_dir)?;

    let tarball_name = format!("d4c_{}_snapshot_{}.tar.gz", service, export_tag);
    let tarball_path = dest_dir.join(tarball_name);

    let status = Command::new("tar")
        .args(["-czf"])
        .arg(&tarball_path)
        .arg("-C")
        .arg(snapshot_dir.parent().context("Snapshot dir has no parent")?)
        .arg(snapshot_dir.file_name().context("Snapshot dir has no name")?)
        .status()
        .context("Failed to execute tar command for snapshot export")?;

    if !status.success() {
        bail!("tar command failed with non-zero exit status during export");
    }

    Ok(tarball_path)
}

/// Completely wipes a service's main data volume and all associated snapshots
pub fn wipe_service_storage(service: &str) -> Result<()> {
    let base_dir = get_volumes_dir()?;
    let prefix = format!("d4c_vol_{}_", service);

    if !base_dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(&base_dir)? {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        if file_name.starts_with(&prefix) {
            let path = entry.path();
            let _ = delete_subvolume_or_dir(&path);
        }
    }

    Ok(())
}

/// Deletes a Btrfs subvolume or falls back to directory removal
pub fn delete_subvolume_or_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    // If it's a read-only subvolume, set it to read-write first
    let _ = Command::new("btrfs")
        .args(["property", "set", "-ts"])
        .arg(path)
        .args(["ro", "false"])
        .output();

    // Clean contents inside the directory first so empty subvolume can be deleted by unprivileged user
    let _ = clean_dir_contents(path);

    // Try btrfs subvolume delete (capture output to suppress non-root permission warnings)
    let btrfs_del = Command::new("btrfs")
        .args(["subvolume", "delete"])
        .arg(path)
        .output();

    if btrfs_del.is_ok_and(|out| out.status.success()) {
        return Ok(());
    }

    // Fallback: standard directory removal
    if path.is_dir() {
        if fs::remove_dir(path).is_ok() {
            return Ok(());
        }
        let _ = fs::remove_dir_all(path);
    } else {
        let _ = fs::remove_file(path);
    }

    Ok(())
}

fn clean_dir_contents(path: &Path) -> Result<()> {
    if !path.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let child_path = entry.path();
        if child_path.is_dir() {
            let _ = clean_dir_contents(&child_path);
            let _ = delete_subvolume_or_dir(&child_path);
        } else {
            let _ = fs::remove_file(&child_path);
        }
    }
    Ok(())
}

fn check_is_readonly(path: &Path) -> bool {
    let output = Command::new("btrfs")
        .args(["property", "get", "-ts"])
        .arg(path)
        .arg("ro")
        .output();

    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        text.contains("ro=true")
    } else {
        false
    }
}

fn calculate_dir_size(path: &Path) -> u64 {
    let mut total_size = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    total_size += calculate_dir_size(&entry.path());
                } else {
                    total_size += meta.len();
                }
            }
        }
    }
    total_size
}

fn copy_dir_reflink(src: &Path, dst: &Path) -> Result<()> {
    // Attempt cp --reflink=auto -a src dst
    let status = Command::new("cp")
        .args(["--reflink=auto", "-a"])
        .arg(src)
        .arg(dst)
        .status();

    if status.is_ok_and(|s| s.success()) {
        return Ok(());
    }

    // Fallback: copy tree manually
    copy_dir_recursive(src, dst)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dest_child = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_child)?;
        } else {
            fs::copy(entry.path(), dest_child)?;
        }
    }
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_volume_path() {
        let path = get_service_volume_path("test-service").expect("Failed to get service volume path");
        assert!(path.to_string_lossy().ends_with("d4c_vol_test-service_data"));
    }

    #[test]
    fn test_volume_lifecycle() {
        let test_svc = format!("test_unit_{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros());

        // 1. Ensure volume
        let vol = ensure_service_volume(&test_svc).expect("Failed to ensure service volume");
        assert!(vol.exists());

        // Write a test file into volume
        let test_file = vol.join("hello.txt");
        fs::write(&test_file, "initial data").expect("Failed to write test file");

        // 2. Create snapshot
        let snap = create_service_snapshot(&test_svc, Some("v1"), false).expect("Failed to create snapshot");
        assert!(snap.exists());
        assert!(snap.join("hello.txt").exists());

        // 3. List snapshots
        let list = list_service_snapshots(&test_svc).expect("Failed to list snapshots");
        assert!(list.iter().any(|s| s.tag == "v1"));

        // 4. Branch volume
        let branch_svc = format!("{}_branch", test_svc);
        let branch_vol = branch_service_volume(&test_svc, &branch_svc).expect("Failed to branch volume");
        assert!(branch_vol.exists());
        assert_eq!(fs::read_to_string(branch_vol.join("hello.txt")).unwrap(), "initial data");

        // Mutate branch volume
        fs::write(branch_vol.join("hello.txt"), "branched data").unwrap();
        fs::write(branch_vol.join("branch_only.txt"), "new file").unwrap();

        // 5. Merge branch into target
        merge_service_volumes(&branch_svc, &test_svc).expect("Failed to merge volumes");
        assert_eq!(fs::read_to_string(vol.join("hello.txt")).unwrap(), "branched data");
        assert!(vol.join("branch_only.txt").exists());

        // 6. Restore from original v1 snapshot
        restore_service_snapshot(&test_svc, "v1").expect("Failed to restore snapshot");
        assert_eq!(fs::read_to_string(vol.join("hello.txt")).unwrap(), "initial data");

        // 7. Export snapshot to tarball
        let temp_dir = std::env::temp_dir().join(format!("d4c_test_export_{}", test_svc));
        fs::create_dir_all(&temp_dir).expect("Failed to create tempdir");
        let exported = export_service_snapshot(&test_svc, Some(&temp_dir)).expect("Failed to export snapshot");
        assert!(exported.exists());
        assert!(exported.to_string_lossy().ends_with(".tar.gz"));
        let _ = fs::remove_dir_all(&temp_dir);

        // 8. Wipe service storage
        wipe_service_storage(&test_svc).expect("Failed to wipe test service");
        wipe_service_storage(&branch_svc).expect("Failed to wipe branch service");
        assert!(!vol.exists());
        assert!(!branch_vol.exists());
    }
}
