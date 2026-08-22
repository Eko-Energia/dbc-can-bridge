[🇵🇱 Wersja polska](readme.pl.md)

# CAN-Bridge: CAN Receiver and Decoder 📡

![Rust](https://img.shields.io/badge/Rust-2024-orange)
![License](https://img.shields.io/badge/License-MIT-blue)

Real-time application for receiving, decoding, and monitoring CAN frames. Supports integration with web applications through a WebSocket API.

## Supported Platforms

| Architecture | Interface | Systems |
|---|---|---|
| **x86-64** | Waveshare USB-CAN-A | Windows, macOS, Linux |
| **ARM64** | SocketCAN | Raspberry Pi, NVIDIA Jetson, Linux |

## Key Features

- **Automatic CAN decoding**: Full DBC format support
- **Error code mapping**: Error frame codes translated into readable names from a CSV file
- **Real-time processing**: Continuous CAN frame reception and processing
- **WebSocket API**: Remote access to live data
- **Snapshot + Delta updates**: Optimized data transfer
- **Message filtering**: Subscription-based filtering
- **Configuration**: Automatic `config.txt` generation

## Detailed Features

1. **Configuration file creation**: The program creates a configuration file and loads settings from it.
2. **DBC file loading**: Loads and parses a `.dbc` file located in the same directory as the executable.
3. **Error map loading**: Loads a `.csv` file from the same directory, mapping error codes to readable names - see [Error Code Mapping](#error-code-mapping).
4. **CAN frame reading**: Reads CAN frames and decodes them into physical values using the DBC file. Empty frames (0 data bytes) are accepted; frames longer than 8 bytes are rejected.
5. **WebSocket API**: Exposes CAN data via WebSocket on port 8080 with snapshot + delta updates and message filtering.

## First Run

1. **Download the executable**: From [Releases](https://github.com/Eko-Energia/Perla-Monitor/releases), download the binary for your system.
2. **Place it in a new directory** and run:
   - **Windows**:
     ```bash
     .\can-receiver.exe
     ```
     (for example in cmd, PowerShell, or Windows Terminal)
   - **Linux**:
     ```bash
     # grant execute permissions once
     chmod u+r+w+x can-receiver

     ./can-receiver
     ```
     (in terminal)

3. **Port configuration**: After startup, a `config.txt` file is created. Set `device_port=` after connecting the USB CAN dongle:
   - **Windows**:
     - Open Device Manager, expand "Ports (COM & LPT)", find the Waveshare USB CAN port, then set it in config, for example:
     ```plaintext
     device_port=COM4
     ```
   - **Linux**:
     - Run in terminal:
     ```bash
     ls /dev/ttyUSB*
     ```
     - Then try each available device (for example `/dev/ttyUSB0`) until it works.

4. **Place the DBC file**: Put the DBC file in the same directory as the executable.
5. **Place the error map (optional)**: Put the `.csv` file with error codes in the same directory - see [Error Code Mapping](#error-code-mapping). Without it the program runs normally, only without error name mapping.
6. **Run the program**: Use the same command as above.

## Configuration Options

The `config.txt` file (created on first run, see step 3 above) supports the following keys:

- `device_port` - path to the CAN device
- `save_logs` - `true`/`false`, default `true` - save logs to a file
- `can_baud_rate` - CAN bus speed (Waveshare only), default `500k`
- `broadcast_raw_frames` - `true`/`false`, default `true` - broadcast raw CAN frames over WebSocket, see [WEBSOCKET_API.md](WEBSOCKET_API.md)

## Error Code Mapping

Error frames get their numeric code translated into a readable name, taken from a CSV file placed next to the executable.

### Error map file (CSV)

The first `.csv` file found in the executable's directory is used. The format is one header line (skipped, so its content does not matter) followed by one `code,name` row per error:

```csv
Error Code,Name
33040,CAN overrun
33041,CAN error passive
33296,Motor overtemperature
```

- **Codes must be decimal** - hex notation such as `0x8110` is not accepted; write it as `33040`.
- Whitespace around the code and the name is trimmed.
- Only the first comma separates the fields, so a name may itself contain commas.
- A repeated code overwrites the previous entry.
- A single unparsable code aborts loading of the whole file and disables mapping - lines without any comma are simply skipped.

### What counts as an error frame

A DBC message is treated as an error frame when its **name ends with** `_NODE` or `_EMCY` - for example `MOTOR_EMCY` or `BMS_NODE`. The suffix has to be at the end, so `EMCY_MOTOR` is an ordinary message. The list of suffixes is the `ERROR_SUFFIXES` constant in [src/integration/dbc_handler.rs](src/integration/dbc_handler.rs) and can be changed or extended in code (requires a rebuild).

### Which signal holds the code

By convention it is always the **first signal** of the error frame. Its decoded value is looked up in the CSV and the matching name is returned in that signal's `unit` field (also over the WebSocket API), in place of the DBC unit. The remaining signals of the frame are decoded normally, with their own units.

If the code is not present in the CSV, the DBC unit is used instead. The number of loaded entries is printed at startup.

Mapping is best-effort and the `.csv` file is optional: a missing file and a malformed file are handled the same way - a warning is logged, mapping stays off, and decoding keeps working.

## Running on ARM64 (with socketcan)

SocketCAN requires connecting and configuring a compatible module. Below is an example for `MCP2515` on Raspberry Pi 4B:

1. Connect the module and enable SPI using [this guide](https://github.com/tolgakarakurt/CANBus-MCP2515-Raspi?tab=readme-ov-file).
2. Run:
    ```
    sudo nano /boot/firmware/config.txt
    ```
    and append to the end of the file:
    ```
    dtoverlay=mcp2515-can0,oscillator=8000000,interrupt=25
    dtoverlay=spi-dma
    ```
3. Reboot.
4. Install dependencies:
    ```
    sudo apt install autoconf autogen
    sudo apt install libtool
    sudo apt install can-utils
    ```
5. Check whether the CAN interface is visible (for example as `can0`):
    ```
    ls /sys/bus/spi/devices/spi0.0/net
    ```

    Use that value as `device_port=` in `config.txt`.

6. Configure socketcan:
    ```
    sudo ip link set can0 up type can bitrate 500000
    ```

    Note: this command must be run after every reboot.
7. Start the program normally.

---

## WebSocket API

The program automatically starts a WebSocket server at `ws://0.0.0.0:8080`, providing remote access to CAN data in real time.

### Quick start

1. Open `tools/websocket-test-client.html` in your browser
2. Click "Connect" - it will connect automatically and fetch all data
3. Observe real-time CAN updates

For the raw CAN frame stream (`broadcast_raw_frames`, enabled by default) use `tools/raw-frames-test-client.html` instead.

### Capabilities

- **Snapshot + Delta**: First receive full state, then only changes
- **Filtering**: Subscribe only to selected CAN messages (for example `BMS_Status, Motor_Speed`)
- **Multiple connections**: Supports many concurrent clients

API details and multi-language examples: [WEBSOCKET_API.md](WEBSOCKET_API.md)

---

## Build from Source

### Standard

```bash
cargo build --release
```

### Cross-compile for ARM64 (for example Raspberry Pi)
```bash
sudo apt install zig
cargo install cargo-zigbuild
rustup target add aarch64-unknown-linux-gnu
```
```bash
cargo zigbuild --target aarch64-unknown-linux-gnu --release
```

---

## Documentation

- **WebSocket API**: [WEBSOCKET_API.md](WEBSOCKET_API.md)
- **CAN Simulator**: [tools/vcan-sim/readme.md](tools/vcan-sim/readme.md)
- **WebSocket test clients**: [tools/websocket-test-client.html](tools/websocket-test-client.html) (decoded messages), [tools/raw-frames-test-client.html](tools/raw-frames-test-client.html) (raw frames)

---

## License

MIT License - see [LICENSE](LICENSE)