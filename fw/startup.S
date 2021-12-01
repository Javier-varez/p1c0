.extern start_rust
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

_jump_to_start_rust:
    mov x0, x19
    mov x1, x20
    adrp x2, _stack_bot
    bl start_rust

_infinite_loop:
    b .

.globl _memcpy128_aligned
.type _memcpy128_aligned, @function
_memcpy128_aligned:
    ands    x2, x2, #~15
    beq     memcpy128_done
memcpy128_body:
    ldp     x3, x4, [x1], #16
    stp     x3, x4, [x0], #16
    subs    x2, x2, #16
    bne     memcpy128_body
memcpy128_done:
    ret
