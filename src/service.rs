use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::aorus::AorusDevice;
use crate::device_service::v1::device_service_server::DeviceService;
use crate::device_service::v1::{
    CustomFunctionOneRequest, CustomFunctionOneResponse, EnableManualFanControlRequest,
    EnableManualFanControlResponse, FixedDutyRequest, FixedDutyResponse, HealthRequest,
    HealthResponse, InitializeDeviceRequest, InitializeDeviceResponse, LcdRequest, LcdResponse,
    LightingRequest, LightingResponse, ListDevicesRequest, ListDevicesResponse,
    ResetChannelRequest, ResetChannelResponse, ShutdownRequest, ShutdownResponse,
    SpeedProfileRequest, SpeedProfileResponse, StatusRequest, StatusResponse, health_response,
};
use crate::models::v1::Device;
use crate::{SERVICE_ID, VERSION, models};

use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct MyDeviceService {
    aorus: Option<Arc<AorusDevice>>,
    devices: Vec<Device>,
}

impl MyDeviceService {
    pub async fn monitor_control(&self, stop: CancellationToken) {
        let Some(aorus) = &self.aorus else {
            return;
        };
        let mut failure_logged = false;
        loop {
            tokio::select! {
                _ = stop.cancelled() => return,
                _ = tokio::time::sleep(Duration::from_secs(1)) => {},
            }
            let result = tokio::select! {
                _ = stop.cancelled() => return,
                result = aorus.recover_requested_control() => result,
            };
            match result {
                Ok(()) => failure_logged = false,
                Err(err) => {
                    if !failure_logged {
                        log::warn!("Fan control recovery pending: {err:#}");
                        failure_logged = true;
                    }
                }
            }
        }
    }

    pub async fn restore_firmware_on_shutdown(&self) -> anyhow::Result<()> {
        if let Some(aorus) = &self.aorus {
            if let Err(err) = aorus.restore_firmware_on_shutdown().await {
                log::error!("Failed to restore firmware fan control on shutdown: {err:#}");
                return Err(err);
            }
            log::info!("Firmware fan control restored through D-Bus on shutdown");
        }
        Ok(())
    }
}

impl Default for MyDeviceService {
    fn default() -> Self {
        match AorusDevice::detect() {
            Ok(aorus) => {
                log::info!(
                    "Detected AORUS device at {} with hwmon at {}",
                    aorus.sysfs_path.display(),
                    aorus.hwmon_path.display()
                );

                let driver_info = models::v1::DriverInfo {
                    name: Some("aorus_laptop".to_string()),
                    version: None,
                    locations: vec![aorus.sysfs_path.display().to_string()],
                };

                let device = Device {
                    id: "aorus_laptop".to_string(),
                    name: "AORUS Laptop".to_string(),
                    uid_info: None,
                    info: Some(models::v1::DeviceInfo {
                        channels: HashMap::from([
                            (
                                "cpu_fan".to_string(),
                                models::v1::ChannelInfo {
                                    label: Some("CPU Fan".to_string()),
                                    options: Some(models::v1::channel_info::Options::SpeedOptions(
                                        models::v1::SpeedOptions {
                                            min_duty: 0,
                                            max_duty: 100,
                                            fixed_enabled: false,
                                            extension: None,
                                        },
                                    )),
                                },
                            ),
                            (
                                "gpu_fan".to_string(),
                                models::v1::ChannelInfo {
                                    label: Some("GPU Fan".to_string()),
                                    options: Some(models::v1::channel_info::Options::SpeedOptions(
                                        models::v1::SpeedOptions {
                                            min_duty: 0,
                                            max_duty: 100,
                                            fixed_enabled: false,
                                            extension: None,
                                        },
                                    )),
                                },
                            ),
                            (
                                "fan_control".to_string(),
                                models::v1::ChannelInfo {
                                    label: Some("Laptop Fans".to_string()),
                                    options: Some(models::v1::channel_info::Options::SpeedOptions(
                                        models::v1::SpeedOptions {
                                            min_duty: 10,
                                            max_duty: 100,
                                            fixed_enabled: true,
                                            extension: None,
                                        },
                                    )),
                                },
                            ),
                        ]),
                        temps: HashMap::new(),
                        lighting_speeds: vec![],
                        temp_min: None,
                        temp_max: None,
                        profile_min_length: None,
                        profile_max_length: None,
                        model: None,
                        driver_info: Some(driver_info),
                    }),
                };

                Self {
                    aorus: Some(Arc::new(aorus)),
                    devices: vec![device],
                }
            }
            Err(err) => {
                log::warn!("AORUS device not detected: {err}");

                Self {
                    aorus: None,
                    devices: Vec::new(),
                }
            }
        }
    }
}

