ENTRY(main)

MEMORY
{
    FLASH (rx) : ORIGIN = 0x00000000, LENGTH = 60K
    RAM (rwx) : ORIGIN = 0x20000000, LENGTH = 16K
}

SECTIONS
{
    .text :
    {
        KEEP(*(.text.main))
        *(.text*)
        *(.rodata*)
    } > FLASH

    .ARM.exidx :
    {
        *(.ARM.exidx*)
    } > FLASH

    .data :
    {
        *(.data*)
    } > RAM AT> FLASH

    .bss (NOLOAD) :
    {
        *(.bss*)
        *(COMMON)
    } > RAM

    /DISCARD/ :
    {
        *(.comment*)
        *(.note*)
    }
}
