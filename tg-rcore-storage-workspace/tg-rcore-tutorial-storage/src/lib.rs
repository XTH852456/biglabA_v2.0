#![no_std]

use core::cmp::min;
use core::hint::spin_loop;
use core::mem::size_of;
use core::ptr::{addr_of, addr_of_mut, read_volatile, write_volatile};
use core::sync::atomic::{Ordering, fence};

pub const SECTOR_SIZE: usize = 512;

const VIRTIO_MMIO_START: usize = 0x1000_1000;
const VIRTIO_MMIO_STRIDE: usize = 0x1000;
const VIRTIO_MMIO_SLOTS: usize = 8;

const MMIO_MAGIC_VALUE: usize = 0x000;
const MMIO_VERSION: usize = 0x004;
const MMIO_DEVICE_ID: usize = 0x008;
const MMIO_DEVICE_FEATURES: usize = 0x010;
const MMIO_DEVICE_FEATURES_SEL: usize = 0x014;
const MMIO_DRIVER_FEATURES: usize = 0x020;
const MMIO_DRIVER_FEATURES_SEL: usize = 0x024;
const MMIO_QUEUE_SEL: usize = 0x030;
const MMIO_QUEUE_NUM_MAX: usize = 0x034;
const MMIO_QUEUE_NUM: usize = 0x038;
const MMIO_QUEUE_READY: usize = 0x044;
const MMIO_QUEUE_NOTIFY: usize = 0x050;
const MMIO_INTERRUPT_STATUS: usize = 0x060;
const MMIO_INTERRUPT_ACK: usize = 0x064;
const MMIO_STATUS: usize = 0x070;
const MMIO_QUEUE_DESC_LOW: usize = 0x080;
const MMIO_QUEUE_DESC_HIGH: usize = 0x084;
const MMIO_QUEUE_DRIVER_LOW: usize = 0x090;
const MMIO_QUEUE_DRIVER_HIGH: usize = 0x094;
const MMIO_QUEUE_DEVICE_LOW: usize = 0x0a0;
const MMIO_QUEUE_DEVICE_HIGH: usize = 0x0a4;
const MMIO_CONFIG_SPACE: usize = 0x100;

const VIRTIO_MAGIC: u32 = 0x7472_6976;
const VIRTIO_VERSION_2: u32 = 2;
const VIRTIO_DEVICE_BLOCK: u32 = 2;

const STATUS_ACKNOWLEDGE: u32 = 1;
const STATUS_DRIVER: u32 = 2;
const STATUS_DRIVER_OK: u32 = 4;
const STATUS_FEATURES_OK: u32 = 8;

const VIRTQ_DESC_F_NEXT: u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2;

const VIRTIO_BLK_T_IN: u32 = 0;
const VIRTIO_BLK_T_OUT: u32 = 1;

const QUEUE_SIZE: usize = 8;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StorageError {
    DeviceNotFound,
    BadMagic(u32),
    UnsupportedVersion(u32),
    UnexpectedDevice(u32),
    QueueUnavailable,
    QueueTooSmall(u16),
    FeatureNegotiationFailed,
    NotInitialized,
    DeviceNotReady,
    IoError(u8),
    UsedIdMismatch(u32),
}

#[repr(C)]
#[derive(Clone, Copy)]
struct VirtqDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

const EMPTY_DESC: VirtqDesc = VirtqDesc {
    addr: 0,
    len: 0,
    flags: 0,
    next: 0,
};

#[repr(C)]
#[derive(Clone, Copy)]
struct VirtqUsedElem {
    id: u32,
    len: u32,
}

const EMPTY_USED_ELEM: VirtqUsedElem = VirtqUsedElem { id: 0, len: 0 };

#[repr(C)]
struct VirtqAvail<const N: usize> {
    flags: u16,
    idx: u16,
    ring: [u16; N],
    used_event: u16,
}

#[repr(C)]
struct VirtqUsed<const N: usize> {
    flags: u16,
    idx: u16,
    ring: [VirtqUsedElem; N],
    avail_event: u16,
}

#[repr(C, align(4096))]
struct VirtQueueMemory {
    desc: [VirtqDesc; QUEUE_SIZE],
    avail: VirtqAvail<QUEUE_SIZE>,
    used: VirtqUsed<QUEUE_SIZE>,
}

static mut QUEUE_MEMORY: VirtQueueMemory = VirtQueueMemory {
    desc: [EMPTY_DESC; QUEUE_SIZE],
    avail: VirtqAvail {
        flags: 0,
        idx: 0,
        ring: [0; QUEUE_SIZE],
        used_event: 0,
    },
    used: VirtqUsed {
        flags: 0,
        idx: 0,
        ring: [EMPTY_USED_ELEM; QUEUE_SIZE],
        avail_event: 0,
    },
};

