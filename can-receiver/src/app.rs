use std::collections::HashMap;
use std::time::Duration;

use color_eyre::eyre::Result;
use jiff::Timestamp;
use embedded_can::blocking::Can;
use waveshare_usb_can_a::sync::Usb2Can;
use waveshare_usb_can_a::{self as ws};
use crate::setup::config;

use crate::integration::dbc_handler::{DbcHandler, SignalValue};

// lifetime specifiers and borrows can possibly break actix web
// if so I'll make copies instead
pub struct App<'a> {
    dbc_handler: DbcHandler,
    device: Usb2Can,
    pub data_map: HashMap<&'a String, MapEntry<'a>>
}

pub struct MapEntry<'a> {
    pub signals: Vec<SignalValue<'a>>,
    pub timestamp: Timestamp,
}

impl<'a> App<'a> {
    pub fn new() -> Result<Self> {
        // Initialize DBC decoding
        let dbc_handler = DbcHandler::new()?;

        println!("DBC loaded: {} message definitions available", dbc_handler.dbc.messages.len());
        
        // Get settings from configuration
        let device_port = config::get_device_port()?;
        let can_baud_rate = config::get_can_baud_rate()?;
        
        println!("Using device: {}", device_port);
        println!("CAN speed: {:?}", can_baud_rate);

        // CAN configuration
        let ws_config = ws::Usb2CanConfiguration::new(can_baud_rate)
            .set_loopback(false)
            .set_silent(false);  // Enable receiving frames from the bus

        // Initialize connection
        let device = ws::sync::new(&device_port, &ws_config)
            // not really infinite timeout
            .set_serial_receive_timeout(Duration::from_secs(60 * 60 * 24))
            .open()?;

        Ok(Self {
            dbc_handler,
            device,
            data_map: HashMap::new() })
    }

    pub fn run(&'a mut self) -> Result<()> {
        println!("Starting to receive CAN frames... (Press Ctrl+C to stop)");
        loop {
            match self.device.receive() {
                Ok(frame) => {
                    match self.dbc_handler.decode(frame) {
                        Ok((msg_name, signals)) => {
                            // temporary print
                            println!("{}:", msg_name);
                            signals.iter().for_each(
                                |s| println!("  {}: {} {}", s.name, s.value, s.unit));
                            // end of temporary print
                            // push decoded frame into a map
                            self.data_map.insert(
                                msg_name,
                                MapEntry { signals, timestamp: Timestamp::now() }
                            );
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
}