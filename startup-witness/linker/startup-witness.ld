ENTRY(Reset_Handler)

MEMORY
{
    WITNESS (rx) : ORIGIN = 0x10000000, LENGTH = 16K
    RAM (rwx) : ORIGIN = 0x20010000, LENGTH = 4K
}

SECTIONS
{
    .vector_table ORIGIN(WITNESS) : ALIGN(4)
    {
        KEEP(*(.vector_table))
    } > WITNESS

    .text : ALIGN(4)
    {
        *(.text.Reset_Handler)
        *(.text.SysTick_Handler)
        *(.text .text.*)
        *(.rodata .rodata.*)
        . = ALIGN(4);
    } > WITNESS

    .ARM.extab : ALIGN(4)
    {
        *(.ARM.extab* .gnu.linkonce.armextab.*)
    } > WITNESS

    .ARM.exidx : ALIGN(4)
    {
        *(.ARM.exidx* .gnu.linkonce.armexidx.*)
    } > WITNESS

    .data : ALIGN(4)
    {
        *(.data .data.*)
        . = ALIGN(4);
    } > RAM AT> WITNESS

    .bss (NOLOAD) : ALIGN(4)
    {
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(4);
    } > RAM

    /DISCARD/ :
    {
        *(.comment)
        *(.note .note.*)
        *(.eh_frame .eh_frame.*)
    }

    ASSERT(SIZEOF(.vector_table) == 0xC0, "witness vector table must contain 48 words")
    ASSERT(SIZEOF(.vector_table) + SIZEOF(.text) + SIZEOF(.ARM.extab)
        + SIZEOF(.ARM.exidx) + SIZEOF(.data) <= LENGTH(WITNESS),
        "witness exceeds its 16 KiB link budget")
    ASSERT(SIZEOF(.data) + SIZEOF(.bss) <= LENGTH(RAM),
        "witness exceeds its 4 KiB RAM budget")
}
