.extern kernel_main
.extern _stack_bot

.section .init, "ax"

.globl _start
.type _start, @function
_start:
    mov x9, x0

    adrp x1, _stack_bot
    mov sp, x1
    mov x0, x9
    mov x1, x10
    bl kernel_main
    b .
