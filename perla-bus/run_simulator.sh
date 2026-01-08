#!/bin/bash
# Quick start script for serial simulator

cd "$(dirname "$0")"

# Activate virtual environment
if [ ! -d ".venv" ]; then
    echo "Creating virtual environment..."
    python3 -m venv .venv
    source .venv/bin/activate
    pip install -r requirements.txt
else
    source .venv/bin/activate
fi

# Run simulator
echo "Starting serial CAN simulator..."
python symulator_serial.py "$@"
