mod setup;
mod integration;
mod app;
mod websocket;

use std::fs::File;
use std::net::SocketAddr;

use color_eyre::eyre::{Result, eyre};
use log::LevelFilter;
use setup::config;
use app::App;
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

    CombinedLogger::init(
        vec![
        TermLogger::new(LevelFilter::Debug, log_config.clone(), TerminalMode::Mixed, ColorChoice::Auto),
        WriteLogger::new(LevelFilter::Debug, log_config, File::create("can-receiver.log")?),
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