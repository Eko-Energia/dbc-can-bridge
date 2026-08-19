[🇵🇱 Wersja polska](readme.pl.md)

# Perla Bus Simulator

CAN bus simulator sending random frames according to definitions from a DBC file.

Works only on Linux!

**Available versions:**
- `simulator_serial.py` - Serial device simulator (emulates /dev/ttyUSB0)

## Setup

### 1. Installing Dependencies

```bash
# Create Python virtual environment
python3 -m venv .venv

# Activate environment
source .venv/bin/activate

# Install dependencies
pip install -r requirements.txt
```

**Note:** Remember to always activate the environment before running simulators:
```bash
source .venv/bin/activate
```

## Usage

#### Basic usage

```bash

# Creates a virtual serial port (e.g., /dev/pts/4)
python simulator_serial.py

# Use a configuration file
python simulator_serial.py --config simulator_config.py
```

After startup, the simulator will display the path to the virtual port and the stable path (symlink), e.g.:
```
✓ Virtual serial port created: /dev/pts/4
✓ Stable device path: /tmp/perla-bus-tty
  Use this port in can-receiver config: device_port=/tmp/perla-bus-tty
```

#### Configuring can-receiver

Edit `can-receiver/config.txt` and set the stable path:
```
device_port=/tmp/perla-bus-tty
can_baud_rate=500k
```

Then run can-receiver:
```bash
cd ../can-receiver
cargo run
```

#### Serial simulator options

```bash
# Use your own DBC file
./run_simulator.sh --dbc my_file.dbc
# or
python simulator_serial.py --dbc my_file.dbc

# Simulate for a specific time (30 seconds)
python simulator_serial.py --duration 30

# Send each message once and exit
python simulator_serial.py --single-cycle

# Use a specific serial port instead of PTY
python simulator_serial.py --port /dev/ttyUSB1

# Speed up simulation 2x (cycles 2x shorter)
python simulator_serial.py --cycle-multiplier 0.5

# Slow down simulation 2x (cycles 2x longer)
python simulator_serial.py --cycle-multiplier 2.0

# Realistic mode - gradual value changes
python simulator_serial.py --realistic

# Use configuration from file
python simulator_serial.py --config simulator_config.py

# Help
python simulator_serial.py --help
```

#### Waveshare Protocol Format

The simulator implements the Waveshare USB-CAN-A protocol:
```
[0xAA] [0x55] [0x01] [ID_TYPE] [FRAME_TYPE] [ID0] [ID1] [ID2] [ID3] [DLC] [D0] ... [D7] [0x00] [Checksum]
```
- `ID_TYPE`: `0x01` = standard (11-bit), `0x02` = extended (29-bit)
- `FRAME_TYPE`: `0x01` = data frame
- `ID0..ID3`: CAN ID as `u32` little-endian (for standard ID, the lower 11 bits are used)
- `D0..D7`: data (padded with `0x00` to 8 bytes)
- Byte `0x00` before checksum is a reserved field
- `Checksum`: sum of bytes from `[0x01]` to reserved field (inclusive) modulo 256

## DBC File

The `perla_bus.dbc` file contains definitions of the following messages:

- **MotorStatus (0x80)**: RPM and engine temperature
- **BatteryInfo (0x82)**: Voltage, current, and battery charge state
- **SensorData (0x8B)**: Speed, temperature, pressure, humidity
- **ControlCommands (0x100)**: Control commands
- **DiagnosticInfo (0x200)**: Diagnostic information and errors

Each message has defined:
- Signals with their bit position, length, and byte order
- Value ranges (minimum/maximum)
- Units and scaling factors
- Transmission cycles

## Testing

### Integration with can-receiver

The simulator can be used to test the `can-receiver` application:

```bash
# Terminal 1: Run the simulator
python simulator_serial.py

# Terminal 2: Run can-receiver (after compiling)
cd ../can-receiver
cargo run
```

## Message Structure

### MotorStatus (ID: 128 / 0x80)
- Length: 3 bytes
- Cycle: 100ms
- Signals:
  - `MotorRPM`: 16-bit, 0-8000 rpm
  - `MotorTemp`: 8-bit, -40 to 215°C

### BatteryInfo (ID: 130 / 0x82)
- Length: 6 bytes
- Cycle: 200ms
- Signals:
  - `Voltage`: 16-bit, 0-655.35V (scale 0.01)
  - `Current`: 16-bit signed, -3276.8 to 3276.7A (scale 0.1)
  - `StateOfCharge`: 8-bit, 0-100% (scale 0.5)

### SensorData (ID: 139 / 0x8B)
- Length: 7 bytes
- Cycle: 50ms
- Signals:
  - `Speed`: 16-bit, 0-300 km/h (scale 0.01)
  - `AmbientTemp`: 8-bit, -40 to 100°C
  - `Pressure`: 16-bit, 0-6553.5 kPa (scale 0.1)
  - `Humidity`: 8-bit, 0-100% (scale 0.5)