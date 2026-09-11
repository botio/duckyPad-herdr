#!/usr/bin/env python3
"""Validate a linked EVO image and its DfuSe container without flashing it."""
import argparse
import hashlib
from pathlib import Path
import re
import struct
import subprocess
import zlib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("elf", type=Path)
    parser.add_argument("dfu", type=Path, nargs="?")
    args = parser.parse_args()
    binary = args.elf.with_suffix(".bin").read_bytes()
    dfu_path = args.dfu or args.elf.with_suffix(".dfu")
    dfu = dfu_path.read_bytes()
    assert 192 <= len(binary) <= 131072, "Image exceeds the CB's 128 KiB Flash"
    assert struct.unpack_from("<5sBIB", dfu) == (b"DfuSe", 1, len(dfu), 1)
    signature, alt, named, name, target_size, elements = struct.unpack_from("<6sBI255sII", dfu, 11)
    assert signature == b"Target" and alt == 0 and elements == 1
    address, size = struct.unpack_from("<II", dfu, 285)
    assert address == 0x08000000 and size == len(binary)
    assert target_size == 8 + size and len(dfu) == 293 + size + 16
    assert dfu[293:-16] == binary, "DFU payload differs from linked BIN"
    device, product, vendor, version, signature, length, crc = struct.unpack_from("<HHHH3sBI", dfu, len(dfu) - 16)
    assert (product, vendor, version, signature, length) == (0xdf11, 0x0483, 0x011a, b"UFD", 16)
    assert crc == (zlib.crc32(dfu[:-4]) ^ 0xffffffff), "Invalid DFU CRC"

    output = subprocess.check_output(["arm-none-eabi-nm", "-n", str(args.elf)], text=True)
    symbols = {parts[2]: int(parts[0], 16) for line in output.splitlines()
               if len(parts := line.split()) == 3 and re.fullmatch("[0-9a-fA-F]+", parts[0])}
    assert symbols["_estack"] == 0x20004000
    assert symbols["__heap_start"] >= symbols["_ebss"]
    assert symbols["__heap_end"] == symbols["_estack"] - 2048
    assert symbols["__heap_end"] - symbols["__heap_start"] >= 1024
    for symbol in ("ws_spi_buf", "ws_padding_buf"):
        assert symbols[symbol] % 2 == 0, f"{symbol} must be halfword-aligned for 16-bit HAL SPI"
    # Check all vectors against the vendor's existing Keil startup, not a
    # second manually transcribed table in this verification program.
    startup = (Path(__file__).resolve().parents[1] / "MDK-ARM/startup_stm32f072xb.s").read_text()
    start = startup.index("__Vectors       DCD")
    table = startup[start:startup.index("__Vectors_End\n", start)]
    expected = re.findall(r"\bDCD\s+(\w+)", table)
    assert len(expected) == 48
    vectors = struct.unpack_from("<48I", binary)
    for index, symbol in enumerate(expected):
        value = 0 if symbol == "0" else symbols["_estack"] if index == 0 else symbols[symbol] | 1
        assert vectors[index] == value, f"Incorrect vector {index}: {symbol}"
        if index and value:
            assert 0x08000000 <= (value & ~1) < 0x08000000 + len(binary)
    for symbol in ("USB_IRQHandler", "TIM17_IRQHandler", "EXTI4_15_IRQHandler", "SysTick_Handler"):
        assert symbols[symbol] != symbols["Default_Handler"], f"Unbound interrupt: {symbol}"
    print(f"PASS DfuSe CRC/payload/address; 48 vectors; Flash {len(binary)}/131072 bytes; "
          f"heap {symbols['__heap_end'] - symbols['__heap_start']} bytes; stack 2048 bytes")
    print(f"SHA256 {hashlib.sha256(dfu).hexdigest()}  {dfu_path}")


if __name__ == "__main__":
    main()
