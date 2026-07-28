use anyhow::{anyhow, Context, Result};
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, PartialEq, Eq)]
enum Distro {
    Arch,
    Fedora,
    Ubuntu,
    Unknown,
}

fn detect_distro() -> Distro {
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        let content_lower = content.to_lowercase();
        if content_lower.contains("cachyos")
            || content_lower.contains("arch")
            || content_lower.contains("manjaro")
            || content_lower.contains("endeavouros")
        {
            return Distro::Arch;
        }
        if content_lower.contains("fedora") || content_lower.contains("rhel") {
            return Distro::Fedora;
        }
        if content_lower.contains("ubuntu")
            || content_lower.contains("debian")
            || content_lower.contains("pop")
            || content_lower.contains("mint")
        {
            return Distro::Ubuntu;
        }
    }
    Distro::Unknown
}

fn run_cmd(cmd: &str, args: &[&str]) -> Result<()> {
    println!("--> Running: {} {}", cmd, args.join(" "));
    let status = Command::new(cmd)
        .args(args)
        .status()
        .with_context(|| format!("Failed to execute command: {} {}", cmd, args.join(" ")))?;
    if !status.success() {
        return Err(anyhow!(
            "Command failed with exit code: {} ({} {})",
            status.code().unwrap_or(-1),
            cmd,
            args.join(" ")
        ));
    }
    Ok(())
}

fn run_cmd_ignore_fail(cmd: &str, args: &[&str]) {
    let _ = Command::new(cmd).args(args).status();
}

fn ensure_root() -> Result<()> {
    let uid = Command::new("id")
        .arg("-u")
        .output()
        .context("Failed to check UID")?;
    let uid_str = String::from_utf8_lossy(&uid.stdout).trim().to_string();
    if uid_str != "0" {
        return Err(anyhow!(
            "Installer must be run with root privileges (sudo ./install.sh)."
        ));
    }
    Ok(())
}

fn run_cmd_env(cmd: &str, args: &[&str], envs: &[(&str, &str)]) -> Result<()> {
    println!("--> Running: {} {}", cmd, args.join(" "));
    let mut command = Command::new(cmd);
    command.args(args);
    for (k, v) in envs {
        command.env(k, v);
    }
    let status = command
        .status()
        .with_context(|| format!("Failed to execute command: {} {}", cmd, args.join(" ")))?;
    if !status.success() {
        return Err(anyhow!(
            "Command failed with exit code: {} ({} {})",
            status.code().unwrap_or(-1),
            cmd,
            args.join(" ")
        ));
    }
    Ok(())
}

fn install_dependencies(distro: &Distro) -> Result<()> {
    println!("--> Installing required system packages...");
    match distro {
        Distro::Arch => {
            let mut pkgs = vec!["rust", "cargo", "gtk4", "git", "dkms", "sudo", "libnotify"];
            // Detect running kernels for headers
            if let Ok(output) = Command::new("uname").arg("-r").output() {
                let kernel_release = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if kernel_release.contains("cachyos-lts") {
                    pkgs.push("linux-cachyos-lts-headers");
                } else if kernel_release.contains("cachyos") {
                    pkgs.push("linux-cachyos-headers");
                } else if kernel_release.contains("zen") {
                    pkgs.push("linux-zen-headers");
                } else if kernel_release.contains("lts") {
                    pkgs.push("linux-lts-headers");
                } else {
                    pkgs.push("linux-headers");
                }
            }
            let mut args = vec!["-Sy", "--needed", "--noconfirm"];
            args.extend(pkgs);
            run_cmd("pacman", &args)?;
        }
        Distro::Fedora => {
            run_cmd(
                "dnf",
                &[
                    "install",
                    "-y",
                    "rust",
                    "cargo",
                    "gtk4-devel",
                    "git",
                    "dkms",
                    "sudo",
                    "libnotify",
                    "kernel-devel",
                    "kernel-headers",
                ],
            )?;
        }
        Distro::Ubuntu => {
            run_cmd_env(
                "apt-get",
                &["update"],
                &[("DEBIAN_FRONTEND", "noninteractive")],
            )?;

            let mut header_pkg = "linux-headers-generic".to_string();
            if let Ok(output) = Command::new("uname").arg("-r").output() {
                let kernel_release = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let specific_headers = format!("linux-headers-{}", kernel_release);
                // Check if specific kernel headers are installed (dpkg-query) or available in apt repositories (apt-cache)
                let is_installed = Command::new("dpkg-query")
                    .args(["-W", "-f='${Status}'", &specific_headers])
                    .output()
                    .map(|out| {
                        out.status.success()
                            && !String::from_utf8_lossy(&out.stdout).contains("unknown")
                    })
                    .unwrap_or(false);

                let is_available = Command::new("apt-cache")
                    .args(["show", &specific_headers])
                    .output()
                    .map(|out| out.status.success())
                    .unwrap_or(false);

                if is_installed || is_available {
                    header_pkg = specific_headers;
                }
            }

            run_cmd_env(
                "apt-get",
                &[
                    "install",
                    "-y",
                    "rustc",
                    "cargo",
                    "libgtk-4-dev",
                    "git",
                    "dkms",
                    "sudo",
                    "libnotify-bin",
                    &header_pkg,
                ],
                &[("DEBIAN_FRONTEND", "noninteractive")],
            )?;
        }
        Distro::Unknown => {
            println!("--> Unknown Linux distribution; skipping package manager auto-install.");
        }
    }
    Ok(())
}

