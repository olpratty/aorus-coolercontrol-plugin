use anyhow::{Context, Result, anyhow};
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

    pub fn cpu_fan_rpm(&self) -> Result<u32> {
        self.read_hwmon_u32("fan1_input")
    }

    pub fn gpu_fan_rpm(&self) -> Result<u32> {
        self.read_hwmon_u32("fan2_input")
    }

    pub fn motherboard_temp_c(&self) -> Result<f64> {
        let millidegrees = self.read_hwmon_u32("temp3_input")?;
        Ok(millidegrees as f64 / 1000.0)
    }

    pub fn enable_fixed_fan_mode(&self) -> Result<()> {
        self.write_sysfs_value("fan_mode", 5)
    }

    pub fn reset_fan_mode(&self) -> Result<()> {
        self.write_sysfs_value("fan_mode", 0)
    }

    pub fn set_fan_duty_percent(&self, duty: i32) -> Result<()> {
        if !(0..=100).contains(&duty) {
            return Err(anyhow!("Fan duty must be between 0 and 100"));
        }

        let driver_value = ((duty as f64 * 255.0) / 100.0).round() as u32;
        self.write_sysfs_value("fan_custom_speed", driver_value)
    }

    fn read_hwmon_u32(&self, filename: &str) -> Result<u32> {
        let path = self.hwmon_path.join(filename);

        let value = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        value
            .trim()
            .parse::<u32>()
            .with_context(|| format!("Invalid value in {}", path.display()))
    }

    fn write_sysfs_value<T: std::fmt::Display>(&self, filename: &str, value: T) -> Result<()> {
        let path = self.sysfs_path.join(filename);

        fs::write(&path, value.to_string())
            .with_context(|| format!("Failed to write {}", path.display()))
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
