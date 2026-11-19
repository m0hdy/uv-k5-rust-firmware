.syntax unified
.cpu cortex-m0
.thumb

.equ .Linitial_sp, 0x20003FF0

.section .vector_table,"a",%progbits
.balign 4
.global __witness_vector_table
.type __witness_vector_table,%object
__witness_vector_table:
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
.size __witness_vector_table, . - __witness_vector_table

.section .text.Reset_Handler,"ax",%progbits
.balign 2
.global Reset_Handler
.type Reset_Handler,%function
.thumb_func
Reset_Handler:
    bl witness_main
1:
    b 1b
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
