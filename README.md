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

## D-Bus integration

Since v0.1.2, the plugin runs unprivileged and sends fan-control writes
through the system D-Bus service provided by
[the AORUS gigabyted fork](https://github.com/olpratty/gigabyte-dbus).

Fan RPM and PWM feedback are still read directly from read-only hwmon
files. The shared Laptop Fans channel reports the lower of the two
measured fan duties; individual CPU/GPU channels retain their own readings.

When CoolerControl requests manual control, the plugin re-establishes
hardware mode. This restores control after suspend/resume on the tested
AORUS 15P XD.

Fixed duty, software curves, unmanaged mode and suspend/resume recovery
have been tested with CoolerControl 5.0.

## Shutdown and recovery

Since v0.1.3, the plugin attempts to restore firmware fan control through
D-Bus during orderly shutdown. This requires gigabyted to remain available.

While the plugin remains running, it remembers the latest requested duty
and monitors read-only fan_mode feedback. If gigabyted restarts and the
hardware returns to firmware mode, the plugin can re-establish the
requested control through D-Bus. Selecting Unmanaged clears that request.

A plugin process crash loses this in-memory request. The Bazzite AORUS
image supplies a systemd ExecStopPost hook that requests firmware control
through gigabyted after plugin termination. Separately, gigabyted v1.0.2
provides its own service cleanup command for daemon stop or crash.
These hooks are deployment components, not part of the plugin binary.

CoolerControl continues scheduling software curves and periodically
resends duty commands. A fresh duty command lets the restarted plugin
re-establish manual hardware mode without an external recovery helper.

Hardware testing on the AORUS 15P XD with CoolerControl 5.0.1, plugin
v0.1.3 and gigabyted v1.0.2 observed:

| Setting before plugin crash | Observed result without an API helper |
| --- | --- |
| CPU Graph curve | Control returned after approximately 26 seconds |
| Mix Curve | Control returned after approximately 117 seconds |
| Unmanaged | Firmware control remained active |

These timings are individual observations, not maximum recovery delays.
Profile processing, output suppression and changing temperatures affect
when the next duty command is sent. During the observed recovery interval,
fan_mode was 0, leaving fan control to the laptop firmware.

Automatic restoration of a directly selected manual percentage or a Fixed
profile after a plugin crash has not been established. The inspected
CoolerControl 5.0.1 fixed-speed path does not use the periodic curve
scheduler. Reapply the setting in CoolerControl if necessary.

Normal plugin operation and the curve recovery described above require
no CoolerControl API token or separate API recovery service.

## Requirements

- CoolerControl with device-service plugin support.
- `gigabyte-laptop-wmi` kernel driver.
- AORUS-integrated `gigabyted` v1.0.1, or a compatible daemon and policy.
- System-bus permission for the plugin account to call `SetFanMode`
  and `SetFanSpeed`.

See [INSTALL.md](INSTALL.md) for build and installation instructions.
