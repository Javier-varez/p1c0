ENTRY(_start)

. = 0x1000000;

PHDRS
{
  text PT_LOAD;
  rodata PT_LOAD;
  data PT_LOAD;
}

SECTIONS {
    .text : {
        PROVIDE_HIDDEN(__dso_handle = .);
        *(.text)
        *(.text.*)
        . = ALIGN(8);
        *(.got.plt)
    } :text

    . = ALIGN(0x4000);
    . += 0x4000;

    .rodata : {
        *(.rodata)
        *(.rodata.*)
        . = ALIGN(8);

        PROVIDE_HIDDEN(__preinit_array_start = .);
        KEEP(*(.preinit_array.*))
        KEEP(*(.preinit_array))
        PROVIDE_HIDDEN(__preinit_array_end = .);
        . = ALIGN(8);

        PROVIDE_HIDDEN(__init_array_start = .);
        KEEP(*(.init_array.*))
        KEEP(*(.init_array))
        PROVIDE_HIDDEN(__init_array_end = .);
        . = ALIGN(8);

        PROVIDE_HIDDEN(__fini_array_start = .);
        KEEP(*(.fini_array.*))
        KEEP(*(.fini_array))
        PROVIDE_HIDDEN(__fini_array_end = .);
        . = ALIGN(8);
    } :rodata

    .rela.dyn : {
        _rela_start = .;
        *(.rela)
        *(.rela.text)
        *(.rela.got)
        *(.rela.plt)
        *(.rela.bss)
        *(.rela.ifunc)
        *(.rela.text.*)
        *(.rela.data)
        *(.rela.data.*)
        *(.rela.dyn)
        *(.rela.*)
        _rela_end = .;
    } :rodata

    . = ALIGN(0x4000);
    . += 0x4000;

    .data : {
        *(.data)
        *(.data.*)

        . = ALIGN(8);

        *(.got)
    } :data

    .bss : {
        *(.bss)
        *(.bss.*)
        *(.dynbss)
        *(COMMON)
    } :data
}
