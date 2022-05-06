#define UART_BASE 0x39b200000

#define UTRSTAT 0x010
#define UTXH 0x020

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
    mov x1, x0
    mov x20, x0
    adrp x2, _rela_start
    add x2, x2, :lo12:_rela_start
    adrp x3, _rela_end
    add x3, x3, :lo12:_rela_end
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

.globl _uart_putc
.type _uart_putc, @function
_uart_putc:
#ifdef UART_BASE
    ldr x1, =UART_BASE

1:
    ldr w2, [x1, UTRSTAT]
    tst w2, #2
    beq 1b
    str w0, [x1, UTXH]
#endif
    ret

.globl _uart_puts
.type _uart_puts, @function
_uart_puts:
    str x30, [sp, #-8]!
    str x19, [sp, #-8]!
    cbz x0, 2f
    mov x19, x0
1:
    ldrb w0, [x19]
    cbz w0, 2f
    bl _uart_putc
    add x19, x19, #1
    b 1b
2:
    ldr x19, [sp], #8
    ldr x30, [sp], #8
    ret

.globl _uart_puthex
.type _uart_puthex, @function
_uart_puthex:
    str x30, [sp, #-8]!
    str x19, [sp, #-8]!
    str x20, [sp, #-8]!
    mov x19, x0
    mov x20, #60

    mov x0, '0'
    bl _uart_putc
    mov x0, 'x'
    bl _uart_putc

1:
    mov x0, x19
    lsr x0, x0, x20
    and x0, x0, #0xF
    cmp x0, #10
    b.ge 2f
    add x0, x0, '0'
    b 3f
2:
    add x0, x0, 'A' - 10
3:
    bl _uart_putc
    cbz x20, 4f
    sub x20, x20, #4
    b 1b
4:
    ldr x20, [sp], #8
    ldr x19, [sp], #8
    ldr x30, [sp], #8
    ret

.globl _uart_putendl
.type _uart_putendl, @function
_uart_putendl:
    mov x0, '\n'
    b _uart_putc

// Old Base x0
// Base x1
// rela Start x2
// rela End x3
.globl apply_rela
.type apply_rela, @function
apply_rela:
    str x30, [sp, #-8]!
    str x19, [sp, #-8]!
    str x20, [sp, #-8]!

1:
    // If begin and end ptrs are equal we are at the end of the relocation list
    cmp x2, x3
    bhs 3f

    // Load in x19 the rela.type
    ldr x19, [x2, #8]

    // Check if type is 1027
    mov x30, #1027
    cmp x19, x30
    bne 2f

    // It is a relative relocation, handle it
    // ptr = Old base + rela.offset
    ldr x19, [x2, #0]
    add x19, x19, x0

    // value = base + rela.addend
    ldr x20, [x2, #16]
    add x20, x20, x1

    str x20, [x19]

2:
    // Goto next entry
    add x2, x2, #24
    b 1b

3:
    ldr x20, [sp], #8
    ldr x19, [sp], #8
    ldr x30, [sp], #8
    ret

// This function assumes the new address is in high memory!
.globl relocate_and_jump_to_relocated_kernel
.type relocate_and_jump_to_relocated_kernel, @function
relocate_and_jump_to_relocated_kernel:
    str x4, [sp, #-8]!
    str x5, [sp, #-8]!

    bl apply_rela

    ldr x1, [sp], #8
    ldr x0, [sp], #8

    mov x30, x0
    mov sp, x1

    dsb sy
    ret
