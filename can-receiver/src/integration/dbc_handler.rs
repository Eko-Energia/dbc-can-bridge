use color_eyre::eyre::{Result, eyre};
use std::{collections::HashMap, ffi::OsStr, fs, path::PathBuf};
use embedded_can::{Frame as FrameTrait, Id};
use waveshare_usb_can_a::Frame;
use can_dbc::{Dbc, ByteOrder, ValueType};

#[derive(Debug)]
pub struct SignalValue<'a> {
    pub name: &'a String,
    pub value: f64,
    pub unit: &'a String,
}

pub struct DbcHandler {
    pub dbc: Dbc,
    message_index_by_id: HashMap<u32, usize>
}

impl DbcHandler {
    pub fn new() -> Result<Self> {
        let data = fs::read_to_string(find_first_dbc_in_exe_dir()?).expect("Unable to read input file");
        let dbc = Dbc::try_from(data.as_str()).expect("Failed to parse dbc file");

        // for debug purposes
        // println!("{:#?}", dbc);

        let map: HashMap<u32, usize> = dbc
            .messages
            .iter()
            .enumerate()
            .map(|(i, msg)| (msg.id.raw(), i))
            .collect();

        // another debug
        // println!("{:#?}", dbc.messages[map[&130]]);

        Ok(DbcHandler {
            dbc,
            message_index_by_id: map
        })
    }

    pub fn decode(&'_ self, frame: Frame) -> Result<(&'_ String, Vec<SignalValue<'_>>)> {
        if frame.data().is_empty() || frame.data().len() > 8 {
            return Err(eyre!("Error: Frame ID: {:?} is either empty or data exceedes 8 bytes!", frame.id()));
        }

        let idx = *self.message_index_by_id
            .get(&id_to_u32(&frame.id()))
            .ok_or_else(|| eyre!("No message definition found for frame ID: {:?}", frame.id()))?;

        let message = self.dbc.messages
            .get(idx)
            .ok_or_else(|| eyre!("Message index {} out of bounds for frame ID: {:?}", idx, frame.id()))?;

        let mut results: Vec<SignalValue> = Vec::new();

        for signal in &message.signals {
            let value = decode_signal_value(
                signal.start_bit, signal.size, signal.byte_order, signal.value_type, frame.data())?;
            // decode collected value - apply factor and offset
            let result = value * signal.factor + signal.offset;
            // add to a vector
            results.push(SignalValue {
                name: &signal.name,
                value: result,
                unit: &signal.unit,
            });
        }

        Ok((&message.name, results))
    }
}

/// Attempts to find the first .dbc file in the same directory as the running binary.
/// Returns Ok(None) if none found.
fn find_first_dbc_in_exe_dir() -> Result<PathBuf> {
    let mut exe_dir = std::env::current_exe()?;
    exe_dir.pop();

    if let Ok(read_dir) = fs::read_dir(&exe_dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension() == Some(OsStr::new("dbc")) {
                return Ok(path)
            }
        }
    }
    Err(eyre!(format!("No .dbc file found in {:?}", exe_dir)))
}

fn id_to_u32(id: &Id) -> u32 {
        match id {
        Id::Standard(sid) => sid.as_raw() as u32,
        Id::Extended(eid) => eid.as_raw() | 1 << 31,
    }
}

// inspired by: https://github.com/PurdueElectricRacing/can_decode/
/// Decodes a single signal from raw CAN data.
/// Extracts the raw bits for a signal, converts to signed/unsigned as needed
fn decode_signal_value(
    start_bit: u64,
    size: u64,
    byte_order: ByteOrder,
    value_type: ValueType,
    data: &[u8]
)-> Result<f64> {
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
        let max_unsigned = (1u64 << size) - 1;
        let sign_bit = 1u64 << (size - 1);

        if raw_value & sign_bit != 0 {
            // Negative number - extend sign
            (raw_value | (!max_unsigned)) as i64 as f64
        } else {
            raw_value as f64
        }
    } else {
        raw_value as f64
    };

    Ok(raw_value)
}

