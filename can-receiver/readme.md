# Serial Port Configuration

Depending on your operating system, you need to provide the correct port name:

- **Windows:** use names like `COM1`, `COM2`, ..., e.g. `"COM3"`
- **Linux:** use paths like `/dev/ttyUSB0`, `/dev/ttyACM0`, etc.

Example usage in code:
```rust
// Windows
let mut device = ws::sync::new("COM3", &config).open()?;

// Linux
let mut device = ws::sync::new("/dev/ttyUSB0", &config).open()?;
```

Make sure to provide the correct port name according to the system you are running the program on.
