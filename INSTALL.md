# Build and installation

## Requirements

The kernel driver must expose `/sys/devices/platform/aorus_laptop`
and its hwmon fan RPM/PWM attributes.

The AORUS-integrated gigabyted service and D-Bus policy must be installed.
See https://github.com/olpratty/gigabyte-dbus/blob/master/AORUS-INTEGRATION.md.

The plugin runs as `cc-plugin-user`; `privileged = false` belongs in its
manifest. Hardware writes go through gigabyted. The packaged policy grants
the two fan setters to this account, including other plugins using it.

## Bazzite AORUS image

The custom image installs both components automatically. Do not use
`make install` to update an image-managed installation.

The packaged plugin lives in:
`/usr/lib/coolercontrol/plugins/cc-plugin-aorus/`

Before CoolerControl starts, `aorus-plugin-prepare.service` copies the
packaged binary and manifest to the writable runtime directory:
`/var/lib/coolercontrol/plugins/cc-plugin-aorus/`

The preparation script preserves the packaged SELinux labels. The copy is
refreshed on every boot, following the booted image on upgrades or rollbacks.
Direct modifications inside this runtime plugin directory are replaced.
CoolerControl profiles and settings are managed separately.

## Recovery components and credentials

The plugin does not require a CoolerControl API token. It receives device
commands from CoolerControl over the device-service protocol and sends
hardware writes to gigabyted over D-Bus.

For the deployment tested with plugin v0.1.3:

- Use AORUS gigabyted v1.0.2 with its packaged service to obtain daemon
  stop/crash cleanup via ExecStopPost and --restore-fan-auto.
- The Bazzite AORUS image also installs a plugin service drop-in named
  30-aorus-firmware-handover.conf. It orders the plugin after gigabyted
  and requests SetFanMode(0) through D-Bus in ExecStopPost.
- The plugin cleanup hook runs as cc-plugin-user and requires gigabyted
  to be available. It grants no additional direct sysfs write access.

Installing only the plugin binary and manifest with make install does
not install the Bazzite plugin service drop-in. Orderly plugin shutdown
can request firmware control, but SIGKILL cannot run cleanup inside the
plugin process; that case relies on the external service hook.

Firmware handover depends on the cleanup command being able to run and
reach the hardware. It is not a guarantee for every failure scenario.

See the README's Shutdown and recovery section for tested curve recovery
behaviour and the limitation concerning manual fixed-speed restoration.

## Build

Build as an ordinary user. On Bazzite, use the development container.

Required tools include Rust/Cargo, a C compiler, make, and
`protobuf-compiler`. See Cargo.toml for the minimum Rust version.

```bash
make build
```

The output is `target/release/cc-plugin-aorus`.

## Manual installation on a writable system

Install and activate gigabyted first. Stop CoolerControl and ensure its
plugin process has exited before replacing an existing plugin.

After building as your ordinary user:

```bash
sudo make install
```

This installs the binary and manifest under
`/var/lib/coolercontrol/plugins/cc-plugin-aorus/`.

Restart CoolerControl to discover the plugin. SELinux systems also need
labels permitting the plugin service to execute the binary; the custom
Bazzite image handles this through its preparation service.

For package staging without changing the host:

```bash
make install DESTDIR=/absolute/path/to/staging
```

## Development run

Build first, then run on the host with the driver and daemon available.
Stop any existing instance to avoid a socket conflict.

```bash
sudo -u cc-plugin-user ./target/release/cc-plugin-aorus
```

`make run` executes the built plugin as the current user, without elevation.
That user must already have the required D-Bus permissions.

## Manual uninstall

Stop CoolerControl and its plugin process first:

```bash
sudo make uninstall
```

This removes the plugin binary and manifest, not CoolerControl settings
or the separately installed gigabyted service. Restart CoolerControl afterward.
