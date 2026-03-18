#![no_std]

use core::fmt;

const UART0: usize = 0x10000000;
const RBR: *mut u8 = (UART0 + 0) as *mut u8;
const THR: *mut u8 = (UART0 + 0) as *mut u8;
const IER: *mut u8 = (UART0 + 1) as *mut u8;
const FCR: *mut u8 = (UART0 + 2) as *mut u8;
const LCR: *mut u8 = (UART0 + 3) as *mut u8;
const LSR: *mut u8 = (UART0 + 5) as *mut u8;

const LCR_8BIT: u8 = 0x03;
const FCR_ENABLE: u8 = 0x01;
const LSR_TX_READY: u8 = 1 << 5;

pub fn uart_init() {
    unsafe {
        core::ptr::write_volatile(IER, 0x00);
        core::ptr::write_volatile(LCR, LCR_8BIT);
        core::ptr::write_volatile(FCR, FCR_ENABLE);
    }
}

pub fn uart_putc(c: u8) {
    unsafe {
        while (core::ptr::read_volatile(LSR) & LSR_TX_READY) == 0 {}
        core::ptr::write_volatile(THR, c);
        if c == b'\n' {
            while (core::ptr::read_volatile(LSR) & LSR_TX_READY) == 0 {}
            core::ptr::write_volatile(THR, b'\r');
        }
    }
}

pub fn uart_puts(s: &str) {
    for &c in s.as_bytes() {
        uart_putc(c);
    }
}

pub struct UartWriter;
impl fmt::Write for UartWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        uart_puts(s);
        Ok(())
    }
}

#[macro_export]
macro_rules! uart_println {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!($crate::UartWriter, $($arg)*);
        let _ = write!($crate::UartWriter, "\n");
    }};
}