#[repr(C)]
#[derive(Clone, Copy)]
struct VirtioBlkReqHeader {
    req_type: u32,
    reserved: u32,
    sector: u64,
}

struct DriverState {
    initialized: bool,
    mmio_base: usize,
    queue_size: u16,
    last_used_idx: u16,
}

static mut DRIVER_STATE: DriverState = DriverState {
    initialized: false,
    mmio_base: 0,
    queue_size: 0,
    last_used_idx: 0,
};

#[inline(always)]
fn mmio_read32(base: usize, offset: usize) -> u32 {
    unsafe { read_volatile((base + offset) as *const u32) }
}

#[inline(always)]
fn mmio_write32(base: usize, offset: usize, value: u32) {
    unsafe { write_volatile((base + offset) as *mut u32, value) }
}

fn read_capacity_sectors(base: usize) -> u64 {
    let low = mmio_read32(base, MMIO_CONFIG_SPACE) as u64;
    let high = mmio_read32(base, MMIO_CONFIG_SPACE + 4) as u64;
    (high << 32) | low
}

fn find_block_device() -> Option<(usize, u32)> {
    for i in 0..VIRTIO_MMIO_SLOTS {
        let base = VIRTIO_MMIO_START + i * VIRTIO_MMIO_STRIDE;
        let magic = mmio_read32(base, MMIO_MAGIC_VALUE);
        if magic != VIRTIO_MAGIC {
            continue;
        }

        let device_id = mmio_read32(base, MMIO_DEVICE_ID);
        if device_id == VIRTIO_DEVICE_BLOCK {
            let version = mmio_read32(base, MMIO_VERSION);
            return Some((base, version));
        }
    }

    None
}

pub fn init() -> Result<u64, StorageError> {
    unsafe {
        if DRIVER_STATE.initialized {
            return Ok(read_capacity_sectors(DRIVER_STATE.mmio_base));
        }

        let (mmio_base, version) = find_block_device().ok_or(StorageError::DeviceNotFound)?;

        if version != VIRTIO_VERSION_2 {
            return Err(StorageError::UnsupportedVersion(version));
        }

        let device_id = mmio_read32(mmio_base, MMIO_DEVICE_ID);
        if device_id != VIRTIO_DEVICE_BLOCK {
            return Err(StorageError::UnexpectedDevice(device_id));
        }

        let magic = mmio_read32(mmio_base, MMIO_MAGIC_VALUE);
        if magic != VIRTIO_MAGIC {
            return Err(StorageError::BadMagic(magic));
        }

        mmio_write32(mmio_base, MMIO_STATUS, 0);
        let mut status = STATUS_ACKNOWLEDGE;
        mmio_write32(mmio_base, MMIO_STATUS, status);
        status |= STATUS_DRIVER;
        mmio_write32(mmio_base, MMIO_STATUS, status);

        mmio_write32(mmio_base, MMIO_DEVICE_FEATURES_SEL, 0);
        let _ = mmio_read32(mmio_base, MMIO_DEVICE_FEATURES);
        mmio_write32(mmio_base, MMIO_DRIVER_FEATURES_SEL, 0);
        mmio_write32(mmio_base, MMIO_DRIVER_FEATURES, 0);

        status |= STATUS_FEATURES_OK;
        mmio_write32(mmio_base, MMIO_STATUS, status);
        if (mmio_read32(mmio_base, MMIO_STATUS) & STATUS_FEATURES_OK) == 0 {
            return Err(StorageError::FeatureNegotiationFailed);
        }

        mmio_write32(mmio_base, MMIO_QUEUE_SEL, 0);
        let max_queue_size = mmio_read32(mmio_base, MMIO_QUEUE_NUM_MAX) as u16;
        if max_queue_size == 0 {
            return Err(StorageError::QueueUnavailable);
        }

        let queue_size = min(max_queue_size as usize, QUEUE_SIZE) as u16;
        if queue_size < 3 {
            return Err(StorageError::QueueTooSmall(max_queue_size));
        }
        mmio_write32(mmio_base, MMIO_QUEUE_NUM, queue_size as u32);

        let desc_addr = addr_of!(QUEUE_MEMORY.desc) as u64;
        let avail_addr = addr_of!(QUEUE_MEMORY.avail) as u64;
        let used_addr = addr_of!(QUEUE_MEMORY.used) as u64;

        mmio_write32(mmio_base, MMIO_QUEUE_DESC_LOW, desc_addr as u32);
        mmio_write32(mmio_base, MMIO_QUEUE_DESC_HIGH, (desc_addr >> 32) as u32);
        mmio_write32(mmio_base, MMIO_QUEUE_DRIVER_LOW, avail_addr as u32);
        mmio_write32(mmio_base, MMIO_QUEUE_DRIVER_HIGH, (avail_addr >> 32) as u32);
        mmio_write32(mmio_base, MMIO_QUEUE_DEVICE_LOW, used_addr as u32);
        mmio_write32(mmio_base, MMIO_QUEUE_DEVICE_HIGH, (used_addr >> 32) as u32);

        mmio_write32(mmio_base, MMIO_QUEUE_READY, 1);

        DRIVER_STATE.mmio_base = mmio_base;
        DRIVER_STATE.last_used_idx = read_volatile(addr_of!(QUEUE_MEMORY.used.idx));
        DRIVER_STATE.queue_size = queue_size;

        status |= STATUS_DRIVER_OK;
        mmio_write32(mmio_base, MMIO_STATUS, status);
        if (mmio_read32(mmio_base, MMIO_STATUS) & STATUS_DRIVER_OK) == 0 {
            return Err(StorageError::DeviceNotReady);
        }

        DRIVER_STATE.initialized = true;
        Ok(read_capacity_sectors(mmio_base))
    }
}

