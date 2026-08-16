# Build and Installation

The plugin expects the AORUS platform device at:

`/sys/devices/platform/aorus_laptop`

## Build

```bash
cargo build --locked --release
```

The resulting executable is:

`target/release/cc-plugin-aorus`

## Installation

The plugin consists of:

- `cc-plugin-aorus`
- `manifest.toml`

CoolerControl loads these from a plugin directory named:

`cc-plugin-aorus`

On the current target system, the installation path is:

`/etc/coolercontrol/plugins/cc-plugin-aorus/`

The `/etc/coolercontrol/plugins` path may be backed by persistent storage depending on the operating system.
