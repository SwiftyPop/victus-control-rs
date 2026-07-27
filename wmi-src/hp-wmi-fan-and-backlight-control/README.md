## hp-wmi manual fan + keyboard RGB control

This is an Initial module for manual fan control and keyboard RGB for devices that support it.

> [!IMPORTANT]
> Four-zone RGB support in this branch is currently scoped to hardware that actually exposes four physical lighting zones. It was reported working by the PR author on **OMEN 16-wf0xxx (board 8BAB)**. On **Victus 16-s0xxx / board 8BD4**, fan control works, but keyboard RGB still behaves as a single physical zone in testing.

> [!NOTE]
> **HP Victus 15:** For some reason, HP didnt mark manual fan control supported on HP Victus 15 laptops, even though the hardware supports it. So by default manual fan control is not enabled in this module. But if you want to turn it on, load the module with the `force_fan_control_support=true` parameter.
>
> ```bash
> sudo insmod hp-wmi.ko force_fan_control_support=true
> ```

### Installation:

Dkms:
```
git clone https://github.com/Vilez0/hp-wmi-fan-and-backlight-control
cd hp-wmi-fan-and-backlight-control
make
sudo make install-dkms
```

Arch package:
```
git clone https://github.com/Vilez0/hp-wmi-fan-and-backlight-control
cd hp-wmi-fan-and-backlight-control
make install-arch
```

or if you just want to test the module (not permanent):
```bash
git clone https://github.com/Vilez0/hp-wmi-fan-and-backlight-control
cd hp-wmi-fan-and-backlight-control/
make
sudo rmmod hp-wmi
sudo modprobe led_class_multicolor
sudo insmod hp-wmi.ko
```

### Usage 
- Keyboard (single‑zone RGB): control under `/sys/class/leds/hp::kbd_backlight`.
  - Use the multicolor interface attribute `multi_intensity` which accepts `R G B` (0–255 each).
  - Example:
    ```bash
    echo "255 0 0" | sudo tee /sys/class/leds/hp::kbd_backlight/multi_intensity # Set to red
    echo 128 | sudo tee /sys/class/leds/hp::kbd_backlight/brightness # Change brightness to 50% (0-255)
    ```

- Keyboard (four-zone RGB, supported hardware only): control under `/sys/devices/platform/hp-wmi/rgb_zones/`.
  - Files `zone00` through `zone03` accept `RRGGBB` hex values.
  - Example:
    ```bash
    echo FF0000 | sudo tee /sys/devices/platform/hp-wmi/rgb_zones/zone00
    echo 00FF00 | sudo tee /sys/devices/platform/hp-wmi/rgb_zones/zone01
    echo 0000FF | sudo tee /sys/devices/platform/hp-wmi/rgb_zones/zone02
    echo FFFFFF | sudo tee /sys/devices/platform/hp-wmi/rgb_zones/zone03
    ```

- Fans: control via `fanX_target` files (check fanX_max for the max values. Anything above the max values will return -EINVAL).
    ```bash
    echo 5500 | sudo tee /sys/devices/platform/hp-wmi/hwmon/hwmon*/fan1_target  # will set fan1 to 5500 rpm
    echo 5200 | sudo tee /sys/devices/platform/hp-wmi/hwmon/hwmon*/fan2_target # will set fan2 to 5200 rpm
    ```

- Fn+P Shortcut: On laptops that support it, the Fn+P shortcut for switching performance profiles should work OOTB. You can verify if its working by monitoring the `/sys/firmware/acpi/platform_profile` file.

### Tested on:
- OMEN 16-wf0xxx (board 8BAB) — four-zone RGB reported working by the PR author.
- Board 88D2 — four-zone keyboard backlight reported working.
- Victus 16-s1011nt (8C9C) — reported working.
- Victus 16-s0063nt (8BD4) — reported working.
- Victus 16-S0022NT (8BD4) — reported working; fan speed readable via `lm-sensors`.
- Victus 16-s0003nt (8BD4) — reported working.
- Victus s0011ci (8BD4) — reported working; `Fn+P` is not supported by that BIOS.
- HP Victus 16-s0177ng — reported working.
- HP Victus 16-R1077NT (9J278EA) — fan and backlight reported working; `Fn+P` also reported working.
- Victus 16-s0xxx (board 8BD4) — fan control validated locally here, but keyboard RGB behaved as a single physical zone in testing.
- Victus 15-fa10010nt (8BC4) — fan control works with `force_fan_control_support=true`; no backlight support.
- Victus 15-fa1031nt — fan control works with `force_fan_control_support=true`; backlight unsupported and reported `fan*_max` reads as `0`.
- HP Victus 15-fb1013dx — fan settings work with `force_fan_control_support=true`; backlight control unavailable.
- HP Victus 15-fa0031dx (8A50) — fan target settings work with `force_fan_control_support=true`; backlight control unavailable.

More reports and discussion: https://github.com/TUXOV/hp-wmi-fan-and-backlight-control/issues/1

### Disclaimer
USE IT AT YOUR OWN RISK. I DO NOT ACCEPT ANY RESPONSIBILITY.
