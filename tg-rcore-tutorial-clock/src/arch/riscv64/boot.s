    .section .text.entry
    .globl _start
_start:
    la sp, boot_stack_top

    # 关闭中断，防止干扰
    csrw sie, zero
    csrw sstatus, zero

    # 直接进入内核 C/Rust 函数
    call kernel_main

loop:
    j loop

    .section .bss.stack
    .globl boot_stack
boot_stack:
    .space 4096 * 16
    .globl boot_stack_top
boot_stack_top: