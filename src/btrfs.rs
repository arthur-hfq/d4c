use anyhow::{Context, Result, bail};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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

    let snap_name = match tag {
        Some(t) => format!("d4c_vol_{}_snap_{}", service, t),
        None => {
            let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            format!("d4c_vol_{}_snap_{}", service, ts)
        }
    };

    let dest_subvol = base_dir.join(&snap_name);

    let mut cmd = Command::new("btrfs");
    cmd.args(["subvolume", "snapshot"]);
    if readonly {
        cmd.arg("-r");
    }
    cmd.arg(&source_subvol).arg(&dest_subvol);

    let status = cmd
        .status()
        .context("Failed to execute the command 'btrfs'")?;

    if !status.success() {
        bail!("Failed to create Btrfs snapshot");
    }

    Ok(dest_subvol)
}
