#![no_std]
#![no_main]
#![cfg_attr(target_arch = "riscv64", deny(warnings))]
#![cfg_attr(not(target_arch = "riscv64"), allow(dead_code))]

use core::fmt::{self, Write};

use tg_rcore_tutorial_storage::{SECTOR_SIZE, StorageError, init, read_sector, write_sector};
use tg_sbi::{console_putchar, shutdown};

struct Console;

impl Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.as_bytes() {
            console_putchar(*b);
        }
        Ok(())
    }
}

macro_rules! sprint {
    ($($arg:tt)*) => {{
        let _ = write!(Console, $($arg)*);
    }};
}

macro_rules! sprintln {
    () => {{
        let _ = writeln!(Console);
    }};
    ($($arg:tt)*) => {{
        let _ = writeln!(Console, $($arg)*);
    }};
}

#[cfg(target_arch = "riscv64")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
unsafe extern "C" fn _start() -> ! {
    const STACK_SIZE: usize = 4096;

    #[unsafe(link_section = ".bss.uninit")]
    static mut STACK: [u8; STACK_SIZE] = [0u8; STACK_SIZE];

    core::arch::naked_asm!(
        "la sp, {stack} + {stack_size}",
        "j  {main}",
        stack_size = const STACK_SIZE,
        stack = sym STACK,
        main = sym rust_main,
    )
}

extern "C" fn rust_main() -> ! {
    sprintln!("========================================");
    sprintln!("TG-RCORE CH1 STORAGE DEMO (S-MODE)");
    sprintln!("========================================");

    let capacity = match init() {
        Ok(cap) => {
            sprintln!("virtio-blk init: ok");
            sprintln!("capacity: {} sectors ({} KiB)", cap, cap / 2);
            cap
        }
        Err(e) => fail("virtio-blk init", e),
    };

    let target_sector = 1u64;
    if capacity <= target_sector {
        sprintln!("capacity too small, need at least sector {}", target_sector);
        shutdown(true);
    }

    let mut original = [0u8; SECTOR_SIZE];
    if let Err(e) = read_sector(target_sector, &mut original) {
        fail("read original sector", e);
    }
    dump_prefix("before", &original);

    let mut write_buf = [0u8; SECTOR_SIZE];
    build_pattern(&mut write_buf, target_sector);

    if let Err(e) = write_sector(target_sector, &write_buf) {
        fail("write sector", e);
    }
    sprintln!("write sector {}: ok", target_sector);

    let mut verify = [0u8; SECTOR_SIZE];
    if let Err(e) = read_sector(target_sector, &mut verify) {
        fail("read verify sector", e);
    }
    dump_prefix("after ", &verify);

    if let Some(idx) = first_mismatch(&write_buf, &verify) {
        sprintln!(
            "verify failed at byte {}: expected {:02x}, got {:02x}",
            idx,
            write_buf[idx],
            verify[idx]
        );
        shutdown(true);
    }

    sprintln!("verify: pass");

    if let Err(e) = write_sector(target_sector, &original) {
        fail("restore original sector", e);
    }
    sprintln!("restore sector {}: ok", target_sector);

    sprintln!("storage read/write demo finished.");
    shutdown(false);
}

fn build_pattern(buf: &mut [u8; SECTOR_SIZE], sector: u64) {
    let message = b"TG-RCORE STORAGE DEMO";
    buf[..message.len()].copy_from_slice(message);

    let sector_tag = b"SECTOR=";
    let start = 32;
    buf[start..start + sector_tag.len()].copy_from_slice(sector_tag);

    let mut number = [0u8; 20];
    let len = u64_to_dec(sector, &mut number);
    buf[start + sector_tag.len()..start + sector_tag.len() + len].copy_from_slice(&number[..len]);

    for (i, byte) in buf.iter_mut().enumerate().skip(64) {
        *byte = (i as u8).wrapping_mul(3).wrapping_add(1);
    }
}

fn u64_to_dec(mut value: u64, out: &mut [u8; 20]) -> usize {
    if value == 0 {
        out[0] = b'0';
        return 1;
    }

    let mut rev = [0u8; 20];
    let mut len = 0usize;
    while value != 0 {
        rev[len] = b'0' + (value % 10) as u8;
        value /= 10;
        len += 1;
    }

    for i in 0..len {
        out[i] = rev[len - 1 - i];
    }
    len
}

fn dump_prefix(tag: &str, buf: &[u8; SECTOR_SIZE]) {
    sprint!("{} first16:", tag);
    for b in &buf[..16] {
        sprint!(" {:02x}", *b);
    }
    sprintln!();
}

fn first_mismatch(a: &[u8; SECTOR_SIZE], b: &[u8; SECTOR_SIZE]) -> Option<usize> {
    for i in 0..SECTOR_SIZE {
        if a[i] != b[i] {
            return Some(i);
        }
    }
    None
}

fn fail(step: &str, err: StorageError) -> ! {
    sprintln!("{} failed: {:?}", step, err);
    shutdown(true)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    shutdown(true)
}

#[cfg(not(target_arch = "riscv64"))]
mod stub {
    #[unsafe(no_mangle)]
    pub extern "C" fn main() -> i32 {
        0
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn __libc_start_main() -> i32 {
        0
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn rust_eh_personality() {}
}
