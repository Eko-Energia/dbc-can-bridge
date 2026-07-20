use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;

use color_eyre::eyre::Result;
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::tungstenite::Message;

use super::types::{CanUpdate, ClientMessage, MapEntryDto, RawFrame, ServerMessage, SignalValueDto};
use super::types::RawFrameDto;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use super::types::CanTransmitRequest;
use crate::setup::config;

type ClientId = usize;

struct ClientState {
    tx: mpsc::UnboundedSender<Message>,
    subscriptions: Option<HashSet<String>>, // None = everything, Some = selected
    raw: bool,                              // opted into the raw frame stream
}

pub struct WebSocketServer {
    update_rx: mpsc::UnboundedReceiver<CanUpdate>,
    update_tx: mpsc::UnboundedSender<CanUpdate>,
    raw_update_rx: mpsc::UnboundedReceiver<RawFrame>,
    raw_update_tx: mpsc::UnboundedSender<RawFrame>,
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    can_tx: Option<mpsc::UnboundedSender<CanTransmitRequest>>,
    clients: Arc<RwLock<HashMap<ClientId, ClientState>>>,
    next_client_id: Arc<RwLock<ClientId>>,
    // Cache of latest states for snapshot
    cache: Arc<RwLock<HashMap<String, CanUpdate>>>,
    raw_cache: Arc<RwLock<HashMap<u32, RawFrame>>>,
}

