# victus-control-rs — Implementation Plan

## 1. Purpose

This document lists the known problems in the victus-control-rs code.
This document gives an action for each problem.
This document uses Simplified Technical English (STE), per the ASD-STE100 specification.
STE uses short sentences and a controlled set of words.
STE helps different readers understand the same text in the same way.

## 2. Technical names used in this document

STE permits technical names that are not in the general word list.
This document uses these technical names. Each name has one meaning.

- **D-Bus**: a message system for programs on the same computer.
- **sysfs**: a file system that shows kernel data as files.
- **hwmon**: the kernel sub-system for hardware monitor data in sysfs.
- **DKMS**: Dynamic Kernel Module Support. DKMS rebuilds a kernel module after a kernel update.
- **PWM**: Pulse Width Modulation. The system uses PWM values to set fan speed.
- **RPM**: Revolutions Per Minute. The system uses RPM values to show fan speed.
- **mutex**: a lock that stops two program threads from using the same data at the same time.
- **async runtime**: the part of the program that runs many tasks on a small set of threads.
- **udev**: the kernel component that manages device files and their file permissions.
- **CWE**: Common Weakness Enumeration. CWE is a public list of software weakness types.

## 3. Priority levels

This document gives one priority to each item.

- **Critical**: the problem can damage hardware, harm data, or remove access control.
- **High**: the problem can cause wrong behavior or loss of service.
- **Medium**: the problem reduces code quality or user experience.
- **Low**: the problem is small. The fix is easy.

## 4. Fan control logic

### IP-1 — The set_mode function does not report write errors (Critical)

**Problem.** The `set_mode` function is in `backend/src/fan.rs`.
The function writes a new value to the file `pwm1_enable`.
The function does not stop when this write fails.
The function always returns `Ok(())`.
The D-Bus client and the user interface cannot see the failure.

**Action.**
1. Change the return type logic so `set_mode` returns an error when the file write fails.
2. Send this error to the D-Bus caller.
3. Show this error in the user interface.

**Reference.** CWE-252, Unchecked Return Value [1].

### IP-2 — A failed temperature read gives a false low value (Critical)

**Problem.** The functions `get_cpu_temp` and `get_gpu_temp` are in `backend/src/hwmon.rs`.
Each function returns `0.0` when the sensor file read fails.
The value `0.0` is below the minimum fan curve threshold of 45°C.
The auto fan loop reads this false value.
The auto fan loop then sets the fans to the minimum speed.
This action is not safe. The real temperature is not known at this time.

**Action.**
1. Change the return type to an optional value.
2. When a read fails, do not send a new value to the fan curve function.
3. Use the last known good temperature value instead.
4. Write a log warning after 3 failed reads in a row.

### IP-3 — A failed fan speed read gives a false zero value (High)

**Problem.** The function `get_fan_speed` is in `backend/src/fan.rs`.
The function returns `0` RPM when the sysfs file read fails.
A monitor tool cannot tell a real stopped fan from a failed sensor read.

**Action.** Apply the same fix method as item IP-2. Use an optional return value.

### IP-4 — The hwmon directory search uses the wrong sort order (High)

**Problem.** The function `find_hp_wmi_hwmon_dir` is in `backend/src/fan.rs`.
The function sorts hwmon directory names as text, not as numbers.
The Linux kernel hwmon interface names directories `hwmon0`, `hwmon1`, up to `hwmonN` [2].
A text sort places `hwmon10` before `hwmon2`.
On a system with 10 or more hwmon devices, the function can select the wrong device.

**Action.**
1. Read the number part of each directory name.
2. Sort the directory list by this number, not by the full text.

### IP-5 — The daemon uses a blocking sleep call inside async code (High)

**Problem.** The `set_mode` function calls `std::thread::sleep` for 100 milliseconds.
This function runs inside the Tokio async runtime.
A blocking sleep call stops the runtime thread from other work during this time.
Other D-Bus calls and the auto fan loop must then wait [3].

**Action.**
1. Change `set_mode` to an async function.
2. Replace `std::thread::sleep` with `tokio::time::sleep`.
3. Update all callers of `set_mode` to use `.await`.

**Reference.** Tokio task documentation, "Blocking and asynchronous code" [3].

### IP-6 — A single panic can stop the whole daemon (High)

**Problem.** The backend uses `std::sync::Mutex` in 4 places.
Each lock call uses `.unwrap()`.
A panic while a thread holds one of these locks poisons the mutex [4].
After this event, every later `.unwrap()` call on the same mutex also panics.
The whole daemon process can then stop.