#[tonic::async_trait]
impl DeviceService for MyDeviceService {
    /// Used to confirm service connection and retrieve service health information.
    async fn health(
        &self,
        _request: Request<HealthRequest>,
    ) -> Result<Response<HealthResponse>, Status> {
        let reply = HealthResponse {
            name: SERVICE_ID.to_string(),
            version: VERSION.to_string(),
            status: if self
                .aorus
                .as_ref()
                .is_some_and(|aorus| aorus.has_control_warning())
            {
                health_response::Status::Warning.into()
            } else {
                health_response::Status::Ok.into()
            },
            // information purposes only
            uptime_seconds: 1,
        };
        Ok(Response::new(reply))
    }

    /// This is the first message sent to the device service after establishing a connection
    /// and is used to detect the service's devices and capabilities.
    /// The device models should be filled out for each device and all of their
    /// available channels. This information is used to populate the CoolerControl device
    /// list and available features in the UI.
    async fn list_devices(
        &self,
        _request: Request<ListDevicesRequest>,
    ) -> Result<Response<ListDevicesResponse>, Status> {
        // TODO: Device model mapping logic
        // Note: internal device representations may not be the same as the device service's
        // device representations, as services often need additional device information.
        Ok(Response::new(ListDevicesResponse {
            devices: self.devices.clone(),
        }))
    }

    /// This is called and used by some devices to initialize hardware, before starting to send
    /// commands to it. It is also be called after resuming from sleep, as many firmwares are rest.
    async fn initialize_device(
        &self,
        _request: Request<InitializeDeviceRequest>,
    ) -> Result<Response<InitializeDeviceResponse>, Status> {
        if let Some(aorus) = &self.aorus {
            aorus.invalidate_manual_control().await;
        }
        Ok(Response::new(InitializeDeviceResponse {}))
    }

    async fn shutdown(
        &self,
        _request: Request<ShutdownRequest>,
    ) -> Result<Response<ShutdownResponse>, Status> {
        self.restore_firmware_on_shutdown().await.map_err(|err| {
            Status::internal(format!("Failed to restore firmware fan control: {err:#}"))
        })?;
        Ok(Response::new(ShutdownResponse {}))
    }

    /// This is called to retrieve the status per device and their respective channels
    /// and is called at a regular intervals (default 1 second).
    ///
    /// Device _channels_ usually can not be done concurrently, but that depends on the hardware and drivers.
    async fn status(
        &self,
        request: Request<StatusRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let req = request.get_ref();

        if req.device_id != "aorus_laptop" {
            return Err(Status::not_found("Device not found"));
        }

        let Some(aorus) = &self.aorus else {
            return Err(Status::not_found("AORUS device not available"));
        };

        let cpu_rpm = aorus
            .cpu_fan_rpm()
            .map_err(|err| Status::internal(format!("Failed to read CPU fan RPM: {err}")))?;

        let gpu_rpm = aorus
            .gpu_fan_rpm()
            .map_err(|err| Status::internal(format!("Failed to read GPU fan RPM: {err}")))?;

        let cpu_pwm = aorus
            .cpu_fan_pwm()
            .map_err(|err| Status::internal(format!("Failed to read CPU fan PWM: {err}")))?;

        let gpu_pwm = aorus
            .gpu_fan_pwm()
            .map_err(|err| Status::internal(format!("Failed to read GPU fan PWM: {err}")))?;

        // The driver exposes PWM values on a nominal 0-255 scale, but testing on the
        // AORUS 15P XD found 227 to be the maximum usable value. Keep PWM feedback
        // normalized to the same 0-227 range used by fan_custom_speed.
        let cpu_duty = ((cpu_pwm as f64 * 100.0) / 227.0).min(100.0);
        let gpu_duty = ((gpu_pwm as f64 * 100.0) / 227.0).min(100.0);

        let status = vec![
            // Shared channel reports the lower measured fan duty.
            models::v1::Status {
                id: "fan_control".to_string(),
                metric: Some(models::v1::status::Metric::Speed(
                    models::v1::status::FanSpeed {
                        duty: Some(cpu_duty.min(gpu_duty)),
                        rpm: None,
                    },
                )),
            },
            models::v1::Status {
                id: "cpu_fan".to_string(),
                metric: Some(models::v1::status::Metric::Speed(
                    models::v1::status::FanSpeed {
                        duty: Some(cpu_duty),
                        rpm: Some(cpu_rpm),
                    },
                )),
            },
            models::v1::Status {
                id: "gpu_fan".to_string(),
                metric: Some(models::v1::status::Metric::Speed(
                    models::v1::status::FanSpeed {
                        duty: Some(gpu_duty),
                        rpm: Some(gpu_rpm),
                    },
                )),
            },
        ];

        Ok(Response::new(StatusResponse { status }))
    }

