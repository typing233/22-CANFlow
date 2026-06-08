#!/usr/bin/env python3
"""Example CANFlow Python agent: outputs CAN frames as JSON lines to stdout."""
import json
import sys

for i in range(5):
    frame = {
        "timestamp_ns": i * 1000000,
        "id": 0x7DF,
        "dlc": 8,
        "data": [0x02, 0x10, i + 1, 0x00, 0x00, 0x00, 0x00, 0x00],
        "is_error": False,
        "is_remote": False,
        "interface": 0
    }
    print(json.dumps(frame), flush=True)

sys.exit(0)
