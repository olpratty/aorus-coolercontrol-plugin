use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
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

#[derive(Default)]
struct ControlState {
    ready: bool,
    requested_duty: Option<i32>,
}

pub struct AorusDevice {
    pub sysfs_path: PathBuf,
    pub hwmon_path: PathBuf,
    connection: OnceCell<Connection>,
    control: Mutex<ControlState>,
    control_warning: AtomicBool,
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
            control: Mutex::new(ControlState::default()),
            control_warning: AtomicBool::new(false),
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
        self.control.lock().await.ready = false;
    }

    // Prevent later control requests from undoing the firmware handover.
    // Bound the entire operation, including waiting for an in-flight setter.
    pub async fn restore_firmware_on_shutdown(&self) -> Result<()> {
        self.shutdown_token.cancel();
        timeout(Duration::from_secs(2), async {
            let mut control = self.control.lock().await;
            control.requested_duty = None;
            control.ready = false;
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
        let mut control = self.control.lock().await;
        self.ensure_control_active()?;
        // Wait for the new profile/manual setting's duty before recovering it.
        control.requested_duty = None;
        control.ready = false;
        log::info!("Manual fan control requested; re-establishing hardware mode");
        let result = self.prepare_manual_control(&mut control.ready).await;
        self.control_warning
            .store(result.is_err(), Ordering::SeqCst);
        result
    }

    pub async fn reset_fan_mode(&self) -> Result<()> {
        let mut control = self.control.lock().await;
        // Clear intent before attempting the write, so Unmanaged never causes
        // the monitor to replay an earlier manual duty, even if this call fails.
        control.requested_duty = None;
        control.ready = false;
        let result = self.set_fan_mode(FanMode::Normal).await;
        self.control_warning
            .store(result.is_err(), Ordering::SeqCst);
        result
    }

    pub async fn set_fan_duty_percent(&self, duty: i32) -> Result<()> {
        if !(10..=100).contains(&duty) {
            return Err(anyhow!("Fan duty must be between 10 and 100"));
        }

        let mut control = self.control.lock().await;
        self.ensure_control_active()?;
        // Retain the latest valid request if D-Bus is temporarily unavailable.
        control.requested_duty = Some(duty);
        let result = self.apply_duty(&mut control, duty).await;
        self.control_warning
            .store(result.is_err(), Ordering::SeqCst);
        result
    }

    async fn apply_duty(&self, control: &mut ControlState, duty: i32) -> Result<()> {
        let result = async {
            self.prepare_manual_control(&mut control.ready).await?;
            let driver_value = ((duty as f64 * 227.0) / 100.0).round() as i32;
            self.call_setter("SetFanSpeed", driver_value).await
        }
        .await;
        if result.is_err() {
            control.ready = false;
        }
        result
    }

    pub fn has_control_warning(&self) -> bool {
        self.control_warning.load(Ordering::SeqCst)
    }

    pub async fn recover_requested_control(&self) -> Result<()> {
        let mut control = self.control.lock().await;
        if self.shutdown_token.is_cancelled() {
            return Ok(());
        }
        let Some(duty) = control.requested_duty else {
            // Unmanaged, startup, or awaiting the first duty: do not take control.
            return Ok(());
        };

        // Read-only feedback detects a hardware reset even when no D-Bus call
        // failed and CoolerControl has not changed the curve's target duty.
        let mode_path = self.sysfs_path.join("fan_mode");
        let mode = fs::read_to_string(&mode_path)
            .with_context(|| format!("Failed to read {}", mode_path.display()))
            .and_then(|value| value.trim().parse::<i32>().context("Invalid fan_mode"));
        let mode = match mode {
            Ok(mode) => mode,
            Err(err) => {
                control.ready = false;
                self.control_warning.store(true, Ordering::SeqCst);
                return Err(err);
            }
        };
        if mode == FanMode::Fixed as i32 && control.ready {
            self.control_warning.store(false, Ordering::SeqCst);
            return Ok(());
        }

        if !self.control_warning.swap(true, Ordering::SeqCst) {
            log::warn!(
                "Requested fan control is inactive (mode {mode}); attempting D-Bus recovery"
            );
        }
        control.ready = false;
        self.apply_duty(&mut control, duty).await?;
        self.control_warning.store(false, Ordering::SeqCst);
        log::info!("Requested fan control restored through D-Bus at {duty}%");
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

#[cfg(test)]
mod tests {
    use super::*;

    fn device_without_hardware() -> AorusDevice {
        AorusDevice {
            sysfs_path: PathBuf::from("/nonexistent-aorus-recovery-test"),
            hwmon_path: PathBuf::from("/nonexistent-aorus-recovery-test/hwmon"),
            connection: OnceCell::new(),
            control: Mutex::new(ControlState::default()),
            control_warning: AtomicBool::new(false),
            shutdown_token: CancellationToken::new(),
        }
    }

    #[tokio::test]
    async fn unmanaged_monitor_never_opens_a_bus_connection() {
        let device = device_without_hardware();
        device.recover_requested_control().await.unwrap();
        assert!(device.connection.get().is_none());
        assert!(!device.has_control_warning());
    }

    #[tokio::test]
    async fn shutdown_prevents_replaying_a_remembered_duty() {
        let device = device_without_hardware();
        device.control.lock().await.requested_duty = Some(90);
        device.shutdown_token.cancel();
        device.recover_requested_control().await.unwrap();
        assert!(device.connection.get().is_none());
    }

    #[tokio::test]
    async fn missing_mode_reports_warning_without_sending_a_write() {
        let device = device_without_hardware();
        {
            let mut control = device.control.lock().await;
            control.requested_duty = Some(80);
            control.ready = true;
        }
        assert!(device.recover_requested_control().await.is_err());
        assert!(device.has_control_warning());
        assert!(!device.control.lock().await.ready);
        assert!(device.connection.get().is_none());
    }
}
