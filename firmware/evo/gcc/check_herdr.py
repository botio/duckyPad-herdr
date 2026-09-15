#!/usr/bin/env python3
"""Host regressions for profile ownership, IRQ handoff, navigation and F9.

Compiles production C bodies. Hardware boundary stubs do not emulate LED
waveforms, USB endpoint timing, or the physical switches.
"""
import os
from pathlib import Path
import subprocess
import tempfile

from check_behavior import ROOT, block


def span(path, start, end):
    text = (ROOT / path).read_text()
    return text[text.index(start):text.index(end)]


def main():
    definitions = "\n".join(
        line for path in ("Inc/hid_task.h", "Inc/input_task.h", "Inc/profiles.h", "Inc/neopixel.h")
        for line in (ROOT / path).read_text().splitlines()
        if line.startswith("#define ")
    )
    profile_type = block("Inc/profiles.h", "typedef struct\n{\n  char sw_name") + " profile_cache;"
    settings_type = block("Inc/profiles.h", "typedef struct\n{\n  uint8_t brightness_index") + " dp_global_settings;"
    animation_type = block("Inc/neopixel.h", "typedef struct") + " led_animation;"
    source = r'''
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define DP_HID_MSG_SIZE 9
#define USBD_CUSTOMHID_OUTREPORT_BUF_SIZE 64
#define HERDR_OLED_MAX_TEXT 56
#define OLED_CONTRAST_BRIGHT 255
#define USBD_STATE_CONFIGURED 3
#define USBD_OK 0
#define SSD1306_HEIGHT 64
#define Black 0
#define White 1
''' + definitions + '\n' + profile_type + '\n' + settings_type + '\n' + animation_type + r'''
static uint32_t irq_mask;
static uint32_t __get_PRIMASK(void) { return irq_mask; }
static void __disable_irq(void) { irq_mask = 1; }
static void __set_PRIMASK(uint32_t value) { irq_mask = value; }
static int in_usb, draws, oled_draws, macro_presses, macro_releases;
static uint8_t visible[15][3], last_report[9], hid_tx_buf[64];
static uint32_t now, physical_keys, key_reply;
static int usb_busy, report_count, key_replies;
static struct { int dev_state; } hUsbDeviceFS = {USBD_STATE_CONFIGURED};
static int is_busy;
static profile_cache curr_pf_info;
static dp_global_settings dp_settings;
static uint32_t millis(void) { return now; }
static void sw_scan(void) {}
static uint32_t get_sw_state_bitfield(void) { return physical_keys; }
static uint8_t poll_sw_state(int sw, int scan) { return (physical_keys >> sw) & 1; }
''' + span("Src/neopixel.c", "volatile uint32_t frame_counter;", "void neopixel_draw_current_buffer(") + r'''
static void neopixel_show(uint8_t *red, uint8_t *green, uint8_t *blue, uint8_t brightness) {
  assert(!in_usb); ++draws;
  for(int i = 0; i < NEOPIXEL_COUNT; ++i) {
    visible[i][0] = red[pixel_map[i]];
    visible[i][1] = green[pixel_map[i]];
    visible[i][2] = blue[pixel_map[i]];
  }
}
''' + block("Src/neopixel.c", "void neopixel_draw_current_buffer(") + r'''
typedef struct { int FontWidth, FontHeight; } FontDef;
static FontDef Font_6x10 = {6, 10};
static void ssd1306_Fill(int c) { assert(!in_usb); }
static void ssd1306_SetCursor(int x, int y) {}
static void ssd1306_WriteChar(char c, FontDef f, int color) {}
static void ssd1306_UpdateScreen(void) { assert(!in_usb); ++oled_draws; }
static void ssd1306_SetContrast(int c) {}
static int USBD_CUSTOM_HID_SendReport(void *device, uint8_t *data, size_t size) {
  assert(!in_usb && irq_mask && size == sizeof last_report);
  if(usb_busy) return 1;
  memcpy(last_report, data, size); ++report_count; return USBD_OK;
}
static void send_hid_cmd_response(void *data) { assert(!in_usb); ++key_replies; memcpy(&key_reply, (uint8_t*)data + 3, sizeof key_reply); }
static void draw_current_profile(void) {}
''' + span("Src/hid_task.c", "volatile uint8_t needs_gv_save", "// Mirror diagnostic:") + '\n' + span("Src/hid_task.c", "static uint8_t f9_sample;", "void herdr_key_task(") + '\n' + block("Src/hid_task.c", "void herdr_key_task(") + '\n' + block("Src/hid_task.c", "void herdr_set_rgb_frame(") + '\n' + block("Src/hid_task.c", "void herdr_set_oled_text(") + r'''
static void handle_hid_command(uint8_t *this_msg) {
  uint8_t command_type = this_msg[2];
''' + '\n'.join(block("Src/hid_task.c", f"if(command_type == {command})") for command in (
        "HID_COMMAND_SET_RGB_FRAME", "HID_COMMAND_SET_OLED_TEXT", "HID_COMMAND_SET_HERDR_MODE", "HID_COMMAND_GET_HERDR_KEYS"
    )) + r'''
}
''' + '\n'.join(block("Src/hid_task.c", marker) for marker in (
        "static uint8_t is_storage_hid_command(", "void receive_hid_report(", "static void herdr_display_task(", "void hid_command_task("
    )) + '\n' + span("Src/profiles.c", "const char cmd_BG_COLOR", "FRESULT sd_fresult;") + r'''
static const char cmd_sw_name_firstline[] = "z";
static uint8_t current_profile_number;
static char profile_name_list[MAX_PROFILES][PROFILE_NAME_MAX_LEN];
static char *goto_next_arg(char *s, char *end) {
  char *p = strchr(s, ' '); return p && p < end ? p + 1 : NULL;
}
''' + block("Src/profiles.c", "void parse_profile_config_line(") + r'''
static uint8_t load_profile(uint8_t n) {
  if(n != 0 && n != 2) return 1;
  memset(&curr_pf_info, 0, sizeof curr_pf_info);
  if(n == 2) parse_profile_config_line("HERDR_PROFILE 1", &curr_pf_info);
  else curr_pf_info.dsb_exists[0] = DSB_ON_PRESS_EXISTS;
  return 0;
}
static void save_settings(dp_global_settings *settings) {}
static void load_persistent_state(void) {}
''' + block("Src/neopixel.c", "void neopixel_redraw_bg(") + r'''
''' + '\n'.join(block("Src/profiles.c", marker) for marker in (
        "uint8_t goto_profile_without_updating_rgb_LED(", "void goto_profile(", "void goto_next_profile(", "void goto_prev_profile("
    )) + r'''
static char dsb_on_press_path_buf[FILENAME_BUFSIZE], dsb_on_release_path_buf[FILENAME_BUFSIZE];
static uint32_t last_execution_exit;
static int is_plus_minus_button(int n) { return n == SW_PLUS || n == SW_MINUS; }
static void settings_menu(void) {}
#define DSB_ALLOW_AUTOREPEAT 0
#define DSB_DONT_PLAY_KEYUP_ANIMATION_RETURN_IMMEDIATELY 1
#define DSB_DONT_REPEAT_RETURN_IMMEDIATELY 2
/* VM boundary: record actual routed executions without USB or SD hardware. */
static uint8_t run_once(uint8_t swid, char *path, uint8_t *to_increment) {
  if(strstr(path, "-release.dsb")) ++macro_releases;
  else ++macro_presses;
  return DSB_DONT_REPEAT_RETURN_IMMEDIATELY;
}
''' + '\n'.join(block("Src/neopixel.c", marker) for marker in (
        "void led_start_animation(", "void play_keydown_animation(", "void play_keyup_animation("
    )) + '\n' + '\n'.join(block("Src/keypress_task.c", marker) for marker in (
        "void onboard_offboard_switch_press(", "void onboard_offboard_switch_release("
    )) + r'''
''' + block("Src/keypress_task.c", "void process_keyevent(") + r'''
static uint32_t current_tick;
static int animations, scans;
typedef int TIM_HandleTypeDef;
static void led_animation_handler(void) { ++animations; }
static void kb_scan_task(void) { ++scans; }
''' + block("Src/main.c", "void HAL_TIM_PeriodElapsedCallback(") + r'''
static void receive(uint8_t *msg) { in_usb = 1; receive_hid_report(msg); in_usb = 0; }
static void poll_keys(uint8_t *msg) { receive(msg); hid_command_task(); }
static void assert_frame(uint8_t value) {
  for(int i = 0; i < HERDR_F9_SWITCH; ++i)
    for(int c = 0; c < 3; ++c) assert(visible[i][c] == value);
}

static void assert_local_color(uint8_t r, uint8_t g, uint8_t b) {
  assert(visible[HERDR_F9_SWITCH][0] == r);
  assert(visible[HERDR_F9_SWITCH][1] == g);
  assert(visible[HERDR_F9_SWITCH][2] == b);
}
int main(void) {
  strcpy(profile_name_list[0], "Herdr"); /* Name alone has no meaning. */
  strcpy(profile_name_list[2], "Dashboard");
  goto_profile(0); assert(!herdr_mode);
  uint8_t host[64] = {5, 0, 36, 1}, frame[64] = {5, 0, 34};
  uint8_t text[64] = {5, 0, 35, 2, 'O', 'K'}, keys[64] = {5, 0, 37};
  receive(host); hid_command_task(); assert(!herdr_mode);
  memset(frame + 3, 17, 45);
  int before = draws;
  receive(frame); receive(text); hid_command_task();
  assert(draws == before && oled_draws == 0);
  physical_keys = 0x1ffff; poll_keys(keys); assert(key_reply == 0);
  process_keyevent(0, SW_EVENT_SHORT_PRESS); assert(macro_presses == 1);
  process_keyevent(SW_PLUS, SW_EVENT_RELEASE); assert(herdr_mode && current_profile_number == 2);
  assert(herdr_bridge_enabled == 0);
  /* Herdr entry must not paint profile BG green onto empty agent keys. */
  assert(visible[0][0] == 0 && visible[0][1] == 0 && visible[0][2] == 0);
  /* Bridge must re-assert cmd36 after profile entry. */
  receive(host); hid_command_task(); assert(herdr_bridge_enabled == 1);
  before = draws;
  receive(frame); receive(text); assert(draws == before && oled_draws == 0);
  /* Back-to-back USB frames coalesce without touching the live LED buffer. */
  memset(frame + 3, 99, 45); receive(frame); assert(draws == before);
  hid_command_task(); assert_frame(99); assert(oled_draws == 0);
  /* RGB apply + post-title LED redraw (names packed into draw_current_profile). */
  assert(draws == before + 2);
  assert(visible[14][0] == 255 && visible[14][1] == 255);
  poll_keys(keys); assert(key_reply == 0x3fff); /* No F9 or navigation bits. */
  process_keyevent(0, SW_EVENT_SHORT_PRESS); process_keyevent(0, SW_EVENT_RELEASE);
  assert(macro_presses == 1 && macro_releases == 0);
  for(int i = 0; i < 20; ++i) HAL_TIM_PeriodElapsedCallback(NULL);
  assert(animations == 0 && scans == 10);
  host[3] = 0; receive(host); hid_command_task();
  before = draws; receive(frame); receive(text); hid_command_task(); poll_keys(keys);
  assert(herdr_mode && draws == before && oled_draws == 0 && key_reply == 0);
  host[3] = 1; receive(host); hid_command_task();
  is_in_file_access_mode = 1; before = draws;
  int replies_before = key_replies;
  receive(frame); receive(text); hid_command_task(); poll_keys(keys); herdr_key_task();
  assert(draws == before && oled_draws == 0 && key_replies == replies_before);
  is_in_file_access_mode = 0;
  /* A busy endpoint must not replay an unsent F9 press into a macro profile. */
  physical_keys = 1U << 14; usb_busy = 1; herdr_key_task(); now += 5; herdr_key_task();
  process_keyevent(SW_MINUS, SW_EVENT_RELEASE); usb_busy = 0; herdr_key_task();
  assert(!herdr_mode && current_profile_number == 0 && report_count == 0);
  for(int i = 0; i < 4; ++i) HAL_TIM_PeriodElapsedCallback(NULL);
  assert(animations == 1);
  /* Wrap backwards, press F9, then leave while physically held: release once. */
  physical_keys = 0; process_keyevent(SW_MINUS, SW_EVENT_RELEASE); herdr_key_task();
  assert(current_profile_number == 2 && herdr_mode);
  assert(herdr_bridge_enabled == 0);
  host[3] = 1; receive(host); hid_command_task();
  physical_keys = 1U << 14; now += 5; herdr_key_task(); now += 5; herdr_key_task();
  assert(report_count == 1 && last_report[3] == 0x42);
  process_keyevent(SW_PLUS, SW_EVENT_RELEASE); herdr_key_task();
  assert(current_profile_number == 0 && !herdr_mode && report_count == 2 && last_report[3] == 0);
  herdr_key_task(); assert(report_count == 2);
  process_keyevent(0, SW_EVENT_SHORT_PRESS); assert(macro_presses == 2);
  /* File access releases a sent F9 without repainting over SD operations. */
  physical_keys = 0; goto_profile(2); herdr_key_task();
  host[3] = 1; receive(host); hid_command_task();
  physical_keys = 1U << 14; now += 5; herdr_key_task(); now += 5; herdr_key_task();
  assert(last_report[3] == 0x42);
  is_in_file_access_mode = 1; before = draws; herdr_key_task();
  assert(last_report[3] == 0 && draws == before);
  /* Press-only, release-only and paired scripts never send the F9 shortcut. */
  is_in_file_access_mode = 0;
  const uint8_t scripts[] = {DSB_ON_PRESS_EXISTS, DSB_ON_RELEASE_EXISTS,
                            DSB_ON_PRESS_EXISTS | DSB_ON_RELEASE_EXISTS};
  for(unsigned int s = 0; s < sizeof scripts; ++s) {
    goto_profile(2);
    assert(herdr_bridge_enabled == 0);
    host[3] = 1; receive(host); hid_command_task(); assert(herdr_bridge_enabled == 1);
    curr_pf_info.dsb_exists[HERDR_F9_SWITCH] = scripts[s];
    uint8_t *rest = curr_pf_info.sw_color_default[HERDR_F9_SWITCH];
    rest[0] = 13; rest[1] = 27; rest[2] = 41;
    uint8_t *down = curr_pf_info.sw_color_keydown[HERDR_F9_SWITCH];
    down[0] = 211; down[1] = 73; down[2] = 19;
    neopixel_redraw_bg();
    receive(frame); hid_command_task();
    assert_frame(99); assert_local_color(13, 27, 41);
    int mp = macro_presses, mr = macro_releases, rc = report_count;
    physical_keys = 1U << HERDR_F9_SWITCH;
    herdr_key_task(); now += 5; herdr_key_task(); now += 5; herdr_key_task();
    assert(report_count == rc && last_report[3] == 0);
    process_keyevent(HERDR_F9_SWITCH, SW_EVENT_SHORT_PRESS);
    assert(macro_presses == mp + !!(scripts[s] & DSB_ON_PRESS_EXISTS));
    assert(macro_releases == mr && report_count == rc);
    /* A paired script keeps keydown feedback until the release handler. */
    if(scripts[s] == (DSB_ON_PRESS_EXISTS | DSB_ON_RELEASE_EXISTS)) {
      assert_local_color(211, 73, 19);
      receive(frame); hid_command_task();
      assert_frame(99); assert_local_color(211, 73, 19);
    }
    physical_keys = 0;
    process_keyevent(HERDR_F9_SWITCH, SW_EVENT_RELEASE);
    /* No heartbeat or timer tick between release and the visible rest color. */
    assert_local_color(13, 27, 41); assert_frame(99);
    assert(macro_releases == mr + !!(scripts[s] & DSB_ON_RELEASE_EXISTS));
    herdr_key_task(); now += 5; herdr_key_task();
    receive(frame); hid_command_task();
    assert_local_color(13, 27, 41); assert_frame(99);
    assert(report_count == rc);
  }
  /* Removing both scripts restores the default F9 shortcut and LED colors. */
  curr_pf_info.dsb_exists[HERDR_F9_SWITCH] = 0;
  int rc = report_count;
  herdr_key_task(); assert_local_color(255, 255, 255);
  physical_keys = 1U << HERDR_F9_SWITCH;
  herdr_key_task(); now += 5; herdr_key_task();
  assert(report_count == rc + 1 && last_report[3] == 0x42);
  assert_local_color(255, 0, 0);
  receive(frame); hid_command_task(); assert_local_color(255, 0, 0); assert_frame(99);
  physical_keys = 0; herdr_key_task(); now += 5; herdr_key_task();
  assert(report_count == rc + 2 && last_report[3] == 0);
  assert_local_color(255, 255, 255);
  parse_profile_config_line("HERDR_PROFILE 10", &curr_pf_info); assert(!curr_pf_info.is_herdr);
  parse_profile_config_line("HERDR_PROFILE 1", &curr_pf_info); assert(curr_pf_info.is_herdr);
  parse_profile_config_line("HERDR_PROFILE 0", &curr_pf_info); assert(!curr_pf_info.is_herdr);
  herdr_boot_without_storage();
  assert(herdr_mode && herdr_no_storage && herdr_bridge_enabled == 0);
  host[3] = 1; receive(host); hid_command_task();
  assert(herdr_bridge_enabled == 1);
  memset(frame + 3, 42, 45);
  receive(frame); hid_command_task();
  assert_frame(42);
  puts("PASS Herdr profile authority, foreground/coalesced RGB+OLED, host enable/disable, file-access exclusion, +/- wrap, macro isolation, timer gate, F9 busy/exit/release, key15 press/release scripts and local LED ownership, exact marker, no-SD Bridge");
}
'''
    with tempfile.TemporaryDirectory(prefix="duckypad-herdr-check-") as temporary:
        path = Path(temporary)
        (path / "check.c").write_text(source)
        subprocess.run([os.environ.get("HOST_CC", "cc"), "-std=c99", "-O2", "-Wall", "-Werror=implicit-function-declaration", str(path / "check.c"), "-o", str(path / "check")], check=True)
        subprocess.run([str(path / "check")], check=True)


if __name__ == "__main__":
    main()
