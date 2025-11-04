use color_eyre::eyre::{Result, eyre};
use std::{collections::HashMap, ffi::OsStr, fs, path::PathBuf};
use embedded_can::{Frame as FrameTrait, Id};
use waveshare_usb_can_a::Frame;
use can_dbc::{Dbc};

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

        let map = dbc
            .messages()
            .iter()
            .enumerate()
            .map(|(i, msg)| (msg.id().raw(), i))
            .collect();

        Ok(DbcHandler {
            dbc,
            message_index_by_id: map
        })
    }

    pub fn decode(&self, frame: Frame) -> (&String, f64, &String) {
        let idx = *self.message_index_by_id
            .get(&id_to_u32(&frame.id()))
            .unwrap();

        let message = self.dbc.messages()
            .get(idx)
            .unwrap();

        let signal = message
            .signals()
            .iter()
            .next()
            .unwrap();

        let data = connect_bytes(frame.data()).unwrap();

        // finally decoding part
        let result = data * signal.factor() - signal.offset();

        (message.name(), result, signal.unit())
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
        Id::Extended(eid) => eid.as_raw(),
    }
}

fn connect_bytes(data: &[u8]) -> Option<f64> {
    if data.is_empty() || data.len() > 8 {
        return None;
    }
    let mut value: u64 = 0;
    for &byte in data {
        value = (value << 8) | (byte as u64);
    }
    Some(value as f64)
}

#[cfg(test)]
mod tests {
    use crate::integration::dbc_handler::DbcHandler;
    use color_eyre::Result;
    use embedded_can::{Frame as FrameTrait, Id};
    use waveshare_usb_can_a::Frame;

    #[test]
    fn decode_two_sample_frames() -> Result<()> {
        // test with the BMS's dbc file
        // to see print in tests:
        // cargo test -- --show-output
        let dbc = DbcHandler::new()?;
        println!("DBC loaded: {} message definitions available\n", dbc.dbc.messages().len());

        let frame1 = Frame::new(
            Id::Standard(embedded_can::StandardId::new(129).unwrap()), &[11, 223]).unwrap();

        let frame2 = Frame::new(
            Id::Standard(embedded_can::StandardId::new(130).unwrap()), &[49, 231]).unwrap();

        let mut name;
        let mut value;
        let mut unit;

        (name, value, unit) = dbc.decode(frame1);

        println!("{}: {} {}", name, value, unit);

        (name, value, unit) = dbc.decode(frame2);

        println!("{}: {} {}", name, value, unit);

        Ok(())
    }
}