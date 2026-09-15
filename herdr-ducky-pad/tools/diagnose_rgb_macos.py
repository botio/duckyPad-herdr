#!/usr/bin/env python3
"""Isolate RGB, OLED, and key-poll traffic without rebuilding the Bridge.

Requires macOS and Python 3. Uses Apple's system frameworks, not pip packages.
Select a Herdr profile before running. Only HID commands 34-37 are sent; no
SD writes. The launchd Bridge is paused during the test and resumed on exit.
"""

import argparse
import ctypes as C
from contextlib import ExitStack, contextmanager
import os
import plistlib
import subprocess
import sys
import time


def report(command, payload=b""):
    if len(payload) > 61:
        raise ValueError("HID payload exceeds 61 bytes")
    return bytes((5, 0, command)) + payload + bytes(61 - len(payload))


TEXT = b"FIXED RGB TEST"
REPORTS = {
    "mode": report(36, b"\x01"),
    "rgb": report(34, bytes((0, 64, 0)) * 14 + bytes((255, 255, 255))),
    "oled": report(35, bytes((len(TEXT),)) + TEXT),
    "keys": report(37),
}
PHASES = (
    ("A: fixed RGB only", False, False),
    ("B: fixed RGB + fixed OLED", True, False),
    ("C: mode + fixed RGB + fixed OLED + key polling", True, True),
)


def check_iokit(result, operation):
    if result != 0:
        raise RuntimeError(f"{operation}: IOKit error 0x{result & 0xffffffff:08X}")


def bind(library, name, result, *args):
    function = getattr(library, name)
    function.restype = result
    function.argtypes = args
    return function


@contextmanager
def open_pad():
    """Open the one matching physical HID device in shared (non-seizing) mode."""
    cf = C.CDLL("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")
    io = C.CDLL("/System/Library/Frameworks/IOKit.framework/IOKit")
    ptr = C.c_void_p
    release = bind(cf, "CFRelease", None, ptr)
    data_create = bind(cf, "CFDataCreate", ptr, ptr, C.c_char_p, C.c_long)
    plist_create = bind(cf, "CFPropertyListCreateWithData", ptr,
                        ptr, ptr, C.c_ulong, ptr, ptr)
    set_count = bind(cf, "CFSetGetCount", C.c_long, ptr)
    set_values = bind(cf, "CFSetGetValues", None, ptr, C.POINTER(ptr))
    current_loop = bind(cf, "CFRunLoopGetCurrent", ptr)
    run_loop = bind(cf, "CFRunLoopRunInMode", C.c_int32, ptr, C.c_double, C.c_bool)
    loop_mode = ptr.in_dll(cf, "kCFRunLoopDefaultMode")
    manager_create = bind(io, "IOHIDManagerCreate", ptr, ptr, C.c_uint32)
    manager_match = bind(io, "IOHIDManagerSetDeviceMatching", None, ptr, ptr)
    schedule = bind(io, "IOHIDManagerScheduleWithRunLoop", None, ptr, ptr, ptr)
    unschedule = bind(io, "IOHIDManagerUnscheduleFromRunLoop", None, ptr, ptr, ptr)
    copy_devices = bind(io, "IOHIDManagerCopyDevices", ptr, ptr)
    device_open = bind(io, "IOHIDDeviceOpen", C.c_int32, ptr, C.c_uint32)
    device_close = bind(io, "IOHIDDeviceClose", C.c_int32, ptr, C.c_uint32)
    set_report = bind(io, "IOHIDDeviceSetReport", C.c_int32,
                      ptr, C.c_int, C.c_long, ptr, C.c_long)

    with ExitStack() as resources:
        def own(reference, operation):
            if not reference:
                raise RuntimeError(f"{operation} returned no result")
            resources.callback(release, reference)
            return reference

        matching_xml = plistlib.dumps({"VendorID": 0x0483, "ProductID": 0xd11c})
        data = own(data_create(None, matching_xml, len(matching_xml)), "CFDataCreate")
        matching = own(plist_create(None, data, 0, None, None), "CFPropertyListCreateWithData")
        manager = own(manager_create(None, 0), "IOHIDManagerCreate")
        loop = current_loop()
        schedule(manager, loop, loop_mode)
        resources.callback(unschedule, manager, loop, loop_mode)
        manager_match(manager, matching)
        while run_loop(loop_mode, 0.001, False) == 4:  # kCFRunLoopRunHandledSource
            pass
        devices = own(copy_devices(manager), "IOHIDManagerCopyDevices (0483:d11c)")
        count = set_count(devices)
        if count != 1:
            raise RuntimeError(f"Expected one duckyPad HID device, found {count}; no reports sent")
        values = (ptr * count)()
        set_values(devices, values)
        device = values[0]
        check_iokit(device_open(device, 0), "IOHIDDeviceOpen(shared)")
        resources.callback(device_close, device, 0)

        # Match hidapi: numbered output reports include their report ID byte.
        buffers = {name: C.create_string_buffer(data, len(data))
                   for name, data in REPORTS.items()}

        def send(name):
            buffer = buffers[name]
            check_iokit(set_report(device, 1, 5, buffer, len(buffer)),
                        f"IOHIDDeviceSetReport({name})")

        yield send


def run_phases(send):
    send("mode")
    for label, oled, poll in PHASES:
        print(f"\n{label} -- 8 seconds; observe whether colors jump", flush=True)
        # Four identical RGB reports, two seconds apart. Phase C adds the
        # Bridge's 10 ms key-poll cadence; all reply contents are irrelevant.
        ticks_per_frame = 200 if poll else 1
        delay = 0.01 if poll else 2.0
        for tick in range(4 * ticks_per_frame):
            if tick % ticks_per_frame == 0:
                if poll:
                    send("mode")
                send("rgb")
                if oled:
                    send("oled")
            if poll:
                send("keys")
            time.sleep(delay)
        print(f"Completed {label}: four identical RGB reports accepted", flush=True)


def diagnose():
    service = f"gui/{os.getuid()}/com.botio.ducky-pad-bridge"
    print("Keep a Herdr profile selected; do not press keys during the test.", flush=True)
    print("First 14 keys request steady green; key 15 remains firmware-controlled.", flush=True)
    print("Bridge will pause for about 24 seconds. No SD writes or permission resets.", flush=True)
    print(f"Fixed RGB payload: {REPORTS['rgb'][3:48].hex()}", flush=True)
    # An open failure never pauses the Bridge. SIGCONT is attempted even if
    # pausing raises or Ctrl-C interrupts a subprocess wait.
    with open_pad() as send:
        try:
            subprocess.run(["launchctl", "kill", "SIGSTOP", service], check=True)
            run_phases(send)
        finally:
            result = subprocess.run(["launchctl", "kill", "SIGCONT", service], check=False)
            if result.returncode:
                raise RuntimeError(f"Could not resume Bridge. Run: launchctl kill SIGCONT {service}")
            print("\nBridge resumed. Its next heartbeat restores the Herdr display.", flush=True)


def main():
    argparse.ArgumentParser(description=__doc__).parse_args()
    if sys.platform != "darwin":
        print("This hardware diagnostic requires macOS.", file=sys.stderr)
        return 2
    try:
        diagnose()
    except KeyboardInterrupt:
        print("Diagnostic interrupted.", file=sys.stderr)
        return 130
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"Diagnostic failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
