use color_eyre::eyre::{Result, eyre};
use std::{cmp::min, collections::HashMap, fs};
use can_dbc::{Dbc, ByteOrder, ValueType};
use embedded_can::{Frame, Id};
use crate::integration::file_helpers::{find_first_extension_file_in_exe_dir, load_error_map};

const ERROR_SUFFIXES: [&str; 2] = ["_NODE", "_EMCY"];

#[derive(Debug, Clone)]
pub struct SignalValue<'a> {
    pub name: &'a String,
    pub value: f64,
    pub unit: &'a String,
}

pub struct DbcHandler {
    pub dbc: Dbc,
    pub(crate) message_index_by_id: HashMap<u32, (usize, bool)>, // bool means "is error frame"
    pub(crate) error_map: Option<HashMap<u32, String>>
}

impl DbcHandler {
    pub fn new() -> Result<Self> {
        let data = fs::read_to_string(find_first_extension_file_in_exe_dir("dbc")?)?;
        let dbc = Dbc::try_from(data.as_str())?;

        // for debug purposes
        // println!("{:#?}", dbc);

        let map: HashMap<u32, (usize, bool)> = dbc
            .messages
            .iter()
            .enumerate()
            .map(|(i, msg)| (msg.id.raw(), (i, is_error_frame(&msg.name))))
            .collect();

        // another debug
        // println!("{:#?}", dbc.messages[map[&130]]);

        let error_map = match load_error_map(find_first_extension_file_in_exe_dir("csv")?) {
            Ok(map) => Some(map),
            Err(e) => {
                warn!("Problem with loading error map: {}. Error mapping will be disabled!", e);
                None
            }
        };

        // println!("{:#?}", error_map);

        Ok(DbcHandler {
            dbc,
            message_index_by_id: map,
            error_map
        })
    }

    pub fn decode<T: Frame>(&'_ self, frame: T) -> Result<(&'_ String, Vec<SignalValue<'_>>)> {
        if frame.data().is_empty() || frame.data().len() > 8 {
            return Err(eyre!("Error: Frame ID: {:?} is either empty or data exceeds 8 bytes!", frame.id()));
        }

        let (idx, is_error_frame) = *self.message_index_by_id
            .get(&id_to_u32(&frame.id()))
            .ok_or_else(|| eyre!("No message definition found for frame ID: {:?}", frame.id()))?;

        let message = self.dbc.messages
            .get(idx)
            .ok_or_else(|| eyre!("Message index {} out of bounds for frame ID: {:?}", idx, frame.id()))?;

        let mut results: Vec<SignalValue> = Vec::new();

        let mut skip_first = 0;

        // best-effort error mapping
        // by convention first signal in error frame is an error code
        if let Some(err_map) = &self.error_map
            && is_error_frame
            && let Some(first_signal) = message.signals.first() {

                let value = decode_signal_value(
                    first_signal.start_bit, first_signal.size, first_signal.byte_order, first_signal.value_type,
                    first_signal.factor, first_signal.offset, frame.data())?;
                // add to a vector
                results.push(SignalValue {
                    name: &first_signal.name,
                    value,
                    unit: err_map.get(&(value.round() as u32)).unwrap_or(&first_signal.unit)
                });

                skip_first = 1;
                
            }

        for signal in message.signals.iter().skip(skip_first) {
            let value = decode_signal_value(
                signal.start_bit, signal.size, signal.byte_order, signal.value_type, signal.factor, signal.offset, frame.data())?;
            // add to a vector
            results.push(SignalValue {
                name: &signal.name,
                value,
                unit: &signal.unit,
            });
        }

        Ok((&message.name, results))
    }
}

fn is_error_frame(msg_name: &str) -> bool {
    ERROR_SUFFIXES.iter().any(|s| msg_name.ends_with(s))
}

pub(crate) fn id_to_u32(id: &Id) -> u32 {
        match id {
        Id::Standard(sid) => sid.as_raw() as u32,
        Id::Extended(eid) => eid.as_raw() | 1 << 31,
    }
}

/// Splits a CAN identifier into its raw arbitration id and whether it is an
/// extended (29-bit) frame. Unlike `id_to_u32`, this does NOT encode the
/// extended flag into the value - the flag is returned separately.
pub(crate) fn unpack_id(id: &Id) -> (u32, bool) {
    match id {
        Id::Standard(s) => (s.as_raw() as u32, false),
        Id::Extended(e) => (e.as_raw(), true),
    }
}

