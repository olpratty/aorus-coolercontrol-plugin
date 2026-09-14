use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::{Mutex, OnceCell};
use tokio::time::{sleep, timeout};
use tokio_util::sync::CancellationToken;
use zbus::{Connection, Proxy};

pub const AORUS_SYSFS_PATH: &str = "/sys/devices/platform/aorus_laptop";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum FanMode {
    Normal = 0,
    Fixed = 5,
}

pub struct AorusDevice {
    pub sysfs_path: PathBuf,
    pub hwmon_path: PathBuf,
    connection: OnceCell<Connection>,
    manual_ready: Mutex<bool>,
    shutdown_token: CancellationToken,
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
            connection: OnceCell::new(),
            manual_ready: Mutex::new(false),
            shutdown_token: CancellationToken::new(),
        })
    }

    pub fn cpu_fan_rpm(&self) -> Result<u32> {
        self.read_hwmon_u32("fan1_input")
    }

    pub fn gpu_fan_rpm(&self) -> Result<u32> {
        self.read_hwmon_u32("fan2_input")
    }

    pub fn cpu_fan_pwm(&self) -> Result<u32> {
        self.read_hwmon_u32("pwm1")
    }

    pub fn gpu_fan_pwm(&self) -> Result<u32> {
        self.read_hwmon_u32("pwm2")
    }

    // Called at initialization and after resume. The next control request
    // re-establishes manual mode before applying its requested duty.
    pub async fn invalidate_manual_control(&self) {
        *self.manual_ready.lock().await = false;
    }

    // Prevent later control requests from undoing the firmware handover.
    // Bound the entire operation, including waiting for an in-flight setter.
    pub async fn restore_firmware_on_shutdown(&self) -> Result<()> {
        self.shutdown_token.cancel();
        timeout(Duration::from_secs(2), async {
            let mut ready = self.manual_ready.lock().await;
            *ready = false;
            self.set_fan_mode(FanMode::Normal).await
        })
        .await
        .context("Timed out restoring firmware fan control during shutdown")?
    }

    fn ensure_control_active(&self) -> Result<()> {
        if self.shutdown_token.is_cancelled() {
            return Err(anyhow!(
                "Plugin is shutting down; manual fan control is disabled"
            ));
        }
        Ok(())
    }

    pub async fn enable_fixed_fan_mode(&self) -> Result<()> {
        let mut ready = self.manual_ready.lock().await;
        log::info!("Manual fan control requested; re-establishing hardware mode");
        *ready = false;
        self.prepare_manual_control(&mut ready).await
    }

    pub async fn reset_fan_mode(&self) -> Result<()> {
        let mut ready = self.manual_ready.lock().await;
        *ready = false;
        self.set_fan_mode(FanMode::Normal).await
    }

    pub async fn set_fan_duty_percent(&self, duty: i32) -> Result<()> {
        if !(10..=100).contains(&duty) {
            return Err(anyhow!("Fan duty must be between 10 and 100"));
        }

        let driver_value = ((duty as f64 * 227.0) / 100.0).round() as i32;
        let mut ready = self.manual_ready.lock().await;

        self.prepare_manual_control(&mut ready).await?;

        if let Err(err) = self.call_setter("SetFanSpeed", driver_value).await {
            *ready = false;
            return Err(err);
        }

        Ok(())
    }

    async fn prepare_manual_control(&self, ready: &mut bool) -> Result<()> {
        self.ensure_control_active()?;
        if *ready {
            return Ok(());
        }

        // Reproduce the recovery sequence verified on this laptop.
        self.set_fan_mode(FanMode::Normal).await?;
        tokio::select! {
            _ = sleep(Duration::from_secs(2)) => {},
            _ = self.shutdown_token.cancelled() => {
                return Err(anyhow!("Plugin shut down during manual-mode preparation"));
            }
        }
        self.set_fan_mode(FanMode::Fixed).await?;

        *ready = true;
        log::info!("AORUS manual fan mode established through D-Bus");
        Ok(())
    }

    async fn set_fan_mode(&self, mode: FanMode) -> Result<()> {
        self.call_setter("SetFanMode", mode as i32).await
    }

    async fn call_setter(&self, method: &str, value: i32) -> Result<()> {
        timeout(Duration::from_secs(5), async {
            let connection = self
                .connection
                .get_or_try_init(Connection::system)
                .await
                .context("Failed to connect to the system D-Bus")?;

            let proxy = Proxy::new(
                connection,
                "com.gigabyte.daemon",
                "/com/gigabyte/Platform",
                "com.gigabyte.Platform",
            )
            .await
            .context("Failed to create the gigabyted D-Bus proxy")?;

            // Normal mode remains permitted for reset and shutdown. All other
            // writes must stop once shutdown begins, including a setter that
            // was waiting for a connection or the manual-mode delay.
            if method != "SetFanMode" || value != FanMode::Normal as i32 {
                self.ensure_control_active()?;
            }

            let result: i32 = proxy
                .call(method, &(value,))
                .await
                .with_context(|| format!("gigabyted {method}({value}) failed"))?;

            if result != 0 {
                return Err(anyhow!("gigabyted {method} returned {result}"));
            }

            Ok(())
        })
        .await
        .with_context(|| format!("Timed out calling gigabyted {method}"))?
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
}

fn find_hwmon_path(sysfs_path: &Path) -> Result<PathBuf> {
    let hwmon_root = sysfs_path.join("hwmon");

    for entry in fs::read_dir(&hwmon_root)? {
        let path = entry?.path();

        if !path.is_dir() {
            continue;
        }

        let Ok(name) = fs::read_to_string(path.join("name")) else {
            continue;
        };

        if name.trim() == "aorus_laptop" {
            return Ok(path);
        }
    }

    Err(anyhow!("AORUS hwmon device not found"))
}
