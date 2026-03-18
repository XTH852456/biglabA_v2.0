use core::panic::PanicInfo;
use tg_rcore_tutorial_uart::uart_println;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}