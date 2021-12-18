// x0 will be pointing to a struct with the following layout:
//   * ELR_EL1
//   * SPSR_EL1
//   * ESR_EL1
//   * registers[0:30]

.macro save_context_and_call_handler handler
	// push all general purpose registers on the stack.
    str x30, [sp, #-8]!
	stp	x28, x29, [sp, #-16]!
	stp	x26, x27, [sp, #-16]!
	stp	x24, x25, [sp, #-16]!
	stp	x22, x23, [sp, #-16]!
	stp	x20, x21, [sp, #-16]!
	stp	x18, x19, [sp, #-16]!
	stp	x16, x17, [sp, #-16]!
	stp	x14, x15, [sp, #-16]!
	stp	x12, x13, [sp, #-16]!
	stp	x10, x11, [sp, #-16]!
	stp	x8,  x9,  [sp, #-16]!
	stp	x6,  x7,  [sp, #-16]!
	stp	x4,  x5,  [sp, #-16]!
	stp	x2,  x3,  [sp, #-16]!
	stp	x0,  x1,  [sp, #-16]!

	mrs	x1,  ELR_EL1
	mrs	x2,  SPSR_EL1
	mrs	x3,  ESR_EL1

	stp	x2, x3, [sp, #-16]!
	str	x1, [sp, #-8]!

	mov	x0,  sp

	bl	\handler

	b	__exception_restore_context
.endm

// Suspend enters a while true loop in a low power state (using wait for event)
.macro fiq_suspend
1:	wfe
	b	1b
.endm

// We need to align to 2048 bytes the exception table
.align 11

.globl __exception_vector_start
__exception_vector_start:

// Current EL with SP_EL0
.org 0x000
.p2align 7
	save_context_and_call_handler current_el0_synchronous
.p2align 7
	save_context_and_call_handler current_el0_irq
.p2align 7
	fiq_suspend
.p2align 7
	save_context_and_call_handler current_el0_serror

// Current EL with SP_ELx, x > 0
.p2align 7
	save_context_and_call_handler current_elx_synchronous
.p2align 7
	save_context_and_call_handler current_elx_irq
.p2align 7
	fiq_suspend
.p2align 7
	save_context_and_call_handler current_elx_serror

// Lower EL in AARCH64
.p2align 7
	save_context_and_call_handler lower_el_aarch64_synchronous
.p2align 7
	save_context_and_call_handler lower_el_aarch64_irq
.p2align 7
	fiq_suspend
.p2align 7
	save_context_and_call_handler lower_el_aarch64_serror

// Lower EL in AARCH32
.p2align 7
	save_context_and_call_handler lower_el_aarch32_synchronous
.p2align 7
	save_context_and_call_handler lower_el_aarch32_irq
.p2align 7
	fiq_suspend
.p2align 7
	save_context_and_call_handler lower_el_aarch32_serror

__exception_restore_context:
	ldp	x18, x19, [sp], #16
	ldr	x20, [sp], #8

	msr	SPSR_EL1, x19
	msr	ELR_EL1,  x18

	ldp	x0,  x1,  [sp], #16
	ldp	x2,  x3,  [sp], #16
	ldp	x4,  x5,  [sp], #16
	ldp	x6,  x7,  [sp], #16
	ldp	x8,  x9,  [sp], #16
	ldp	x10, x11, [sp], #16
	ldp	x12, x13, [sp], #16
	ldp	x14, x15, [sp], #16
	ldp	x16, x17, [sp], #16
	ldp	x18, x19, [sp], #16
	ldp	x20, x21, [sp], #16
	ldp	x22, x23, [sp], #16
	ldp	x24, x25, [sp], #16
	ldp	x26, x27, [sp], #16
	ldp	x28, x29, [sp], #16
	ldr x30, [sp], #8

	eret

.size	__exception_restore_context, . - __exception_restore_context
.type	__exception_restore_context, function

// We need to align to 2048 bytes the exception table
.align 11

.globl __el2_exception_vector_start
__el2_exception_vector_start:

// Current EL with SP_EL0
.p2align 7
	save_context_and_call_handler current_el0_synchronous
.p2align 7
	save_context_and_call_handler current_el0_irq
.p2align 7
	fiq_suspend
.p2align 7
	save_context_and_call_handler current_el0_serror

// Current EL with SP_ELx, x > 0
.p2align 7
	save_context_and_call_handler current_elx_synchronous
.p2align 7
	save_context_and_call_handler current_elx_irq
.p2align 7
	fiq_suspend
.p2align 7
	save_context_and_call_handler current_elx_serror

// Lower EL in AARCH64
.p2align 7
	save_context_and_call_handler lower_el_aarch64_synchronous
.p2align 7
	save_context_and_call_handler lower_el_aarch64_irq
.p2align 7
	fiq_suspend
.p2align 7
	save_context_and_call_handler lower_el_aarch64_serror

// Lower EL in AARCH32
.p2align 7
	save_context_and_call_handler lower_el_aarch32_synchronous
.p2align 7
	save_context_and_call_handler lower_el_aarch32_irq
.p2align 7
	fiq_suspend
.p2align 7
	save_context_and_call_handler lower_el_aarch32_serror

// __rel_exception_vector_start:
// .reloc __rel_exception_vector_start, R_AARCH64_RELATIVE, __exception_vector_start
