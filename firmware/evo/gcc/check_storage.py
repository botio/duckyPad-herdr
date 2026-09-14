#!/usr/bin/env python3
"""Exercise production storage mutations using vendored FatFs on a disposable image.

Requires HOST_CC (default cc) and mkfs.fat. Only the block device and HID/UI
boundaries are supplied by the harness; no FatFs results are mocked. This does
not emulate SD/USB timing, power loss, or the MCU.
"""
from pathlib import Path
import os
import subprocess
import tempfile

from check_behavior import ROOT, block


def main():
    delete = block("Src/hid_task.c", "uint8_t delete_node (")
    commands = ["DELETE_FILE", "CREATE_DIR", "DELETE_DIR"]
    handlers = "\n".join(
        block("Src/hid_task.c", "if(command_type == HID_COMMAND_" + command + ")")
        for command in commands
    )
    # Keep protocol constants in step with firmware without pulling in HAL.
    constants = "\n".join(
        line for line in (ROOT / "Inc/hid_task.h").read_text().splitlines()
        if line.startswith("#define HID_COMMAND_")
        or line.startswith("#define HID_RESPONSE_")
    )
    source = r'''
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "ff.h"
#include "diskio.h"

static FILE *image;
static int fail_reads, fail_sync, write_protected;
static FATFS volume;
static FIL sd_file;
static FILINFO fno;
#define HID_TX_BUF_SIZE 64
static uint8_t hid_tx_buf[HID_TX_BUF_SIZE];
static uint8_t response[HID_TX_BUF_SIZE];
static unsigned responses;

DSTATUS disk_initialize(BYTE drive) { return disk_status(drive); }
DSTATUS disk_status(BYTE drive) {
    if (drive || !image) return STA_NOINIT;
    return write_protected ? STA_PROTECT : 0;
}
DRESULT disk_read(BYTE drive, BYTE *buffer, DWORD sector, UINT count) {
    if (drive || fail_reads || !count) return RES_ERROR;
    if (fseek(image, (long)sector * 512, SEEK_SET)) return RES_ERROR;
    return fread(buffer, 512, count, image) == count ? RES_OK : RES_ERROR;
}
DRESULT disk_write(BYTE drive, const BYTE *buffer, DWORD sector, UINT count) {
    if (drive || !count) return RES_ERROR;
    if (write_protected) return RES_WRPRT;
    if (fseek(image, (long)sector * 512, SEEK_SET)) return RES_ERROR;
    return fwrite(buffer, 512, count, image) == count ? RES_OK : RES_ERROR;
}
DRESULT disk_ioctl(BYTE drive, BYTE command, void *buffer) {
    if (drive) return RES_PARERR;
    switch (command) {
        case CTRL_SYNC:
            return fail_sync || fflush(image) ? RES_ERROR : RES_OK;
        case GET_SECTOR_COUNT: *(DWORD *)buffer = 65536; return RES_OK;
        case GET_SECTOR_SIZE: *(WORD *)buffer = 512; return RES_OK;
        case GET_BLOCK_SIZE: *(DWORD *)buffer = 1; return RES_OK;
        default: return RES_PARERR;
    }
}
void *ff_memalloc(UINT size) { return malloc(size); }
void ff_memfree(void *memory) { free(memory); }
static void enter_file_access_mode(void) {}
static void send_hid_cmd_response(uint8_t *report) {
    memcpy(response, report, sizeof(response));
    responses++;
}
''' + constants + '\n' + delete + r'''
static void dispatch(uint8_t *this_msg) {
    uint8_t command_type = this_msg[2];
    memset(hid_tx_buf, 0, sizeof(hid_tx_buf));
    hid_tx_buf[0] = 4;
    hid_tx_buf[2] = HID_RESPONSE_OK;
''' + handlers + r'''
}

static void command(uint8_t code, const char *path, FRESULT expected) {
    uint8_t request[64] = {5, 0};
    assert(strlen(path) < sizeof(request) - 3);
    strcpy((char *)request + 3, path);
    request[2] = code;
    responses = 0;
    dispatch(request);
    if (responses != 1 || response[0] != 4 || response[1] != 0 ||
        response[2] != (expected == FR_OK ? HID_RESPONSE_OK : HID_RESPONSE_GENERIC_ERROR) ||
        response[3] != expected) {
        fprintf(stderr, "command %u %s: expected FatFs %u, got replies=%u [%u,%u,%u,%u]\n",
                code, path, expected, responses, response[0], response[1], response[2], response[3]);
        abort();
    }
}
static void exists(const char *path, int directory) {
    FILINFO info = {0};
    assert(f_stat(path, &info) == FR_OK);
    assert(!!(info.fattrib & AM_DIR) == !!directory);
}
static void absent(const char *path) {
    FILINFO info = {0};
    FRESULT result = f_stat(path, &info);
    assert(result == FR_NO_FILE || result == FR_NO_PATH);
}
static void file(const char *path) {
    FIL object;
    UINT written;
    assert(f_open(&object, path, FA_CREATE_ALWAYS | FA_WRITE) == FR_OK);
    assert(f_write(&object, "payload", 7, &written) == FR_OK && written == 7);
    assert(f_close(&object) == FR_OK);
}
static void remount(void) {
    assert(f_mount(NULL, "", 0) == FR_OK);
    memset(&volume, 0, sizeof(volume));
    assert(f_mount(&volume, "", 1) == FR_OK);
}
static void recursive_delete(void) {
    command(HID_COMMAND_CREATE_DIR, "/profile_autohotkey", FR_OK);
    command(HID_COMMAND_CREATE_DIR, "/profile_autohotkey/sub", FR_OK);
    file("/profile_autohotkey/config.txt");
    file("/profile_autohotkey/sub/key1.txt");
    file("/neighbor.txt");
    command(HID_COMMAND_DELETE_DIR, "/profile_autohotkey", FR_OK);
    absent("/profile_autohotkey");
    exists("/neighbor.txt", 0);
    command(HID_COMMAND_DELETE_DIR, "/profile_autohotkey", FR_OK);
    command(HID_COMMAND_DELETE_DIR, "/missingparent/profile_autohotkey", FR_OK);
    exists("/neighbor.txt", 0);
}
static void partial_delete(void) {
    command(HID_COMMAND_CREATE_DIR, "/partial", FR_OK);
    file("/partial/first.txt");
    file("/partial/blocked.txt");
    file("/partial/last.txt");
    assert(f_chmod("/partial/blocked.txt", AM_RDO, AM_RDO) == FR_OK);
    command(HID_COMMAND_DELETE_DIR, "/partial", FR_DENIED);
    absent("/partial/first.txt");
    exists("/partial/blocked.txt", 0);
    exists("/partial/last.txt", 0);
    command(HID_COMMAND_DELETE_DIR, "/partial", FR_DENIED);
    assert(f_chmod("/partial/blocked.txt", 0, AM_RDO) == FR_OK);
    command(HID_COMMAND_DELETE_DIR, "/partial", FR_OK);
    absent("/partial");
    command(HID_COMMAND_DELETE_DIR, "/partial", FR_OK);
}
static void creation_and_file_delete(void) {
    command(HID_COMMAND_CREATE_DIR, "/existing", FR_OK);
    file("/existing/keep.txt");
    command(HID_COMMAND_CREATE_DIR, "/existing", FR_OK);
    exists("/existing/keep.txt", 0);
    file("/ordinary.txt");
    command(HID_COMMAND_CREATE_DIR, "/ordinary.txt", FR_EXIST);
    command(HID_COMMAND_DELETE_DIR, "/ordinary.txt", FR_NO_PATH);
    exists("/ordinary.txt", 0);
    assert(f_chmod("/ordinary.txt", AM_RDO, AM_RDO) == FR_OK);
    command(HID_COMMAND_DELETE_FILE, "/ordinary.txt", FR_DENIED);
    exists("/ordinary.txt", 0);
    assert(f_chmod("/ordinary.txt", 0, AM_RDO) == FR_OK);
    command(HID_COMMAND_DELETE_FILE, "/ordinary.txt", FR_OK);
    absent("/ordinary.txt");
    command(HID_COMMAND_DELETE_FILE, "/ordinary.txt", FR_OK);
    command(HID_COMMAND_DELETE_FILE, "/missingparent/file.txt", FR_OK);
    command(HID_COMMAND_CREATE_DIR, "/missingparent/child", FR_NO_PATH);
}
static void storage_errors(void) {
    command(HID_COMMAND_CREATE_DIR, "/protected", FR_OK);
    write_protected = 1;
    command(HID_COMMAND_DELETE_DIR, "/protected", FR_WRITE_PROTECTED);
    command(HID_COMMAND_CREATE_DIR, "/cannotcreate", FR_WRITE_PROTECTED);
    command(HID_COMMAND_DELETE_FILE, "/neighbor.txt", FR_WRITE_PROTECTED);
    write_protected = 0;
    exists("/protected", 1);
    exists("/neighbor.txt", 0);

    // Remount to discard the sector cache, then fail real block-device reads.
    remount();
    fail_reads = 1;
    command(HID_COMMAND_DELETE_DIR, "/protected", FR_DISK_ERR);
    fail_reads = 0;
    remount();
    exists("/protected", 1);

    // An open modified file must sync before DELETE_FILE can unlink anything.
    UINT written;
    assert(f_open(&sd_file, "/pending.txt", FA_CREATE_ALWAYS | FA_WRITE) == FR_OK);
    assert(f_write(&sd_file, "pending", 7, &written) == FR_OK && written == 7);
    fail_sync = 1;
    command(HID_COMMAND_DELETE_FILE, "/neighbor.txt", FR_DISK_ERR);
    fail_sync = 0;
    assert(f_close(&sd_file) == FR_OK);
    exists("/neighbor.txt", 0);
    command(HID_COMMAND_DELETE_FILE, "/neighbor.txt", FR_OK);
    absent("/neighbor.txt");
}
static void path_bounds(void) {
    command(HID_COMMAND_CREATE_DIR, "/overflow", FR_OK);
    // Long entry first: a truncated unlink must not delete the shorter neighbor.
    file("/overflow/victimex");
    file("/overflow/victim");
    char path[64] = "/overflow";
    assert(delete_node(path, strlen("/overflow/victim"), &fno) == FR_INVALID_NAME);
    assert(strcmp(path, "/overflow") == 0);
    exists("/overflow/victimex", 0);
    exists("/overflow/victim", 0);

    command(HID_COMMAND_CREATE_DIR, "/overflowdir", FR_OK);
    command(HID_COMMAND_CREATE_DIR, "/overflowdir/childex", FR_OK);
    command(HID_COMMAND_CREATE_DIR, "/overflowdir/child", FR_OK);
    file("/overflowdir/child/keep.txt");
    memset(path, 0, sizeof(path));
    strcpy(path, "/overflowdir");
    assert(delete_node(path, strlen("/overflowdir/child"), &fno) == FR_INVALID_NAME);
    exists("/overflowdir/childex", 1);
    exists("/overflowdir/child/keep.txt", 0);

    // The HID-sized buffer also rejects a child that cannot fit.
    command(HID_COMMAND_CREATE_DIR, "/overflow/abcdefgh", FR_OK);
    command(HID_COMMAND_CREATE_DIR, "/overflow/abcdefgh/ijklmnop", FR_OK);
    command(HID_COMMAND_CREATE_DIR, "/overflow/abcdefgh/ijklmnop/qrstuvwx", FR_OK);
    command(HID_COMMAND_CREATE_DIR, "/overflow/abcdefgh/ijklmnop/qrstuvwx/yz123456", FR_OK);
    const char *longdir = "/overflow/abcdefgh/ijklmnop/qrstuvwx/yz123456/abcdefg";
    command(HID_COMMAND_CREATE_DIR, longdir, FR_OK);
    file("/overflow/abcdefgh/ijklmnop/qrstuvwx/yz123456/abcdefg/child123");
    command(HID_COMMAND_DELETE_DIR, longdir, FR_INVALID_NAME);
    exists("/overflow/abcdefgh/ijklmnop/qrstuvwx/yz123456/abcdefg/child123", 0);

    // No room even for the initial slash and terminator; preserve both guards.
    unsigned char guarded[16];
    memset(guarded, 0xa5, sizeof(guarded));
    memcpy(guarded + 1, "/overflow", sizeof("/overflow"));
    assert(delete_node((char *)guarded + 1, sizeof("/overflow"), &fno) == FR_INVALID_NAME);
    assert(guarded[0] == 0xa5 && guarded[11] == 0xa5);
    assert(strcmp((char *)guarded + 1, "/overflow") == 0);
    memset(guarded, 'x', sizeof(guarded));
    assert(delete_node((char *)guarded + 1, 10, &fno) == FR_INVALID_NAME);
    for (unsigned i = 0; i < sizeof(guarded); i++) assert(guarded[i] == 'x');
    assert(delete_node((char *)guarded + 1, 0, &fno) == FR_INVALID_NAME);
}
int main(int argc, char **argv) {
    assert(argc == 2);
    image = fopen(argv[1], "r+b");
    assert(image);
    assert(f_mount(&volume, "", 1) == FR_OK);
    recursive_delete();
    partial_delete();
    creation_and_file_delete();
    storage_errors();
    path_bounds();
    assert(f_mount(NULL, "", 0) == FR_OK);
    assert(fclose(image) == 0);
    puts("Storage checks passed (production mutations + vendored FatFs).");
    return 0;
}
'''
    with tempfile.TemporaryDirectory(prefix="duckypad-storage-") as tmp:
        path = Path(tmp)
        image = path / "storage.img"
        with image.open("wb") as fixture:
            fixture.truncate(32 * 1024 * 1024)
        subprocess.run(["mkfs.fat", "-F", "16", str(image)], check=True)
        # Copy the unchanged production configuration so its quoted HAL includes
        # resolve to the fixture headers rather than the MCU headers beside it.
        (path / "ffconf.h").write_text((ROOT / "Inc/ffconf.h").read_text())
        (path / "main.h").write_text("/* Host FatFs fixture: no MCU dependencies. */\n")
        (path / "stm32f0xx_hal.h").write_text("/* Host FatFs fixture: no HAL dependencies. */\n")
        (path / "check.c").write_text(source)
        fatfs = ROOT / "Middlewares/Third_Party/FatFs/src"
        subprocess.run([
            os.environ.get("HOST_CC", "cc"), "-std=c99", "-O2",
            "-fsanitize=alignment", "-fno-sanitize-recover=alignment",
            "-I" + str(path), "-I" + str(ROOT / "Inc"), "-I" + str(fatfs),
            str(path / "check.c"), str(fatfs / "ff.c"),
            str(fatfs / "option/ccsbcs.c"), "-o", str(path / "check"),
        ], check=True)
        subprocess.run([str(path / "check"), str(image)], check=True)


if __name__ == "__main__":
    main()