// inspired by: https://github.com/PurdueElectricRacing/can_decode/
/// Decodes a single signal from raw CAN data.
/// Extracts the raw bits for a signal, converts to signed/unsigned as needed.
/// Applies factor and offset.
pub(crate) fn decode_signal_value(
    start_bit: u64,
    size: u64,
    byte_order: ByteOrder,
    value_type: ValueType,
    factor: f64,
    offset: f64,
    data: &[u8]
)-> Result<f64> {
    // Guard rails: avoid shift/underflow for size==0 and shift-out-of-range for size>64
    if size == 0 {
        return Err(eyre!("Invalid signal size: 0"));
    }
    if size > 64 {
        return Err(eyre!("Invalid signal size: {} (max 64)", size));
    }

    // Extract raw value based on byte order and signal properties
    let raw_value = extract_signal_value(
        data,
        start_bit as usize,
        size as usize,
        byte_order,
    )?;

    // Convert to signed if needed
    let raw_value = if value_type == ValueType::Signed {
        // Convert to signed based on signal size
        if size == 64 {
            // Full 64-bit two's complement: bit pattern cast is enough
            (raw_value as i64) as f64
        } else {
            let max_unsigned = (1u64 << size) - 1;
            let sign_bit = 1u64 << (size - 1);

            if raw_value & sign_bit != 0 {
                // Negative number - extend sign
                (raw_value | (!max_unsigned)) as i64 as f64
            } else {
                raw_value as f64
            }
        }
    } else {
        raw_value as f64
    };

    // decode collected value - apply factor and offset
    let result = raw_value * factor + offset;

    Ok(result)
}

// inspired by: https://github.com/PurdueElectricRacing/can_decode/
/// Extracts raw signal bits from CAN data.
/// Handles both little-endian and big-endian byte ordering according to
/// the signal definition.
pub(crate) fn extract_signal_value(
    data: &[u8],
    start_bit: usize,
    size: usize,
    byte_order: ByteOrder,
) -> Result<u64> {
    let mut result = 0u64;

    match byte_order {
        ByteOrder::LittleEndian => {
            let start_byte = start_bit / 8;
            let start_bit_in_byte = start_bit % 8;

            let mut remaining_bits = size;
            let mut current_byte = start_byte;
            let mut bit_offset = start_bit_in_byte;

            while remaining_bits > 0 && current_byte < data.len() {
                let bits_in_this_byte = min(remaining_bits, 8 - bit_offset);
                let mask = ((1u64 << bits_in_this_byte) - 1) << bit_offset;
                let byte_value = ((data[current_byte] as u64) & mask) >> bit_offset;

                result |= byte_value << (size - remaining_bits);

                remaining_bits -= bits_in_this_byte;
                current_byte += 1;
                bit_offset = 0;
            }

            if remaining_bits > 0 {
                return Err(eyre!(
                    "Not enough data to decode little-endian signal: start_bit={}, size_bits={}, data_len_bytes={}",
                    start_bit, size, data.len()
                ));
            }
        }
        
        ByteOrder::BigEndian => {
            // Motorola (@0) without bit-by-bit:
            // Take enough bytes, build a big-endian u64 window, then shift+mask.
            //
            // start_bit is the MSB position of the signal, where bit index inside a byte is LSB0
            // i believe that in most DBCs Big endian uses LSB0.
            // (0 = LSB/rightmost, 7 = MSB/leftmost).
            // https://github.com/ebroecker/canmatrix/wiki/signal-Byteorder

            let start_byte = start_bit / 8;
            let start_bit_in_byte = start_bit % 8;

            // How many bits do we span from the MSB position downwards?
            // If start_bit_in_byte == 7, we're on MSB and span_bits == size.
            let span_bits = (7 - start_bit_in_byte) + size;
            let byte_count = span_bits.div_ceil(8);

            if start_byte + byte_count > data.len() {
                return Err(eyre!(
                    "Not enough data to decode big-endian signal: start_bit={}, size_bits={}, data_len_bytes={}",
                    start_bit, size, data.len()
                ));
            }

            // Build window as big-endian bytes: [b0][b1]...[bn]
            let mut acc = 0u64;
            for i in 0..byte_count {
                acc = (acc << 8) | data[start_byte + i] as u64;
            }

            let total_bits = byte_count * 8;
            // Position the signal LSB at bit 0.
            let shift = total_bits - (7 - start_bit_in_byte) - size;

            let mask = if size == 64 { u64::MAX } else { (1u64 << size) - 1 };
            result = (acc >> shift) & mask;
        }
    }

    Ok(result)
}

#[cfg(test)]
#[path = "./dbc_handler_test.rs"]
mod dbc_handler_test;
