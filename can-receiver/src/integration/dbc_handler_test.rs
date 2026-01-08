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

#[test]
fn test_decode_big_endian_signal() {
    // Test with big-endian (Motorola) byte order using @0
    let dbc_content = r#"
VERSION ""

NS_ :

BS_:

BO_ 100 TestMessage: 8 Vector__XXX
 SG_ BigEndianSignal : 7|8@0+ (1,0) [0|255] "" Vector__XXX
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

    // Create a frame with data
    let frame = Frame::new(
        Id::Standard(embedded_can::StandardId::new(100).unwrap()), 
        &[0x42, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
    ).unwrap();

    let result = handler.decode(frame);
    assert!(result.is_ok());
    let (msg_name, signals) = result.unwrap();
    assert_eq!(msg_name, "TestMessage");
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].name, "BigEndianSignal");
    assert_eq!(signals[0].value, 66.0); // 0x42 = 66
}

#[test]
fn test_decode_big_endian_16bit_signal() {
    // Test with 16-bit big-endian signal
    let dbc_content = r#"
VERSION ""

NS_ :

BS_:

BO_ 100 TestMessage: 8 Vector__XXX
 SG_ BigEndian16 : 7|16@0+ (1,0) [0|65535] "" Vector__XXX
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

    // Create a frame with 16-bit value 0x1234 in big-endian
    let frame = Frame::new(
        Id::Standard(embedded_can::StandardId::new(100).unwrap()), 
        &[0x12, 0x34, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
    ).unwrap();

    let result = handler.decode(frame);
    assert!(result.is_ok());
    let (_, signals) = result.unwrap();
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].value, 0x1234 as f64); // 4660 in decimal
}

#[test]
fn test_decode_big_endian_signal_with_factor() {
    // Test big-endian signal with factor and offset
    let dbc_content = r#"
VERSION ""

NS_ :

BS_:

BO_ 100 TestMessage: 8 Vector__XXX
 SG_ Speed : 7|16@0+ (0.01,0) [0|655.35] "km/h" Vector__XXX
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

    // Raw value 10000 -> 10000 * 0.01 = 100.0 km/h
    // 10000 = 0x2710 in big-endian: 0x27, 0x10
    let frame = Frame::new(
        Id::Standard(embedded_can::StandardId::new(100).unwrap()), 
        &[0x27, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
    ).unwrap();

    let result = handler.decode(frame);
    assert!(result.is_ok());
    let (_, signals) = result.unwrap();
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].name, "Speed");
    assert_eq!(signals[0].unit, "km/h");
    assert_eq!(signals[0].value, 100.0);
}

#[test]
fn test_decode_mixed_endianness_signals() {
    // Test message with both little-endian and big-endian signals
    let dbc_content = r#"
VERSION ""

NS_ :

BS_:

BO_ 100 TestMessage: 8 Vector__XXX
 SG_ LittleEndianSignal : 0|8@1+ (1,0) [0|255] "" Vector__XXX
 SG_ BigEndianSignal : 15|8@0+ (1,0) [0|255] "" Vector__XXX
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

    // Byte 0: 0xAA (little-endian signal)
    // Byte 1: 0xBB (big-endian signal)
    let frame = Frame::new(
        Id::Standard(embedded_can::StandardId::new(100).unwrap()), 
        &[0xAA, 0xBB, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
    ).unwrap();

    let result = handler.decode(frame);
    assert!(result.is_ok());
    let (msg_name, signals) = result.unwrap();
    assert_eq!(msg_name, "TestMessage");
    assert_eq!(signals.len(), 2);
    assert_eq!(signals[0].name, "LittleEndianSignal");
    assert_eq!(signals[0].value, 0xAA as f64); // 170
    assert_eq!(signals[1].name, "BigEndianSignal");
    assert_eq!(signals[1].value, 0xBB as f64); // 187
}

#[test]
fn test_decode_big_endian_signed_signal() {
    // Test big-endian signed signal using @0-
    let dbc_content = r#"
VERSION ""

NS_ :

BS_:

BO_ 100 TestMessage: 8 Vector__XXX
 SG_ SignedBigEndian : 7|16@0- (1,0) [-32768|32767] "" Vector__XXX
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

    // Test negative value: -1 in 16-bit two's complement = 0xFFFF
    let frame = Frame::new(
        Id::Standard(embedded_can::StandardId::new(100).unwrap()), 
        &[0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
    ).unwrap();

    let result = handler.decode(frame);
    assert!(result.is_ok());
    let (_, signals) = result.unwrap();
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].value, -1.0);
}

#[test]
fn test_decode_big_endian_32bit_signal() {
    // Test 32-bit big-endian signal
    let dbc_content = r#"
VERSION ""

NS_ :

BS_:

BO_ 100 TestMessage: 8 Vector__XXX
 SG_ BigEndian32 : 7|32@0+ (1,0) [0|4294967295] "" Vector__XXX
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

    // Create a frame with 32-bit value 0x12345678 in big-endian
    let frame = Frame::new(
        Id::Standard(embedded_can::StandardId::new(100).unwrap()), 
        &[0x12, 0x34, 0x56, 0x78, 0x00, 0x00, 0x00, 0x00]
    ).unwrap();

    let result = handler.decode(frame);
    assert!(result.is_ok());
    let (_, signals) = result.unwrap();
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].value, 0x12345678 as f64); // 305419896 in decimal
}