impl WebSocketServer {
    pub fn new() -> Self {
        let (update_tx, update_rx) = mpsc::unbounded_channel();
        let (raw_update_tx, raw_update_rx) = mpsc::unbounded_channel();

        Self {
            update_rx,
            update_tx,
            raw_update_rx,
            raw_update_tx,
            #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
            can_tx: None,
            clients: Arc::new(RwLock::new(HashMap::new())),
            next_client_id: Arc::new(RwLock::new(0)),
            cache: Arc::new(RwLock::new(HashMap::new())),
            raw_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Returns sender for sending CAN updates
    pub fn get_update_sender(&self) -> mpsc::UnboundedSender<CanUpdate> {
        self.update_tx.clone()
    }

    /// Returns sender for pushing raw CAN frames
    pub fn get_raw_update_sender(&self) -> mpsc::UnboundedSender<RawFrame> {
        self.raw_update_tx.clone()
    }

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    pub fn set_can_tx_sender(&mut self, tx: mpsc::UnboundedSender<CanTransmitRequest>) {
        self.can_tx = Some(tx);
    }

    /// Starts the WebSocket server
    pub async fn run(mut self, addr: SocketAddr) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;
        info!("WebSocket server listening on {}", addr);

        let raw_enabled = config::get_broadcast_raw_frames()?;

        // Task handling CAN updates and broadcasting to clients
        let clients = self.clients.clone();
        let cache = self.cache.clone();
        tokio::spawn(async move {
            while let Some(update) = self.update_rx.recv().await {
                // Update cache
                {
                    let mut cache_guard = cache.write().await;
                    cache_guard.insert(update.message_name.clone(), update.clone());
                }

                // Send to all interested clients
                let clients_guard = clients.read().await;
                for client in clients_guard.values() {
                    // Check if client is interested in this message
                    let should_send = match &client.subscriptions {
                        None => true, // subscribes to everything
                        Some(subs) => subs.contains(&update.message_name),
                    };

                    if should_send {
                        // Convert to DTO and serialize
                        let signals_dto: Vec<SignalValueDto> = update
                            .signals
                            .iter()
                            .map(|s| SignalValueDto {
                                name: &s.name,
                                value: s.value,
                                unit: &s.unit,
                            })
                            .collect();

                        let entry_dto = MapEntryDto {
                            signals: &signals_dto,
                            timestamp: update.timestamp,
                        };

                        let msg = ServerMessage::Update {
                            message_name: &update.message_name,
                            entry: entry_dto,
                        };

                        if let Ok(json) = serde_json::to_string(&msg) {
                            let _ = client.tx.send(Message::Text(json.into()));
                        }
                    }
                }
            }
        });

        // Task handling raw frames: update cache and broadcast to raw subscribers.
        if raw_enabled {
            let clients = self.clients.clone();
            let raw_cache = self.raw_cache.clone();
            tokio::spawn(async move {
                while let Some(frame) = self.raw_update_rx.recv().await {
                    // Broadcast from a borrow, then move the frame into the cache (no clone).
                    broadcast_raw(&clients, &frame).await;
                    let key = raw_cache_key(&frame);
                    raw_cache.write().await.insert(key, frame);
                }
            });
        }

        // Accept new connections
        let clients = self.clients.clone();
        let next_client_id = self.next_client_id.clone();
        let cache = self.cache.clone();
        let raw_cache = self.raw_cache.clone();
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        let can_tx = self.can_tx.clone();

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let client_id = {
                        let mut id = next_client_id.write().await;
                        let current = *id;
                        *id += 1;
                        current
                    };

                    info!("New WebSocket connection from {}, assigned ID {}", addr, client_id);
                    
                    tokio::spawn(handle_connection(
                        stream,
                        client_id,
                        clients.clone(),
                        cache.clone(),
                        raw_cache.clone(),
                        raw_enabled,
                        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
                        can_tx.clone(),
                    ));
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e);
                }
            }
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    client_id: ClientId,
    clients: Arc<RwLock<HashMap<ClientId, ClientState>>>,
    cache: Arc<RwLock<HashMap<String, CanUpdate>>>,
    raw_cache: Arc<RwLock<HashMap<u32, RawFrame>>>,
    raw_enabled: bool,
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    can_tx: Option<mpsc::UnboundedSender<CanTransmitRequest>>,
) {
    let ws_stream = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            error!("WebSocket handshake failed for client {}: {}", client_id, e);
            return;
        }
    };

    info!("WebSocket handshake completed for client {}", client_id);

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel();

    // Register client (no subscriptions by default = receives everything)
    {
        let mut clients_guard = clients.write().await;
        clients_guard.insert(
            client_id,
            ClientState {
                tx: tx.clone(),
                subscriptions: None,
                raw: false,
            },
        );
    }

    // Task sending messages to client
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Main loop receiving messages from client
    while let Some(msg_result) = ws_receiver.next().await {
        match msg_result {
            Ok(Message::Text(text)) => {
                if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                    match client_msg {
                        ClientMessage::Subscribe { message_names } => {
                            info!("Client {} subscribing to: {:?}", client_id, message_names);
                            
                            let subscriptions = if message_names.is_empty() {
                                None // everything
                            } else {
                                Some(message_names.iter().cloned().collect())
                            };

                            // Update subscriptions
                            {
                                let mut clients_guard = clients.write().await;
                                if let Some(client) = clients_guard.get_mut(&client_id) {
                                    client.subscriptions = subscriptions.clone();
                                }
                            }

                            // Send snapshot from cache for subscribed messages
                            send_snapshot(&tx, &cache, subscriptions.as_ref()).await;
                        }

                        ClientMessage::SubscribeRaw => {
                            if !raw_enabled {
                                warn!("Client {} requested raw stream, but broadcast_raw_frames is disabled", client_id);
                            } else {
                                info!("Client {} subscribing to raw frames", client_id);
                                {
                                    let mut clients_guard = clients.write().await;
                                    if let Some(client) = clients_guard.get_mut(&client_id) {
                                        client.raw = true;
                                    }
                                }
                                send_raw_snapshot(&tx, &raw_cache).await;
                            }
                        }

                        ClientMessage::UnsubscribeRaw => {
                            info!("Client {} unsubscribing from raw frames", client_id);
                            let mut clients_guard = clients.write().await;
                            if let Some(client) = clients_guard.get_mut(&client_id) {
                                client.raw = false;
                            }
                        }

                        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
                        ClientMessage::Transmit {
                            message_id,
                            data,
                            is_extended,
                        } => {
                            if let Some(ref tx_can) = can_tx {
                                let req = CanTransmitRequest {
                                    message_id,
                                    data,
                                    is_extended,
                                };

                                if tx_can.send(req).is_err() {
                                    error!("CAN transmit channel closed, client {} request dropped", client_id);
                                }
                            } else {
                                warn!("Client {} sent transmit request, but CAN TX channel is not configured", client_id);
                            }
                        }
                    }
                } else {
                    warn!("Client {} sent invalid message: {}", client_id, text);
                }
            }
            Ok(Message::Close(_)) => {
                info!("Client {} closed connection", client_id);
                break;
            }
            Ok(Message::Ping(data)) => {
                let _ = tx.send(Message::Pong(data));
            }
            Err(e) => {
                error!("WebSocket error for client {}: {}", client_id, e);
                break;
            }
            _ => {}
        }
    }

    // Cleanup
    send_task.abort();
    let mut clients_guard = clients.write().await;
    clients_guard.remove(&client_id);
    info!("Client {} disconnected and removed", client_id);
}

