use std::str::FromStr;
use std::sync::Arc;
use zbus::interface;
use victus_common::FanMode;
use crate::fan::FanController;
use crate::hwmon::HwmonMonitor;

pub struct VictusControlService {
    hwmon: Arc<HwmonMonitor>,
    fan: Arc<FanController>,
}

impl VictusControlService {
    pub fn new(
        hwmon: Arc<HwmonMonitor>,
        fan: Arc<FanController>,
    ) -> Self {
        Self {
            hwmon,
            fan,
        }
    }
}

#[interface(name = "org.hp.VictusControl")]
impl VictusControlService {
    async fn get_cpu_temp(&self) -> f64 {
        self.hwmon.get_cpu_temp()
    }

    async fn get_gpu_temp(&self) -> f64 {
        self.hwmon.get_gpu_temp()
    }

    async fn get_fan_speed(&self, fan_id: u32) -> u32 {
        self.fan.get_fan_speed(fan_id)
    }

    async fn get_fan_max_speed(&self, fan_id: u32) -> u32 {
        self.fan.get_fan_max_speed(fan_id)
    }

    async fn set_fan_speed(&self, fan_id: u32, speed: u32) -> String {
        match self.fan.set_fan_speed(fan_id, speed) {
            Ok(_) => "OK".to_string(),
            Err(e) => format!("ERROR: {}", e),
        }
    }

    async fn get_fan_mode(&self) -> String {
        self.fan.get_mode().to_string()
    }

    async fn set_fan_mode(&self, mode: String) -> String {
        match FanMode::from_str(&mode) {
            Ok(parsed_mode) => match self.fan.set_mode(parsed_mode) {
                Ok(_) => "OK".to_string(),
                Err(e) => format!("ERROR: {}", e),
            },
            Err(e) => format!("ERROR: {}", e),
        }
    }
}
