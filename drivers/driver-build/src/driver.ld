ENTRY(_start)

. = 0;

PHDRS
{
  text PT_LOAD;
  rodata PT_LOAD;
  data PT_LOAD;
}

SECTIONS {
    _text_start = .;
    .text : ALIGN(0x4000) {
        *(.text)
        *(.text.*)
        . = ALIGN(8);
        *(.got.plt)
    } :text
    _text_end = .;
    _text_size = . - _text_start;
    . = ALIGN(0x4000);

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
    } :rodata
    _rodata_end = .;
    . = ALIGN(0x4000);

    _data_start = .;
    .data : ALIGN(0x4000) {
        *(.data)
        *(.data.*)
        . = ALIGN(8);
        _got_start = .;
        *(.got)
        _got_end = .;
    } :data
    .bss : {
        _bss_start = .;
        *(.bss)
        *(.bss.*)
        *(.dynbss)
        *(COMMON)
        _bss_end = .;
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
    _end = .;

    .__llvm_covmap  0 : { *(.__llvm_covmap) }
    .__llvm_covfun  0 : { *(.__llvm_covfun) }
    .__llvm_orderfile  0 : { *(.__llvm_orderfile) }
}

PROVIDE(__llvm_profile_runtime = 0);