**Action.**
1. Replace `std::sync::Mutex` with `parking_lot::Mutex`, which does not use poisoning. Or:
2. Add explicit poison-error handling at each `.lock()` call.

**Reference.** Rust standard library documentation, module `std::sync`, type `Mutex` [4].

### IP-7 — The daemon finds hardware sensor paths only one time (Medium)

**Problem.** The function `HwmonMonitor::new` finds sensor file paths one time at start-up.
The daemon does not check these paths again while it runs.
A kernel module reload can change these paths.
After a module reload, the daemon can show old or wrong data.

**Action.**
1. Add a re-check function.
2. Call this function when a sensor read fails 3 times in a row.
3. Update the cached file paths after a successful re-check.

## 5. Fail-safe behavior at shutdown

### IP-8 — The daemon does not handle the stop signal from systemd (Critical)

**Problem.** The file `backend/src/main.rs` waits for the SIGINT signal only.
The `systemctl stop` command sends the SIGTERM signal by default [5].
The daemon does not catch SIGTERM.
The daemon then stops without a clean exit.

**Action.**
1. Add a handler for the SIGTERM signal.
2. Use this handler together with the existing SIGINT handler.

**Reference.** systemd.kill man page, section KillSignal [5].

### IP-9 — The fans do not return to a safe mode at shutdown (Critical)

**Problem.** The daemon has no shutdown action for the fan mode.
A user can stop or restart the daemon while it is in MANUAL or MAX mode.
The fans then stay at the last set speed.
The fans do not return to the firmware AUTO mode.

**Action.**
1. Add a shutdown function that runs before the process exits.
2. In this function, write the AUTO value to `pwm1_enable`.
3. Run this function for both the SIGTERM handler and the SIGINT handler.
4. Test this function with `systemctl stop victus-backend.service`.

## 6. Access control

### IP-10 — The D-Bus policy file allows all local users (Critical)

**Problem.** The file `backend/org.hp.VictusControl.conf` has a policy block for `context="default"`.
This block allows every local user to send and receive D-Bus messages to the service [6].
This block gives the same access as the `victus` group policy block.
The `victus` group check has no effect while this default block is present.
Any local user can then change the fan mode and fan speed.

**Action.**
1. Remove the `<policy context="default">` block, or
2. Change this block to `<deny>` for `send_destination`.
3. Keep only the `victus` group block with `<allow>`.
4. Test D-Bus access with a user account outside the `victus` group.

**Reference.** dbus-daemon man page, section CONFIGURATION FILE [6]. CWE-284, Improper Access Control [7].

### IP-11 — Two udev rule files set overlapping permissions (Medium)

**Problem.** The files `99-hp-wmi-permissions.rules` and `backend/victus-control.rules` are both present.
Both files grant the `victus` group write access to the same sysfs fan files.
This write path does not go through the daemon.
This write path does not go through the D-Bus policy file.
The Linux kernel hwmon interface states that writable attributes must be for privileged users only [2].
One comment line in `victus-control.rules` names the group `victus-backend`.
The rule on that line uses the group `victus` instead.

**Action.**
1. Keep one udev rule file only.
2. Decide if direct sysfs write access is still needed now that the daemon uses D-Bus.
3. If not needed, remove the write permission from the udev rule.
4. Fix the comment line to match the group name in the rule.

**Reference.** Linux kernel hwmon sysfs interface documentation [2].

## 7. Files from the earlier source project

The victus-control-rs project was built with reference to the C++ project victus-control.
Some files from that reference are still present but are not used.
This section lists these files.

### IP-12 — An empty fallback directory (Medium)

**Problem.** The directory `wmi-project/hp-wmi-fan-and-backlight-control` has no files inside it.
The file `installer/src/main.rs` checks this path as a fallback.
This fallback path cannot work, because the directory is empty.

**Action.** Remove the empty directory and remove the fallback check in the installer code.

### IP-13 — An unused tmpfiles rule for a socket path (Medium)

**Problem.** The file `backend/victus-control.tmpfiles` creates the directory `/run/victus-control`.
This directory was for a Unix socket in the earlier C++ project.
The current backend uses D-Bus, not a Unix socket.
The installer does not install this file.

**Action.** Remove the file `backend/victus-control.tmpfiles`.

### IP-14 — A reference to a service that does not exist (Low)

**Problem.** The file `backend/victus-backend.service` has the line `Conflicts=victus-fan.service`.
No file named `victus-fan.service` exists in this project.

**Action.** Remove this line from the service file.

### IP-15 — A GNOME extension file still names the other project (Low)

**Problem.** The file `gnome-extension/metadata.json` has the URL field set to the Batuhan4/victus-control repository.

**Action.** Change the URL field to the SwiftyPop/victus-control-rs repository.

