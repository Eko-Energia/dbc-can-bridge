use std::sync::Arc;

use color_eyre::eyre::{Result, eyre};
use embedded_can::{ExtendedId, Id, StandardId};
use socketcan::{CanFrame, EmbeddedFrame};
use socketcan::tokio::CanSocket;
use time::OffsetDateTime;
use tokio::sync::mpsc;

use crate::integration::dbc_handler::DbcHandler;
use crate::setup::config;
use crate::websocket::{CanTransmitRequest, CanUpdate, SignalData};

pub struct App {
    dbc_handler: Arc<DbcHandler>,
    interface_name: String,
    ws_tx: Option<mpsc::UnboundedSender<CanUpdate>>,
    ws_rx: Option<mpsc::UnboundedReceiver<CanTransmitRequest>>,
}

impl App {
    pub fn new() -> Result<Self> {
        // Initialize DBC decoding
        let dbc_handler = Arc::new(DbcHandler::new()?);

        info!("DBC loaded: {} message definitions available", dbc_handler.dbc.messages.len());
        
        // Get settings from configuration
        let interface_name = config::get_device_port()?;
        
        info!("Using socketcan interface: {}", interface_name);

        Ok(Self {
            dbc_handler,
            interface_name,
            ws_tx: None,
            ws_rx: None,
        })
    }

    /// Sets the sender for WebSocket updates
    pub fn set_websocket_sender(&mut self, tx: mpsc::UnboundedSender<CanUpdate>) {
        self.ws_tx = Some(tx);
    }

    pub fn set_websocket_receiver(&mut self, rx: mpsc::UnboundedReceiver<CanTransmitRequest>) {
        self.ws_rx = Some(rx);
    }

    pub fn run(&mut self) -> Result<()> {
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(self.run_async())
    }

    async fn run_async(&mut self) -> Result<()> {
        info!("Starting socketcan async receiver/transmitter... (Press Ctrl+C to stop)");

        let read_socket = CanSocket::open(&self.interface_name)?;
        let write_socket = CanSocket::open(&self.interface_name)?;

        let dbc_handler = Arc::clone(&self.dbc_handler);
        let ws_tx = self.ws_tx.clone();

        let rx_task: tokio::task::JoinHandle<Result<()>> = tokio::spawn(async move {
            loop {
                match read_socket.read_frame().await {
                    Ok(frame) => match dbc_handler.decode(frame) {
                        Ok((msg_name, signals)) => {
                            let timestamp = OffsetDateTime::now_local()?;

                            if let Some(ref tx) = ws_tx {
                                let update = CanUpdate {
                                    message_name: msg_name.to_string(),
                                    signals:
                                        signals
                                        .iter()
                                        .map(|s| SignalData {
                                            name: s.name.to_string(),
                                            value: s.value,
                                            unit: s.unit.to_string(),
                                        })
                                        .collect(),
                                    timestamp,
                                };

                                let _ = tx.send(update);
                            }
                        }
                        Err(e) => {
                            error!("Error decoding frame: {}", e);
                        }
                    },
                    Err(e) => {
                        return Err(eyre!("SocketCAN read error: {}", e));
                    }
                }
            }
        });

        if let Some(mut rx) = self.ws_rx.take() {
            let tx_task: tokio::task::JoinHandle<Result<()>> = tokio::spawn(async move {
                while let Some(request) = rx.recv().await {
                    let frame = build_frame_from_request(&request)?;
                    write_socket
                        .write_frame(frame)
                        .await
                        .map_err(|e| eyre!("SocketCAN write error: {}", e))?;
                }
                Ok(())
            });

            let (rx_result, tx_result) = tokio::join!(rx_task, tx_task);

            if let Err(e) = rx_result {
                return Err(eyre!("Receiver task failed: {}", e));
            }
            if let Err(e) = tx_result {
                return Err(eyre!("Transmitter task failed: {}", e));
            }

            return Ok(());
        }

        match rx_task.await {
            Ok(result) => result,
            Err(e) => Err(eyre!("Receiver task join error: {}", e)),
        }
    }
}

fn build_frame_from_request(request: &CanTransmitRequest) -> Result<CanFrame> {
    if request.data.len() > 8 {
        return Err(eyre!(
            "Invalid CAN payload length: {} (max 8)",
            request.data.len()
        ));
    }

    let id = match request.is_extended {
        Some(true) => Id::Extended(
            ExtendedId::new(request.message_id)
                .ok_or_else(|| eyre!("Invalid extended CAN id: {}", request.message_id))?,
        ),
        Some(false) => Id::Standard(
            StandardId::new(request.message_id as u16)
                .ok_or_else(|| eyre!("Invalid standard CAN id: {}", request.message_id))?,
        ),
        None => {
            if request.message_id <= 0x7FF {
                Id::Standard(
                    StandardId::new(request.message_id as u16)
                        .ok_or_else(|| eyre!("Invalid standard CAN id: {}", request.message_id))?,
                )
            } else {
                Id::Extended(
                    ExtendedId::new(request.message_id)
                        .ok_or_else(|| eyre!("Invalid extended CAN id: {}", request.message_id))?,
                )
            }
        }
    };

    CanFrame::new(id, &request.data)
        .ok_or_else(|| eyre!("Failed to construct CAN frame from payload"))
}