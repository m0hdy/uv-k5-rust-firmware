ENTRY(Reset_Handler)

MEMORY
{
    FLASH (rx)  : ORIGIN = 0x00000000, LENGTH = 60K
    RAM   (rwx) : ORIGIN = 0x20000000, LENGTH = 16K
}

SECTIONS
{
    .vector_table ORIGIN(FLASH) : ALIGN(4)
    {
        __vector_table_start = .;
        KEEP(*(.vector_table))
        __vector_table_end = .;
    } > FLASH

    .text : ALIGN(4)
    {
        *(.text .text.*)
        *(.rodata .rodata.*)
        . = ALIGN(4);
    } > FLASH

    .ARM.extab : ALIGN(4)
    {
        *(.ARM.extab* .gnu.linkonce.armextab.*)
    } > FLASH

    .ARM.exidx : ALIGN(4)
    {
        *(.ARM.exidx* .gnu.linkonce.armexidx.*)
    } > FLASH

    .data : ALIGN(4)
    {
        __data_start = .;
        *(.data .data.*)
        . = ALIGN(4);
        __data_end = .;
    } > RAM AT> FLASH
    __data_load = LOADADDR(.data);

    .bss (NOLOAD) : ALIGN(4)
    {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(4);
        __bss_end = .;
    } > RAM

    .stack ORIGIN(RAM) + LENGTH(RAM) - 0x800 (NOLOAD) : ALIGN(8)
    {
        __stack_start = .;
        . += 0x800;
        __stack_end = .;
    } > RAM

    __stack_top = ORIGIN(RAM) + LENGTH(RAM);

    /DISCARD/ :
    {
        *(.comment)
        *(.note .note.*)
        *(.eh_frame .eh_frame.*)
    }

    ASSERT(__vector_table_start == ORIGIN(FLASH), "vector table must start at flash origin")
    ASSERT(SIZEOF(.vector_table) == 0xC0, "Cortex-M0 vector table must contain 48 words")
    ASSERT(ORIGIN(FLASH) == 0 && LENGTH(FLASH) == 60K, "application flash range must be 60 KiB")
    ASSERT(ORIGIN(RAM) == 0x20000000 && LENGTH(RAM) == 16K, "RAM range must be 16 KiB")
    ASSERT(__bss_end <= __stack_start, "static RAM overlaps reserved stack")
}