### IP-16 — A monitor service file still names the other project (Low)

**Problem.** The file `monitor/victus-monitor.service` has the line `Documentation=` set to the Batuhan4/victus-control repository.

**Action.** Change this line to the SwiftyPop/victus-control-rs repository, or remove the line.

### IP-17 — Unused keyboard light rules in the udev file (Low)

**Problem.** The file `backend/victus-control.rules` has rules for the file `hp::kbd_backlight`.
This project has no keyboard light code at this time.

**Action.** Remove these lines, or add a code comment that states these lines are for later use.

### IP-18 — A placeholder value in the authors field (Low)

**Problem.** The root file `Cargo.toml` has `authors = ["Victus Control Team"]`.

**Action.** Change this value to the correct author name or names.

## 8. Installer

### IP-19 — The installer does not set up the GNOME extension (High)

**Problem.** The function `build_and_deploy_workspace` in `installer/src/main.rs` does not call the script `gnome-extension/install.sh`.
A user must find and run this script by hand.
The main README file does not name the GNOME extension.

**Action.**
1. Add a step in the installer to detect a GNOME Shell session.
2. When a GNOME Shell session is present, run the extension install script for the correct user.
3. Add a section about the GNOME extension to the README file.

### IP-20 — The Ubuntu package list may not match the running kernel (Medium)

**Problem.** The function `install_dependencies` installs the package `linux-headers-generic` on Ubuntu systems.
This package can hold headers for a different kernel version than the one now running.
This difference is common right after a kernel update, before a restart.

**Action.** Change the package name to match the output of the `uname -r` command.

### IP-21 — The Ubuntu install step can wait for user input (Medium)

**Problem.** The function `install_dependencies` runs `apt-get install` on Ubuntu systems.
This command can show an interactive prompt during the install.
An interactive prompt stops an unattended install script.

**Action.** Set the environment variable `DEBIAN_FRONTEND=noninteractive` before this command.

### IP-22 — The installer force-removes the hp_wmi kernel module (High)

**Problem.** The function `install_dkms_module` runs `rmmod -f hp_wmi` in some cases.
The `-f` flag removes a module even while other code uses it [8].
The `hp_wmi` module also manages other functions on many HP computers.
Examples of these functions are the keyboard hotkeys and the wireless radio switch.
A forced removal of this module can stop these other functions.

**Action.**
1. Remove the `-f` flag from the `rmmod` command.
2. Use `modprobe -r hp_wmi` first, without force.
3. If this command fails, show a message that asks the user to restart the computer.

**Reference.** rmmod man page, section OPTIONS, flag `-f` [8].

### IP-23 — The installer does not check the module build for the current kernel (Medium)

**Problem.** The function `install_dkms_module` calls `dkms install` and trusts a success exit code.
The function does not check that the built module matches the output of `uname -r`.
The healthcheck script performs this check. The installer script does not.

**Action.** Add the same check method that the healthcheck script uses, inside the installer.

## 9. User interface

### IP-24 — The manual mode sliders use a fixed RPM range (Medium)

**Problem.** The file `frontend/src/ui.rs` sets each fan slider to a fixed range of 2000 to 6000 RPM.
The backend constant `DEFAULT_MAX_RPM_FAN2` is 6100 RPM.
The D-Bus method `get_fan_max_speed` gives the real maximum RPM for each fan.
The user interface does not call this method to set the slider range.

**Action.**
1. Call `get_fan_max_speed` for fan 1 and fan 2 at start-up.
2. Set each slider range with these values.

### IP-25 — The manual mode sliders do not show the current fan speed (Medium)

**Problem.** A slider does not move to the real current RPM value when MANUAL mode starts.
A user can then move the slider a small amount and cause a large speed change.

**Action.**
1. Read the current fan speed with `get_fan_speed` when MANUAL mode starts.
2. Set the slider value to this speed before the user moves it.

### IP-26 — The user interface does not show D-Bus call errors (Medium)

**Problem.** The functions `connect_selected_notify` and `connect_value_changed` in `frontend/src/ui.rs` discard D-Bus call errors.
The user interface gives no sign of a failed mode change or a failed speed change.

**Action.** Add an error message label. Show this label when a D-Bus call returns an error.

### IP-27 — The start-up code sends an extra D-Bus write (Low)

**Problem.** The start-up code sets the mode dropdown to the current mode.
This action triggers the `connect_selected_notify` handler.
This handler then sends a `set_fan_mode` call back to the daemon with the same mode value.

**Action.** Add a flag that blocks the handler during the start-up read. Remove the flag after the read.

### IP-28 — A systemd command runs on the main user interface thread (Low)

