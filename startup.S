.extern kernel_main
.extern _stack_bot
.extern _bss_start
.extern _bss_end

.section .init, "ax"

.globl _start
.type _start, @function
_start:
    mov x9, x0

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

_jump_to_kernel_main:
    mov x0, x9
    mov x1, x10
    bl kernel_main

_infinite_loop:
    b .
