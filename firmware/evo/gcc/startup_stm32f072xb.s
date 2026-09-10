.syntax unified
.cpu cortex-m0
.thumb

.global g_pfnVectors
.global Default_Handler


.section .text.Reset_Handler,"ax",%progbits
.weak Reset_Handler
.type Reset_Handler, %function
.thumb_func
Reset_Handler:
  ldr r0, =_estack
  mov sp, r0
  bl SystemInit

  ldr r0, =_sdata
  ldr r1, =_edata
  ldr r2, =_sidata
.Lcopy_data:
  cmp r0, r1
  bcc .Lcopy_data_word
  b .Lzero_bss_setup
.Lcopy_data_word:
  ldr r3, [r2]
  str r3, [r0]
  adds r0, #4
  adds r2, #4
  b .Lcopy_data

.Lzero_bss_setup:
  ldr r0, =_sbss
  ldr r1, =_ebss
  movs r2, #0
.Lzero_bss:
  cmp r0, r1
  bcc .Lzero_bss_word
  b .Lrun_main
.Lzero_bss_word:
  str r2, [r0]
  adds r0, #4
  b .Lzero_bss

.Lrun_main:
  bl __libc_init_array
  bl main
.Lhang:
  b .Lhang
.size Reset_Handler, . - Reset_Handler

/* No CRT init/fini fragments: constructors are dispatched by libc above. */
.section .text._init,"ax",%progbits
.global _init
.type _init, %function
.thumb_func
_init:
  bx lr
.size _init, . - _init

.section .isr_vector,"a",%progbits
.type g_pfnVectors, %object
g_pfnVectors:
  .word _estack
  .word Reset_Handler
  .word NMI_Handler
  .word HardFault_Handler
  .word 0
  .word 0
  .word 0
  .word 0
  .word 0
  .word 0
  .word 0
  .word SVC_Handler
  .word 0
  .word 0
  .word PendSV_Handler
  .word SysTick_Handler
  .word WWDG_IRQHandler
  .word PVD_VDDIO2_IRQHandler
  .word RTC_IRQHandler
  .word FLASH_IRQHandler
  .word RCC_CRS_IRQHandler
  .word EXTI0_1_IRQHandler
  .word EXTI2_3_IRQHandler
  .word EXTI4_15_IRQHandler
  .word TSC_IRQHandler
  .word DMA1_Channel1_IRQHandler
  .word DMA1_Channel2_3_IRQHandler
  .word DMA1_Channel4_5_6_7_IRQHandler
  .word ADC1_COMP_IRQHandler
  .word TIM1_BRK_UP_TRG_COM_IRQHandler
  .word TIM1_CC_IRQHandler
  .word TIM2_IRQHandler
  .word TIM3_IRQHandler
  .word TIM6_DAC_IRQHandler
  .word TIM7_IRQHandler
  .word TIM14_IRQHandler
  .word TIM15_IRQHandler
  .word TIM16_IRQHandler
  .word TIM17_IRQHandler
  .word I2C1_IRQHandler
  .word I2C2_IRQHandler
  .word SPI1_IRQHandler
  .word SPI2_IRQHandler
  .word USART1_IRQHandler
  .word USART2_IRQHandler
  .word USART3_4_IRQHandler
  .word CEC_CAN_IRQHandler
  .word USB_IRQHandler
.size g_pfnVectors, . - g_pfnVectors

.weak NMI_Handler
.thumb_set NMI_Handler, Default_Handler
.weak HardFault_Handler
.thumb_set HardFault_Handler, Default_Handler
.weak SVC_Handler
.thumb_set SVC_Handler, Default_Handler
.weak PendSV_Handler
.thumb_set PendSV_Handler, Default_Handler
.weak SysTick_Handler
.thumb_set SysTick_Handler, Default_Handler
.weak WWDG_IRQHandler
.thumb_set WWDG_IRQHandler, Default_Handler
.weak PVD_VDDIO2_IRQHandler
.thumb_set PVD_VDDIO2_IRQHandler, Default_Handler
.weak RTC_IRQHandler
.thumb_set RTC_IRQHandler, Default_Handler
.weak FLASH_IRQHandler
.thumb_set FLASH_IRQHandler, Default_Handler
.weak RCC_CRS_IRQHandler
.thumb_set RCC_CRS_IRQHandler, Default_Handler
.weak EXTI0_1_IRQHandler
.thumb_set EXTI0_1_IRQHandler, Default_Handler
.weak EXTI2_3_IRQHandler
.thumb_set EXTI2_3_IRQHandler, Default_Handler
.weak EXTI4_15_IRQHandler
.thumb_set EXTI4_15_IRQHandler, Default_Handler
.weak TSC_IRQHandler
.thumb_set TSC_IRQHandler, Default_Handler
.weak DMA1_Channel1_IRQHandler
.thumb_set DMA1_Channel1_IRQHandler, Default_Handler
.weak DMA1_Channel2_3_IRQHandler
.thumb_set DMA1_Channel2_3_IRQHandler, Default_Handler
.weak DMA1_Channel4_5_6_7_IRQHandler
.thumb_set DMA1_Channel4_5_6_7_IRQHandler, Default_Handler
.weak ADC1_COMP_IRQHandler
.thumb_set ADC1_COMP_IRQHandler, Default_Handler
.weak TIM1_BRK_UP_TRG_COM_IRQHandler
.thumb_set TIM1_BRK_UP_TRG_COM_IRQHandler, Default_Handler
.weak TIM1_CC_IRQHandler
.thumb_set TIM1_CC_IRQHandler, Default_Handler
.weak TIM2_IRQHandler
.thumb_set TIM2_IRQHandler, Default_Handler
.weak TIM3_IRQHandler
.thumb_set TIM3_IRQHandler, Default_Handler
.weak TIM6_DAC_IRQHandler
.thumb_set TIM6_DAC_IRQHandler, Default_Handler
.weak TIM7_IRQHandler
.thumb_set TIM7_IRQHandler, Default_Handler
.weak TIM14_IRQHandler
.thumb_set TIM14_IRQHandler, Default_Handler
.weak TIM15_IRQHandler
.thumb_set TIM15_IRQHandler, Default_Handler
.weak TIM16_IRQHandler
.thumb_set TIM16_IRQHandler, Default_Handler
.weak TIM17_IRQHandler
.thumb_set TIM17_IRQHandler, Default_Handler
.weak I2C1_IRQHandler
.thumb_set I2C1_IRQHandler, Default_Handler
.weak I2C2_IRQHandler
.thumb_set I2C2_IRQHandler, Default_Handler
.weak SPI1_IRQHandler
.thumb_set SPI1_IRQHandler, Default_Handler
.weak SPI2_IRQHandler
.thumb_set SPI2_IRQHandler, Default_Handler
.weak USART1_IRQHandler
.thumb_set USART1_IRQHandler, Default_Handler
.weak USART2_IRQHandler
.thumb_set USART2_IRQHandler, Default_Handler
.weak USART3_4_IRQHandler
.thumb_set USART3_4_IRQHandler, Default_Handler
.weak CEC_CAN_IRQHandler
.thumb_set CEC_CAN_IRQHandler, Default_Handler
.weak USB_IRQHandler
.thumb_set USB_IRQHandler, Default_Handler

.section .text.Default_Handler,"ax",%progbits
.type Default_Handler, %function
.thumb_func
Default_Handler:
  b Default_Handler
