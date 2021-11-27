.extern kernel_main
.extern _stack_bot
.extern _bss_start
.extern _bss_end

.section .init, "ax"

.globl _start
.type _start, @function
_start:
    mov x19, x0

_clear_bss:
    adrp x0, _bss_start
    adrp x1, _bss_end
    sub x1, x1, x0
    ands x1, x1, #~7
    beq _clear_bss_done
_bss_loop:
    str xzr, [x0], #8
    subs x1, x1, 8
    bne _bss_loop
_clear_bss_done:

_configure_stack:
    adrp x1, _stack_bot
    mov sp, x1

_apply_relocations:
    adrp x0, _base
    mov x20, x0
    adrp x1, _rela_start
    add x1, x1, :lo12:_rela_start
    adrp x2, _rela_end
    add x2, x2, :lo12:_rela_end
    sub x2, x2, x1
    bl apply_rela

_jump_to_kernel_main:
    mov x0, x19
    mov x1, x20
    bl kernel_main

_infinite_loop:
    b .
