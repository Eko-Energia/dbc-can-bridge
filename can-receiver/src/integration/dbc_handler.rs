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
            return Err(eyre!("Error: Frame ID: {:?} is either empty or data exceeds 8 bytes!", frame.id()));
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
mod tests {
    use super::*;
    use color_eyre::Result;
    use embedded_can::{Frame as FrameTrait, Id};
    use waveshare_usb_can_a::Frame;

    #[test]
    #[ignore] // Requires actual DBC file in exe directory
    fn decode_two_sample_frames() -> Result<()> {
        // test with the CAN_DB file
        // to see print in tests:
        // cargo test -- --show-output
        let dbc = DbcHandler::new()?;
        println!("DBC loaded: {} message definitions available\n", dbc.dbc.messages.len());

        let frame = Frame::new(
            Id::Standard(embedded_can::StandardId::new(130).unwrap()), &[231, 49, 0, 0, 223, 11]).unwrap();

        let (msg_name, signals) = dbc.decode(frame)?;
        println!("{}:", msg_name);
        signals.iter().for_each(
            |s| println!("  {}: {} {}", s.name, s.value, s.unit));

        let frame1 = Frame::new(
            Id::Standard(embedded_can::StandardId::new(139).unwrap()), &[231, 49, 4, 3, 223, 11, 6]).unwrap();

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

    // Error case tests
    
    #[test]
    fn test_decode_signal_value_zero_size() {
        let data = [0x12, 0x34, 0x56, 0x78];
        let result = decode_signal_value(0, 0, ByteOrder::LittleEndian, ValueType::Unsigned, &data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid signal size: 0"));
    }

    #[test]
    fn test_decode_signal_value_size_exceeds_64() {
        let data = [0x12, 0x34, 0x56, 0x78];
        let result = decode_signal_value(0, 65, ByteOrder::LittleEndian, ValueType::Unsigned, &data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid signal size: 65"));
    }

    #[test]
    fn test_extract_signal_value_little_endian_insufficient_data() {
        let data = [0x12, 0x34];
        // Try to extract 24 bits starting at bit 8, which would need 4 bytes total
        let result = extract_signal_value(&data, 8, 24, ByteOrder::LittleEndian);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Not enough data to decode little-endian signal"));
    }

    #[test]
    fn test_extract_signal_value_big_endian_insufficient_data() {
        let data = [0x12, 0x34];
        // Try to extract signal that requires more bytes than available
        // Start at bit 7 (MSB of first byte), size 24 bits would need 3 bytes total
        let result = extract_signal_value(&data, 7, 24, ByteOrder::BigEndian);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Not enough data to decode big-endian signal"));
    }

    #[test]
    fn test_decode_empty_frame() {
        // Create a minimal DBC for testing
        let dbc_content = r#"
VERSION ""

NS_ :

BS_:

BO_ 100 TestMessage: 8 Vector__XXX
"#;
        let dbc = Dbc::try_from(dbc_content).unwrap();
        let map: HashMap<u32, usize> = dbc
            .messages
            .iter()
            .enumerate()
            .map(|(i, msg)| (msg.id.raw(), i))
            .collect();
        
        let handler = DbcHandler {
            dbc,
            message_index_by_id: map
        };

        // Create frame with empty data
        let frame = Frame::new(
            Id::Standard(embedded_can::StandardId::new(100).unwrap()), 
            &[]
        ).unwrap();

        let result = handler.decode(frame);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("is either empty or data exceeds 8 bytes"));
    }

    #[test]
    fn test_decode_frame_data_exceeds_8_bytes() {
        // Note: The Frame::new API prevents creating frames with more than 8 bytes,
        // so this error case is actually prevented at the Frame creation level.
        // This test verifies that Frame::new properly rejects invalid data lengths.
        
        // Attempt to create frame with 9 bytes should fail
        let result = Frame::new(
            Id::Standard(embedded_can::StandardId::new(100).unwrap()), 
            &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09]
        );
        
        // Frame creation should fail, protecting our decode function
        assert!(result.is_none());
    }

    #[test]
    fn test_decode_no_message_definition_found() {
        // Create a minimal DBC with a different message ID
        let dbc_content = r#"
VERSION ""

NS_ :

BS_:

BO_ 100 TestMessage: 8 Vector__XXX
"#;
        let dbc = Dbc::try_from(dbc_content).unwrap();
        let map: HashMap<u32, usize> = dbc
            .messages
            .iter()
            .enumerate()
            .map(|(i, msg)| (msg.id.raw(), i))
            .collect();
        
        let handler = DbcHandler {
            dbc,
            message_index_by_id: map
        };

        // Create frame with ID 200 which doesn't exist in DBC
        let frame = Frame::new(
            Id::Standard(embedded_can::StandardId::new(200).unwrap()), 
            &[0x01, 0x02, 0x03, 0x04]
        ).unwrap();

        let result = handler.decode(frame);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No message definition found for frame ID"));
    }

    #[test]
    fn test_decode_signal_value_signed_positive() {
        let data = [0b00001111]; // 15 in binary
        let result = decode_signal_value(0, 8, ByteOrder::LittleEndian, ValueType::Signed, &data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 15.0);
    }

    #[test]
    fn test_decode_signal_value_signed_negative() {
        let data = [0b11111111]; // -1 in 8-bit two's complement
        let result = decode_signal_value(0, 8, ByteOrder::LittleEndian, ValueType::Signed, &data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), -1.0);
    }