fn create_users_and_groups() -> Result<()> {
    println!("--> Creating secure users and groups...");
    run_cmd_ignore_fail("groupadd", &["-f", "victus"]);
    run_cmd_ignore_fail("groupadd", &["-f", "victus-backend"]);
    run_cmd_ignore_fail(
        "useradd",
        &[
            "-r",
            "-g",
            "victus-backend",
            "-s",
            "/sbin/nologin",
            "victus-backend",
        ],
    );

    if let Ok(sudo_user) = env::var("SUDO_USER") {
        if !sudo_user.is_empty() && sudo_user != "root" {
            println!("--> Adding user '{}' to group 'victus'...", sudo_user);
            run_cmd_ignore_fail("usermod", &["-aG", "victus", &sudo_user]);
        }
    }
    Ok(())
}

fn configure_hp_wmi_options() -> Result<()> {
    println!("--> Configuring hp-wmi module options (force_fan_control_support=1)...");
    fs::write(
        "/etc/modprobe.d/hp-wmi.conf",
        "options hp_wmi force_fan_control_support=1\n",
    )
    .context("Failed to write /etc/modprobe.d/hp-wmi.conf")?;
    Ok(())
}

fn install_dkms_module(workspace_dir: &Path) -> Result<()> {
    println!("--> Building and installing patched hp-wmi kernel module...");
    let wmi_dir = workspace_dir.join("wmi-src/hp-wmi-fan-and-backlight-control");
    if !wmi_dir.exists() {
        return Err(anyhow!(
            "DKMS module source not found at {}",
            wmi_dir.display()
        ));
    }

    let dkms_dest = Path::new("/usr/src/hp-wmi-fan-and-backlight-control-0.0.2");
    if dkms_dest.exists() {
        let _ = fs::remove_dir_all(dkms_dest);
    }

    run_cmd(
        "cp",
        &["-r", wmi_dir.to_str().unwrap(), dkms_dest.to_str().unwrap()],
    )?;

    // Stop active services holding sysfs nodes open
    run_cmd_ignore_fail(
        "systemctl",
        &[
            "stop",
            "victus-backend.service",
            "victus-healthcheck.service",
        ],
    );

    run_cmd_ignore_fail(
        "dkms",
        &["remove", "hp-wmi-fan-and-backlight-control/0.0.2", "--all"],
    );
    run_cmd("dkms", &["add", "hp-wmi-fan-and-backlight-control/0.0.2"])?;
    run_cmd(
        "dkms",
        &["install", "hp-wmi-fan-and-backlight-control/0.0.2"],
    )?;

    // Verify built module matches current running kernel
    if let Ok(output) = Command::new("uname").arg("-r").output() {
        let running_kernel = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if let Ok(status_out) = Command::new("dkms")
            .args(["status", "hp-wmi-fan-and-backlight-control/0.0.2"])
            .output()
        {
            let status_str = String::from_utf8_lossy(&status_out.stdout);
            if status_str.contains(&running_kernel) {
                println!(
                    "--> Verified DKMS module built cleanly for running kernel ({})",
                    running_kernel
                );
            } else {
                println!(
                    "⚠️ Warning: DKMS module installation status does not show running kernel ({})",
                    running_kernel
                );
            }
        }
    }

    run_cmd_ignore_fail("depmod", &["-a"]);

    if Path::new("/sys/module/hp_wmi").exists() {
        println!("--> Unloading running hp_wmi module (without force)...");
        if run_cmd("modprobe", &["-r", "hp_wmi"]).is_err() {
            println!("⚠️ Could not unload active hp_wmi module. Please restart your system to complete the module update.");
        } else {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }

    println!("--> Loading patched hp_wmi module with force_fan_control_support=1...");
    let _ = run_cmd("modprobe", &["hp_wmi", "force_fan_control_support=1"]);

    Ok(())
}

fn build_and_deploy_workspace(workspace_dir: &Path) -> Result<()> {
    println!("--> Compiling Rust workspace release binaries...");
    let sudo_user = env::var("SUDO_USER").unwrap_or_else(|_| "root".to_string());

    if sudo_user != "root" {
        run_cmd(
            "sudo",
            &[
                "-u",
                &sudo_user,
                "cargo",
                "build",
                "--release",
                "--workspace",
                "--manifest-path",
                workspace_dir.join("Cargo.toml").to_str().unwrap(),
            ],
        )?;
    } else {
        run_cmd("cargo", &["build", "--release", "--workspace"])?;
    }

    println!("--> Deploying binaries and configuration assets...");
    let release_dir = workspace_dir.join("target/release");

    run_cmd(
        "install",
        &[
            "-m",
            "0755",
            release_dir.join("victus-backend").to_str().unwrap(),
            "/usr/bin/victus-backend",
        ],
    )?;
    run_cmd(
        "install",
        &[
            "-m",
            "0755",
            release_dir.join("victus-control").to_str().unwrap(),
            "/usr/bin/victus-control",
        ],
    )?;
    run_cmd(
        "install",
        &[
            "-m",
            "0755",
            release_dir.join("victus-monitor").to_str().unwrap(),
            "/usr/bin/victus-monitor",
        ],
    )?;

    // Monitor User Systemd Service
    fs::create_dir_all("/usr/lib/systemd/user")?;
    run_cmd(
        "install",
        &[
            "-m",
            "0644",
            workspace_dir
                .join("monitor/victus-monitor.service")
                .to_str()
                .unwrap(),
            "/usr/lib/systemd/user/victus-monitor.service",
        ],
    )?;

    // D-Bus Policy
    run_cmd(
        "install",
        &[
            "-m",
            "0644",
            workspace_dir
                .join("backend/org.hp.VictusControl.conf")
                .to_str()
                .unwrap(),
            "/usr/share/dbus-1/system.d/org.hp.VictusControl.conf",
        ],
    )?;

    // Systemd service
    run_cmd(
        "install",
        &[
            "-m",
            "0644",
            workspace_dir
                .join("backend/victus-backend.service")
                .to_str()
                .unwrap(),
            "/usr/lib/systemd/system/victus-backend.service",
        ],
    )?;

    // Desktop Launcher & Icon
    run_cmd(
        "install",
        &[
            "-m",
            "0644",
            workspace_dir
                .join("frontend/victus-control.desktop")
                .to_str()
                .unwrap(),
            "/usr/share/applications/victus-control.desktop",
        ],
    )?;
    fs::create_dir_all("/usr/share/icons/hicolor/scalable/apps")?;
    run_cmd(
        "install",
        &[
            "-m",
            "0644",
            workspace_dir
                .join("frontend/victus-icon.svg")
                .to_str()
                .unwrap(),
            "/usr/share/icons/hicolor/scalable/apps/victus-icon.svg",
        ],
    )?;

    // Healthcheck Service
    fs::create_dir_all("/usr/lib/victus-control")?;
    run_cmd(
        "install",
        &[
            "-m",
            "0755",
            workspace_dir
                .join("backend/victus-healthcheck.sh")
                .to_str()
                .unwrap(),
            "/usr/lib/victus-control/victus-healthcheck.sh",
        ],
    )?;
    run_cmd(
        "install",
        &[
            "-m",
            "0644",
            workspace_dir
                .join("backend/victus-healthcheck.service")
                .to_str()
                .unwrap(),
            "/usr/lib/systemd/system/victus-healthcheck.service",
        ],
    )?;

    // Systemd reload & enable
    run_cmd("systemctl", &["daemon-reload"])?;
    run_cmd("systemctl", &["enable", "--now", "victus-backend.service"])?;
    run_cmd(
        "systemctl",
        &["enable", "--now", "victus-healthcheck.service"],
    )?;

    run_cmd_ignore_fail("update-desktop-database", &[]);
    run_cmd_ignore_fail(
        "gtk-update-icon-cache",
        &["-f", "-t", "/usr/share/icons/hicolor"],
    );

    Ok(())
}

fn install_gnome_extension(workspace_dir: &Path) -> Result<()> {
    let has_gnome = Path::new("/usr/bin/gnome-shell").exists()
        || env::var("XDG_CURRENT_DESKTOP")
            .map(|d| d.to_lowercase().contains("gnome"))
            .unwrap_or(false);

    if has_gnome {
        let script = workspace_dir.join("gnome-extension/install.sh");
        if script.exists() {
            println!("--> GNOME Shell detected; installing GNOME extension...");
            let sudo_user = env::var("SUDO_USER").unwrap_or_default();
            if !sudo_user.is_empty() && sudo_user != "root" {
                run_cmd_ignore_fail(
                    "sudo",
                    &["-u", &sudo_user, "bash", script.to_str().unwrap()],
                );
            } else {
                run_cmd_ignore_fail("bash", &[script.to_str().unwrap()]);
            }
        }
    }
    Ok(())
}

fn verify_installation() -> Result<()> {
    println!("--> Verifying hardware fan control interface...");
    let hwmon_base = Path::new("/sys/devices/platform/hp-wmi/hwmon");
    if !hwmon_base.exists() {
        return Err(anyhow!(
            "hp_wmi hwmon directory not found under /sys/devices/platform/hp-wmi/hwmon"
        ));
    }

    let mut found_node = false;
    if let Ok(entries) = fs::read_dir(hwmon_base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && (path.join("pwm1_enable").exists()
                    || path.join("pwm1").exists()
                    || path.join("fan1_target").exists())
            {
                found_node = true;
                println!(
                    "--> Verified hardware fan control interface at: {}",
                    path.display()
                );
                break;
            }
        }
    }

    if !found_node {
        return Err(anyhow!("Fan control sysfs nodes (pwm1_enable / pwm1 / fan1_target) not found under /sys/devices/platform/hp-wmi/hwmon/"));
    }

    Ok(())
}

fn main() -> Result<()> {
    println!("=== Starting Victus Control Installation (Pure Rust Installer) ===");

    ensure_root()?;

    let distro = detect_distro();
    println!("--> Detected distribution: {:?}", distro);

    let current_dir = env::current_dir().context("Failed to get current directory")?;

    install_dependencies(&distro)?;
    create_users_and_groups()?;
    configure_hp_wmi_options()?;
    install_dkms_module(&current_dir)?;
    build_and_deploy_workspace(&current_dir)?;
    install_gnome_extension(&current_dir)?;
    verify_installation()?;

    println!("\n=== Installation Completed Successfully! ===");
    println!("Run 'victus-control' or launch 'HP Victus Fan Control' from your application menu.");

    Ok(())
}
