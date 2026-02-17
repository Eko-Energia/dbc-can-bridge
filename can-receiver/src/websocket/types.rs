use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::OffsetDateTime;

/// Messages from client to server
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    /// Client subscribes to selected message names (or all if empty list)
    #[serde(rename = "subscribe")]
    Subscribe { 
        #[serde(default)]
        message_names: Vec<String> 
    },

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    #[serde(rename = "transmit")]
    Transmit {
        message_id: u32,
        data: Vec<u8>,
        #[serde(default)]
        is_extended: Option<bool>,
    },
}

/// Messages from server to client
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ServerMessage<'a> {
    /// Full map snapshot (sent at connection start)
    #[serde(rename = "snapshot")]
    Snapshot { 
        data: HashMap<&'a str, MapEntryDto<'a>> 
    },
    
    /// Update of a single entry
    #[serde(rename = "update")]
    Update { 
        message_name: &'a str,
        entry: MapEntryDto<'a> 
    },
}

/// DTO for MapEntry - serialized without copying
#[derive(Debug, Serialize)]
pub struct MapEntryDto<'a> {
    pub signals: &'a [SignalValueDto<'a>],
    #[serde(with = "time::serde::rfc3339")]
    pub timestamp: OffsetDateTime,
}

/// DTO for SignalValue - serialized without copying
#[derive(Debug, Serialize)]
pub struct SignalValueDto<'a> {
    pub name: &'a str,
    pub value: f64,
    pub unit: &'a str,
}

/// CAN frame update sent through the channel
#[derive(Debug, Clone)]
pub struct CanUpdate {
    pub message_name: String,
    pub signals: Vec<SignalData>,
    pub timestamp: OffsetDateTime,
}

/// Owned version of signal data for passing through channel
#[derive(Debug, Clone)]
pub struct SignalData {
    pub name: String,
    pub value: f64,
    pub unit: String,
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
#[derive(Debug, Clone)]
pub struct CanTransmitRequest {
    pub message_id: u32,
    pub data: Vec<u8>,
    pub is_extended: Option<bool>,
}
