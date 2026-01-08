#!/bin/bash
# Test script - runs simulator and can-receiver together

echo "=== Perla Monitor Test Setup ==="
echo ""

# Check if can-receiver is built
if [ ! -f "../can-receiver/target/debug/can-receiver" ] && [ ! -f "../can-receiver/target/release/can-receiver" ]; then
    echo "Building can-receiver..."
    cd ../can-receiver
    cargo build
    cd ../perla-bus
fi

# Start simulator in background and capture PTY path
echo "Starting simulator..."
source .venv/bin/activate
python symulator_serial.py > /tmp/simulator.log 2>&1 &
SIM_PID=$!

# Wait for simulator to start and create PTY
sleep 2

# The simulator exposes a stable device path via symlink.
PTY_PATH="/tmp/perla-bus-tty"

if [ ! -L "$PTY_PATH" ]; then
    echo "Error: Simulator did not create stable PTY symlink: $PTY_PATH"
    echo "--- simulator log ---"
    tail -n 50 /tmp/simulator.log
    kill $SIM_PID 2>/dev/null
    exit 1
fi

echo "Simulator started on (stable): $PTY_PATH"
echo ""

# Update can-receiver config
echo "Updating can-receiver config..."
cat > ../can-receiver/config.txt << EOF
# CAN Receiver Configuration
# Path to CAN device
device_port=$PTY_PATH

# CAN transmission speed (5k, 10k, 20k, 50k, 100k, 125k, 200k, 250k, 400k, 500k, 800k, 1000k)
can_baud_rate=500k
EOF

echo "Config updated!"
echo ""
echo "Starting can-receiver..."
echo "Press Ctrl+C to stop both processes"
echo ""
echo "========================================"
echo ""

# Run can-receiver
cd ../can-receiver
cargo run

# Cleanup
echo ""
echo "Stopping simulator..."
kill $SIM_PID 2>/dev/null
rm /tmp/simulator.log 2>/dev/null
echo "Done!"
