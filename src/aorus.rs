use anyhow::{Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};

pub const AORUS_SYSFS_PATH: &str = "/sys/devices/platform/aorus_laptop";

#[derive(Debug, Clone)]
pub struct AorusDevice {
    pub sysfs_path: PathBuf,
    pub hwmon_path: PathBuf,
}

impl AorusDevice {
    pub fn detect() -> Result<Self> {
        let sysfs_path = PathBuf::from(AORUS_SYSFS_PATH);

        if !sysfs_path.is_dir() {
            return Err(anyhow!("AORUS platform device not found"));
        }

        let hwmon_path = find_hwmon_path(&sysfs_path)?;

        Ok(Self {
            sysfs_path,
            hwmon_path,
        })
    }
}

fn find_hwmon_path(sysfs_path: &Path) -> Result<PathBuf> {
    let hwmon_root = sysfs_path.join("hwmon");

    for entry in fs::read_dir(&hwmon_root)? {
        let path = entry?.path();

        if !path.is_dir() {
            continue;
        }

        let name_path = path.join("name");
        let Ok(name) = fs::read_to_string(name_path) else {
            continue;
        };

        if name.trim() == "aorus_laptop" {
            return Ok(path);
        }
    }

    Err(anyhow!("AORUS hwmon device not found"))
}