    /// Reset the device channel to it's default state if applicable. (Auto)
    async fn reset_channel(
        &self,
        request: Request<ResetChannelRequest>,
    ) -> Result<Response<ResetChannelResponse>, Status> {
        let req = request.get_ref();

        if req.device_id != "aorus_laptop" {
            return Err(Status::not_found("Device not found"));
        }

        if req.channel_id != "fan_control" {
            return Err(Status::invalid_argument("Channel is not controllable"));
        }

        let Some(aorus) = &self.aorus else {
            return Err(Status::not_found("AORUS device not available"));
        };

        aorus
            .reset_fan_mode()
            .await
            .map_err(|err| Status::internal(format!("Failed to reset fan control: {err:#}")))?;

        Ok(Response::new(ResetChannelResponse {}))
    }

    async fn enable_manual_fan_control(
        &self,
        request: Request<EnableManualFanControlRequest>,
    ) -> Result<Response<EnableManualFanControlResponse>, Status> {
        let req = request.get_ref();

        if req.device_id != "aorus_laptop" {
            return Err(Status::not_found("Device not found"));
        }

        if req.channel_id != "fan_control" {
            return Err(Status::invalid_argument("Channel is not controllable"));
        }

        let Some(aorus) = &self.aorus else {
            return Err(Status::not_found("AORUS device not available"));
        };

        aorus.enable_fixed_fan_mode().await.map_err(|err| {
            Status::internal(format!("Failed to enable manual fan control: {err:#}"))
        })?;

        Ok(Response::new(EnableManualFanControlResponse {}))
    }

    async fn fixed_duty(
        &self,
        request: Request<FixedDutyRequest>,
    ) -> Result<Response<FixedDutyResponse>, Status> {
        let req = request.get_ref();

        if req.device_id != "aorus_laptop" {
            return Err(Status::not_found("Device not found"));
        }

        if req.channel_id != "fan_control" {
            return Err(Status::invalid_argument("Channel is not controllable"));
        }

        let Some(aorus) = &self.aorus else {
            return Err(Status::not_found("AORUS device not available"));
        };

        aorus
            .set_fan_duty_percent(req.duty)
            .await
            .map_err(|err| Status::internal(format!("Failed to set fan duty: {err:#}")))?;

        Ok(Response::new(FixedDutyResponse {}))
    }

    async fn speed_profile(
        &self,
        _request: Request<SpeedProfileRequest>,
    ) -> Result<Response<SpeedProfileResponse>, Status> {
        // TODO: Apply a speed profile to the device channel
        Err(Status::unimplemented("No Firmware Profiles"))
    }

    async fn lighting(
        &self,
        _request: Request<LightingRequest>,
    ) -> Result<Response<LightingResponse>, Status> {
        // TODO: Apply a lighting mode to the device channel
        Err(Status::unimplemented("No Lighting Channels"))
    }

    async fn lcd(&self, _request: Request<LcdRequest>) -> Result<Response<LcdResponse>, Status> {
        // TODO: Apply a LCD mode
        Err(Status::unimplemented("No LCD Channels"))
    }

    /// This is a placeholder for any custom functions that the device service might expose.
    async fn custom_function_one(
        &self,
        _request: Request<CustomFunctionOneRequest>,
    ) -> Result<Response<CustomFunctionOneResponse>, Status> {
        Err(Status::unimplemented("No Custom Function"))
    }
}
