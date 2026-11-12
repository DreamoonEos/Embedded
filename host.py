#!/usr/bin/env python3
"""Exercise the two serial interfaces of the classroom bridge."""

import argparse


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("port", help="USB serial device name (for example /dev/ttyACM0)")
    commands = parser.add_subparsers(dest="command", required=True)
    for command in ("ping", "swd", "can-recv", "uart-loopback"):
        commands.add_parser(command)
    send = commands.add_parser("can-send")
    send.add_argument("id", help="11-bit hex ID, without 0x")
    send.add_argument("data", help="0–8 bytes in hex, or '-' for no data")
    args = parser.parse_args()

    try:
        import serial
    except ImportError:
        parser.exit(2, "Install pyserial 3.5 in a project environment first.\n")

    try:
        with serial.Serial(args.port, 115200, timeout=2, write_timeout=2) as port:
            port.reset_input_buffer()
            if args.command == "uart-loopback":
                expected = b"HerKules UART\n"
                port.write(expected)
                got = port.read(len(expected))
                if got != expected:
                    parser.exit(1, f"Expected {expected!r}, received {got!r}\n")
                print("OK UART loopback")
                return
            if args.command == "can-send":
                command = f"CAN TX {args.id} {args.data}"
            else:
                command = {"ping": "PING", "swd": "SWD ID", "can-recv": "CAN RX"}[args.command]
            port.write((command + "\n").encode("ascii"))
            reply = port.readline(96).decode("ascii", errors="replace").strip()
            if not reply:
                parser.exit(1, "No response. Check that this is the control interface.\n")
            print(reply)
            if not (reply.startswith("OK ") or reply == "EMPTY"):
                parser.exit(1)
    except serial.SerialException as err:
        parser.exit(1, f"Serial error: {err}\n")


if __name__ == "__main__":
    main()
