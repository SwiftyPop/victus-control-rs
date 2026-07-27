mod alert;

use std::time::{Duration, Instant};
use anyhow::Result;
use tracing::{info, warn};
use zbus::proxy;

use alert::{Category, SustainedAlert};

pub const CPU_HOT_C: f64 = 83.0;
pub const GPU_HOT_C: f64 = 83.0;
pub const FAN_IDLE_RPM: u32 = 2200;
pub const POLL_INTERVAL: Duration = Duration::from_secs(3);
pub const APP_NAME: &str = "Victus Control";

#[proxy(
    default_service = "org.hp.VictusControl",
    interface = "org.hp.VictusControl",
    default_path = "/org/hp/VictusControl"
)]
pub trait VictusControl {
    fn get_cpu_temp(&self) -> zbus::Result<f64>;
    fn get_gpu_temp(&self) -> zbus::Result<f64>;
    fn get_fan_speed(&self, fan_id: u32) -> zbus::Result<u32>;
    fn get_fan_max_speed(&self, fan_id: u32) -> zbus::Result<u32>;
    fn get_fan_mode(&self) -> zbus::Result<String>;
}

#[proxy(
    default_service = "org.freedesktop.Notifications",
    interface = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
pub trait DesktopNotifications {
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: std::collections::HashMap<&str, zbus::zvariant::Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;
}

async fn send_notification(
    session_conn: &Option<zbus::Connection>,
    summary: &str,
    body: &str,
    critical: bool,
) {
    let urgency_str = if critical { "critical" } else { "normal" };

    if let Some(conn) = session_conn {
        if let Ok(proxy) = DesktopNotificationsProxy::builder(conn).build().await {
            let mut hints = std::collections::HashMap::new();
            let urgency_byte: u8 = if critical { 2 } else { 1 };
            hints.insert("urgency", zbus::zvariant::Value::U8(urgency_byte));

            if proxy
                .notify(
                    APP_NAME,
                    0,
                    "sensors-temperature-symbolic",
                    summary,
                    body,
                    &[],
                    hints,
                    -1,
                )
                .await
                .is_ok()
            {
                return;
            }
        }
    }

    // Fallback to notify-send CLI if D-Bus session notification proxy fails
    let _ = std::process::Command::new("notify-send")
        .args([
            "-a",
            APP_NAME,
            "-u",
            urgency_str,
            "-i",
            "sensors-temperature-symbolic",
            summary,
            body,
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("Starting Victus Control Temperature Monitor (Rust)...");

    let system_conn = match zbus::Connection::system().await {
        Ok(conn) => conn,
        Err(e) => {
            warn!("Failed to connect to system D-Bus: {}. Will retry...", e);
            tokio::time::sleep(POLL_INTERVAL).await;
            zbus::Connection::system().await?
        }
    };

    let session_conn = zbus::Connection::session().await.ok();

    let proxy = VictusControlProxy::new(&system_conn).await?;

    let start_instant = Instant::now();
    let get_now = move || start_instant.elapsed().as_secs_f64();

    let mut cooling = Category::new(CPU_HOT_C.min(GPU_HOT_C));
    let mut cpu_85 = SustainedAlert::new(85.0, 12.0);
    let mut cpu_90 = SustainedAlert::new(90.0, 9.0);
    let mut cpu_95 = SustainedAlert::new(95.0, 6.0);
    let mut cpu_100 = SustainedAlert::new(100.0, 3.0);
    let mut gpu_80 = SustainedAlert::new(80.0, 12.0);
    let mut gpu_85 = SustainedAlert::new(85.0, 9.0);

    info!("victus-monitor: watching temperatures via system D-Bus");

    loop {
        let now = get_now();

        let cpu = proxy.get_cpu_temp().await.ok();
        let gpu = proxy.get_gpu_temp().await.ok();
        let mode = proxy.get_fan_mode().await.ok();
        let fan1 = proxy.get_fan_speed(1).await.ok();
        let fan2 = proxy.get_fan_speed(2).await.ok();

        if let Some(c) = cpu {
            if cpu_85.update(Some(c), now) {
                send_notification(
                    &session_conn,
                    "CPU running hot",
                    &format!("CPU above 85 °C for 12 s (now {:.0} °C). Check active workloads.", c),
                    false,
                )
                .await;
            }

            if cpu_90.update(Some(c), now) {
                send_notification(
                    &session_conn,
                    "CPU very hot",
                    &format!("CPU above 90 °C for 9 s (now {:.0} °C). Close heavy workloads.", c),
                    false,
                )
                .await;
            }

            if cpu_95.update(Some(c), now) {
                send_notification(
                    &session_conn,
                    "CPU critical temperature",
                    &format!("CPU above 95 °C for 6 s (now {:.0} °C). Close heavy workloads immediately.", c),
                    true,
                )
                .await;
            }

            if cpu_100.update(Some(c), now) {
                send_notification(
                    &session_conn,
                    "CPU dangerously hot",
                    &format!("CPU above 100 °C for 3 s (now {:.0} °C). System may throttle or shut down.", c),
                    true,
                )
                .await;
            }
        }

        if let Some(g) = gpu {
            if gpu_80.update(Some(g), now) {
                send_notification(
                    &session_conn,
                    "GPU running hot",
                    &format!("GPU above 80 °C for 12 s (now {:.0} °C). Check active workloads.", g),
                    false,
                )
                .await;
            }

            if gpu_85.update(Some(g), now) {
                send_notification(
                    &session_conn,
                    "GPU critical temperature",
                    &format!("GPU above 85 °C for 9 s (now {:.0} °C). Close heavy workloads immediately.", g),
                    true,
                )
                .await;
            }
        }

        let hottest = match (cpu, gpu) {
            (Some(c), Some(g)) => Some(c.max(g)),
            (Some(c), None) => Some(c),
            (None, Some(g)) => Some(g),
            (None, None) => None,
        };

        let fans_idle = match (fan1, fan2) {
            (Some(f1), Some(f2)) => f1 < FAN_IDLE_RPM && f2 < FAN_IDLE_RPM,
            _ => false,
        };

        let is_manual = mode.as_deref() == Some("MANUAL");
        let cooling_not_engaged = fans_idle || is_manual;

        if let Some(h) = hottest {
            if cooling_not_engaged && cooling.should_fire(Some(h), now) {
                let reason = if is_manual {
                    "fan mode is MANUAL"
                } else {
                    "the fans are barely spinning"
                };
                let comp_name = if cpu.map_or(false, |c| (c - h).abs() < 0.1) {
                    "CPU"
                } else {
                    "GPU"
                };

                send_notification(
                    &session_conn,
                    "Cooling may not keep up",
                    &format!(
                        "{} is at {:.0} °C but {}. Consider Better Auto or a higher fan speed.",
                        comp_name, h, reason
                    ),
                    false,
                )
                .await;
            }
        }

        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