    #[test]
    fn test_decode_signal_value_unsigned() {
        let data = [0xFF]; // 255 in unsigned
        let result = decode_signal_value(0, 8, ByteOrder::LittleEndian, ValueType::Unsigned, &data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 255.0);
    }

    #[test]
    fn test_extract_signal_value_little_endian_basic() {
        let data = [0x12, 0x34, 0x56, 0x78];
        let result = extract_signal_value(&data, 0, 8, ByteOrder::LittleEndian);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0x12);
    }

    #[test]
    fn test_extract_signal_value_big_endian_basic() {
        let data = [0x12, 0x34, 0x56, 0x78];
        let result = extract_signal_value(&data, 7, 8, ByteOrder::BigEndian);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0x12);
    }

    #[test]
    fn test_decode_with_valid_signal() {
        // Create a DBC with a signal
        let dbc_content = r#"
VERSION ""

NS_ :

BS_:

BO_ 100 TestMessage: 8 Vector__XXX
 SG_ TestSignal : 0|8@1+ (1,0) [0|255] "" Vector__XXX
"#;
        let dbc = Dbc::try_from(dbc_content).unwrap();
        let map: HashMap<u32, usize> = dbc
            .messages
            .iter()
            .enumerate()
            .map(|(i, msg)| (msg.id.raw(), i))
            .collect();
        
        let handler = DbcHandler {
            dbc,
            message_index_by_id: map
        };

        // Create a valid frame
        let frame = Frame::new(
            Id::Standard(embedded_can::StandardId::new(100).unwrap()), 
            &[0x42, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
        ).unwrap();

        let result = handler.decode(frame);
        assert!(result.is_ok());
        let (msg_name, signals) = result.unwrap();
        assert_eq!(msg_name, "TestMessage");
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].name, "TestSignal");
        assert_eq!(signals[0].value, 66.0); // 0x42 = 66
    }

    #[test]
    fn test_extract_signal_value_little_endian_multi_byte() {
        // Test extracting a 16-bit value across two bytes
        let data = [0x34, 0x12]; // Little-endian 0x1234
        let result = extract_signal_value(&data, 0, 16, ByteOrder::LittleEndian);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0x1234);
    }

    #[test]
    fn test_extract_signal_value_little_endian_partial_byte() {
        // Test extracting 4 bits from middle of a byte
        let data = [0b11110000];
        let result = extract_signal_value(&data, 4, 4, ByteOrder::LittleEndian);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0b1111);
    }

    #[test]
    fn test_decode_signal_value_64_bit_signed() {
        // Test the edge case of 64-bit signed value
        let data = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let result = decode_signal_value(0, 64, ByteOrder::LittleEndian, ValueType::Signed, &data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), -1.0);
    }

    #[test]
    fn test_decode_message_index_out_of_bounds() {
        // Create a handler with corrupted internal state
        let dbc_content = r#"
VERSION ""

NS_ :

BS_:

BO_ 100 TestMessage: 8 Vector__XXX
"#;
        let dbc = Dbc::try_from(dbc_content).unwrap();
        
        // Create a map with an invalid index
        let mut map: HashMap<u32, usize> = HashMap::new();
        map.insert(100, 999); // Index 999 is out of bounds
        
        let handler = DbcHandler {
            dbc,
            message_index_by_id: map
        };

        // Create a valid frame
        let frame = Frame::new(
            Id::Standard(embedded_can::StandardId::new(100).unwrap()), 
            &[0x01, 0x02, 0x03, 0x04]
        ).unwrap();

        let result = handler.decode(frame);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Message index 999 out of bounds"));
    }

    #[test]
    fn test_extract_signal_value_little_endian_edge_of_data() {
        // Test extracting signal that ends exactly at data boundary
        let data = [0xFF, 0xFF];
        let result = extract_signal_value(&data, 0, 16, ByteOrder::LittleEndian);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0xFFFF);
    }

    #[test]
    fn test_extract_signal_value_big_endian_single_bit() {
        // Test extracting a single bit in big-endian
        let data = [0b10000000]; // MSB set
        let result = extract_signal_value(&data, 7, 1, ByteOrder::BigEndian);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[test]
    fn test_decode_signal_value_signed_16_bit_negative() {
        // Test 16-bit signed negative value
        let data = [0xFF, 0xFF]; // -1 in 16-bit two's complement
        let result = decode_signal_value(0, 16, ByteOrder::LittleEndian, ValueType::Signed, &data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), -1.0);
    }

    #[test]
    fn test_decode_signal_value_signed_16_bit_positive() {
        // Test 16-bit signed positive value (max positive)
        let data = [0xFF, 0x7F]; // 32767 in 16-bit two's complement
        let result = decode_signal_value(0, 16, ByteOrder::LittleEndian, ValueType::Signed, &data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 32767.0);
    }

    #[test]
    fn test_extract_signal_value_little_endian_crossing_bytes() {
        // Test signal that crosses byte boundaries
        let data = [0x00, 0xFF, 0x00];
        // Extract middle byte (8 bits starting at bit 8)
        let result = extract_signal_value(&data, 8, 8, ByteOrder::LittleEndian);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0xFF);
    }

    #[test]
    fn test_extract_signal_value_big_endian_multi_byte() {
        // Test extracting multiple bytes in big-endian
        let data = [0x12, 0x34, 0x56, 0x78];
        // Extract 16 bits starting from bit 7 (MSB of first byte)
        let result = extract_signal_value(&data, 7, 16, ByteOrder::BigEndian);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0x1234);
    }

    #[test]
    fn test_id_to_u32_standard() {
        let id = Id::Standard(embedded_can::StandardId::new(0x123).unwrap());
        let result = id_to_u32(&id);
        assert_eq!(result, 0x123);
    }

    #[test]
    fn test_id_to_u32_extended() {
        let id = Id::Extended(embedded_can::ExtendedId::new(0x12345).unwrap());
        let result = id_to_u32(&id);
        // Extended ID should have bit 31 set
        assert_eq!(result, 0x12345 | (1 << 31));
    }

    #[test]
    fn test_decode_with_multiple_signals() {
        // Create a DBC with multiple signals
        let dbc_content = r#"
VERSION ""

NS_ :

BS_:

BO_ 100 TestMessage: 8 Vector__XXX
 SG_ Signal1 : 0|8@1+ (1,0) [0|255] "" Vector__XXX
 SG_ Signal2 : 8|8@1+ (1,0) [0|255] "" Vector__XXX
 SG_ Signal3 : 16|16@1+ (0.1,0) [0|6553.5] "" Vector__XXX
"#;
        let dbc = Dbc::try_from(dbc_content).unwrap();
        let map: HashMap<u32, usize> = dbc
            .messages
            .iter()
            .enumerate()
            .map(|(i, msg)| (msg.id.raw(), i))
            .collect();
        
        let handler = DbcHandler {
            dbc,
            message_index_by_id: map
        };

        // Create a frame with data for all signals
        let frame = Frame::new(
            Id::Standard(embedded_can::StandardId::new(100).unwrap()), 
            &[0x10, 0x20, 0xE8, 0x03, 0x00, 0x00, 0x00, 0x00]
        ).unwrap();

        let result = handler.decode(frame);
        assert!(result.is_ok());
        let (msg_name, signals) = result.unwrap();
        assert_eq!(msg_name, "TestMessage");
        assert_eq!(signals.len(), 3);
        assert_eq!(signals[0].value, 16.0); // 0x10
        assert_eq!(signals[1].value, 32.0); // 0x20
        assert_eq!(signals[2].value, 100.0); // 0x03E8 * 0.1 = 100.0
    }

    #[test]
    fn test_decode_signal_with_offset_and_factor() {
        // Test signal decoding with factor and offset
        let dbc_content = r#"
VERSION ""

NS_ :

BS_:

BO_ 100 TestMessage: 8 Vector__XXX
 SG_ Temperature : 0|16@1+ (0.1,-40) [-40|615.5] "°C" Vector__XXX
"#;
        let dbc = Dbc::try_from(dbc_content).unwrap();
        let map: HashMap<u32, usize> = dbc
            .messages
            .iter()
            .enumerate()
            .map(|(i, msg)| (msg.id.raw(), i))
            .collect();
        
        let handler = DbcHandler {
            dbc,
            message_index_by_id: map
        };

        // Raw value 400 -> 400 * 0.1 - 40 = 0°C
        let frame = Frame::new(
            Id::Standard(embedded_can::StandardId::new(100).unwrap()), 
            &[0x90, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
        ).unwrap();

        let result = handler.decode(frame);
        assert!(result.is_ok());
        let (_, signals) = result.unwrap();
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].name, "Temperature");
        assert_eq!(signals[0].unit, "°C");
        assert_eq!(signals[0].value, 0.0); // 400 * 0.1 - 40 = 0
    }
}