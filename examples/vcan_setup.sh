#!/bin/bash
# CANFlow - Virtual CAN Setup Script
# Sets up vcan interfaces for development and testing

set -e

echo "=== CANFlow Virtual CAN Setup ==="

# Load vcan kernel module
if ! lsmod | grep -q vcan; then
    echo "[*] Loading vcan kernel module..."
    sudo modprobe vcan
fi

# Create vcan0 interface
if ! ip link show vcan0 &>/dev/null; then
    echo "[*] Creating vcan0 interface..."
    sudo ip link add dev vcan0 type vcan
    sudo ip link set up vcan0
    echo "[+] vcan0 is up"
else
    echo "[+] vcan0 already exists"
fi

# Create vcan1 for testing (second interface)
if ! ip link show vcan1 &>/dev/null; then
    echo "[*] Creating vcan1 interface..."
    sudo ip link add dev vcan1 type vcan
    sudo ip link set up vcan1
    echo "[+] vcan1 is up"
else
    echo "[+] vcan1 already exists"
fi

echo ""
echo "=== Setup Complete ==="
echo "  vcan0: $(ip link show vcan0 | grep -o 'state [A-Z]*')"
echo "  vcan1: $(ip link show vcan1 | grep -o 'state [A-Z]*')"
echo ""
echo "Usage:"
echo "  canflow capture --interface vcan0"
echo "  cansend vcan0 123#DEADBEEF         (generate test traffic)"
echo "  cangen vcan0 -g 1 -I 100 -D r -L 8  (continuous random traffic)"
