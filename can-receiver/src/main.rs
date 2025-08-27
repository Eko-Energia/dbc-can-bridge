use color_eyre::eyre::Result;
use embedded_can::{StandardId, Id, Frame as FrameTrait, blocking::Can};
use std::{time::Duration, thread};
use waveshare_usb_can_a as ws;

fn main() -> Result<()> {
    color_eyre::install()?;

    // CAN configuration
    let config = ws::Usb2CanConfiguration::new(ws::CanBaudRate::R500kBd)
        .set_loopback(false)
        .set_silent(false)
        .set_automatic_retransmission(true);

    // Initialize connection - using builder pattern
    let mut device = ws::sync::new("/dev/ttyUSB0", &config)
        .open()?;

    println!("Starting to send CAN frames...");

    let mut counter: u32 = 0;
    loop {
        // Create a CAN frame with ID 0x123 and data
        let data = [
            (counter >> 24) as u8,
            (counter >> 16) as u8,
            (counter >> 8) as u8,
            counter as u8,
            0xAA,
            0xBB,
            0xCC,
            0xDD,
        ];

        let frame = ws::Frame::new(
            Id::Standard(StandardId::new(0x123).unwrap()),
            &data
        ).unwrap();

        // Send the frame
        match device.transmit(&frame) {
            Ok(_) => {
                println!("Sent frame #{}: {}", counter, frame);
            }
            Err(e) => {
                eprintln!("Error while sending frame: {}", e);
            }
        }

        counter = counter.wrapping_add(1);
        
        // 100ms delay between frames
        thread::sleep(Duration::from_millis(100));
    }
}
