use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing::{info, warn};

pub struct HwmonMonitor {
    cpu_temp_path: Mutex<Option<PathBuf>>,
    gpu_temp_path: Mutex<Option<PathBuf>>,
}

impl HwmonMonitor {
    pub fn new() -> Self {
        let monitor = Self {
            cpu_temp_path: Mutex::new(None),
            gpu_temp_path: Mutex::new(None),
        };
        monitor.init_sensors();
        monitor
    }

    fn init_sensors(&self) {
        let cpu_path = Self::find_hwmon_sensor(&["k10temp", "coretemp", "zenpower", "cpu", "package", "soc"], &["cpu", "package", "soc"])
            .or_else(|| Self::find_thermal_zone(&["x86_pkg", "tctl", "cpu", "soc"]));
        
        let gpu_path = Self::find_hwmon_sensor(&["amdgpu", "radeon", "nvidia", "gpu"], &["edge", "gpu", "junction", "hotspot"])
            .or_else(|| Self::find_thermal_zone(&["gpu", "amdgpu", "nvidia"]));

        if let Some(ref p) = cpu_path {
            info!("CPU thermal sensor located at: {:?}", p);
        } else {
            warn!("CPU thermal sensor not found in sysfs");
        }

        if let Some(ref p) = gpu_path {
            info!("GPU thermal sensor located at: {:?}", p);
        } else {
            warn!("GPU thermal sensor not found in sysfs");
        }

        *self.cpu_temp_path.lock().unwrap() = cpu_path;
        *self.gpu_temp_path.lock().unwrap() = gpu_path;
    }

    fn find_hwmon_sensor(name_hints: &[&str], label_hints: &[&str]) -> Option<PathBuf> {
        let hwmon_dir = Path::new("/sys/class/hwmon");
        if !hwmon_dir.exists() {
            return None;
        }

        let entries = fs::read_dir(hwmon_dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let name_path = path.join("name");
            let name_val = fs::read_to_string(&name_path).unwrap_or_default().trim().to_lowercase();
            let name_matches = name_hints.iter().any(|&hint| name_val.contains(hint));

            let mut candidates = Vec::new();
            if let Ok(dir_entries) = fs::read_dir(&path) {
                for file_entry in dir_entries.flatten() {
                    let file_name = file_entry.file_name().to_string_lossy().to_string();
                    if file_name.starts_with("temp") && file_name.ends_with("_input") {
                        let input_path = path.join(&file_name);
                        candidates.push(input_path.clone());

                        let prefix = file_name.trim_end_matches("_input");
                        let label_path = path.join(format!("{}_label", prefix));
                        if let Ok(label_val) = fs::read_to_string(label_path) {
                            let lowered_label = label_val.trim().to_lowercase();
                            if label_hints.iter().any(|&hint| lowered_label.contains(hint)) {
                                return Some(input_path);
                            }
                        }
                    }
                }
            }

            if name_matches && !candidates.is_empty() {
                return Some(candidates[0].clone());
            }
        }
        None
    }

    fn find_thermal_zone(type_hints: &[&str]) -> Option<PathBuf> {
        let thermal_dir = Path::new("/sys/class/thermal");
        if !thermal_dir.exists() {
            return None;
        }

        let entries = fs::read_dir(thermal_dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !file_name.starts_with("thermal_zone") {
                continue;
            }

            let type_path = path.join("type");
            if let Ok(sensor_type) = fs::read_to_string(type_path) {
                let lowered = sensor_type.trim().to_lowercase();
                if type_hints.iter().any(|&hint| lowered.contains(hint)) {
                    let temp_path = path.join("temp");
                    if temp_path.exists() {
                        return Some(temp_path);
                    }
                }
            }
        }
        None
    }

    pub fn get_cpu_temp(&self) -> f64 {
        let lock = self.cpu_temp_path.lock().unwrap();
        if let Some(ref path) = *lock {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(val) = content.trim().parse::<f64>() {
                    return if val > 1000.0 { val / 1000.0 } else { val };
                }
            }
        }
        0.0
    }

    pub fn get_gpu_temp(&self) -> f64 {
        let lock = self.gpu_temp_path.lock().unwrap();
        if let Some(ref path) = *lock {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(val) = content.trim().parse::<f64>() {
                    return if val > 1000.0 { val / 1000.0 } else { val };
                }
            }
        }
        0.0
    }
}
