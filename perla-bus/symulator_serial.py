#!/usr/bin/env python3
"""
Serial CAN Simulator for Perla Monitor
Emulates Waveshare USB-CAN-A device on a virtual serial port (PTY)
Generates random CAN frames based on DBC file definitions
"""

import cantools
import random
import time
import argparse
import sys
import struct
import os
import pty
import select
import logging
import importlib.util
import termios
import tty
from pathlib import Path
from typing import Dict, Any, Optional


FIXED_PTY_SYMLINK_PATH = "/tmp/perla-bus-tty"


class WaveshareProtocol:
    """Implements Waveshare USB-CAN-A protocol (Fixed format)."""

    # Fixed protocol constants
    PROTO_FIXED_HEADER_0 = 0xAA
    PROTO_FIXED_HEADER_1 = 0x55
    PROTO_FIXED_TYPE_FRAME_FLAG = 0x01
    PROTO_FIXED_ID_STANDARD = 0x01
    PROTO_FIXED_ID_EXTENDED = 0x02
    PROTO_FIXED_FRAME_DATA = 0x01
    PROTO_FIXED_FRAME_REMOTE = 0x02
    
    @staticmethod
    def encode_can_frame(frame_id: int, data: bytes, is_extended: bool = False) -> bytes:
        """
        Encode CAN frame in Waveshare USB-CAN-A Fixed format.

        Fixed frame (20 bytes):
        [0]  0xAA
        [1]  0x55
        [2]  0x01                  (frame flag)
        [3]  0x01 / 0x02           (standard / extended ID)
        [4]  0x01 / 0x02           (data / remote)
        [5:9]  CAN ID (u32 LE)
        [9]  DLC (0..8)
        [10:18] Data (8 bytes, padded with 0x00)
        [18] Reserved (0x00)
        [19] Checksum (u8 sum of bytes [2..18])
        """
        dlc = len(data)
        if dlc > 8:
            raise ValueError(f"CAN DLC too large: {dlc} (max 8)")

        packet = bytearray(20)
        packet[0] = WaveshareProtocol.PROTO_FIXED_HEADER_0
        packet[1] = WaveshareProtocol.PROTO_FIXED_HEADER_1
        packet[2] = WaveshareProtocol.PROTO_FIXED_TYPE_FRAME_FLAG
        packet[3] = (
            WaveshareProtocol.PROTO_FIXED_ID_EXTENDED
            if is_extended
            else WaveshareProtocol.PROTO_FIXED_ID_STANDARD
        )
        packet[4] = WaveshareProtocol.PROTO_FIXED_FRAME_DATA

        if is_extended:
            frame_id &= 0x1FFFFFFF
        else:
            frame_id &= 0x7FF

        packet[5:9] = int(frame_id).to_bytes(4, byteorder='little', signed=False)
        packet[9] = dlc
        packet[10 : 10 + dlc] = data
        packet[18] = 0x00

        checksum = 0
        for b in packet[2:19]:
            checksum = (checksum + b) & 0xFF
        packet[19] = checksum

        return bytes(packet)
    
    @staticmethod
    def create_ack() -> bytes:
        """Create ACK response (not implemented)."""
        return bytes([])
class SimulatorConfig:
    """Configuration for simulator"""
    
    def __init__(self, config_file: Optional[str] = None):
        """Load configuration from file or use defaults"""
        # Defaults
        self.dbc_file = "perla_bus.dbc"
        self.serial_port = None
        self.mode = "continuous"
        self.duration = 30.0
        self.cycle_multiplier = 1.0
        self.cycle_variation = 10
        self.realistic_mode = False
        self.smoothing_factor = 0.8
        self.log_level = "warning"
        self.log_frames = False
        self.log_signals = True
        
        if config_file and Path(config_file).exists():
            self._load_from_file(config_file)
    
    def _load_from_file(self, config_file: str):
        """Load configuration from Python file"""
        try:
            # Load Python module dynamically
            spec = importlib.util.spec_from_file_location("config", config_file)
            if spec and spec.loader:
                config = importlib.util.module_from_spec(spec)
                spec.loader.exec_module(config)
                
                # Load configuration variables
                self.dbc_file = getattr(config, 'DBC_FILE', self.dbc_file)
                self.mode = getattr(config, 'MODE', self.mode)
                self.duration = float(getattr(config, 'DURATION', self.duration))
                self.cycle_multiplier = float(getattr(config, 'CYCLE_MULTIPLIER', self.cycle_multiplier))
                self.cycle_variation = int(getattr(config, 'CYCLE_VARIATION', self.cycle_variation))
                self.realistic_mode = getattr(config, 'REALISTIC_MODE', self.realistic_mode)
                self.smoothing_factor = float(getattr(config, 'SMOOTHING_FACTOR', self.smoothing_factor))
                self.log_level = getattr(config, 'LOG_LEVEL', self.log_level)
                self.log_frames = getattr(config, 'LOG_FRAMES', self.log_frames)
                self.log_signals = getattr(config, 'LOG_SIGNALS', self.log_signals)
        except Exception as e:
            print(f"Error loading config: {e}")


