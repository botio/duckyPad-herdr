#include <string.h>
#include <stdio.h>
#include "main.h"
#include "shared.h"
#include <time.h>
#include "hid_task.h"

char temp_buf[TEMP_BUFSIZE];

uint32_t millis(void)
{
  return htim2.Instance->CNT;
}

void delay_ms(uint32_t amount)
{
  if(amount == 0)
    return;
  uint32_t start = millis();
  while ((millis() - start) < amount)
  {
    ;
  }
}

char* goto_next_arg(char* buf, char* buf_end)
{
  char* curr = buf;  
  if(buf == NULL || curr >= buf_end)
    return NULL;
  while(curr < buf_end && *curr != ' ')
      curr++;
  while(curr < buf_end && *curr == ' ')
      curr++;
  if(curr >= buf_end)
    return NULL;
  return curr;
}

void strip_newline(char* line, uint32_t size)
{
  for(uint32_t i = 0; i < size; ++i)
    if(line[i] == '\n' || line[i] == '\r')
      line[i] = 0;
}

void idle_loop(void)
{
  while(1)
  {
    hid_command_task();
    herdr_key_task();
    delay_ms(1);
  }
}

uint32_t get_uuid(void)
{
  return (*STM32F0_UUID0) ^ (*STM32F0_UUID1) ^ (*STM32F0_UUID2);
}

uint8_t is_rtc_valid(void)
{
  return HAL_RTCEx_BKUPRead(&hrtc, RTC_MAGICNUM_REG) == RTC_MAGIC_NUMBER;
}

void mark_rtc_as_valid(void)
{
  HAL_PWR_EnableBkUpAccess();
  HAL_RTCEx_BKUPWrite(&hrtc, RTC_MAGICNUM_REG, RTC_MAGIC_NUMBER);
  HAL_PWR_DisableBkUpAccess();
}

void set_utc_offset(int16_t minutes)
{
  HAL_PWR_EnableBkUpAccess();
  HAL_RTCEx_BKUPWrite(&hrtc, RTC_UTC_OFFSET_REG, (uint32_t)minutes);
  HAL_PWR_DisableBkUpAccess();
}

int16_t get_utc_offset(void)
{
  return (int16_t)HAL_RTCEx_BKUPRead(&hrtc, RTC_UTC_OFFSET_REG);
}

uint8_t RTC_SetFromUnixTimestamp(RTC_HandleTypeDef *rtc_ptr, uint32_t unix_timestamp)
{
  HAL_StatusTypeDef status;
  RTC_TimeTypeDef sTime = {0};
  RTC_DateTypeDef sDate = {0};
  time_t raw_time = (time_t)unix_timestamp;
  struct tm *time_info = gmtime(&raw_time);
  if (time_info == NULL)
    return 11;
  HAL_PWR_EnableBkUpAccess();
  sTime.Hours = time_info->tm_hour;
  sTime.Minutes = time_info->tm_min;
  sTime.Seconds = time_info->tm_sec;
  sTime.DayLightSaving = RTC_DAYLIGHTSAVING_NONE;
  sTime.StoreOperation = RTC_STOREOPERATION_RESET;
  status = HAL_RTC_SetTime(rtc_ptr, &sTime, RTC_FORMAT_BIN);
  if (time_info->tm_wday == 0)
    sDate.WeekDay = RTC_WEEKDAY_SUNDAY;
  else
    sDate.WeekDay = (uint8_t)time_info->tm_wday;
  // struct tm months are 0-11; STM32 HAL expects 1-12
  sDate.Month = (uint8_t)(time_info->tm_mon + 1);
  sDate.Date = (uint8_t)time_info->tm_mday;
  // struct tm year is "years since 1900" (e.g., 2023 = 123)
  // STM32 HAL usually expects 2-digits for years 2000-2099
  sDate.Year = (uint8_t)(time_info->tm_year - 100);
  status = HAL_RTC_SetDate(rtc_ptr, &sDate, RTC_FORMAT_BIN);
  HAL_PWR_DisableBkUpAccess();
  return 0;
}

uint32_t get_unix_ts(RTC_HandleTypeDef *rtc_ptr)
{
  RTC_TimeTypeDef sTime = {0};
  RTC_DateTypeDef sDate = {0};
  HAL_RTC_GetTime(rtc_ptr, &sTime, RTC_FORMAT_BIN);
  HAL_RTC_GetDate(rtc_ptr, &sDate, RTC_FORMAT_BIN);

  /* RTC stores UTC, with a separate display offset. Avoid libc's timezone
     parser and DST machinery; convert the Gregorian date directly to days
     since 1970-01-01 (the RTC's supported years are 2000 through 2099). */
  int32_t year = 2000 + sDate.Year;
  int32_t month = sDate.Month;
  year -= month <= 2;
  int32_t era = year / 400;
  int32_t year_of_era = year - era * 400;
  int32_t day_of_year = (153 * (month + (month > 2 ? -3 : 9)) + 2) / 5
                        + sDate.Date - 1;
  int32_t days = era * 146097 + year_of_era * 365 + year_of_era / 4
                 - year_of_era / 100 + day_of_year - 719468;
  return (uint32_t)days * 86400U + sTime.Hours * 3600U
         + sTime.Minutes * 60U + sTime.Seconds;
}

struct tm* get_local_time(RTC_HandleTypeDef *rtc_ptr, int16_t offset_minutes)
{
  time_t utc_epoch = get_unix_ts(rtc_ptr);
  time_t local_epoch = utc_epoch + (offset_minutes * 60);
  return gmtime(&local_epoch);
}
