use std::sync::Arc;

use color_eyre::eyre::{Result, eyre};
use embedded_can::{ExtendedId, Id, StandardId};
use socketcan::{CanFrame, EmbeddedFrame};
use socketcan::tokio::CanSocket;
use time::OffsetDateTime;
use tokio::sync::mpsc;

use crate::integration::dbc_handler::{unpack_id, DbcHandler};
use crate::setup::config;
use crate::websocket::{CanTransmitRequest, CanUpdate, RawFrame, SignalData};

pub struct App {
    dbc_handler: Arc<DbcHandler>,
    interface_name: String,
    ws_tx: Option<mpsc::UnboundedSender<CanUpdate>>,
    ws_rx: Option<mpsc::UnboundedReceiver<CanTransmitRequest>>,
    raw_tx: Option<mpsc::UnboundedSender<RawFrame>>,
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
            raw_tx: None,
        })
    }

    /// Sets the sender for WebSocket updates
    pub fn set_websocket_sender(&mut self, tx: mpsc::UnboundedSender<CanUpdate>) {
        self.ws_tx = Some(tx);
    }

    pub fn set_websocket_receiver(&mut self, rx: mpsc::UnboundedReceiver<CanTransmitRequest>) {
        self.ws_rx = Some(rx);
    }

    pub fn set_raw_sender(&mut self, tx: mpsc::UnboundedSender<RawFrame>) {
        self.raw_tx = Some(tx);
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
        let raw_tx = self.raw_tx.clone();

        // Frames we transmit are echoed back by the kernel and picked up here,
        // so this is the only place that publishes to the WebSocket streams.
        let rx_task: tokio::task::JoinHandle<Result<()>> = tokio::spawn(async move {
            loop {
                match read_socket.read_frame().await {
                    Ok(frame) => {
                        let timestamp = OffsetDateTime::now_local()?;

                        // Raw path: capture the frame before decode() consumes it.
                        if let Some(ref raw_tx) = raw_tx {
                            let (message_id, is_extended) = unpack_id(&frame.id());
                            let _ = raw_tx.send(RawFrame {
                                message_id,
                                is_extended,
                                data: frame.data().to_vec(),
                                timestamp,
                            });
                        }

                        match dbc_handler.decode(frame) {
                            Ok((msg_name, signals)) => {
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
                        }
                    }
                    Err(e) => {
                        return Err(eyre!("SocketCAN read error: {}", e));
                    }
                }
            }
        });

        if let Some(mut rx) = self.ws_rx.take() {
            let tx_task: tokio::task::JoinHandle<Result<()>> = tokio::spawn(async move {
                while let Some(request) = rx.recv().await {
                    let frame = match build_frame_from_request(&request) {
                        Ok(frame) => frame,
                        Err(e) => {
                            warn!("Ignoring invalid transmit request: {}", e);
                            continue;
                        }
                    };

                    if let Err(e) = write_socket.write_frame(frame).await {
                        error!("SocketCAN write error: {}", e);
                        continue;
                    }
                }
                Ok(())
            });

            let (rx_result, tx_result) = tokio::join!(rx_task, tx_task);

            match rx_result {
                Err(e) => return Err(eyre!("Receiver task failed: {}", e)),
                Ok(Err(e)) => return Err(e),
                Ok(Ok(())) => {}
            }
            match tx_result {
                Err(e) => return Err(eyre!("Transmitter task failed: {}", e)),
                Ok(Err(e)) => return Err(e),
                Ok(Ok(())) => {}
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