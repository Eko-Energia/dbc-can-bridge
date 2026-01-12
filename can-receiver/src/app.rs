use std::time::Duration;

use color_eyre::eyre::Result;
use time::OffsetDateTime;
use embedded_can::blocking::Can;
use waveshare_usb_can_a::sync::Usb2Can;
use waveshare_usb_can_a::{self as ws};
use crate::setup::config;

use crate::integration::dbc_handler::DbcHandler;
use crate::websocket::{CanUpdate, SignalData};
use tokio::sync::mpsc;

pub struct App {
    dbc_handler: DbcHandler,
    device: Usb2Can,
    ws_tx: Option<mpsc::UnboundedSender<CanUpdate>>,
}

impl App {
    pub fn new() -> Result<Self> {
        // Initialize DBC decoding
        let dbc_handler = DbcHandler::new()?;

        info!("DBC loaded: {} message definitions available", dbc_handler.dbc.messages.len());
        
        // Get settings from configuration
        let device_port = config::get_device_port()?;
        let can_baud_rate = config::get_can_baud_rate()?;
        
        info!("Using device: {}", device_port);
        info!("CAN speed: {:?}", can_baud_rate);

        // CAN configuration
        let ws_config = ws::Usb2CanConfiguration::new(can_baud_rate)
            .set_loopback(false)
            .set_silent(false);  // Enable receiving frames from the bus

        // Initialize connection
        let device = ws::sync::new(&device_port, &ws_config)
            // not really infinite timeout
            .set_serial_receive_timeout(Duration::from_secs(60 * 60 * 24))
            .open()?;

        Ok(Self {
            dbc_handler,
            device,
            ws_tx: None,
        })
    }

    /// Sets the sender for WebSocket updates
    pub fn set_websocket_sender(&mut self, tx: mpsc::UnboundedSender<CanUpdate>) {
        self.ws_tx = Some(tx);
    }

    pub fn run(&mut self) -> Result<()> {
        info!("Starting to receive CAN frames... (Press Ctrl+C to stop)");
        loop {
            match self.device.receive() {
                Ok(frame) => {
                    match self.dbc_handler.decode(frame) {
                        Ok((msg_name, signals)) => {
                            let timestamp = OffsetDateTime::now_local()?;
                            
                            // Send update to WebSocket if connected
                            if let Some(ref tx) = self.ws_tx {
                                let update = CanUpdate {
                                    message_name: msg_name.to_string(),
                                    signals: signals
                                        .iter()
                                        .map(|s| SignalData {
                                            name: s.name.to_string(),
                                            value: s.value,
                                            unit: s.unit.to_string(),
                                        })
                                        .collect(),
                                    timestamp,
                                };
                                
                                // Send without blocking - skip if channel is full
                                let _ = tx.send(update);
                            }
                        }
                        
                        Err(e) => {
                            error!("Error decoding frame: {}", e);
                        }
                    }
                }
                
                Err(ws::sync::Error::SerialReadTimedOut) => {
                    warn!("Timeout - no frame received, continuing...");
                    continue;
                }
                
                Err(e) => {
                    error!("Error receiving frame: {}", e);
                }
            }
        }
    }
}