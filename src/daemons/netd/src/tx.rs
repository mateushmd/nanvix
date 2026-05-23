#![no_std]
#![no_main]

extern crate alloc;
extern crate libc_string;
extern crate nvx;

use ::sys::{ 
    kcall:: {
        mm,
        pm
    },
    mm::{
        MmioRegionInfo
    },
    pm::ProcessIdentifier
};

// Legacy TX Data descriptor type
#[allow(dead_code)]
pub struct TxDataDesc {
    buff_address: u64,
    other: u64               // Special | CSS | RSV | STA | CMD | CSO | Length
}

impl TxDesc {
    pub fn new (
        address: u64, 
        special: u16,
        css: u8,
        rsv: u8,
        sta: u8,
        cmd: u8,
        cso: u8,
        len: u16
    ) -> Self {

        let other : u64 = 0
            | (special << 48)
            | (css << 40)
            | ( (rsv & 0b1111) << 36)
            | ( (sta & 0b1111) << 32)
            | (cmd << 24)
            | (cso << 16)
            | len;

        // Set type to Legacy
        other |= (0b1 << 29);

        Self {
            address,
            other
        }
    }
}

#[repr(C, align(16))]
#[allow(dead_code)]
pub struct TxRing {
    base_address: u64,    // Trasmit Ring Descriptor Base Address (low + high)
    len: u16,             // Trasmit Ring Descriptor Base Length
    head: u32,            // Trasmit Ring Descriptor Head
    tail: u32             // Trasmit Ring Descriptor Tail
}

impl TxRing {
    #[allow(dead_code)]
    pub fn new (base_address: u64, len: u16, head: u32, tail: u32) -> Self {
        Self {
            base_address,
            len,
            head,
            tail
        }
    }
    #[allow(dead_code)]
    pub fn empty () -> Self {
        Self {
            base_address: 0,
            len: 0,
            head: 0,
            tail: 0
        }
    }
}
