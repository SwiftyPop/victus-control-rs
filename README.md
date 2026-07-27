# victus-control-rs

`victus-control-rs` is a Linux fan control utility for HP Victus laptops. It replaces the default ACPI thermal profile with automated temperature curves and manual fan speed controls.

---

## Features

- **Automated Fan Curves (`BETTER_AUTO`)**: Adjusts fan speeds every 2 seconds based on CPU and GPU temperatures.
- **Manual Control (`MANUAL`)**: Sets fan speeds between 2000 RPM and 6000 RPM.
- **System D-Bus Daemon (`victus-backend`)**: Interacts with Linux sysfs interfaces using root privileges.
- **GTK4 User Interface (`victus-control`)**: Displays live temperatures, fan RPMs, and fan control modes.

---

## System Requirements

- **Linux Kernel**: 6.x or newer
- **Init System**: `systemd`
- **Dependencies**: `rust`, `gtk4`, `dkms`, `dbus`
- **Supported Distributions**: CachyOS, Arch Linux, Fedora, Ubuntu

---

## Installation

Run the installation script:

```bash
sudo ./install.sh
```

The installer performs the following actions:
1. Installs package dependencies.
2. Compiles the Rust binaries in release mode (`cargo build --release`).
3. Installs `victus-backend` and `victus-control` to `/usr/bin/`.
4. Copies the D-Bus security policy to `/usr/share/dbus-1/system.d/`.
5. Compiles and loads the `hp_wmi` DKMS kernel module.
6. Enables and starts the `victus-backend.service` systemd unit.

---

## Usage

### Launch User Interface
Run the application from a terminal or desktop menu:

```bash
victus-control
```

### Check Backend Daemon Status
Verify that the backend service is active:

```bash
systemctl status victus-backend.service
```

---

## Technical Details

### Hardware Interface
The backend communicates with the `hp_wmi` kernel driver via sysfs:
- `/sys/devices/platform/hp-wmi/hwmon/hwmon*/pwm1_enable`: Sets the control mode (0=MAX, 1=MANUAL/BETTER_AUTO, 2=AUTO).
- `/sys/devices/platform/hp-wmi/hwmon/hwmon*/fan1_target`: Sets fan target speed (RPM).
- `/sys/devices/platform/hp-wmi/hwmon/hwmon*/pwm1`: Fallback PWM control node (0-255).

If your laptop BIOS does not report manual fan control support, the installer adds `options hp_wmi force_fan_control_support=1` to `/etc/modprobe.d/hp-wmi.conf`.

---

## License

GNU General Public License v3.0 (GPL-3.0).
