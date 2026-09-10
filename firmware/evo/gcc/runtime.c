#include <errno.h>
#include <stddef.h>
#include <stdint.h>
#include <reent.h>
#include "main.h"

extern char __heap_start, __heap_end;
static char *heap_break = &__heap_start;

/* Never let newlib's heap consume the reserved interrupt/foreground stack. */
void *_sbrk(ptrdiff_t increment)
{
  char *previous = heap_break;
  if(increment >= 0)
  {
    if((uintptr_t)increment > (uintptr_t)&__heap_end - (uintptr_t)heap_break)
    {
      errno = ENOMEM;
      return (void *)-1;
    }
  }
  else if((uintptr_t)(-(increment + 1)) + 1 > (uintptr_t)heap_break - (uintptr_t)&__heap_start)
  {
    errno = ENOMEM;
    return (void *)-1;
  }
  heap_break += increment;
  return previous;
}

static uint32_t malloc_irq_state;
static unsigned malloc_lock_depth;

void __malloc_lock(struct _reent *reent)
{
  (void)reent;
  uint32_t state = __get_PRIMASK();
  __disable_irq();
  if(malloc_lock_depth++ == 0)
    malloc_irq_state = state;
}

void __malloc_unlock(struct _reent *reent)
{
  (void)reent;
  if(--malloc_lock_depth == 0)
    __set_PRIMASK(malloc_irq_state);
}

/* Match the Keil fputc UART sink; newlib printf uses _write instead. */
int _write(int fd, const void *buffer, size_t length)
{
  if(fd != 1 && fd != 2)
  {
    errno = EBADF;
    return -1;
  }
  const uint8_t *data = buffer;
  for(size_t offset = 0; offset < length;)
  {
    uint16_t chunk = length - offset > UINT16_MAX ? UINT16_MAX : length - offset;
    if(HAL_UART_Transmit(&huart1, (uint8_t *)(data + offset), chunk, 100) != HAL_OK)
    {
      errno = EIO;
      return offset ? (int)offset : -1;
    }
    offset += chunk;
  }
  return (int)length;
}
