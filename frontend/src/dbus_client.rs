use zbus::proxy;

#[proxy(
    default_service = "org.hp.VictusControl",
    interface = "org.hp.VictusControl",
    default_path = "/org/hp/VictusControl"
)]
pub trait VictusControl {
    fn get_cpu_temp(&self) -> zbus::Result<f64>;
    fn get_gpu_temp(&self) -> zbus::Result<f64>;
    fn get_fan_speed(&self, fan_id: u32) -> zbus::Result<i32>;

    fn get_fan_max_speed(&self, fan_id: u32) -> zbus::Result<u32>;
    fn set_fan_speed(&self, fan_id: u32, speed: u32) -> zbus::Result<String>;
    fn get_fan_mode(&self) -> zbus::Result<String>;
    fn set_fan_mode(&self, mode: String) -> zbus::Result<String>;
}
