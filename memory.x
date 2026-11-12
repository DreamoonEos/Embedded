/* STM32H723VG: internal flash and a conservative 128 KiB AXI SRAM region.
   No bootloader offset. This example uses no peripheral DMA and enables no cache. */
MEMORY
{
  FLASH : ORIGIN = 0x08000000, LENGTH = 1024K
  RAM   : ORIGIN = 0x24000000, LENGTH = 128K
}
