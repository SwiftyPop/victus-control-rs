# victus-control-rs

`victus-control-rs` is a Linux fan control utility for HP Victus laptops. It replaces the default ACPI thermal profile with automated temperature curves and manual fan speed controls.

---

## Features

- **Automated Fan Curves (`BETTER_AUTO`)**: Adjusts fan speeds every 2 seconds based on CPU and GPU temperatures.
- **Manual Control (`MANUAL`)**: Sets fan speeds dynamically based on hardware limits.
- **System D-Bus Daemon (`victus-backend`)**: Interacts with Linux sysfs interfaces securely via D-Bus (`org.hp.VictusControl`).
- **GTK4 User Interface (`victus-control`)**: Displays live temperatures, fan RPMs, and fan control modes.
- **GNOME Shell Extension**: Provides quick top-bar panel integration for GNOME desktop environments.

---

## System Requirements

- **Linux Kernel**: 6.x or newer
- **Init System**: `systemd`
- **Dependencies**: `rust`, `gtk4`, `dkms`, `dbus`
- **Supported Distributions**: CachyOS, Arch Linux, Fedora, Ubuntu / Debian

---

## Installation

### Option 1: Quick Install One-Liner
```bash
curl -fsSL https://raw.githubusercontent.com/SwiftyPop/victus-control-rs/main/bootstrap.sh | bash
```

### Option 2: Manual Clone & Install
```bash
git clone https://github.com/SwiftyPop/victus-control-rs.git
cd victus-control-rs
sudo ./install.sh
```

The installer performs the following actions:
1. Installs package dependencies.
2. Compiles the Rust binaries in release mode (`cargo build --release`).
3. Installs `victus-backend`, `victus-control`, and `victus-monitor` to `/usr/bin/`.
4. Deploys the D-Bus security policy to `/usr/share/dbus-1/system.d/`.
5. Compiles and loads the patched `hp_wmi` DKMS kernel module.
6. Enables and starts the `victus-backend.service` systemd unit.
7. Installs the GNOME Shell extension if a GNOME desktop environment is detected.

---

## GNOME Shell Extension

If you use GNOME Shell, the installer automatically deploys the extension to `~/.local/share/gnome-shell/extensions/victus-control@victus`.

To manually install or enable the extension:
```bash
./gnome-extension/install.sh
gnome-extensions enable victus-control@victus
```
Restart GNOME Shell (`Alt+F2` -> `r` on X11, or log out and log back in on Wayland) to activate the panel indicator.

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

## Troubleshooting

### D-Bus Access / Permission Denied
If `victus-control` cannot connect to D-Bus or reports access errors:
1. Ensure your user account belongs to the `victus` group:
   ```bash
   sudo usermod -aG victus $USER
   ```
2. Log out and log back in to apply group membership changes.

### DKMS Module Build Failure
If the `hp_wmi` module fails to compile:
1. Ensure your running kernel headers match your kernel version:
   - **Arch**: `sudo pacman -S linux-headers` (or `linux-cachyos-headers` / `linux-zen-headers`)
   - **Fedora**: `sudo dnf install kernel-devel kernel-headers`
   - **Ubuntu**: `sudo apt install linux-headers-$(uname -r)`
2. Re-run `sudo dkms install hp-wmi-fan-and-backlight-control/0.0.2`.

### Hardware Sensor Not Found
If sysfs fan target nodes are missing under `/sys/devices/platform/hp-wmi/hwmon/`:
- Ensure `options hp_wmi force_fan_control_support=1` is present in `/etc/modprobe.d/hp-wmi.conf`.
- Reload the module:
  ```bash
  sudo modprobe -r hp_wmi && sudo modprobe hp_wmi force_fan_control_support=1
  ```

---

## Uninstallation

To remove `victus-control-rs` completely:

```bash
# Stop and disable systemd services
sudo systemctl disable --now victus-backend.service victus-healthcheck.service

# Remove installed binaries and configuration files
sudo rm -f /usr/bin/victus-backend /usr/bin/victus-control /usr/bin/victus-monitor
sudo rm -f /usr/share/dbus-1/system.d/org.hp.VictusControl.conf
sudo rm -f /usr/lib/systemd/system/victus-backend.service
sudo rm -f /usr/lib/systemd/system/victus-healthcheck.service
sudo rm -rf /usr/lib/victus-control

# Remove DKMS kernel module
sudo dkms remove hp-wmi-fan-and-backlight-control/0.0.2 --all
sudo rm -rf /usr/src/hp-wmi-fan-and-backlight-control-0.0.2

# Remove GNOME extension (optional)
rm -rf ~/.local/share/gnome-shell/extensions/victus-control@victus
```

---

## Technical Details

### Hardware Interface
The backend communicates with the `hp_wmi` kernel driver via sysfs:
- `/sys/devices/platform/hp-wmi/hwmon/hwmon*/pwm1_enable`: Sets the control mode (0=MAX, 1=MANUAL/BETTER_AUTO, 2=AUTO).
- `/sys/devices/platform/hp-wmi/hwmon/hwmon*/fan1_target`: Sets fan target speed (RPM).
- `/sys/devices/platform/hp-wmi/hwmon/pwm1`: Fallback PWM control node (0-255).

If your laptop BIOS does not report manual fan control support, the installer adds `options hp_wmi force_fan_control_support=1` to `/etc/modprobe.d/hp-wmi.conf`.

---

## License

GNU General Public License v3.0 (GPL-3.0).