**Problem.** The function `notify_switch.connect_active_notify` runs the `systemctl` command in a direct call.
This call can pause the user interface for a short time.

**Action.** Move this command call to an async task.

## 10. Build checks and tests

### IP-29 — No workflow builds or tests the code (High)

**Problem.** The directory `.github/workflows` has 5 files.
Each file runs an AI review or triage tool.
No file runs `cargo build`, `cargo test`, `cargo clippy`, or `cargo fmt --check`.
A change with a build error can merge without a warning.

**Action.**
1. Add a new workflow file.
2. In this file, run `cargo build --release --workspace` on each push and each pull request.
3. In this file, run `cargo test --workspace`.
4. In this file, run `cargo clippy --workspace -- -D warnings`.
5. In this file, run `cargo fmt --check`.

### IP-30 — Test coverage is not complete (Medium)

**Problem.** Unit tests exist for `fan.rs`, `common/src/lib.rs`, and `monitor/src/alert.rs`.
No unit test exists for `hwmon.rs`, `dbus_service.rs`, or the installer code.
No test calls the D-Bus interface directly.

**Action.**
1. Add unit tests for the sensor path search functions in `hwmon.rs`.
2. Add a test that starts the D-Bus service and calls each method.

## 11. Documents

### IP-31 — The README does not name the GNOME extension (Low)

**Action.** Add a section to the README file. State that the GNOME extension is present. State the install steps.

### IP-32 — The README has no troubleshooting section (Low)

**Action.** Add a troubleshooting section. Cover: a failed D-Bus connection, a failed DKMS build, and a permission error.

### IP-33 — The README has no uninstall steps (Low)

**Action.** Add an uninstall section. State the commands to stop the services and remove the DKMS module.

## 12. Summary table

| ID | Area | Priority |
|----|------|----------|
| IP-1 | Fan control logic | Critical |
| IP-2 | Fan control logic | Critical |
| IP-3 | Fan control logic | High |
| IP-4 | Fan control logic | High |
| IP-5 | Async code | High |
| IP-6 | Async code | High |
| IP-7 | Fan control logic | Medium |
| IP-8 | Shutdown behavior | Critical |
| IP-9 | Shutdown behavior | Critical |
| IP-10 | Access control | Critical |
| IP-11 | Access control | Medium |
| IP-12 | Repository cleanup | Medium |
| IP-13 | Repository cleanup | Medium |
| IP-14 | Repository cleanup | Low |
| IP-15 | Repository cleanup | Low |
| IP-16 | Repository cleanup | Low |
| IP-17 | Repository cleanup | Low |
| IP-18 | Repository cleanup | Low |
| IP-19 | Installer | High |
| IP-20 | Installer | Medium |
| IP-21 | Installer | Medium |
| IP-22 | Installer | High |
| IP-23 | Installer | Medium |
| IP-24 | User interface | Medium |
| IP-25 | User interface | Medium |
| IP-26 | User interface | Medium |
| IP-27 | User interface | Low |
| IP-28 | User interface | Low |
| IP-29 | Build checks | High |
| IP-30 | Build checks | Medium |
| IP-31 | Documents | Low |
| IP-32 | Documents | Low |
| IP-33 | Documents | Low |

## 13. References

[1] MITRE Corporation. "CWE-252: Unchecked Return Value." Common Weakness Enumeration.
https://cwe.mitre.org/data/definitions/252.html

[2] The Linux Kernel Documentation. "Naming and data format standards for sysfs files — hwmon."
https://www.kernel.org/doc/Documentation/hwmon/sysfs-interface

[3] Tokio project. "tokio::task — Blocking and asynchronous code." Tokio API documentation.
https://docs.rs/tokio/latest/tokio/task/

[4] Rust project. "std::sync::Mutex." The Rust Standard Library documentation.
https://doc.rust-lang.org/std/sync/struct.Mutex.html

[5] freedesktop.org. "systemd.kill — Process killing procedure configuration." systemd manual pages.
https://www.freedesktop.org/software/systemd/man/latest/systemd.kill.html

[6] freedesktop.org / Arch Linux manual pages. "dbus-daemon(1) — Message bus daemon." Section: bus configuration files, policy element, context="default".
https://man.archlinux.org/man/dbus-daemon.1.en

[7] MITRE Corporation. "CWE-284: Improper Access Control." Common Weakness Enumeration.
https://cwe.mitre.org/data/definitions/284.html

[8] kernel.org man-pages project. "rmmod(8) — remove a module from the Linux kernel." Section: OPTIONS, flag -f / --force.
https://man7.org/linux/man-pages/man8/rmmod.8.html
