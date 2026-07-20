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

    /// Client opts into the raw frame stream (snapshot + updates).
    #[serde(rename = "subscribe_raw")]
    SubscribeRaw,

    /// Client opts out of the raw frame stream.
    #[serde(rename = "unsubscribe_raw")]
    UnsubscribeRaw,

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

    /// Full raw-frame snapshot (sent after subscribe_raw).
    #[serde(rename = "raw_snapshot")]
    RawSnapshot {
        frames: Vec<RawFrameDto<'a>>,
    },

    /// A single raw frame update.
    #[serde(rename = "raw_update")]
    RawUpdate(RawFrameDto<'a>),
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

/// Zero-copy DTO for a raw CAN frame - serialized without copying the payload.
#[derive(Debug, Serialize)]
pub(crate) struct RawFrameDto<'a> {
    pub message_id: u32,
    pub is_extended: bool,
    pub data: &'a [u8],
    #[serde(with = "time::serde::rfc3339")]
    pub timestamp: OffsetDateTime,
}

/// CAN frame update sent through the channel
#[derive(Debug, Clone)]
pub struct CanUpdate {
    pub message_name: String,
    pub signals: Vec<SignalData>,
    pub timestamp: OffsetDateTime,
}

/// Owned raw CAN frame passed through the channel. `data` carries its own
/// length (0..=8 bytes for classic CAN), so no separate length field is needed.
#[derive(Debug, Clone)]
pub struct RawFrame {
    pub message_id: u32,
    pub is_extended: bool,
    pub data: Vec<u8>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_update_serializes_with_expected_fields() {
        let ts = OffsetDateTime::from_unix_timestamp(0).unwrap();
        let dto = RawFrameDto { message_id: 291, is_extended: false, data: &[1, 2, 3], timestamp: ts };
        let json = serde_json::to_string(&ServerMessage::RawUpdate(dto)).unwrap();
        assert!(json.contains("\"type\":\"raw_update\""));
        assert!(json.contains("\"message_id\":291"));
        assert!(json.contains("\"is_extended\":false"));
        assert!(json.contains("\"data\":[1,2,3]"));
        assert!(json.contains("\"timestamp\":\"1970-01-01T00:00:00Z\""));
    }

    #[test]
    fn raw_snapshot_serializes_frames_array() {
        let ts = OffsetDateTime::from_unix_timestamp(0).unwrap();
        let dto = RawFrameDto { message_id: 5, is_extended: true, data: &[255], timestamp: ts };
        let json = serde_json::to_string(&ServerMessage::RawSnapshot { frames: vec![dto] }).unwrap();
        assert!(json.contains("\"type\":\"raw_snapshot\""));
        assert!(json.contains("\"frames\":[{"));
        assert!(json.contains("\"is_extended\":true"));
    }

    #[test]
    fn subscribe_raw_deserializes() {
        let msg: ClientMessage = serde_json::from_str(r#"{"type":"subscribe_raw"}"#).unwrap();
        assert!(matches!(msg, ClientMessage::SubscribeRaw));
    }

    #[test]
    fn unsubscribe_raw_deserializes() {
        let msg: ClientMessage = serde_json::from_str(r#"{"type":"unsubscribe_raw"}"#).unwrap();
        assert!(matches!(msg, ClientMessage::UnsubscribeRaw));
    }
}
