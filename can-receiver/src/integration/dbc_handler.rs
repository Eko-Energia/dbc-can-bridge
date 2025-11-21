use color_eyre::eyre::{Result, eyre};
use std::{collections::HashMap, ffi::OsStr, fs, path::PathBuf};
use embedded_can::{Frame as FrameTrait, Id};
use waveshare_usb_can_a::Frame;
use can_dbc::{Dbc};

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
            let value = decode_signal_value(signal.start_bit, signal.size, frame.data())?;
            // decode collected value
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

fn decode_signal_value(start_bit: u64, size: u64, data: &[u8]) -> Result<f64> {
    let start = (start_bit / 8) as usize;         // starting byte
    let end = start + (size / 8).max(1) as usize; // ending byte (exclusive in for)
    let mut value: u64 = 0;

    for i in start..end {
        let byte = *data.get(i).ok_or_else(|| eyre!("Signal: Index {} out of bounds", i))?;
        value = (value << 8) | (byte as u64);
    }

    Ok(value as f64)
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