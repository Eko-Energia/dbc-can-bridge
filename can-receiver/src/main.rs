mod config;

use color_eyre::eyre::Result;
use embedded_can::{blocking::Can, Frame as FrameTrait, Id};
use waveshare_usb_can_a as ws;

fn id_to_u32(id: &Id) -> u32 {
    match id {
        Id::Standard(sid) => sid.as_raw() as u32,
        Id::Extended(eid) => eid.as_raw(),
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
