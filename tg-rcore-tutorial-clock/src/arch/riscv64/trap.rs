use riscv::register::{scause, sie, sstatus, stvec};

const UART_BASE: usize = 0x10000000;
const TIMER_INTERVAL: u64 = 10_000_000;
const SBI_EXT_TIME: usize = 0x5449_4D45;
const SBI_FID_SET_TIMER: usize = 0;

pub fn init_uart() {}

pub fn put_char(c: u8) {
    unsafe { core::ptr::write_volatile(UART_BASE as *mut u8, c); }
}

#[inline]
fn read_time() -> u64 {
    let now: u64;
    unsafe { core::arch::asm!("rdtime {}", out(reg) now); }
    now
}

#[inline]
fn set_next_timer() {
    unsafe { sbi_set_timer(read_time() + TIMER_INTERVAL); }
}

#[inline]
unsafe fn sbi_set_timer(stime_value: u64) {
    // SBI v0.2 TIME extension: ext=0x54494D45, fid=0 (set_timer)
    core::arch::asm!(
        "ecall",
        inlateout("a0") stime_value as usize => _,
        in("a6") SBI_FID_SET_TIMER,
        in("a7") SBI_EXT_TIME,
        lateout("a1") _,
    );
}

pub fn init_timer() {
    unsafe {
        stvec::write(trap_handler as *const () as usize, stvec::TrapMode::Direct);
        sie::set_stimer();
        sstatus::set_sie();
    }
    set_next_timer();
}

#[no_mangle]
pub extern "C" fn trap_handler() {
    let cause = scause::read();
    if cause.is_interrupt() && cause.code() == 5 {
        put_char(b'T');
        put_char(b'\n');
        set_next_timer();
        return;
    }

    put_char(b'E');
    put_char(b'\n');
    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}