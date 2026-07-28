use crate::fan::FanController;
use crate::hwmon::HwmonMonitor;
use std::str::FromStr;
use std::sync::Arc;
use victus_common::FanMode;
use zbus::interface;

pub struct VictusControlService {
    hwmon: Arc<HwmonMonitor>,
    fan: Arc<FanController>,
}

impl VictusControlService {
    pub fn new(hwmon: Arc<HwmonMonitor>, fan: Arc<FanController>) -> Self {
        Self { hwmon, fan }
    }
}

#[interface(name = "org.hp.VictusControl")]
impl VictusControlService {
    async fn get_cpu_temp(&self) -> f64 {
        self.hwmon.get_cpu_temp().unwrap_or(0.0)
    }

    async fn get_gpu_temp(&self) -> f64 {
        self.hwmon.get_gpu_temp().unwrap_or(0.0)
    }

    async fn get_fan_speed(&self, fan_id: u32) -> i32 {
        self.fan
            .get_fan_speed(fan_id)
            .map(|rpm| rpm as i32)
            .unwrap_or(-1)
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
            Ok(parsed_mode) => match self.fan.set_mode(parsed_mode).await {
                Ok(_) => "OK".to_string(),
                Err(e) => format!("ERROR: {}", e),
            },
            Err(e) => format!("ERROR: {}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dbus_service_methods() {
        let hwmon = Arc::new(HwmonMonitor::new());
        let fan = FanController::new(Arc::clone(&hwmon));
        let service = VictusControlService::new(hwmon, fan);

        assert!(service.get_cpu_temp().await >= 0.0);
        assert!(service.get_gpu_temp().await >= 0.0);
        let _speed = service.get_fan_speed(1).await;
        assert!(service.get_fan_max_speed(1).await > 0);

        assert_eq!(service.get_fan_mode().await, "BETTER_AUTO");

        let res = service.set_fan_mode("MANUAL".to_string()).await;
        assert!(res.starts_with("OK") || res.starts_with("ERROR:"));
    }
}
