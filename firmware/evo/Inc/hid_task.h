#ifndef __HID_TASK_H
#define __HID_TASK_H

#ifdef __cplusplus
 extern "C" {
#endif 

#define HID_COMMAND_GET_INFO 0
#define HID_COMMAND_GOTO_PROFILE_BY_NUMBER 1
#define HID_COMMAND_PREV_PROFILE 2
#define HID_COMMAND_NEXT_PROFILE 3
#define HID_COMMAND_SET_LED_SINGLE 4

#define HID_COMMAND_READ_FILE 11

#define HID_COMMAND_OPEN_FILE_FOR_WRITING 14
#define HID_COMMAND_WRITE_FILE 15
#define HID_COMMAND_CLOSE_FILE 16
#define HID_COMMAND_DELETE_FILE 17
#define HID_COMMAND_CREATE_DIR 18
#define HID_COMMAND_DELETE_DIR 19
#define HID_COMMAND_SW_RESET 20
#define HID_COMMAND_SLEEP 21
#define HID_COMMAND_WAKEUP 22
#define HID_COMMAND_GOTO_PROFILE_BY_NAME 23
#define HID_COMMAND_DUMP_GV 24
#define HID_COMMAND_WRITE_GV 25
#define HID_COMMAND_SET_RTC 0x1A

#define HID_COMMAND_DUMP_SD 32
#define HID_COMMAND_OPEN_FILE_FOR_READING 33
#define HID_COMMAND_SET_RGB_FRAME 34
#define HID_COMMAND_SET_OLED_TEXT 35
#define HID_COMMAND_SET_HERDR_MODE 36
#define HID_COMMAND_GET_HERDR_KEYS 37
#define HID_COMMAND_EXIT_FILE_ACCESS 38

// In herdr mode, the final mechanical key is reserved as a local F9 shortcut
// instead of an agent-focus key. Switches and LEDs are both zero-indexed.
#define HERDR_F9_SWITCH 14
#define HERDR_F9_HID_USAGE 0x42

#define HID_RESPONSE_OK 0
#define HID_RESPONSE_GENERIC_ERROR 1
#define HID_RESPONSE_BUSY 2

#define HID_RESPONSE_NO_PROFILE 4
#define HID_RESPONSE_INVALID_ARG 5
#define HID_RESPONSE_UNKNOWN_CMD 6

#define HERDR_IN_KEY_STATE 0xF1

#define HID_USAGE_ID_KEYBOARD 1
#define HID_USAGE_ID_MEDIA_KEY 2
#define HID_USAGE_ID_MOUSE 3
#define HID_USAGE_ID_NAMED_PIPE 4
// PC -> duckyPad command report (report ID 5 on the OUT endpoint).
#define HID_USAGE_ID_PC_DATA 5

#define HID_READ_FILE_PATH_SIZE_MAX 55
#define HID_FILE_READ_PAYLOAD_SIZE 61

void receive_hid_report(const uint8_t* hid_msg);
void hid_command_task(void);
void herdr_key_task(void);
void sd_walk(uint8_t* res_buf);
void md5_test(void);
uint8_t make_file_walk_hid_packet(char* file_name, char* profile_name, uint8_t* tx_buf);

extern volatile uint8_t is_in_file_access_mode;
extern volatile uint8_t needs_gv_save;
extern volatile uint8_t herdr_mode;
extern uint8_t herdr_bridge_enabled;
void herdr_hold_dark_until_bridge(void);

#ifdef __cplusplus
}
#endif

#endif


