use color_eyre::eyre::{Result, eyre};
use std::{collections::HashMap, ffi::OsStr, fs, path::PathBuf};
use embedded_can::{Frame as FrameTrait, Id};
use waveshare_usb_can_a::Frame;
use can_dbc::{Dbc};

#[derive(Debug)]
pub struct SignalValue<'a> {
    pub name: &'a str,
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

    pub fn decode(&'_ self, frame: Frame) -> Option<(&'_ String, Vec<SignalValue<'_>>)> {
        let idx = *self.message_index_by_id
            .get(&id_to_u32(&frame.id()))?;

        let message = self.dbc.messages()
            .get(idx)?;

        if frame.data().is_empty() || frame.data().len() > 8 {
            return None;
        }

        let mut loading = false;
        let mut results: Vec<SignalValue> = Vec::new();
        let mut value: u64 = 0;

        for (i, signal) in message.signals().iter().enumerate() {
            let byte = *frame.data().get(i)?;
            
            match (loading, signal.name()) {
                (false, name) if name.ends_with('H') => {
                    loading = true;
                    value = byte as u64;
                }
                (false, _) => {
                    // a single byte value
                    let result = (byte as f64) * signal.factor() - signal.offset();
                    // add to a vector
                    results.push(SignalValue {
                        name: signal.name(),
                        value: result,
                        unit: signal.unit(),
                    });
                }
                (true, name) if name.ends_with('L') => {
                    // decode collected value
                    loading = false;
                    value = (value << 8) | (byte as u64);
                    let result = (value as f64) * signal.factor() - signal.offset();
                    // add to a vector
                    results.push(SignalValue {
                        name: &name[..name.len() - 2],
                        value: result,
                        unit: signal.unit(),
                    });
                }
                (true, _) => {
                    // load more bytes if in loading
                    value = (value << 8) | (byte as u64);
                }
            }
        }

        Some((message.name(), results))
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
        println!("DBC loaded: {} message definitions available\n", dbc.dbc.messages().len());

        let frame = Frame::new(
            Id::Standard(embedded_can::StandardId::new(130).unwrap()), &[49, 231, 0, 0, 11, 223]).unwrap();

        let (msg_name, signals) = dbc.decode(frame).unwrap();
        println!("{}:", msg_name);
        signals.iter().for_each(
            |s| println!("  {}: {} {}", s.name, s.value, s.unit));

        let frame1 = Frame::new(
            Id::Standard(embedded_can::StandardId::new(139).unwrap()), &[49, 231, 3, 4, 11, 223, 6]).unwrap();

        let (msg_name, signals) = dbc.decode(frame1).unwrap();
        println!("{}:", msg_name);
        signals.iter().for_each(
            |s| println!("  {}: {} {}", s.name, s.value, s.unit));

        let frame2 = Frame::new(
            Id::Standard(embedded_can::StandardId::new(129).unwrap()), &[0]).unwrap();

        let (msg_name, signals) = dbc.decode(frame2).unwrap();
        println!("{}:", msg_name);
        signals.iter().for_each(
            |s| println!("  {}: {} {}", s.name, s.value, s.unit));

        Ok(())
    }
}