class SerialCANSimulator:
    """Simulates CAN bus on virtual serial port"""
    
    def __init__(self, dbc_path: str, serial_port: Optional[str] = None, config: Optional[SimulatorConfig] = None):
        """
        Initialize serial CAN simulator
        
        Args:
            dbc_path: Path to DBC file
            serial_port: Optional path to existing serial port, if None creates PTY
            config: Optional configuration object
        """
        self.config = config or SimulatorConfig()
        self.dbc_path = Path(dbc_path)
        if not self.dbc_path.exists():
            raise FileNotFoundError(f"DBC file not found: {dbc_path}")
        
        # Setup logging
        self._setup_logging()
        
        # Load DBC database
        logging.info(f"Loading DBC file: {self.dbc_path}")
        self.db = cantools.database.load_file(str(self.dbc_path))
        logging.info(f"Loaded {len(self.db.messages)} message definitions")
        
        # Setup serial port
        self.master_fd = None
        self.slave_fd = None
        self.serial_port = serial_port
        self.pty_path = None
        self.device_port_path = None
        
        if serial_port is None:
            # Create virtual serial port (PTY)
            self.master_fd, self.slave_fd = pty.openpty()
            self.pty_path = os.ttyname(self.slave_fd)
            
            # Configure PTY for raw binary mode (crucial for CAN frames)
            self._configure_pty_raw_mode()
            
            print(f"\n✓ Virtual serial port created: {self.pty_path}")
            self.device_port_path = self._create_fixed_pty_symlink(self.pty_path)
            print(f"✓ Stable device path: {self.device_port_path}")
            print(f"  Use this port in can-receiver config: device_port={self.device_port_path}\n")
        else:
            # Use existing serial port
            import serial
            self.serial = serial.Serial(serial_port, baudrate=115200, timeout=1)
            logging.info(f"Connected to serial port: {serial_port}")
            self.device_port_path = serial_port
        
        # Store message cycle times (in seconds)
        self.message_cycles = self._get_message_cycles()
        self.last_send_times: Dict[int, float] = {}
        
        # For realistic mode - store previous values
        self.previous_values: Dict[str, float] = {}
    
    def _setup_logging(self):
        """Setup logging based on configuration"""
        level_map = {
            'debug': logging.DEBUG,
            'info': logging.INFO,
            'warning': logging.WARNING,
            'error': logging.ERROR
        }
        level = level_map.get(self.config.log_level.lower(), logging.INFO)
        
        logging.basicConfig(
            level=level,
            format='%(asctime)s - %(levelname)s - %(message)s',
            handlers=[logging.StreamHandler()],
            force=True
        )

    def _configure_pty_raw_mode(self):
        """Configure PTY for raw binary mode (no processing of control characters)."""
        try:
            # Set slave side to raw mode - essential for binary CAN frames
            tty.setraw(self.slave_fd)
            
            # Additionally configure for non-blocking and no echo
            attrs = termios.tcgetattr(self.slave_fd)
            # Disable all input/output processing
            attrs[0] = 0  # iflag - no input processing
            attrs[1] = 0  # oflag - no output processing
            attrs[3] = 0  # lflag - no local modes
            termios.tcsetattr(self.slave_fd, termios.TCSANOW, attrs)
            
            logging.debug("PTY configured for raw binary mode")
        except Exception as e:
            logging.warning(f"Could not configure PTY raw mode: {e}")
    
    def _create_fixed_pty_symlink(self, slave_path: str) -> str:
        """Create/refresh a stable symlink pointing at the PTY slave path."""
        link_path = FIXED_PTY_SYMLINK_PATH

        if os.path.lexists(link_path):
            if os.path.islink(link_path):
                os.unlink(link_path)
            else:
                raise RuntimeError(f"Refusing to replace existing non-symlink path: {link_path}")

        os.symlink(slave_path, link_path)
        return link_path
        
    def _get_message_cycles(self) -> Dict[int, float]:
        """Extract message cycle times from DBC attributes"""
        cycles = {}
        for message in self.db.messages:
            cycle_time_ms = 100  # default
            
            if hasattr(message, 'cycle_time') and message.cycle_time:
                cycle_time_ms = message.cycle_time
            
            # Apply cycle multiplier
            cycle_time_s = (cycle_time_ms / 1000.0) * self.config.cycle_multiplier
            cycles[message.frame_id] = cycle_time_s
        return cycles
    
    def _generate_random_signal_value(self, signal) -> float:
        """Generate random value for a signal within its valid range"""
        min_val = signal.minimum if signal.minimum is not None else 0
        max_val = signal.maximum if signal.maximum is not None else 100
        
        # Boolean signal (1 bit)
        if signal.length == 1:
            return random.choice([0, 1])
        
        # Realistic mode - gradual changes
        if self.config.realistic_mode:
            key = signal.name
            if key in self.previous_values:
                prev_value = self.previous_values[key]
                # Small change from previous value
                change_pct = 0.1  # Max 10% change per update
                max_change = (max_val - min_val) * change_pct
                new_value = prev_value + random.uniform(-max_change, max_change)
                # Apply smoothing
                value = prev_value * self.config.smoothing_factor + new_value * (1 - self.config.smoothing_factor)
                value = max(min_val, min(max_val, value))  # Clamp
            else:
                # First time - random value
                if signal.is_signed:
                    value = random.uniform(min_val, max_val)
                else:
                    value = random.uniform(max(0, min_val), max_val)
            
            self.previous_values[key] = value
        else:
            # Pure random mode
            if signal.is_signed:
                value = random.uniform(min_val, max_val)
            else:
                value = random.uniform(max(0, min_val), max_val)
        
        if signal.scale == 1 and signal.offset == 0:
            value = round(value)
        
        return value
    
    def generate_message_data(self, message) -> Dict[str, Any]:
        """Generate random data for all signals in a message"""
        data = {}
        for signal in message.signals:
            data[signal.name] = self._generate_random_signal_value(signal)
        return data
    
    def send_frame(self, frame_id: int, data: bytes, is_extended: bool = False):
        """Send CAN frame over serial port"""
        packet = WaveshareProtocol.encode_can_frame(frame_id, data, is_extended)
        
        try:
            if self.master_fd is not None:
                # Write to PTY
                total = 0
                while total < len(packet):
                    written = os.write(self.master_fd, packet[total:])
                    if written <= 0:
                        raise OSError("PTY write returned 0 bytes")
                    total += written
            else:
                # Write to real serial port
                self.serial.write(packet)
                
            # Log sent frame with packet bytes for debugging
            if self.config.log_frames:
                packet_hex = ' '.join(f'{b:02X}' for b in packet)
                print(f"TX: {packet_hex}")
                
        except Exception as e:
            print(f"Error sending frame: {e}")
    
    def send_message(self, message, data: Dict[str, Any] = None):
        """
        Encode and send a CAN message
        
        Args:
            message: Message definition from DBC
            data: Optional signal values dict, if None random values are generated
        """
        if data is None:
            data = self.generate_message_data(message)
        
        try:
            # Encode message data
            encoded_data = message.encode(data)
            
            # Send over serial
            self.send_frame(message.frame_id, encoded_data, message.is_extended_frame)
            
            if self.config.log_signals:
                signal_str = ", ".join([f"{k}={v:.2f}" for k, v in data.items()])
                print(f"{message.name}: {signal_str}")
            
        except Exception as e:
            logging.error(f"Error encoding message {message.name}: {e}")
            print(f"Error encoding message {message.name}: {e}")
    
    def should_send_message(self, message_id: int) -> bool:
        """Check if enough time has passed to send message based on cycle time"""
        
        current_time = time.time()
        cycle_time = self.message_cycles.get(message_id, 0.1)
        last_send = self.last_send_times.get(message_id, 0)
        
        # Apply cycle variation
        if self.config.cycle_variation > 0:
            variation = cycle_time * (self.config.cycle_variation / 100.0)
            cycle_time += random.uniform(-variation, variation)
            cycle_time = max(0.001, cycle_time)  # Minimum 1ms
        
        if current_time - last_send >= cycle_time:
            self.last_send_times[message_id] = current_time
            return True
        return False
    
    def check_for_commands(self) -> bool:
        """Check if there are incoming commands from the device (non-blocking)"""
        if self.master_fd is not None:
            # Check PTY for data
            readable, _, _ = select.select([self.master_fd], [], [], 0)
            if readable:
                try:
                    data = os.read(self.master_fd, 1024)
                    if data:
                        print(f"RX: {data.hex()}")
                        # Could parse commands here if needed
                        return True
                except:
                    pass
        return False
    
    def run_continuous(self, duration: float = None):
        """
        Run simulator continuously sending messages at their cycle times
        
        Args:
            duration: Optional duration in seconds, None for infinite
        """
        print("\n" + "="*70)
        print("Starting continuous CAN simulation on serial port")
        if self.device_port_path:
            if self.pty_path:
                print(f"Virtual device: {self.pty_path}")
            print(f"Device port: {self.device_port_path}")
            print("\nTo use with can-receiver:")
            print(f"  1. Edit config.txt and set: device_port={self.device_port_path}")
            print(f"  2. Run: cargo run")
        print("="*70)
        print("\nPress Ctrl+C to stop\n")
        
        # Small delay to let receiver open the port and initialize
        time.sleep(0.5)
        
        start_time = time.time()
        frame_count = 0
        
        try:
            while True:
                current_time = time.time()
                
                # Check if duration limit reached
                if duration and (current_time - start_time) >= duration:
                    break
                
                # Send messages that are due
                for message in self.db.messages:
                    if self.should_send_message(message.frame_id):
                        self.send_message(message)
                        frame_count += 1
                
                # Small sleep to prevent busy waiting
                time.sleep(0.01)
                
        except KeyboardInterrupt:
            elapsed = time.time() - start_time
            print(f"\n\n{'='*70}")
            print("Simulation stopped by user")
            print(f"Frames sent: {frame_count}")
            print(f"Duration: {elapsed:.1f}s")
            print(f"Rate: {frame_count/elapsed:.1f} frames/s")
            print("="*70)
        finally:
            self.close()
    
    def run_single_cycle(self):
        """Send each message once"""
        print("\nSending single cycle of all messages...\n")
        for message in self.db.messages:
            self.send_message(message)
            time.sleep(0.05)
        self.close()
    
    def close(self):
        """Close serial port and PTY"""
        if self.master_fd is not None:
            os.close(self.master_fd)
            os.close(self.slave_fd)
            print("\nPTY closed")
        elif hasattr(self, 'serial'):
            self.serial.close()
            print("\nSerial port closed")


