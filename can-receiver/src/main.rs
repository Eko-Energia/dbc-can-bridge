mod setup;
mod integration;
mod websocket;
#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
mod app_waveshare;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
mod app_socketcan;

#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
use app_waveshare::App;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use app_socketcan::App;

use std::fs::{File, create_dir_all};
use std::net::SocketAddr;
use time::{OffsetDateTime, format_description::parse};

use color_eyre::eyre::{Result, eyre};
use log::LevelFilter;
use setup::config;
use websocket::WebSocketServer;
use simplelog::{ColorChoice, CombinedLogger, ConfigBuilder, TermLogger, TerminalMode, WriteLogger};

extern crate simplelog;
#[macro_use] extern crate log;

fn main() -> Result<()> {
    color_eyre::install()?;

    let log_config = ConfigBuilder::new()
        .set_time_offset_to_local()
        .map_err(|_| eyre!("Failed to get local time offset"))?
        .build();

    create_dir_all("logs")?;
    let fmt = parse("[year]-[month]-[day]_[hour]-[minute]-[second]")?;
    let ts = OffsetDateTime::now_local()?.format(&fmt)?;
    let log_filename = format!("logs/can-receiver-{}.log", ts);

    CombinedLogger::init(
        vec![
        TermLogger::new(LevelFilter::Debug, log_config.clone(), TerminalMode::Mixed, ColorChoice::Auto),
        WriteLogger::new(LevelFilter::Debug, log_config, File::create(log_filename)?),
    ]
    )?;

    let result = (|| -> Result<_> {
        // Initialize configuration
        config::init_config()?;
        
        // Create WebSocket server
        let ws_server = WebSocketServer::new();
        let ws_tx = ws_server.get_update_sender();
        
        // Start WebSocket server in background
        let ws_addr: SocketAddr = "0.0.0.0:8080".parse()?;
        info!("Starting WebSocket server on {}", ws_addr);
        
        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    error!("Failed to create tokio runtime for WebSocket server: {}", e);
                    return;
                }
            };
            runtime.block_on(async {
                if let Err(e) = ws_server.run(ws_addr).await {
                    error!("WebSocket server error: {:?}", e);
                }
            });
        });
        
        // Initialize and run CAN receiver app
        let mut app = App::new()?;
        app.set_websocket_sender(ws_tx);
        app.run()
    })();

    match result {
        Err(e) => {
            error!("App error: {:?}", e);
            // fix for double error messages
            Ok(())
        }
        Ok(s) => Ok(s)
    }
}