// inspired by: https://github.com/PurdueElectricRacing/can_decode/
/// Extracts raw signal bits from CAN data.
/// Handles both little-endian and big-endian byte ordering according to
/// the signal definition.
fn extract_signal_value(
    data: &[u8],
    start_bit: usize,
    size: usize,
    byte_order: ByteOrder,
) -> Result<u64> {
    if data.is_empty() || size == 0 {
        return Err(eyre!("Data is empty or signal size is zero in extract_signal_value"));
    }

    let total_bits = data.len() * 8;
    if start_bit + size > total_bits {
        return Err(eyre!(
            "Signal extraction out of bounds: start_bit {} + size {} exceeds total bits {}",
            start_bit, size, total_bits));
    }

    let mut result = 0u64;

    match byte_order {
        ByteOrder::LittleEndian => {
            let start_byte = start_bit / 8;
            let start_bit_in_byte = start_bit % 8;

            let mut remaining_bits = size;
            let mut current_byte = start_byte;
            let mut bit_offset = start_bit_in_byte;

            while remaining_bits > 0 && current_byte < data.len() {
                let bits_in_this_byte = std::cmp::min(remaining_bits, 8 - bit_offset);
                let mask = ((1u64 << bits_in_this_byte) - 1) << bit_offset;
                let byte_value = ((data[current_byte] as u64) & mask) >> bit_offset;

                result |= byte_value << (size - remaining_bits);

                remaining_bits -= bits_in_this_byte;
                current_byte += 1;
                bit_offset = 0;
            }
        }
        ByteOrder::BigEndian => {
            // Big-endian (Motorola) bit extraction: iterate bits from
            // start_bit toward higher bit positions, collecting each bit
            // and appending into the result MSB-first.
            let mut bit_pos = start_bit;

            for _ in 0..size {
                let byte_idx = bit_pos / 8;
                let bit_idx = 7 - (bit_pos % 8);

                if byte_idx >= data.len() {
                    break;
                }

                let bit_val = (data[byte_idx] >> bit_idx) & 1;
                result = (result << 1) | (bit_val as u64);

                bit_pos += 1;
            }
        }
    }

    Ok(result)
}


#[cfg(test)]
mod tests {
    use crate::integration::dbc_handler::DbcHandler;
    use color_eyre::Result;
    use embedded_can::{Frame as FrameTrait, Id};
    use waveshare_usb_can_a::Frame;

    #[test]
    fn decode_two_sample_frames() -> Result<()> {
        // test with the CAN_DB file
        // to see print in tests:
        // cargo test -- --show-output
        let dbc = DbcHandler::new()?;
        println!("DBC loaded: {} message definitions available\n", dbc.dbc.messages.len());

        let frame = Frame::new(
            Id::Standard(embedded_can::StandardId::new(130).unwrap()), &[49, 231, 0, 0, 11, 223]).unwrap();

        let (msg_name, signals) = dbc.decode(frame)?;
        println!("{}:", msg_name);
        signals.iter().for_each(
            |s| println!("  {}: {} {}", s.name, s.value, s.unit));

        let frame1 = Frame::new(
            Id::Standard(embedded_can::StandardId::new(139).unwrap()), &[49, 231, 3, 4, 11, 223, 6]).unwrap();

        let (msg_name, signals) = dbc.decode(frame1)?;
        println!("{}:", msg_name);
        signals.iter().for_each(
            |s| println!("  {}: {} {}", s.name, s.value, s.unit));

        let frame2 = Frame::new(
            Id::Standard(embedded_can::StandardId::new(128).unwrap()), &[0, 2, 1]).unwrap();

        let (msg_name, signals) = dbc.decode(frame2)?;
        println!("{}:", msg_name);
        signals.iter().for_each(
            |s| println!("  {}: {} {}", s.name, s.value, s.unit));

        Ok(())
    }
}