def main():
    parser = argparse.ArgumentParser(
        description='Serial CAN Simulator - Emulates Waveshare USB-CAN-A device',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Use configuration file
  python symulator_serial.py --config simulator_config.txt
  
  # Create virtual serial port and run simulator
  python symulator_serial.py
  
  # Use custom DBC file
  python symulator_serial.py --dbc custom.dbcoml
  
  # Create virtual serial port and run simulator
  python symulator_serial.py
  
  # Use custom DBC file
  python symulator_serial.py --dbc custom.dbc
  
  # Run for 30 seconds
  python symulator_serial.py --duration 30
  
  # Send all messages once
  python symulator_serial.py --single-cycle
  
  # Adjust cycle speed (2x faster)
  python symulator_serial.py --cycle-multiplier 0.5
  
  # Realistic mode (gradual changes)
  python symulator_serial.py --realistic

Integration with can-receiver:
  1. Run this simulator - it will create a PTY and expose a stable path (default: /tmp/perla-bus-tty)
  2. Edit can-receiver/config.txt:
      device_port=/tmp/perla-bus-tty
  3. Run can-receiver:
       cd can-receiver && cargo run
        """
    )
    
    parser.add_argument(
        '--config',
        default='simulator_config.py',
        help='Path to configuration file'
    )
    
    parser.add_argument(
        '--dbc',
        help='Path to DBC file (overrides config)'
    )
    
    parser.add_argument(
        '--port',
        help='Serial port path (overrides config)'
    )
    
    parser.add_argument(
        '--duration',
        type=float,
        help='Duration in seconds (overrides config)'
    )
    
    parser.add_argument(
        '--single-cycle',
        action='store_true',
        help='Send all messages once and exit'
    )
    
    parser.add_argument(
        '--cycle-multiplier',
        type=float,
        help='Message cycle time multiplier (0.5=2x faster, 2.0=2x slower)'
    )
    
    parser.add_argument(
        '--realistic',
        action='store_true',
        help='Use realistic mode (gradual value changes)'
    )
    
    args = parser.parse_args()
    
    try:
        # Load configuration
        config = SimulatorConfig(args.config)
        
        # Override config with command line arguments
        if args.dbc:
            config.dbc_file = args.dbc
        if args.port:
            config.serial_port = args.port
        if args.duration:
            config.duration = args.duration
        if args.cycle_multiplier:
            config.cycle_multiplier = args.cycle_multiplier
        if args.realistic:
            config.realistic_mode = True
        if args.single_cycle:
            config.mode = 'single'
        elif args.duration:
            config.mode = 'duration'
        
        # Initialize simulator
        simulator = SerialCANSimulator(
            dbc_path=config.dbc_file,
            serial_port=config.serial_port,
            config=config
        )
        
        # Run simulation based on mode
        if config.mode == 'single' or args.single_cycle:
            simulator.run_single_cycle()
        elif config.mode == 'duration':
            simulator.run_continuous(duration=config.duration)
        else:
            simulator.run_continuous()
            
    except FileNotFoundError as e:
        print(f"Error: {e}")
        sys.exit(1)
    except KeyboardInterrupt:
        print("\nExiting...")
        sys.exit(0)
    except Exception as e:
        print(f"Fatal error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == '__main__':
    main()
