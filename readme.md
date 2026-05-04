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
- **Real-time processing**: Continuous CAN frame reception and processing
- **WebSocket API**: Remote access to live data
- **Snapshot + Delta updates**: Optimized data transfer
- **Message filtering**: Subscription-based filtering
- **Configuration**: Automatic `config.txt` generation

## Detailed Features

1. **Configuration file creation**: The program creates a configuration file and loads settings from it.
2. **DBC file loading**: Loads and parses a `.dbc` file located in the same directory as the executable.
3. **CAN frame reading**: Reads CAN frames and decodes them into physical values using the DBC file.
4. **WebSocket API**: Exposes CAN data via WebSocket on port 8080 with snapshot + delta updates and message filtering.

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
5. **Run the program**: Use the same command as above.

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

1. Open `websocket-test-client.html` in your browser
2. Click "Connect" - it will connect automatically and fetch all data
3. Observe real-time CAN updates

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
- **CAN Simulator**: [vcan-sim/readme.md](vcan-sim/readme.md)

---

## License

MIT License - see [LICENSE](LICENSE)