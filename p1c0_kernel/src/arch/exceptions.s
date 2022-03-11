// x0 will be pointing to a struct with the following layout:
//   * ELR_EL1
//   * SPSR_EL1
//   * ESR_EL1
//   * SP_EL0
//   * registers[0:30]

.macro el1_save_context_and_call_handler handler stringlabel
    // push all general purpose registers on the stack.
    sub sp, sp, 0x120
    str x30, [sp, #0x110]
    stp x28, x29, [sp, #0x100]
    stp x26, x27, [sp, #0xF0]
    stp x24, x25, [sp, #0xE0]
    stp x22, x23, [sp, #0xD0]
    stp x20, x21, [sp, #0xC0]
    stp x18, x19, [sp, #0xB0]
    stp x16, x17, [sp, #0xA0]
    stp x14, x15, [sp, #0x90]
    stp x12, x13, [sp, #0x80]
    stp x10, x11, [sp, #0x70]
    stp x8,  x9,  [sp, #0x60]
    stp x6,  x7,  [sp, #0x50]
    stp x4,  x5,  [sp, #0x40]
    stp x2,  x3,  [sp, #0x30]
    stp x0,  x1,  [sp, #0x20]

    mrs x1,  ELR_EL1
    mrs x2,  SPSR_EL1
    mrs x3,  ESR_EL1
    mrs x4,  SP_EL0

    stp x3, x4, [sp, #0x10]
    stp x1, x2, [sp, #0x00]

    mov x0,  sp
    bl \handler

    b __exception_restore_context
.endm

// We need to align to 2048 bytes the exception table
.align 11

.globl __exception_vector_start
__exception_vector_start:

// Current EL with SP_EL0
.org 0x000
.p2align 7
    el1_save_context_and_call_handler current_el0_synchronous current_el0_synchronous_str
.p2align 7
    el1_save_context_and_call_handler current_el0_irq current_el0_irq_str
.p2align 7
    el1_save_context_and_call_handler current_el0_fiq current_el0_fiq_str
.p2align 7
    el1_save_context_and_call_handler current_el0_serror current_el0_serror_str

// Current EL with SP_ELx, x > 0
.p2align 7
    el1_save_context_and_call_handler current_elx_synchronous current_elx_synchronous_str
.p2align 7
    el1_save_context_and_call_handler current_elx_irq current_elx_irq_str
.p2align 7
    el1_save_context_and_call_handler current_elx_fiq current_elx_fiq_str
.p2align 7
    el1_save_context_and_call_handler current_elx_serror current_elx_serror_str

// Lower EL in AARCH64
.p2align 7
    el1_save_context_and_call_handler lower_el_aarch64_synchronous lower_el_aarch64_synchronous_str
.p2align 7
    el1_save_context_and_call_handler lower_el_aarch64_irq lower_el_aarch64_irq_str
.p2align 7
    el1_save_context_and_call_handler lower_el_aarch64_fiq lower_el_aarch64_fiq_str
.p2align 7
    el1_save_context_and_call_handler lower_el_aarch64_serror lower_el_aarch64_serror_str

// Lower EL in AARCH32
.p2align 7
    el1_save_context_and_call_handler lower_el_aarch32_synchronous lower_el_aarch32_synchronous_str
.p2align 7
    el1_save_context_and_call_handler lower_el_aarch32_irq lower_el_aarch32_irq_str
.p2align 7
    el1_save_context_and_call_handler lower_el_aarch32_fiq lower_el_aarch32_fiq_str
.p2align 7
    el1_save_context_and_call_handler lower_el_aarch32_serror lower_el_aarch32_serror_str

__exception_restore_context:
    ldp x0, x1, [sp, #0x00]
    ldp x2, x3, [sp, #0x10]

    msr ELR_EL1,  x0
    msr SPSR_EL1, x1
    msr SP_EL0, x3

    ldp x0,  x1,  [sp, #0x20]
    ldp x2,  x3,  [sp, #0x30]
    ldp x4,  x5,  [sp, #0x40]
    ldp x6,  x7,  [sp, #0x50]
    ldp x8,  x9,  [sp, #0x60]
    ldp x10, x11, [sp, #0x70]
    ldp x12, x13, [sp, #0x80]
    ldp x14, x15, [sp, #0x90]
    ldp x16, x17, [sp, #0xA0]
    ldp x18, x19, [sp, #0xB0]
    ldp x20, x21, [sp, #0xC0]
    ldp x22, x23, [sp, #0xD0]
    ldp x24, x25, [sp, #0xE0]
    ldp x26, x27, [sp, #0xF0]
    ldp x28, x29, [sp, #0x100]
    ldr x30, [sp, #0x110]

    add sp, sp, 0x120
    eret

.size    __exception_restore_context, . - __exception_restore_context
.type    __exception_restore_context, function

.macro el2_save_context_and_call_handler stringlabel
    // push all general purpose registers on the stack.
    sub sp, sp, 0x100
    str x30, [sp, #0xF0]
    stp x28, x29, [sp, #0xE0]
    stp x26, x27, [sp, #0xD0]
    stp x24, x25, [sp, #0xC0]
    stp x22, x23, [sp, #0xB0]
    stp x20, x21, [sp, #0xA0]
    stp x18, x19, [sp, #0x90]
    stp x16, x17, [sp, #0x80]
    stp x14, x15, [sp, #0x70]
    stp x12, x13, [sp, #0x60]
    stp x10, x11, [sp, #0x50]
    stp x8,  x9,  [sp, #0x40]
    stp x6,  x7,  [sp, #0x30]
    stp x4,  x5,  [sp, #0x20]
    stp x2,  x3,  [sp, #0x10]
    stp x0,  x1,  [sp, #0x00]

    mov x0,  sp
    adr x1, \stringlabel
    bl debug_handler

    // Halt execution
    b .
.endm

// We need to align to 2048 bytes the exception table
.align 11

.globl __el2_exception_vector_start
__el2_exception_vector_start:

// Current EL with SP_EL0
.p2align 7
    el2_save_context_and_call_handler current_el0_synchronous_str
.p2align 7
    el2_save_context_and_call_handler current_el0_irq_str
.p2align 7
    el2_save_context_and_call_handler current_el0_fiq_str
.p2align 7
    el2_save_context_and_call_handler current_el0_serror_str

// Current EL with SP_ELx, x > 0
.p2align 7
    el2_save_context_and_call_handler current_elx_synchronous_str
.p2align 7
    el2_save_context_and_call_handler current_elx_irq_str
.p2align 7
    el2_save_context_and_call_handler current_elx_fiq_str
.p2align 7
    el2_save_context_and_call_handler current_elx_serror_str

// Lower EL in AARCH64
.p2align 7
    el2_save_context_and_call_handler lower_el_aarch64_synchronous_str
.p2align 7
    el2_save_context_and_call_handler lower_el_aarch64_irq_str
.p2align 7
    el2_save_context_and_call_handler lower_el_aarch64_fiq_str
.p2align 7
    el2_save_context_and_call_handler lower_el_aarch64_serror_str

// Lower EL in AARCH32
.p2align 7
    el2_save_context_and_call_handler lower_el_aarch32_synchronous_str
.p2align 7
    el2_save_context_and_call_handler lower_el_aarch32_irq_str
.p2align 7
    el2_save_context_and_call_handler lower_el_aarch32_fiq_str
.p2align 7
    el2_save_context_and_call_handler lower_el_aarch32_serror_str

// Utility functions for exception debugging
get_current_el_str:
    mrs x0, CurrentEL
    cmp x0, #0b0000
    beq 1f
    cmp x0, #0b0100
    beq 2f
    cmp x0, #0b1000
    beq 3f
    adr x0, el_3_str
    ret
1:  adr x0, el_0_str
    ret
2:  adr x0, el_1_str
    ret
3:  adr x0, el_2_str
    ret

el_0_str:
    .asciz "EL0"
el_1_str:
    .asciz "EL1"
el_2_str:
    .asciz "EL2"
el_3_str:
    .asciz "EL2"

load_elr:
    mrs x0, CurrentEL
    cmp x0, #0b0000
    beq 1f
    cmp x0, #0b0100
    beq 2f
    cmp x0, #0b1000
    beq 3f
    mrs x0, ELR_EL3
    ret
1:  mrs x0, ELR_EL1
    ret
2:  mrs x0, ELR_EL1
    ret
3:  mrs x0, ELR_EL2
    ret

load_spsr:
    mrs x0, CurrentEL
    cmp x0, #0b0000
    beq 1f
    cmp x0, #0b0100
    beq 2f
    cmp x0, #0b1000
    beq 3f
    mrs x0, SPSR_EL3
    ret
1:  mrs x0, SPSR_EL1
    ret
2:  mrs x0, SPSR_EL1
    ret
3:  mrs x0, SPSR_EL2
    ret

load_esr:
    mrs x0, CurrentEL
    cmp x0, #0b0000
    beq 1f
    cmp x0, #0b0100
    beq 2f
    cmp x0, #0b1000
    beq 3f
    mrs x0, ESR_EL3
    ret
1:  mrs x0, ESR_EL1
    ret
2:  mrs x0, ESR_EL1
    ret
3:  mrs x0, ESR_EL2
    ret

load_far:
    mrs x0, CurrentEL
    cmp x0, #0b0000
    beq 1f
    cmp x0, #0b0100
    beq 2f
    cmp x0, #0b1000
    beq 3f
    mrs x0, FAR_EL3
    ret
1:  mrs x0, FAR_EL1
    ret
2:  mrs x0, FAR_EL1
    ret
3:  mrs x0, FAR_EL2
    ret

print_registers:
    str x30, [sp, #-8]!
    str x19, [sp, #-8]!
    str x20, [sp, #-8]!

    mov x19, x0
    mov x20, #0

1:
    mov x0, 'R'
    bl _uart_putc
    mov x0, '['
    bl _uart_putc
    mov x0, x20
    bl _uart_puthex
    mov x0, ']'
    bl _uart_putc
    mov x0, ':'
    bl _uart_putc
    mov x0, ' '
    bl _uart_putc
    ldr x0, [x19]
    bl _uart_puthex
    bl _uart_putendl

    add x19, x19, #8
    add x20, x20, #1
    cmp x20, #31
    bne 1b

    ldr x20, [sp], #8
    ldr x19, [sp], #8
    ldr x30, [sp], #8
    ret

// x0: Regs array pointer
// x1: Exception string
debug_handler:
    str x30, [sp, #-8]!
    str x19, [sp, #-8]!
    str x20, [sp, #-8]!

    mov x19, x0
    mov x20, x1

    adr x0, exception_start_str
    bl _uart_puts

    adr x0, exception_level_str
    bl _uart_puts

    bl get_current_el_str
    bl _uart_puts
    bl _uart_putendl

    // Print exception string
    adr x0, exception_type_str
    bl _uart_puts
    mov x0, x20
    bl _uart_puts
    bl _uart_putendl

    // Print registers
    mov x0, x19
    bl print_registers

    // Print ELR_ELX
    adr x0, elr_elx_str
    bl  _uart_puts

    bl load_elr
    bl _uart_puthex
    bl _uart_putendl

    // Print SPSR_ELX
    adr x0, spsr_elx_str
    bl  _uart_puts

    bl load_spsr
    bl _uart_puthex
    bl _uart_putendl

    // Print ESR_ELX
    adr x0, esr_elx_str
    bl  _uart_puts

    bl load_esr
    bl _uart_puthex
    bl _uart_putendl

    // Print FAR_ELX
    adr x0, far_elx_str
    bl  _uart_puts

    bl load_far
    bl _uart_puthex
    bl _uart_putendl

    adr x0, exception_end_str
    bl _uart_puts

    ldr x20, [sp], #8
    ldr x19, [sp], #8
    ldr x30, [sp], #8
    ret

exception_start_str:
    .asciz "======================== Exception Frame ========================\n"

exception_end_str:
    .asciz "=================================================================\n"

exception_level_str:
    .asciz "Exception level: "

exception_type_str:
    .asciz "Exception type: "

current_el0_synchronous_str:
    .asciz "current_el0_synchronous\n"
current_el0_irq_str:
    .asciz "current_el0_irq\n"
current_el0_fiq_str:
    .asciz "current_el0_fiq\n"
current_el0_serror_str:
    .asciz "current_el0_serror\n"

current_elx_synchronous_str:
    .asciz "current_elx_synchronous\n"
current_elx_irq_str:
    .asciz "current_elx_irq\n"
current_elx_fiq_str:
    .asciz "current_elx_fiq\n"
current_elx_serror_str:
    .asciz "current_elx_serror\n"

lower_el_aarch64_synchronous_str:
    .asciz "lower_el_aarch64_synchronous\n"
lower_el_aarch64_irq_str:
    .asciz "lower_el_aarch64_irq\n"
lower_el_aarch64_fiq_str:
    .asciz "lower_el_aarch64_fiq\n"
lower_el_aarch64_serror_str:
    .asciz "lower_el_aarch64_serror\n"

lower_el_aarch32_synchronous_str:
    .asciz "lower_el_aarch32_synchronous\n"
lower_el_aarch32_irq_str:
    .asciz "lower_el_aarch32_irq\n"
lower_el_aarch32_fiq_str:
    .asciz "lower_el_aarch32_fiq\n"
lower_el_aarch32_serror_str:
    .asciz "lower_el_aarch32_serror\n"

elr_elx_str:
    .asciz "ELR_ELx: "
spsr_elx_str:
    .asciz "SPSR_ELx: "
esr_elx_str:
    .asciz "ESR_ELx: "
far_elx_str:
    .asciz "FAR_ELx: "
