use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;

use color_eyre::eyre::Result;
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::tungstenite::Message;

use super::types::{CanUpdate, ClientMessage, MapEntryDto, ServerMessage, SignalValueDto};

type ClientId = usize;

struct ClientState {
    tx: mpsc::UnboundedSender<Message>,
    subscriptions: Option<HashSet<String>>, // None = wszystko, Some = wybrane
}

pub struct WebSocketServer {
    update_rx: mpsc::UnboundedReceiver<CanUpdate>,
    update_tx: mpsc::UnboundedSender<CanUpdate>,
    clients: Arc<RwLock<HashMap<ClientId, ClientState>>>,
    next_client_id: Arc<RwLock<ClientId>>,
    // Cache ostatnich stanów dla snapshot
    cache: Arc<RwLock<HashMap<String, CanUpdate>>>,
}

impl WebSocketServer {
    pub fn new() -> Self {
        let (update_tx, update_rx) = mpsc::unbounded_channel();
        
        Self {
            update_rx,
            update_tx,
            clients: Arc::new(RwLock::new(HashMap::new())),
            next_client_id: Arc::new(RwLock::new(0)),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Returns sender for sending CAN updates
    pub fn get_update_sender(&self) -> mpsc::UnboundedSender<CanUpdate> {
        self.update_tx.clone()
    }

    /// Starts the WebSocket server
    pub async fn run(mut self, addr: SocketAddr) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;
        info!("WebSocket server listening on {}", addr);

        // Task obsługujący aktualizacje CAN i rozsyłanie do klientów
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

        // Accept new connections
        let clients = self.clients.clone();
        let next_client_id = self.next_client_id.clone();
        let cache = self.cache.clone();

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
