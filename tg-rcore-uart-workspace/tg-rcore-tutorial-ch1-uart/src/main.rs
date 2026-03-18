#![no_std]
#![no_main]

mod lang_items;
use tg_rcore_tutorial_uart::{uart_init, uart_println};

extern "C" fn kernel_main() -> ! {
    uart_init();

    uart_println!("========================================");
    uart_println!("HELLO WORLD FROM S-MODE UART");
    uart_println!("========================================");

    loop {}
}

#[link_section = ".text.entry"]
#[unsafe(naked)]
#[unsafe(no_mangle)]
unsafe extern "C" fn _start() -> ! {
    const STACK_SIZE: usize = 4096;

    #[unsafe(link_section = ".bss.uninit")]
    static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

    core::arch::naked_asm!(
        "la sp, {stack} + {stack_size}",
        "j  {main}",
        stack_size = const STACK_SIZE,
        stack = sym STACK,
        main = sym kernel_main,
    )
}