# AORUS CoolerControl Plugin

A CoolerControl plugin for supported Gigabyte laptops using tangalbert919's `gigabyte-laptop-wmi` driver.

Specifically being developed and tested on a Gigabyte AORUS 15P XD laptop using a custom Bazzite OS.

## Current Functionality

- Detects the `aorus_laptop` platform device.
- Reports CPU fan RPM.
- Reports GPU fan RPM.
- Provides a single `Laptop Fans` control channel for both CPU and GPU fans.
  - Scales CoolerControl's 0–100% fan duty to the driver's 0–255 control range.
- Supports fixed fan duty control.
- Supports CoolerControl software fan curves.
- Returns fan control to the laptop firmware when the channel is reset.

## Planned Future State

### Fans

- Add controls for firmware fan modes:
  - Silent
  - Gaming
  - Auto

### GPU Dynamic Boost

- Investigate what, if any, function the `gpu_boost` states (0–3) have on the AORUS 15P XD.
  - Testing so far shows that changing these states has no observable effect on NVIDIA Dynamic Boost or the GPU power limit.
- Investigate separate control of NVIDIA Dynamic Boost.

### Battery

- Add battery charging limit controls.

## Not Being Implemented

- Temperature readback.
  - The AORUS 15P XD does not appear to expose any additional useful temperature sensors beyond the CPU and GPU temperatures already available to CoolerControl.
- USB sleep charging settings.
- Power-on time.
- Light sensor.

## Requirements

- CoolerControl with device service plugin support.
- `gigabyte-laptop-wmi` kernel driver.

See [INSTALL.md](INSTALL.md) for build and installation instructions.
