ENTRY(_start)

_stack_size = 0x80000;

/* 2MB arena size */
_arena_size = 0x200000;

/* Maximum payload size */
_max_payload_size = 64*1024*1024;

/* We are actually relocatable */
. = 0;

PHDRS
{
  text PT_LOAD;
  rodata PT_LOAD;
  data PT_LOAD;
}

SECTIONS {
    _base = .;
    _text_start = .;
    .init : ALIGN(0x4000) {
        *(.init)
        *(.init.*)
        /* Function pointers should be naturally aligned */
        . = ALIGN(8);
        _initcall_start = .;
        KEEP(*(.initcall));
        KEEP(*(.initcall.prio4.*));
        KEEP(*(.initcall.prio3.*));
        KEEP(*(.initcall.prio2.*));
        KEEP(*(.initcall.prio1.*));
        KEEP(*(.initcall.prio0.*));
        KEEP(*(.initcall.*));
        _initcall_end = .;
    } :text
    .text : ALIGN(0x4000) {
        *(.text)
        *(.text.*)
        . = ALIGN(8);
        *(.got.plt)
        . = ALIGN(0x4000);
    } :text
    _text_end = .;
    _text_size = . - _text_start;
    _rodata_start = .;
    .rodata : ALIGN(0x4000) {
        *(.rodata)
        *(.rodata.*)
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
        . = ALIGN(0x4000);
    } :rodata
    _rodata_end = .;
    _data_start = .;
    .data : ALIGN(0x4000) {
        *(.data)
        *(.data.*)
        . = ALIGN(8);
        _got_start = .;
        *(.got)
        _got_end = .;
        . = ALIGN(0x4000);
        _file_end = .;
    } :data
    .bss : ALIGN(0x4000) {
        _bss_start = .;
        *(.bss)
        *(.bss.*)
        *(.dynbss)
        *(COMMON)
        . = ALIGN(0x4000);
        _bss_end = .;
        PROVIDE(_stack_top = .);
        . += _stack_size;
        PROVIDE(_stack_bot = .);
        . = ALIGN(0x4000);
    } :data
    __llvm_prf_cnts : {
        /* Append the LLVM profiling sections */
        . = ALIGN(4);
        PROVIDE(__start___llvm_prf_cnts = .);
        *(__llvm_prf_cnts)
        PROVIDE(__stop___llvm_prf_cnts = .);
    } :data
    __llvm_prf_data : {
        . = ALIGN(4);
        PROVIDE(__start___llvm_prf_data = .);
        *(__llvm_prf_data)
        PROVIDE(__stop___llvm_prf_data = .);
    } :data
    __llvm_prf_names : {
        . = ALIGN(4);
        PROVIDE(__start___llvm_prf_names = .);
        *(__llvm_prf_names)
        PROVIDE(__stop___llvm_prf_names = .);
    } :data
    __llvm_prf_vnds : {
        . = ALIGN(4);
        PROVIDE(__start___llvm_prf_vnds = .);
        *(__llvm_prf_vnds)
        PROVIDE(__stop___llvm_prf_vnds = .);
    } :data
    _data_end = .;
    _data_size = . - _data_start;

    .arena : ALIGN(0x4000) {
        _arena_start = .;
        . += _arena_size;
    } :data
    _arena_end = .;
    _end = .;

    _payload_start = .;
    . += _max_payload_size;
    _payload_end = .;

    .symtab 0 : { *(.symtab) }
    .strtab 0 : { *(.strtab) }
    .shstrtab 0 : { *(.shstrtab) }

    /DISCARD/ : {
        *(.discard)
        *(.discard.*)
        *(.interp .dynamic)
        *(.dynsym .dynstr .hash .gnu.hash)
        *(.eh_frame)
        *(.gnu.version*)
        *(.note*)
        *(.comment*)
    }

    .empty (NOLOAD) : {
        *(.plt) *(.plt.*) *(.iplt) *(.igot)
        *(.data.rel.ro)
    }
    ASSERT(SIZEOF(.empty) == 0, "Unexpected sections detected!")

    .got.plt (NOLOAD) : {
        *(.got.plt)
    }
    ASSERT(SIZEOF(.got.plt) == 0 || SIZEOF(.got.plt) == 0x18, "Unexpected GOT PLT detected!")

    .stab          0 : { *(.stab) }
    .stabstr       0 : { *(.stabstr) }
    .stab.excl     0 : { *(.stab.excl) }
    .stab.exclstr  0 : { *(.stab.exclstr) }
    .stab.index    0 : { *(.stab.index) }
    .stab.indexstr 0 : { *(.stab.indexstr) }
    .comment       0 : { *(.comment) }
    .debug          0 : { *(.debug) }
    .line           0 : { *(.line) }
    .debug_srcinfo  0 : { *(.debug_srcinfo) }
    .debug_sfnames  0 : { *(.debug_sfnames) }
    .debug_aranges  0 : { *(.debug_aranges) }
    .debug_pubnames 0 : { *(.debug_pubnames) }
    .debug_info     0 : { *(.debug_info .gnu.linkonce.wi.*) }
    .debug_abbrev   0 : { *(.debug_abbrev) }
    .debug_line     0 : { *(.debug_line) }
    .debug_frame    0 : { *(.debug_frame) }
    .debug_str      0 : { *(.debug_str) }
    .debug_loc      0 : { *(.debug_loc) }
    .debug_macinfo  0 : { *(.debug_macinfo) }
    .debug_weaknames 0 : { *(.debug_weaknames) }
    .debug_funcnames 0 : { *(.debug_funcnames) }
    .debug_typenames 0 : { *(.debug_typenames) }
    .debug_varnames  0 : { *(.debug_varnames) }
    .debug_pubtypes 0 : { *(.debug_pubtypes) }
    .debug_ranges   0 : { *(.debug_ranges) }
    .debug_types    0 : { *(.debug_types) }
    .debug_addr     0 : { *(.debug_addr) }
    .debug_line_str 0 : { *(.debug_line_str) }
    .debug_loclists 0 : { *(.debug_loclists) }
    .debug_macro    0 : { *(.debug_macro) }
    .debug_names    0 : { *(.debug_names) }
    .debug_rnglists 0 : { *(.debug_rnglists) }
    .debug_str_offsets 0 : { *(.debug_str_offsets) }
    .debug_sup      0 : { *(.debug_sup) }
    .__llvm_covmap  0 : { *(.__llvm_covmap) }
    .__llvm_covfun  0 : { *(.__llvm_covfun) }
    .__llvm_orderfile  0 : { *(.__llvm_orderfile) }
}

PROVIDE(__llvm_profile_runtime = 0);
