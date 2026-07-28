use crate::hwmon::HwmonMonitor;
use parking_lot::Mutex;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, warn};
use victus_common::FanMode;

const DEFAULT_MIN_RPM: u32 = 2000;
const DEFAULT_MAX_RPM_FAN1: u32 = 5800;
const DEFAULT_MAX_RPM_FAN2: u32 = 6100;
const HYSTERESIS_DEADBAND: f64 = 2.0;

pub struct FanController {
    mode: Mutex<FanMode>,
    monitor: Arc<HwmonMonitor>,
    cached_hwmon_dir: Option<PathBuf>,
    cached_max_rpm_1: u32,
    cached_max_rpm_2: u32,
    last_written_speed_1: Mutex<Option<u32>>,
    last_written_speed_2: Mutex<Option<u32>>,
    last_effective_temp: Mutex<f64>,
    failed_temp_reads: Mutex<u32>,
}

impl FanController {
    pub fn new(monitor: Arc<HwmonMonitor>) -> Arc<Self> {
        let hwmon_dir = Self::find_hp_wmi_hwmon_dir();
        let max_rpm_1 =
            Self::read_sysfs_max_speed(hwmon_dir.as_deref(), 1).unwrap_or(DEFAULT_MAX_RPM_FAN1);
        let max_rpm_2 =
            Self::read_sysfs_max_speed(hwmon_dir.as_deref(), 2).unwrap_or(DEFAULT_MAX_RPM_FAN2);

        let controller = Arc::new(Self {
            mode: Mutex::new(FanMode::BetterAuto),
            monitor,
            cached_hwmon_dir: hwmon_dir,
            cached_max_rpm_1: max_rpm_1,
            cached_max_rpm_2: max_rpm_2,
            last_written_speed_1: Mutex::new(None),
            last_written_speed_2: Mutex::new(None),
            last_effective_temp: Mutex::new(45.0),
            failed_temp_reads: Mutex::new(0),
        });

        // Spawn background task for mode initialization and BETTER_AUTO regulation
        let controller_clone = Arc::clone(&controller);
        tokio::spawn(async move {
            let _ = controller_clone.set_mode(FanMode::BetterAuto).await;
            controller_clone.better_auto_loop().await;
        });

        controller
    }

    fn find_hp_wmi_hwmon_dir() -> Option<PathBuf> {
        let base = Path::new("/sys/devices/platform/hp-wmi/hwmon");
        if !base.exists() {
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
                if p.is_dir()
                    && p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .starts_with("hwmon")
                {
                    hwmon_dirs.push(p);
                }
            }
        }

