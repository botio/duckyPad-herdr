#!/usr/bin/env python3
"""Exercise MCU-independent production C bodies with hardware boundary stubs.

Requires a host C compiler. Does not emulate USB, SD timing, or the MCU.
"""
from pathlib import Path
import os
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def block(path, marker):
    source = (ROOT / path).read_text()
    start = source.index(marker)
    opening = source.index("{", start)
    depth = 1
    end = opening + 1
    while depth:
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    return source[start:end]


def main():
    # Compile the actual function bodies rather than reimplementing their logic.
    rtc = block("Src/shared.c", "uint32_t get_unix_ts(")
    local = block("Src/shared.c", "struct tm* get_local_time(")
    animation = block("Src/neopixel.c", "void led_start_animation(")
    handler = block("Src/neopixel.c", "void led_animation_handler(")
    fastwrite = block("Src/neopixel.c", "void spi_fastwrite_buf_size_even(")
    show = block("Src/neopixel.c", "void neopixel_show(")
    color_type = block("Inc/neopixel.h", "typedef struct") + " led_animation;"
    exit_command = block("Src/hid_task.c", "if(command_type == HID_COMMAND_EXIT_FILE_ACCESS)")
    storage_predicate = block("Src/hid_task.c", "static uint8_t is_storage_hid_command(")
    report_receiver = block("Src/hid_task.c", "void receive_hid_report(")
    command_task = block("Src/hid_task.c", "void hid_command_task(")
    wait_loop = block("Src/keypress_task.c", "void file_access_mode_task(")
    source = r'''
#define _GNU_SOURCE
#include <assert.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#define THREE 3
#define NEOPIXEL_COUNT 1
#define NEOPIXEL_PADDING_BUF_SIZE 4
#define WS_SPI_BUF_SIZE 24
#define WS_BIT_0 0xc0
#define WS_BIT_1 0xf8
#define ANIMATION_NONE 0
#define ANIMATION_CROSS_FADE 1
#define RTC_FORMAT_BIN 0
#define OLED_CONTRAST_BRIGHT 255
#define SW_EVENT_LONG_PRESS 2
#define HID_COMMAND_EXIT_FILE_ACCESS 38
#define HID_RESPONSE_BUSY 2
#define HID_RESPONSE_GENERIC_ERROR 1
#define SD_WALK_STATE_IDLE 0
#define SD_WALK_STATE_NEW_FILE 2
#define FR_OK 0
#define HID_USAGE_ID_PC_DATA 5
#define USBD_CUSTOMHID_OUTREPORT_BUF_SIZE 64
#define HID_COMMAND_READ_FILE 11
#define HID_COMMAND_OPEN_FILE_FOR_WRITING 14
#define HID_COMMAND_WRITE_FILE 15
#define HID_COMMAND_CLOSE_FILE 16
#define HID_COMMAND_DELETE_FILE 17
#define HID_COMMAND_CREATE_DIR 18
#define HID_COMMAND_DELETE_DIR 19
#define HID_COMMAND_DUMP_SD 32
#define HID_COMMAND_OPEN_FILE_FOR_READING 33
#define HID_COMMAND_SET_RGB_FRAME 34
#define HID_COMMAND_SET_OLED_TEXT 35
#define HID_COMMAND_SET_HERDR_MODE 36
#define HID_COMMAND_GET_HERDR_KEYS 37
typedef int RTC_HandleTypeDef;
typedef struct { uint8_t Hours, Minutes, Seconds; } RTC_TimeTypeDef;
typedef struct { uint8_t Year, Month, Date; } RTC_DateTypeDef;
static RTC_TimeTypeDef clock_time;
static RTC_DateTypeDef clock_date;
void HAL_RTC_GetTime(void *p, RTC_TimeTypeDef *t, int f) { *t = clock_time; }
void HAL_RTC_GetDate(void *p, RTC_DateTypeDef *d, int f) { *d = clock_date; }
''' + rtc + '\n' + local + '\n' + color_type + r'''
typedef struct { uint32_t CR2, DR; } SPI_TypeDef;
typedef struct { uint32_t DataSize, BaudRatePrescaler; } SPI_InitTypeDef;
typedef struct { SPI_TypeDef *Instance; SPI_InitTypeDef Init; } SPI_HandleTypeDef;
static SPI_TypeDef spi_regs;
static SPI_HandleTypeDef hspi1 = {&spi_regs, {0, 0}};
static int spi_init_calls, spi_transmit_calls;
static uint8_t ws_padding_buf[NEOPIXEL_PADDING_BUF_SIZE] __attribute__((aligned(2)));
static uint8_t ws_spi_buf[WS_SPI_BUF_SIZE] __attribute__((aligned(2)));
static uint8_t red_after_brightness[NEOPIXEL_COUNT];
static uint8_t green_after_brightness[NEOPIXEL_COUNT];
static uint8_t blue_after_brightness[NEOPIXEL_COUNT];
#define SPI_FLAG_TXE 1
#define SPI_CR2_DS 0xf00
#define SPI_DATASIZE_16BIT 0xf00
#define SPI_BAUDRATEPRESCALER_4 0
#define GPIO_PIN_SET 1
#define GPIO_PIN_RESET 0
#define LED_DATA_EN_GPIO_Port NULL
#define LED_DATA_EN_Pin 0
#define __HAL_SPI_GET_FLAG(handle, flag) 1
#define __disable_irq()
#define __enable_irq()
#define __get_PRIMASK() 0
#define __set_PRIMASK(value) ((void)(value))
static uint8_t herdr_rgb_msg[64], herdr_oled_msg[64];
static uint8_t herdr_rgb_pending, herdr_oled_pending, herdr_host_update;
static uint8_t herdr_keys_pending;
static volatile uint8_t is_in_file_access_mode;
static void herdr_display_task(void) {}
void herdr_key_task(void) {}
static uint8_t queued_hid_msg[USBD_CUSTOMHID_OUTREPORT_BUF_SIZE];
static volatile uint8_t queued_hid_msg_pending;
static int is_busy;
static int handled_hid_commands;
static uint8_t handled_hid_msg[USBD_CUSTOMHID_OUTREPORT_BUF_SIZE];
static void handle_hid_command(uint8_t *hid_msg) {
  ++handled_hid_commands;
  memcpy(handled_hid_msg, hid_msg, USBD_CUSTOMHID_OUTREPORT_BUF_SIZE);
}
''' + storage_predicate + '\n' + report_receiver + '\n' + command_task + r'''
int HAL_SPI_Init(SPI_HandleTypeDef *handle) {
  ++spi_init_calls;
  handle->Instance->CR2 = (handle->Instance->CR2 & ~SPI_CR2_DS) | handle->Init.DataSize;
  return 0;
}
int HAL_SPI_Transmit(SPI_HandleTypeDef *handle, uint8_t *data, uint16_t size, uint32_t timeout) {
  assert(handle == &hspi1 && data == ws_padding_buf);
  assert(((uintptr_t)data & 1) == 0);
  assert(size == NEOPIXEL_PADDING_BUF_SIZE / sizeof(uint16_t));
  ++spi_transmit_calls;
  return 0;
}
void HAL_GPIO_WritePin(void *port, uint16_t pin, int state) {}
''' + fastwrite + '\n' + show + r'''
static led_animation neo_anime[NEOPIXEL_COUNT];
static uint32_t frame_counter;
static uint8_t rendered[3];
void set_pixel_3color(uint8_t n, uint8_t r, uint8_t g, uint8_t b)
{ rendered[0] = r; rendered[1] = g; rendered[2] = b; }
void neopixel_draw_current_buffer(void) {}
static uint8_t red_buf[1], green_buf[1], blue_buf[1];
static const uint8_t brightness_index_to_percent_lookup[1] = {100};
static struct { int brightness_index; } dp_settings;

''' + animation + '\n' + handler + r'''
static int herdr_mode, sd_walk_state, current_profile_number;
static struct { void *fs; } sd_file;
static int dir, close_result, dir_result, closes, draws, sends, delays;
static uint8_t hid_tx_buf[64];
int f_close(void *f) { ++closes; if (!close_result) sd_file.fs = NULL; return close_result; }
int f_closedir(void *d) { return dir_result; }
void clear_sw_queue(void) {}
void update_last_keypress(void) {}
int is_valid_profile_number(int n) { return n == 0; }
void draw_current_profile(void) { ++draws; }
void neopixel_redraw_bg(void) {}
void neopixel_off(void) {}
void oled_say(const char *s) {}
void send_hid_cmd_response(void *p) { ++sends; }
static void exit_file_access(void) {
  int command_type = 38;
  memset(hid_tx_buf, 0, sizeof hid_tx_buf);
''' + exit_command + r'''
}
typedef struct { int id, type; } switch_event_t;
static int switch_event_queue;
void delay_ms(int ms) { assert(++delays < 3); exit_file_access(); }
void ssd1306_SetContrast(int n) {}
int q_pop(void *q, void *e) { return 0; }
int is_plus_minus_button(int id) { return 0; }
void NVIC_SystemReset(void) { abort(); }
''' + wait_loop + r'''
int main(void) {
  uint8_t immediate_msg[USBD_CUSTOMHID_OUTREPORT_BUF_SIZE] = {HID_USAGE_ID_PC_DATA, 0, 0};
  receive_hid_report(immediate_msg);
  assert(handled_hid_commands == 1);
  uint8_t storage_msg[USBD_CUSTOMHID_OUTREPORT_BUF_SIZE] = {HID_USAGE_ID_PC_DATA, 0, HID_COMMAND_OPEN_FILE_FOR_READING, '/', 'x'};
  receive_hid_report(storage_msg);
  storage_msg[4] = 'y';
  assert(handled_hid_commands == 1);
  hid_command_task();
  assert(handled_hid_commands == 2 && handled_hid_msg[4] == 'x');
  hid_command_task();
  assert(handled_hid_commands == 2);
  is_busy = 1;
  receive_hid_report(storage_msg);
  assert(handled_hid_commands == 3);
  is_busy = 0;
  hid_command_task();
  assert(handled_hid_commands == 3);
  union { uint16_t alignment; uint8_t bytes[5]; } odd_storage =
      {.bytes = {0, 0x12, 0x34, 0x56, 0x78}};
  spi_fastwrite_buf_size_even(&odd_storage.bytes[1], 4);
  assert(spi_regs.DR == 0x7856);
  uint8_t pixel[NEOPIXEL_COUNT] = {0};
  neopixel_show(pixel, pixel, pixel, 100);
  assert(spi_init_calls == 1 && spi_transmit_calls == 1);


  /* Compare every supported calendar day to the host UTC oracle, including
     2000's leap day, month/year rollover, and dates after 2038. */
  unsigned days = 0;
  for(time_t ts = 946684800; ts < 4102444800LL; ts += 86400) {
    struct tm t = *gmtime(&ts);
    clock_date = (RTC_DateTypeDef){t.tm_year - 100, t.tm_mon + 1, t.tm_mday};
    clock_time = (RTC_TimeTypeDef){0,0,0};
    assert(get_unix_ts(NULL) == (uint32_t)ts);
    clock_time = (RTC_TimeTypeDef){23,59,59};
    assert(get_unix_ts(NULL) == (uint32_t)(ts + 86399));
    for(int offset = -720; offset <= 840; offset += 15) {
      time_t expected = ts + 86399 + offset * 60;
      struct tm actual = *get_local_time(NULL, offset);
      struct tm reference = *gmtime(&expected);
      assert(actual.tm_year == reference.tm_year && actual.tm_yday == reference.tm_yday);
      assert(actual.tm_hour == reference.tm_hour && actual.tm_min == reference.tm_min);
    }
    ++days;
  }
  /* The existing key-up fade lasts 50 frames. Exhaust every 8-bit endpoint
     pair, checking final color and <=1 LSB against the original double fade. */
  unsigned max_delta = 0;
  for(int start = 0; start < 256; ++start) for(int end = 0; end < 256; ++end) {
    memset(neo_anime, 0, sizeof neo_anime);
    for(int c = 0; c < 3; ++c) neo_anime[0].current_color[c] = start;
    frame_counter = 0;
    uint8_t target[3] = {end,end,end};
    led_start_animation(&neo_anime[0], target, ANIMATION_CROSS_FADE, 50);
    double reference = start, step = (end - start) / 50.0;
    for(int frame = 1; frame <= 51; ++frame) {
      led_animation_handler();
      reference = frame <= 50 ? reference + step : end;
      if(reference < 0) reference = 0;
      if(reference > 255) reference = 255;
      unsigned delta = abs((int)rendered[0] - (int)reference);
      if(delta > max_delta) max_delta = delta;
      assert(delta <= 1);
    }
    assert(rendered[0] == end && neo_anime[0].animation_type == ANIMATION_NONE);
  }
  sd_file.fs = (void*)1;
  is_busy = 1;
  exit_file_access(); /* Idle exit must not close a script-owned file. */
  assert(closes == 0 && sd_file.fs != NULL);
  is_in_file_access_mode = 1;
  exit_file_access();
  assert(hid_tx_buf[2] == HID_RESPONSE_BUSY && closes == 0);
  is_busy = 0; close_result = 1;
  exit_file_access();
  assert(hid_tx_buf[2] == HID_RESPONSE_GENERIC_ERROR && is_in_file_access_mode);
  close_result = 0; sd_walk_state = SD_WALK_STATE_NEW_FILE; dir_result = 1;
  exit_file_access();
  assert(hid_tx_buf[2] == HID_RESPONSE_GENERIC_ERROR && is_in_file_access_mode);
  dir_result = 0;
  file_access_mode_task(); /* Host exit interrupts the real foreground loop. */
  assert(!is_in_file_access_mode && sd_walk_state == SD_WALK_STATE_IDLE && draws == 1);
  exit_file_access(); assert(draws == 1);
  herdr_mode = 1; is_in_file_access_mode = 1;
  exit_file_access(); assert(!is_in_file_access_mode && draws == 1);
  printf("PASS deferred storage HID dispatch; aligned SPI transfer units; unaligned SPI words; RTC %u days and offsets; LED 65536 fades max delta %u LSB; file-access busy/error/exit/idempotency/herdr\n", days, max_delta);
}
'''
    with tempfile.TemporaryDirectory(prefix="duckypad-check-") as tmp:
        path = Path(tmp)
        (path / "check.c").write_text(source)
        subprocess.run([os.environ.get("HOST_CC", "cc"), "-std=c99", "-O2",
                        "-fsanitize=alignment", "-fno-sanitize-recover=alignment",
                        str(path / "check.c"), "-o", str(path / "check")], check=True)
        subprocess.run([str(path / "check")], check=True)


if __name__ == "__main__":
    main()
