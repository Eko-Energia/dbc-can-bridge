mod config;

use color_eyre::eyre::Result;
use embedded_can::{blocking::Can, Frame as FrameTrait, Id};
use waveshare_usb_can_a::{self as ws, Frame};
use std::fs;
use can_dbc::{Dbc, MessageId};

fn id_to_message_id(id: &Id) -> MessageId {
        match id {
        Id::Standard(sid) => MessageId::Standard(sid.as_raw()),
        Id::Extended(eid) => MessageId::Extended(eid.as_raw()),
    }
}

fn connect_bytes(data: &[u8]) -> Option<f64> {
    match data.len() {
        1 => Some(data[0] as f64),
        2 => Some((((data[0] as u16) << 8) | (data[1] as u16)) as f64),
        3 => Some((((data[0] as u32) << 16) | ((data[1] as u32) << 8) | (data[2] as u32)) as f64),
        _ => None,
    }
}

fn main() -> Result<()> {
    color_eyre::install()?;

    // Initialize configuration
    config::init_config()?;
    
    // Get settings from configuration
    let device_port = config::get_device_port()?;
    let can_baud_rate = config::get_can_baud_rate()?;
    
    println!("Using device: {}", device_port);
    println!("CAN speed: {:?}", can_baud_rate);

    // can dbc tests

    let data = fs::read_to_string(config::find_first_dbc_in_exe_dir()?.unwrap()).expect("Unable to read input file");
    let dbc = Dbc::try_from(data.as_str()).expect("Failed to parse dbc file");

    let frame1 = Frame::new(Id::Standard(embedded_can::StandardId::new(129).unwrap()), &[11, 223]).unwrap();
    let frame2 = Frame::new(Id::Standard(embedded_can::StandardId::new(130).unwrap()), &[49, 231]).unwrap();

    let message_id = id_to_message_id(&frame2.id());
    
    // Znajdź wiadomość w DBC
    if let Some(message) = dbc.messages().iter().find(|m| *m.id() == message_id) {
        println!("\nMessage: {} (ID: {:?}, Size: {} bytes)", message.name(), message.id(), message.size());
        
        let signal = message.signals().iter().next().unwrap();
        println!("  Signal: {}", signal.name());
        println!("    Start bit: {}", signal.start_bit);
        println!("    Size: {} bits", signal.size);
        println!("    Factor: {}", signal.factor);
        println!("    Offset: {}", signal.offset);
        println!("    Min: {}", signal.min);
        println!("    Max: {}", signal.max);
        println!("    Unit: {}", signal.unit());
        println!("    Byte order: {:?}", signal.byte_order());
        println!("    Value type: {:?}", signal.value_type());

        let data = connect_bytes(frame2.data()).unwrap();
        let result = data * signal.factor - signal.offset;

        println!("{} {}", result, signal.unit());

    } else {
        println!("\nMessage with ID {:?} not found in DBC", message_id);
    }

    // end of tests

    // CAN configuration
    let ws_config = ws::Usb2CanConfiguration::new(can_baud_rate)
        .set_loopback(false)
        .set_silent(false);  // Enable receiving frames from the bus

    // Initialize connection with receive timeout
    let mut device = ws::sync::new(&device_port, &ws_config)
        // .set_serial_receive_timeout(Duration::from_millis(10000))
        .open()?;

    println!("Starting to receive CAN frames... (Press Ctrl+C to stop)");

    loop {
        match device.receive() {
            Ok(frame) => {
                println!("Frame: ID={:?}, Data={:?}", frame.id(), frame.data());
                // ID dostajemy w decimalu i w DBC jest tak samo (a na CANie w hex, ale nevemind)
            }
            
            Err(ws::sync::Error::SerialReadTimedOut) => {
                // Timeout - normal case, continue listening
                println!("Timeout - no frame received, continuing...");
                continue;
            }
            
            Err(e) => {
                eprintln!("Error receiving frame: {}", e);
                // thread::sleep(Duration::from_millis(100));
            }
        }
    }
}