async fn send_snapshot(
    tx: &mpsc::UnboundedSender<Message>,
    cache: &Arc<RwLock<HashMap<String, CanUpdate>>>,
    subscriptions: Option<&HashSet<String>>,
) {
    let cache_guard = cache.read().await;
    
    // Collect all entries into a vector with owned data
    let entries: Vec<(String, Vec<SignalValueDto>, time::OffsetDateTime)> = cache_guard
        .iter()
        .filter(|(msg_name, _)| {
            subscriptions.is_none_or(|subs| subs.contains(*msg_name))
        })
        .map(|(msg_name, update)| {
            let signals_dto: Vec<SignalValueDto> = update
                .signals
                .iter()
                .map(|s| SignalValueDto {
                    name: &s.name,
                    value: s.value,
                    unit: &s.unit,
                })
                .collect();
            
            (msg_name.clone(), signals_dto, update.timestamp)
        })
        .collect();
    
    // Now build map with references to owned data
    let filtered: HashMap<&str, MapEntryDto> = entries
        .iter()
        .map(|(msg_name, signals_dto, timestamp)| {
            let entry_dto = MapEntryDto {
                signals: signals_dto.as_slice(),
                timestamp: *timestamp,
            };
            (msg_name.as_str(), entry_dto)
        })
        .collect();

    let snapshot = ServerMessage::Snapshot { data: filtered };

    if let Ok(json) = serde_json::to_string(&snapshot) {
        let _ = tx.send(Message::Text(json.into()));
        info!("Sent snapshot with {} entries", entries.len());
    }
}

/// Computes the raw-cache key for a frame. Extended frames set bit 31 so a
/// standard and an extended frame that share the same arbitration id do not
/// collide in the cache. Mirrors the encoding of `id_to_u32`.
fn raw_cache_key(frame: &RawFrame) -> u32 {
    if frame.is_extended {
        frame.message_id | (1 << 31)
    } else {
        frame.message_id
    }
}

/// Serializes a single raw frame update once and sends it to every client that
/// opted into the raw stream. The frame is borrowed, not cloned.
async fn broadcast_raw(
    clients: &Arc<RwLock<HashMap<ClientId, ClientState>>>,
    frame: &RawFrame,
) {
    let dto = RawFrameDto {
        message_id: frame.message_id,
        is_extended: frame.is_extended,
        data: &frame.data,
        timestamp: frame.timestamp,
    };

    let json = match serde_json::to_string(&ServerMessage::RawUpdate(dto)) {
        Ok(json) => json,
        Err(_) => return,
    };
    let msg = Message::Text(json.into());

    let clients_guard = clients.read().await;
    for client in clients_guard.values() {
        if client.raw {
            let _ = client.tx.send(msg.clone());
        }
    }
}

/// Sends the current raw-frame snapshot from the cache to a single client.
async fn send_raw_snapshot(
    tx: &mpsc::UnboundedSender<Message>,
    raw_cache: &Arc<RwLock<HashMap<u32, RawFrame>>>,
) {
    let cache_guard = raw_cache.read().await;

    let frames: Vec<RawFrameDto> = cache_guard
        .values()
        .map(|f| RawFrameDto {
            message_id: f.message_id,
            is_extended: f.is_extended,
            data: &f.data,
            timestamp: f.timestamp,
        })
        .collect();

    let snapshot = ServerMessage::RawSnapshot { frames };

    if let Ok(json) = serde_json::to_string(&snapshot) {
        let _ = tx.send(Message::Text(json.into()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;

    fn sample_frame() -> RawFrame {
        RawFrame {
            message_id: 291,
            is_extended: false,
            data: vec![1, 2, 3],
            timestamp: OffsetDateTime::from_unix_timestamp(0).unwrap(),
        }
    }

    #[test]
    fn raw_cache_key_sets_bit31_for_extended_only() {
        let mut f = sample_frame();
        f.message_id = 0x123;
        f.is_extended = false;
        assert_eq!(raw_cache_key(&f), 0x123);
        f.is_extended = true;
        assert_eq!(raw_cache_key(&f), 0x123 | (1 << 31));
    }

    #[tokio::test]
    async fn send_raw_snapshot_emits_cached_frames() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let cache: Arc<RwLock<HashMap<u32, RawFrame>>> = Arc::new(RwLock::new(HashMap::new()));
        cache.write().await.insert(291, sample_frame());

        send_raw_snapshot(&tx, &cache).await;

        let msg = rx.recv().await.unwrap();
        if let Message::Text(text) = msg {
            assert!(text.contains("\"type\":\"raw_snapshot\""));
            assert!(text.contains("\"message_id\":291"));
            assert!(text.contains("\"data\":[1,2,3]"));
        } else {
            panic!("expected text message");
        }
    }

    #[tokio::test]
    async fn broadcast_raw_only_reaches_subscribed_clients() {
        let clients: Arc<RwLock<HashMap<ClientId, ClientState>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let (tx_on, mut rx_on) = mpsc::unbounded_channel();
        let (tx_off, mut rx_off) = mpsc::unbounded_channel();
        {
            let mut g = clients.write().await;
            g.insert(1, ClientState { tx: tx_on, subscriptions: None, raw: true });
            g.insert(2, ClientState { tx: tx_off, subscriptions: None, raw: false });
        }

        broadcast_raw(&clients, &sample_frame()).await;

        let msg = rx_on.recv().await.unwrap();
        assert!(matches!(msg, Message::Text(_)));
        assert!(rx_off.try_recv().is_err());
    }
}
