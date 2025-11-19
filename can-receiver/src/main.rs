mod setup;
mod integration;

use std::time::Duration;

use color_eyre::eyre::Result;
use embedded_can::blocking::Can;
use waveshare_usb_can_a::{self as ws};
use setup::config;

use crate::integration::dbc_handler::DbcHandler;

fn main() -> Result<()> {
    color_eyre::install()?;

    // Initialize configuration
    config::init_config()?;
    
    // Get settings from configuration
    let device_port = config::get_device_port()?;
    let can_baud_rate = config::get_can_baud_rate()?;
    
    println!("Using device: {}", device_port);
    println!("CAN speed: {:?}", can_baud_rate);

    // Initialize DBC decoding
    let dbc = DbcHandler::new()?;

    println!("DBC loaded: {} message definitions available", dbc.dbc.messages.len());

    // CAN configuration
    let ws_config = ws::Usb2CanConfiguration::new(can_baud_rate)
        .set_loopback(false)
        .set_silent(false);  // Enable receiving frames from the bus

    // Initialize connection
    let mut device = ws::sync::new(&device_port, &ws_config)
        // not really infinite timeout
        .set_serial_receive_timeout(Duration::from_secs(60 * 60 * 24 * 365 * 100))
        .open()?;

    println!("Starting to receive CAN frames... (Press Ctrl+C to stop)");

    loop {
        match device.receive() {
            Ok(frame) => {
                match dbc.decode(frame) {
                    Ok((msg_name, signals)) => {
                        println!("{}:", msg_name);
                        signals.iter().for_each(
                            |s| println!("  {}: {} {}", s.name, s.value, s.unit));                       
                    }
                    
                    Err(e) => {
                        eprintln!("Error decoding frame: {}", e);
                    }
                }
            }
            
            Err(ws::sync::Error::SerialReadTimedOut) => {
                println!("Timeout - no frame received, continuing...");
                continue;
            }
            
            Err(e) => {
                eprintln!("Error receiving frame: {}", e);
            }
        }
    }
}