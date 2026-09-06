# AORUS CoolerControl Plugin

A fan-control plugin for [CoolerControl](https://gitlab.com/coolercontrol/coolercontrol) on supported Gigabyte laptops using the [`gigabyte-laptop-wmi`](https://github.com/tangalbert919/gigabyte-laptop-wmi) kernel driver.

Developed and tested on a Gigabyte AORUS 15P XD running Bazzite.

## Scope

This plugin is intentionally focused on integrating the laptop's fans with CoolerControl.

Other features exposed by `gigabyte-laptop-wmi`, such as battery charging controls and GPU-related settings, are outside the scope of this plugin.

For broader Gigabyte laptop control, see [`gigabyte-cc`](https://github.com/jokelbaf/gigabyte-cc).

## Functionality

- Detects the `aorus_laptop` platform device.
- Reports CPU fan RPM and PWM duty.
- Reports GPU fan RPM and PWM duty.
- Provides a single `Laptop Fans` control channel for both CPU and GPU fans.
- Supports fixed fan duty control.
- Supports CoolerControl software fan curves.
- Returns fan control to the laptop firmware when the channel is reset or unmanaged.

### Fan Control

The current `gigabyte-laptop-wmi` driver provides a single writable `fan_custom_speed` control which applies to both fans.

The plugin maps CoolerControl's usable 10–100% control range to the driver's fan-speed values. Testing on the AORUS 15P XD found 227 to be the maximum usable raw value; higher values can result in invalid fan behaviour.

CPU and GPU PWM are read independently through the driver's hwmon `pwm1` and `pwm2` interfaces and are used to report actual fan duty back to CoolerControl.

## Future PWM Control

The driver currently exposes CPU and GPU PWM through hwmon as read-only values.

If `gigabyte-laptop-wmi` gains writable per-fan PWM support in the future, this plugin may be updated to provide independent CPU and GPU fan control through CoolerControl.

Until then, `fan_custom_speed` remains the control interface and applies to both fans together.

## Requirements

- CoolerControl with device-service plugin support.
- `gigabyte-laptop-wmi` kernel driver.

See [INSTALL.md](INSTALL.md) for build and installation instructions.
