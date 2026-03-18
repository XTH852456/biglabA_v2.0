#![no_std]
#![no_main]

use core::arch::global_asm;
use core::panic::PanicInfo;

global_asm!(include_str!("arch/riscv64/boot.s"));

pub mod arch {
    pub mod riscv64 {
        pub mod clint;
        pub mod trap;
    }
}

use arch::riscv64::trap::{init_uart, put_char, init_timer};

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    init_uart();
    put_char(b'S');
    put_char(b't');
    put_char(b'a');
    put_char(b'r');
    put_char(b't');
    put_char(b'\n');
    init_timer();

    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}