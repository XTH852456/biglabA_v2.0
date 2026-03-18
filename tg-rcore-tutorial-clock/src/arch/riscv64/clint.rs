use core::ptr::{read_volatile, write_volatile};
use lazy_static::lazy_static;

const CLINT_BASE: usize = 0x2000000;
const MTIME: usize = CLINT_BASE + 0xBFF8;
const MTIMECMP: usize = CLINT_BASE + 0x4000;

pub struct Clint;

impl Clint {
    pub fn set_timer(delta: u64) {
        let now = unsafe { read_volatile(MTIME as *const u64) };
        unsafe { write_volatile(MTIMECMP as *mut u64, now + delta) };
    }
}

lazy_static! {
    pub static ref CLINT: Clint = Clint;
}