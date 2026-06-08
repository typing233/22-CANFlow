#!/bin/bash
# CANFlow - End-to-end capture and analysis example
set -e

echo "=== CANFlow Capture & Analyze Example ==="

# Check if vcan0 exists
if ! ip link show vcan0 &>/dev/null; then
    echo "[!] vcan0 not found. Run: ./examples/vcan_setup.sh"
    exit 1
fi

# Generate test traffic in background
echo "[*] Generating test traffic on vcan0..."
cangen vcan0 -g 1 -I 100 -D r -L 8 -n 1000 &
CANGEN_PID=$!

# Capture with analysis enabled, stream mode
echo "[*] Capturing and analyzing..."
timeout 5 canflow capture \
    --interface vcan0 \
    --analyze \
    --format json \
    --record ./capture_session.jsonl \
    2>/dev/null || true

# Stop traffic generator
kill $CANGEN_PID 2>/dev/null || true
wait $CANGEN_PID 2>/dev/null || true

echo ""
echo "[*] Running offline analysis on captured session..."
canflow analyze --input ./capture_session.jsonl --modules entropy,period,burst

echo ""
echo "=== Done ==="
echo "  Recorded session: ./capture_session.jsonl"