        // Sort numerically by hwmon index (e.g. hwmon2 before hwmon10)
        hwmon_dirs.sort_by_key(|path| {
            path.file_name()
                .and_then(|s| s.to_str())
                .and_then(|s| s.strip_prefix("hwmon"))
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0)
        });

        hwmon_dirs.pop()
    }

    fn read_sysfs_max_speed(hwmon_dir: Option<&Path>, fan_id: u32) -> Option<u32> {
        let dir = hwmon_dir?;
        let path = dir.join(format!("fan{}_max", fan_id));
        if let Ok(val_str) = fs::read_to_string(path) {
            if let Ok(val) = val_str.trim().parse::<u32>() {
                return Some(val);
            }
        }
        None
    }

    pub fn get_fan_speed(&self, fan_id: u32) -> Option<u32> {
        if let Some(ref hwmon_dir) = self.cached_hwmon_dir {
            let path = hwmon_dir.join(format!("fan{}_input", fan_id));
            if let Ok(val_str) = fs::read_to_string(path) {
                if let Ok(val) = val_str.trim().parse::<u32>() {
                    return Some(val);
                }
            }
        }
        None
    }

    pub fn get_fan_max_speed(&self, fan_id: u32) -> u32 {
        if fan_id == 2 {
            self.cached_max_rpm_2
        } else {
            self.cached_max_rpm_1
        }
    }

    pub fn get_mode(&self) -> FanMode {
        *self.mode.lock()
    }

    pub async fn set_mode(&self, mode: FanMode) -> Result<(), String> {
        let prev_mode = {
            let mut current_mode = self.mode.lock();
            let prev = *current_mode;
            *current_mode = mode;
            prev
        };

        // Reset last written speeds on mode change to force fresh hardware write
        *self.last_written_speed_1.lock() = None;
        *self.last_written_speed_2.lock() = None;

        if let Some(ref hwmon_dir) = self.cached_hwmon_dir {
            let pwm_enable_path = hwmon_dir.join("pwm1_enable");

            // If coming out of MAX mode, write "2" (AUTO) first to trigger hardware max reset
            if prev_mode == FanMode::Max && (mode == FanMode::BetterAuto || mode == FanMode::Manual)
            {
                if let Err(e) = fs::write(&pwm_enable_path, "2") {
                    return Err(format!(
                        "Failed to write AUTO mode to {}: {}",
                        pwm_enable_path.display(),
                        e
                    ));
                }
                sleep(Duration::from_millis(100)).await;
            }

            let mode_val = match mode {
                FanMode::Auto => "2",
                FanMode::BetterAuto => "1",
                FanMode::Manual => "1",
                FanMode::Max => "0",
            };
            if let Err(e) = fs::write(&pwm_enable_path, mode_val) {
                let err_msg = format!(
                    "Failed to write pwm1_enable ({}): {}",
                    pwm_enable_path.display(),
                    e
                );
                warn!("{}", err_msg);
                return Err(err_msg);
            }
        }

        info!("Fan mode set to: {:?}", mode);
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        info!("Shutting down FanController: resetting fan control to firmware AUTO");
        self.set_mode(FanMode::Auto).await
    }

    pub fn set_fan_speed(&self, fan_id: u32, speed: u32) -> Result<(), String> {
        if fan_id != 1 && fan_id != 2 {
            return Err(format!("Invalid fan_id: {}. Must be 1 or 2", fan_id));
        }

        let hwmon_dir = self
            .cached_hwmon_dir
            .as_ref()
            .ok_or_else(|| "hp-wmi hwmon directory not found".to_string())?;

        // Write Deduplication: Skip if speed value has not changed
        let last_speed_mutex = if fan_id == 2 {
            &self.last_written_speed_2
        } else {
            &self.last_written_speed_1
        };
        let mut last_speed_guard = last_speed_mutex.lock();
        if Some(speed) == *last_speed_guard {
            return Ok(());
        }

        let target_file = hwmon_dir.join(format!("fan{}_target", fan_id));
        let fallback_file = hwmon_dir.join("pwm1");

        if target_file.exists() {
            fs::write(&target_file, speed.to_string()).map_err(|e| {
                format!(
                    "Failed to write fan speed to {}: {}",
                    target_file.display(),
                    e
                )
            })?;
        } else if fallback_file.exists() {
            // Map 2000 RPM -> PWM 55 (active floor prevents motor stall), 5800+ RPM -> PWM 255
            let pwm_val = if speed <= 2000 {
                55
            } else {
                let ratio = ((speed.saturating_sub(2000) as f64) / 3800.0).clamp(0.0, 1.0);
                (55.0 + ratio * 200.0) as u32
            };
            fs::write(&fallback_file, pwm_val.to_string()).map_err(|e| {
                format!(
                    "Failed to write pwm speed to {}: {}",
                    fallback_file.display(),
                    e
                )
            })?;
        } else {
            return Err("No valid fan speed control file (fan_target or pwm1) found".to_string());
        }

        *last_speed_guard = Some(speed);
        debug!("Set fan {} target speed to {} RPM", fan_id, speed);
        Ok(())
    }

    async fn better_auto_loop(&self) {
        loop {
            sleep(Duration::from_secs(2)).await;

            if self.get_mode() != FanMode::BetterAuto {
                continue;
            }

            let cpu_opt = self.monitor.get_cpu_temp();
            let gpu_opt = self.monitor.get_gpu_temp();

            let raw_max_temp = match (cpu_opt, gpu_opt) {
                (Some(c), Some(g)) => {
                    *self.failed_temp_reads.lock() = 0;
                    c.max(g)
                }
                (Some(c), None) => {
                    *self.failed_temp_reads.lock() = 0;
                    c
                }
                (None, Some(g)) => {
                    *self.failed_temp_reads.lock() = 0;
                    g
                }
                (None, None) => {
                    let mut count = self.failed_temp_reads.lock();
                    *count += 1;
                    if *count >= 3 {
                        warn!("3 consecutive thermal sensor read failures detected; re-checking hwmon sensor paths");
                        self.monitor.recheck_sensors();
                    }
                    *self.last_effective_temp.lock()
                }
            };

            // Thermal Hysteresis Deadband Processing
            let mut last_temp_guard = self.last_effective_temp.lock();
            let effective_temp = if raw_max_temp > *last_temp_guard {
                // Temp rising: update immediately
                *last_temp_guard = raw_max_temp;
                raw_max_temp
            } else if raw_max_temp <= *last_temp_guard - HYSTERESIS_DEADBAND {
                // Temp falling beyond deadband margin: update
                *last_temp_guard = raw_max_temp;
                raw_max_temp
            } else {
                // Temp dipping slightly within deadband: maintain previous effective temp
                *last_temp_guard
            };

            let max_rpm_1 = self.cached_max_rpm_1;
            let max_rpm_2 = self.cached_max_rpm_2;

            let target_rpm_1 = Self::calculate_auto_rpm_smooth(effective_temp, max_rpm_1);
            let target_rpm_2 = Self::calculate_auto_rpm_smooth(effective_temp, max_rpm_2);

            // Ramp both fans using maximum required target speed under heavy thermal loads
            let max_target_rpm_1 = target_rpm_1
                .max((target_rpm_2 as f64 * (max_rpm_1 as f64 / max_rpm_2.max(1) as f64)) as u32);
            let max_target_rpm_2 = target_rpm_2
                .max((target_rpm_1 as f64 * (max_rpm_2 as f64 / max_rpm_1.max(1) as f64)) as u32);

            let _ = self.set_fan_speed(1, max_target_rpm_1);
            let _ = self.set_fan_speed(2, max_target_rpm_2);
        }
    }

    fn calculate_auto_rpm_smooth(temp: f64, max_rpm: u32) -> u32 {
        let min_rpm = DEFAULT_MIN_RPM;
        let raw_rpm = if temp < 45.0 {
            min_rpm
        } else if temp >= 85.0 {
            max_rpm
        } else {
            let ratio = (temp - 45.0) / (85.0 - 45.0);
            min_rpm + ((max_rpm - min_rpm) as f64 * ratio) as u32
        };

        // Quantize RPM to nearest 50 RPM step to eliminate noise
        ((raw_rpm + 25) / 50) * 50
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_auto_rpm_smooth() {
        let max_rpm = 6000;
        assert_eq!(
            FanController::calculate_auto_rpm_smooth(35.0, max_rpm),
            2000
        );
        assert_eq!(
            FanController::calculate_auto_rpm_smooth(45.0, max_rpm),
            2000
        );
        assert_eq!(
            FanController::calculate_auto_rpm_smooth(65.0, max_rpm),
            4000
        );
        assert_eq!(
            FanController::calculate_auto_rpm_smooth(85.0, max_rpm),
            6000
        );
        assert_eq!(
            FanController::calculate_auto_rpm_smooth(95.0, max_rpm),
            6000
        );
    }

    #[test]
    fn test_hwmon_dir_numerical_sorting() {
        let mut dirs = [
            PathBuf::from("/sys/devices/platform/hp-wmi/hwmon/hwmon10"),
            PathBuf::from("/sys/devices/platform/hp-wmi/hwmon/hwmon2"),
            PathBuf::from("/sys/devices/platform/hp-wmi/hwmon/hwmon1"),
        ];

        dirs.sort_by_key(|path| {
            path.file_name()
                .and_then(|s| s.to_str())
                .and_then(|s| s.strip_prefix("hwmon"))
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0)
        });

        assert_eq!(
            dirs[0],
            PathBuf::from("/sys/devices/platform/hp-wmi/hwmon/hwmon1")
        );
        assert_eq!(
            dirs[1],
            PathBuf::from("/sys/devices/platform/hp-wmi/hwmon/hwmon2")
        );
        assert_eq!(
            dirs[2],
            PathBuf::from("/sys/devices/platform/hp-wmi/hwmon/hwmon10")
        );
    }
}
