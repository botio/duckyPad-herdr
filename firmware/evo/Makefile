# Reproducibility baseline: ARM GNU Toolchain 13.2.rel1
# (GCC 13.2.1 / bundled newlib). Override TOOLCHAIN only with the same version.
TOOLCHAIN ?= $(HOME)/.local/arm-gnu/bin/arm-none-eabi
CC := $(TOOLCHAIN)-gcc
OBJCOPY := $(TOOLCHAIN)-objcopy
SIZE := $(TOOLCHAIN)-size
CC_VERSION := $(shell $(CC) -dumpfullversion 2>/dev/null)
ifneq ($(CC_VERSION),13.2.1)
$(error ARM GNU Toolchain 13.2.1 required; got "$(CC_VERSION)" from $(CC))
endif
DFUSE_PACK ?= dfuse-pack

BUILD ?= build
VERSION := $(shell python3 -c 'import re; s=open("Src/main.c").read(); print(".".join(re.search(r"fw_version_"+p+r"\s*=\s*(\d+)",s)[1] for p in ("major","minor","patch")))')
TARGET := duckypad_v$(VERSION)-herdr
ELF := $(BUILD)/$(TARGET).elf
BIN := $(BUILD)/$(TARGET).bin
MAP := $(BUILD)/$(TARGET).map
DFU := ../$(TARGET).dfu

INCLUDES := \
	-IInc \
	-IDrivers/STM32F0xx_HAL_Driver/Inc \
	-IDrivers/STM32F0xx_HAL_Driver/Inc/Legacy \
	-IMiddlewares/Third_Party/FatFs/src \
	-IMiddlewares/ST/STM32_USB_Device_Library/Core/Inc \
	-IMiddlewares/ST/STM32_USB_Device_Library/Class/CustomHID/Inc \
	-IDrivers/CMSIS/Device/ST/STM32F0xx/Include \
	-IDrivers/CMSIS/Include

CFLAGS := -mcpu=cortex-m0 -mthumb -mfloat-abi=soft -std=c99 -Os -g3 \
	-ffunction-sections -fdata-sections --specs=nano.specs \
	-DUSE_HAL_DRIVER -DSTM32F072xB $(INCLUDES) \
	-Wall -Wextra -Wno-unused-parameter
LDFLAGS := -mcpu=cortex-m0 -mthumb -mfloat-abi=soft -Os -nostartfiles \
	--specs=nosys.specs --specs=nano.specs \
	-Wl,-T,gcc/STM32F072xB_FLASH.ld -Wl,-Map,$(MAP) \
	-Wl,--gc-sections -Wl,--print-memory-usage

SRCS := \
	gcc/runtime.c Src/main.c Src/user_diskio.c Src/fatfs.c Src/usb_device.c Src/usbd_conf.c \
	Src/usbd_desc.c Src/usbd_custom_hid_if.c Src/stm32f0xx_it.c \
	Src/stm32f0xx_hal_msp.c Src/fonts.c Src/ssd1306.c Src/sd_util.c Src/cQueue.c \
	Src/input_task.c Src/shared.c Src/neopixel.c Src/keyboard.c Src/keypress_task.c \
	Src/profiles.c Src/ui_task.c Src/hid_task.c Src/md5.c Src/ds_vm.c Src/dsb_cache.c \
	Src/system_stm32f0xx.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_cortex.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_dma.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_exti.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_flash.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_flash_ex.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_gpio.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_i2c.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_i2c_ex.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_pcd.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_pcd_ex.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_pwr.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_pwr_ex.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_rcc.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_rcc_ex.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_rtc.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_rtc_ex.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_spi.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_spi_ex.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_tim.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_tim_ex.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_uart.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_hal_uart_ex.c \
	Drivers/STM32F0xx_HAL_Driver/Src/stm32f0xx_ll_usb.c \
	Middlewares/Third_Party/FatFs/src/diskio.c \
	Middlewares/Third_Party/FatFs/src/ff.c \
	Middlewares/Third_Party/FatFs/src/ff_gen_drv.c \
	Middlewares/Third_Party/FatFs/src/option/syscall.c \
	Middlewares/Third_Party/FatFs/src/option/ccsbcs.c \
	Middlewares/ST/STM32_USB_Device_Library/Core/Src/usbd_core.c \
	Middlewares/ST/STM32_USB_Device_Library/Core/Src/usbd_ctlreq.c \
	Middlewares/ST/STM32_USB_Device_Library/Core/Src/usbd_ioreq.c \
	Middlewares/ST/STM32_USB_Device_Library/Class/CustomHID/Src/usbd_customhid.c

OBJS := $(SRCS:%.c=$(BUILD)/%.o) $(BUILD)/gcc/startup_stm32f072xb.o
.DEFAULT_GOAL := all
.PHONY: all clean print-size check

check: all
	python3 gcc/check_behavior.py
all: $(DFU)
	python3 gcc/check_image.py $(ELF) $(DFU)

$(DFU): $(BIN)
	$(DFUSE_PACK) -b 0x08000000:$(BIN) -D 0x0483:0xdf11 $@

$(BIN): $(ELF)
	$(OBJCOPY) -O binary $< $@

$(ELF): $(OBJS) gcc/STM32F072xB_FLASH.ld Makefile
	@mkdir -p $(dir $@)
	$(CC) $(OBJS) $(LDFLAGS) -o $@
	$(SIZE) $@

$(BUILD)/%.o: %.c Makefile
	@mkdir -p $(dir $@)
	$(CC) $(CFLAGS) -MMD -MP -c $< -o $@

$(BUILD)/gcc/%.o: gcc/%.s
	@mkdir -p $(dir $@)
	$(CC) -mcpu=cortex-m0 -mthumb -x assembler-with-cpp -c $< -o $@

print-size: $(ELF)
	$(SIZE) $(ELF)
clean:
	rm -rf $(BUILD)

-include $(OBJS:.o=.d)
