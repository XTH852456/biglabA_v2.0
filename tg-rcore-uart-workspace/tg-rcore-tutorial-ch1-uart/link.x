ENTRY(_start)
BASE = 0x80000000;

SECTIONS
{
    . = BASE;

    .text.init : {
        *(.text.init)
    }

    .text : {
        *(.text .text.*)
    }

    .rodata : {
        *(.rodata .rodata.*)
    }

    .data : {
        *(.data)
    }

    .bss : {
        *(COMMON)
        *(.bss)
    }

    . = ALIGN(8);
    __stack_top = . + 0x10000;
}