/* App region behind 1010music's installer (0x08000000-0x0803FFFF). Never link lower. */
MEMORY
{
    FLASH : ORIGIN = 0x08040000, LENGTH = 768K  /* to the end of bank 1 */
    RAM   : ORIGIN = 0x24000000, LENGTH = 512K  /* AXI SRAM */
}