pub fn read_sector(sector: u64, buf: &mut [u8; SECTOR_SIZE]) -> Result<(), StorageError> {
    do_rw_sector(sector, buf.as_mut_ptr(), false)
}

pub fn write_sector(sector: u64, buf: &[u8; SECTOR_SIZE]) -> Result<(), StorageError> {
    do_rw_sector(sector, buf.as_ptr() as *mut u8, true)
}

fn do_rw_sector(sector: u64, data_ptr: *mut u8, is_write: bool) -> Result<(), StorageError> {
    unsafe {
        if !DRIVER_STATE.initialized {
            return Err(StorageError::NotInitialized);
        }

        let mmio_base = DRIVER_STATE.mmio_base;

        let header = VirtioBlkReqHeader {
            req_type: if is_write { VIRTIO_BLK_T_OUT } else { VIRTIO_BLK_T_IN },
            reserved: 0,
            sector,
        };
        let mut status: u8 = 0xff;

        let desc_ptr = addr_of_mut!(QUEUE_MEMORY.desc) as *mut VirtqDesc;
        write_volatile(
            desc_ptr.add(0),
            VirtqDesc {
                addr: addr_of!(header) as u64,
                len: size_of::<VirtioBlkReqHeader>() as u32,
                flags: VIRTQ_DESC_F_NEXT,
                next: 1,
            },
        );

        let data_flags = if is_write {
            VIRTQ_DESC_F_NEXT
        } else {
            VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE
        };
        write_volatile(
            desc_ptr.add(1),
            VirtqDesc {
                addr: data_ptr as u64,
                len: SECTOR_SIZE as u32,
                flags: data_flags,
                next: 2,
            },
        );

        write_volatile(
            desc_ptr.add(2),
            VirtqDesc {
                addr: addr_of_mut!(status) as u64,
                len: 1,
                flags: VIRTQ_DESC_F_WRITE,
                next: 0,
            },
        );

        fence(Ordering::Release);

        let avail_idx_ptr = addr_of_mut!(QUEUE_MEMORY.avail.idx);
        let avail_ring_ptr = addr_of_mut!(QUEUE_MEMORY.avail.ring) as *mut u16;
        let old_avail_idx = read_volatile(avail_idx_ptr);
        write_volatile(
            avail_ring_ptr.add((old_avail_idx as usize) % (DRIVER_STATE.queue_size as usize)),
            0,
        );
        write_volatile(avail_idx_ptr, old_avail_idx.wrapping_add(1));

        fence(Ordering::SeqCst);
        mmio_write32(mmio_base, MMIO_QUEUE_NOTIFY, 0);

        let used_idx_ptr = addr_of!(QUEUE_MEMORY.used.idx);
        while read_volatile(used_idx_ptr) == DRIVER_STATE.last_used_idx {
            spin_loop();
        }
        let new_used_idx = read_volatile(used_idx_ptr);

        let used_ring_ptr = addr_of!(QUEUE_MEMORY.used.ring) as *const VirtqUsedElem;
        let used_slot = (new_used_idx.wrapping_sub(1) as usize) % (DRIVER_STATE.queue_size as usize);
        let used_elem = read_volatile(used_ring_ptr.add(used_slot));
        DRIVER_STATE.last_used_idx = new_used_idx;

        let irq = mmio_read32(mmio_base, MMIO_INTERRUPT_STATUS);
        if irq != 0 {
            mmio_write32(mmio_base, MMIO_INTERRUPT_ACK, irq);
        }

        fence(Ordering::Acquire);

        if used_elem.id != 0 {
            return Err(StorageError::UsedIdMismatch(used_elem.id));
        }

        let status_code = read_volatile(addr_of!(status));
        if status_code == 0 {
            Ok(())
        } else {
            Err(StorageError::IoError(status_code))
        }
    }
}
