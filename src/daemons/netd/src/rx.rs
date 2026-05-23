#![no_std]
#![no_main]

extern crate alloc;
extern crate libc_string;
extern crate nvx;

use ::sys::{ 
    kcall::{
        mm,
        pm
    },
    mm::{
        MmioRegionInfo
    },
    pm::ProcessIdentifier
};

const NUM_RX_DESCS: u32 = 8;
const RX_DESCS_SIZE: u32 = 16;

// RX descriptor
#[repr(C, 16)]
pub struct RxDesc {
    buff_address: u64,
    other: u64               // Special | Errors | Status | Packet Checksum | Length
}

impl RxDesc {
    pub fn new (
        address: u64, 
        special: u16,
        errors: u8,
        status: u8,
        packet_checksum: u16,
        len: u16,
    ) -> Self {

        let other : u64 = 0
            | (special << 48)
            | (errors << 40)
            | (status << 32)
            | (packet_checksum << 16)
            | len;

        Self {
            address,
            other
        }
    }
}

pub struct RxRing {
    base_address: u64,
    len: u16,
    head: u32,
    tail: u32
}

impl RxRing {

    pub fn new (dma_addr: u32) -> Self {
        syslog::info!("Allocating RX ring in DMA");
        let ring_size: usize = NUM_RX_DESCS * 16;

        for i in 0..NUM_RX_DESCS {
            let desc_addr = alloc();
            unsafe { 
                core::ptr::write_volatile(
                    (vaddr + (i * RX_DESCS_SIZE)) as *const u32,
                    desc_addr
                )
            }
        }
    }
}
