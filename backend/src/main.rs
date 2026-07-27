mod dbus_service;
mod fan;
mod hwmon;

use anyhow::Result;
use dbus_service::VictusControlService;
use fan::FanController;
use hwmon::HwmonMonitor;
use std::sync::Arc;
use tokio::signal::unix::{signal, SignalKind};
use tracing::{info, warn};
use victus_common::{DESTINATION, PATH};
use zbus::connection::Builder;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("Starting Victus Control System Daemon (Rust)...");

    let hwmon = Arc::new(HwmonMonitor::new());
    let fan = FanController::new(Arc::clone(&hwmon));

    let dbus_service = VictusControlService::new(Arc::clone(&hwmon), Arc::clone(&fan));

    info!("Registering D-Bus service name: {}", DESTINATION);
    let _conn = Builder::system()?
        .name(DESTINATION)?
        .serve_at(PATH, dbus_service)?
        .build()
        .await?;

    info!("Victus Control D-Bus service is running.");

    // Listen for SIGINT (Ctrl+C) and SIGTERM (systemctl stop)
    let mut sigterm = signal(SignalKind::terminate())?;
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received SIGINT signal.");
        }
        _ = sigterm.recv() => {
            info!("Received SIGTERM signal.");
        }
    }

    info!("Shutting down Victus Control System Daemon.");
    if let Err(e) = fan.shutdown().await {
        warn!("Failed to reset fan mode to AUTO on shutdown: {}", e);
    }

    Ok(())
}
