use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};
use victus_common::FanMode;
use crate::hwmon::HwmonMonitor;

const DEFAULT_MIN_RPM: u32 = 2000;
const DEFAULT_MAX_RPM_FAN1: u32 = 5800;
const DEFAULT_MAX_RPM_FAN2: u32 = 6100;

pub struct FanController {
    mode: Mutex<FanMode>,
    monitor: Arc<HwmonMonitor>,
}

impl FanController {
    pub fn new(monitor: Arc<HwmonMonitor>) -> Arc<Self> {
        let controller = Arc::new(Self {
            mode: Mutex::new(FanMode::BetterAuto),
            monitor,
        });

        let _ = controller.set_mode(FanMode::BetterAuto);

        // Spawn background task for BETTER_AUTO regulation
        let controller_clone = Arc::clone(&controller);
        tokio::spawn(async move {
            controller_clone.better_auto_loop().await;
        });

        controller
    }

    pub fn get_hp_wmi_hwmon_dir() -> Option<PathBuf> {
        let base = Path::new("/sys/devices/platform/hp-wmi/hwmon");
        if !base.exists() {
            // Fallback to /sys/class/hwmon search for hp-wmi
            if let Ok(entries) = fs::read_dir("/sys/class/hwmon") {
                for entry in entries.flatten() {
                    let p = entry.path();
                    let name_p = p.join("name");
                    if let Ok(name) = fs::read_to_string(name_p) {
                        if name.trim().contains("hp") || name.trim().contains("hp-wmi") {
                            return Some(p);
                        }
                    }
                }
            }
            return None;
        }

        let mut hwmon_dirs = Vec::new();
        if let Ok(entries) = fs::read_dir(base) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() && p.file_name().unwrap_or_default().to_string_lossy().starts_with("hwmon") {
                    hwmon_dirs.push(p);
                }
            }
        }

        hwmon_dirs.sort();
        hwmon_dirs.pop()
    }

    pub fn get_fan_speed(&self, fan_id: u32) -> u32 {
        if let Some(hwmon_dir) = Self::get_hp_wmi_hwmon_dir() {
            let file_name = format!("fan{}_input", fan_id);
            let path = hwmon_dir.join(file_name);
            if let Ok(val_str) = fs::read_to_string(path) {
                if let Ok(val) = val_str.trim().parse::<u32>() {
                    return val;
                }
            }
        }
        0
    }

    pub fn get_fan_max_speed(&self, fan_id: u32) -> u32 {
        if let Some(hwmon_dir) = Self::get_hp_wmi_hwmon_dir() {
            let file_name = format!("fan{}_max", fan_id);
            let path = hwmon_dir.join(file_name);
            if let Ok(val_str) = fs::read_to_string(path) {
                if let Ok(val) = val_str.trim().parse::<u32>() {
                    return val;
                }
            }
        }
        if fan_id == 2 { DEFAULT_MAX_RPM_FAN2 } else { DEFAULT_MAX_RPM_FAN1 }
    }

    pub fn get_mode(&self) -> FanMode {
        *self.mode.lock().unwrap()
    }

    pub fn set_mode(&self, mode: FanMode) -> Result<(), String> {
        let mut current_mode = self.mode.lock().unwrap();
        *current_mode = mode;

        if let Some(hwmon_dir) = Self::get_hp_wmi_hwmon_dir() {
            let pwm_enable_path = hwmon_dir.join("pwm1_enable");
            let mode_val = match mode {
                FanMode::Auto => "2",
                FanMode::BetterAuto => "1",
                FanMode::Manual => "1",
                FanMode::Max => "0",
            };
            if let Err(e) = fs::write(&pwm_enable_path, mode_val) {
                warn!("Failed to write pwm1_enable ({}): {}", pwm_enable_path.display(), e);
            }
        }

        info!("Fan mode set to: {:?}", mode);
        Ok(())
    }

    pub fn set_fan_speed(&self, fan_id: u32, speed: u32) -> Result<(), String> {
        let hwmon_dir = Self::get_hp_wmi_hwmon_dir()
            .ok_or_else(|| "hp-wmi hwmon directory not found".to_string())?;

        let pwm_enable_path = hwmon_dir.join("pwm1_enable");
        let _ = fs::write(&pwm_enable_path, "1");

        let target_file = hwmon_dir.join(format!("fan{}_target", fan_id));
        let fallback_file = hwmon_dir.join("pwm1");

        if target_file.exists() {
            fs::write(&target_file, speed.to_string())
                .map_err(|e| format!("Failed to write fan speed to {}: {}", target_file.display(), e))?;
        } else if fallback_file.exists() {
            let pwm_val = ((speed.saturating_sub(2000) as f64 / 4000.0) * 255.0).clamp(0.0, 255.0) as u32;
            fs::write(&fallback_file, pwm_val.to_string())
                .map_err(|e| format!("Failed to write pwm speed to {}: {}", fallback_file.display(), e))?;
        } else {
            return Err("No valid fan speed control file (fan_target or pwm1) found".to_string());
        }

        info!("Set fan {} target speed to {} RPM", fan_id, speed);
        Ok(())
    }

    async fn better_auto_loop(&self) {
        loop {
            sleep(Duration::from_secs(2)).await;

            let current_mode = self.get_mode();
            if current_mode != FanMode::BetterAuto {
                continue;
            }

            let cpu_temp = self.monitor.get_cpu_temp();
            let gpu_temp = self.monitor.get_gpu_temp();
            let max_temp = cpu_temp.max(gpu_temp);

            let max_rpm_1 = self.get_fan_max_speed(1);
            let max_rpm_2 = self.get_fan_max_speed(2);

            let target_rpm_1 = Self::calculate_auto_rpm(max_temp, max_rpm_1);
            let target_rpm_2 = Self::calculate_auto_rpm(max_temp, max_rpm_2);

            let _ = self.set_fan_speed(1, target_rpm_1);
            let _ = self.set_fan_speed(2, target_rpm_2);
        }
    }

    fn calculate_auto_rpm(temp: f64, max_rpm: u32) -> u32 {
        let min_rpm = DEFAULT_MIN_RPM;
        if temp < 45.0 {
            min_rpm
        } else if temp < 55.0 {
            min_rpm + (max_rpm - min_rpm) * 2 / 10
        } else if temp < 65.0 {
            min_rpm + (max_rpm - min_rpm) * 4 / 10
        } else if temp < 75.0 {
            min_rpm + (max_rpm - min_rpm) * 6 / 10
        } else if temp < 85.0 {
            min_rpm + (max_rpm - min_rpm) * 8 / 10
        } else {
            max_rpm
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_auto_rpm_thresholds() {
        let max_rpm = 6000;
        assert_eq!(FanController::calculate_auto_rpm(35.0, max_rpm), DEFAULT_MIN_RPM);
        assert_eq!(FanController::calculate_auto_rpm(50.0, max_rpm), DEFAULT_MIN_RPM + (max_rpm - DEFAULT_MIN_RPM) * 2 / 10);
        assert_eq!(FanController::calculate_auto_rpm(70.0, max_rpm), DEFAULT_MIN_RPM + (max_rpm - DEFAULT_MIN_RPM) * 6 / 10);
        assert_eq!(FanController::calculate_auto_rpm(90.0, max_rpm), max_rpm);
    }
}
