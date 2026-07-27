mod dbus_service;
mod fan;
mod hwmon;

use std::sync::Arc;
use anyhow::Result;
use tracing::info;
use zbus::connection::Builder;
use victus_common::{DESTINATION, PATH};

use dbus_service::VictusControlService;
use fan::FanController;
use hwmon::HwmonMonitor;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("Starting Victus Control System Daemon (Rust)...");

    let hwmon = Arc::new(HwmonMonitor::new());
    let fan = FanController::new(Arc::clone(&hwmon));

    let dbus_service = VictusControlService::new(hwmon, fan);

    info!("Registering D-Bus service name: {}", DESTINATION);
    let _conn = Builder::system()?
        .name(DESTINATION)?
        .serve_at(PATH, dbus_service)?
        .build()
        .await?;

    info!("Victus Control D-Bus service is running.");
    tokio::signal::ctrl_c().await?;
    info!("Shutting down Victus Control System Daemon.");

    Ok(())
}
