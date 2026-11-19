.syntax unified
.cpu cortex-m0
.thumb

.equ .Linitial_sp, 0x20003FF0

.section .vector_table,"a",%progbits
.balign 4
.global __vector_table
.type __vector_table,%object
__vector_table:
    .word .Linitial_sp
    .word Reset_Handler
    .word Default_Handler
    .word Default_Handler
    .word 0
    .word 0
    .word 0
    .word 0
    .word 0
    .word 0
    .word 0
    .word Default_Handler
    .word 0
    .word 0
    .word Default_Handler
    .word SysTick_Handler
    .rept 32
    .word Default_Handler
    .endr
.size __vector_table, . - __vector_table

.section .text.Reset_Handler,"ax",%progbits
.balign 2
.global Reset_Handler
.type Reset_Handler,%function
.thumb_func
Reset_Handler:
    ldr r0, =.Linitial_sp
    mov sp, r0
    ldr r0, =__data_load
    ldr r1, =__data_start
    ldr r2, =__data_end
1:
    cmp r1, r2
    bcs 2f
    ldr r3, [r0]
    str r3, [r1]
    adds r0, #4
    adds r1, #4
    b 1b
2:
    ldr r1, =__bss_start
    ldr r2, =__bss_end
    movs r3, #0
3:
    cmp r1, r2
    bcs 4f
    str r3, [r1]
    adds r1, #4
    b 3b
4:
    bl witness_main
5:
    b 5b
.size Reset_Handler, . - Reset_Handler

.section .text.Default_Handler,"ax",%progbits
.balign 2
.global Default_Handler
.type Default_Handler,%function
.thumb_func
Default_Handler:
    b Default_Handler
.size Default_Handler, . - Default_Handler

.section .text.SysTick_Handler,"ax",%progbits
.balign 2
.global SysTick_Handler
.type SysTick_Handler,%function
.thumb_func
SysTick_Handler:
    bx lr
.size SysTick_Handler, . - SysTick_Handler
