use std::collections::HashMap;

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

pub struct MyDeviceService {
    aorus: Option<AorusDevice>,
    devices: Vec<Device>,
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
                        ]),
                        temps: HashMap::from([(
                            "motherboard".to_string(),
                            models::v1::TempInfo {
                                label: "Motherboard".to_string(),
                                number: 1,
                            },
                        )]),
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
                    aorus: Some(aorus),
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
            status: health_response::Status::Ok.into(),
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
        // TODO: Device initialization logic
        Ok(Response::new(InitializeDeviceResponse {}))
    }

    async fn shutdown(
        &self,
        _request: Request<ShutdownRequest>,
    ) -> Result<Response<ShutdownResponse>, Status> {
        // TODO: Device shutdown logic
        // Note: The CoolerControl daemon will initiate a service termination after this point.
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

        let motherboard_temp = aorus.motherboard_temp_c().map_err(|err| {
            Status::internal(format!("Failed to read motherboard temperature: {err}"))
        })?;

        let status = vec![
            models::v1::Status {
                id: "cpu_fan".to_string(),
                metric: Some(models::v1::status::Metric::Speed(
                    models::v1::status::FanSpeed {
                        duty: None,
                        rpm: Some(cpu_rpm),
                    },
                )),
            },
            models::v1::Status {
                id: "gpu_fan".to_string(),
                metric: Some(models::v1::status::Metric::Speed(
                    models::v1::status::FanSpeed {
                        duty: None,
                        rpm: Some(gpu_rpm),
                    },
                )),
            },
            models::v1::Status {
                id: "motherboard".to_string(),
                metric: Some(models::v1::status::Metric::Temp(motherboard_temp)),
            },
        ];

        Ok(Response::new(StatusResponse { status }))
    }

    /// Reset the device channel to it's default state if applicable. (Auto)
    async fn reset_channel(
        &self,
        _request: Request<ResetChannelRequest>,
    ) -> Result<Response<ResetChannelResponse>, Status> {
        // TODO: Any Device Channel reset logic (default behavior)
        Ok(Response::new(ResetChannelResponse {}))
    }

    async fn enable_manual_fan_control(
        &self,
        _request: Request<EnableManualFanControlRequest>,
    ) -> Result<Response<EnableManualFanControlResponse>, Status> {
        // TODO: Enable Fan Control for particular fan
        Err(Status::unimplemented("No Fans"))
    }

    async fn fixed_duty(
        &self,
        _request: Request<FixedDutyRequest>,
    ) -> Result<Response<FixedDutyResponse>, Status> {
        // TODO: Set fixed duty for particular fan
        Err(Status::unimplemented("No Fans"))
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
