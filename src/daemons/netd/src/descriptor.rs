extern crate alloc;
extern crate libc_string;
extern crate nvx;

use crate::{
    dma_info::DmaInfo,
    dma_manager::DmaManager,
};

use core::{
    clone::Clone,
    default::Default,
    prelude::rust_2024::derive,
};

use alloc::vec::Vec;

enum DescType {
    Tx,
    Rx,
}

impl DescType {
    pub fn ring_offset(&self, info: &DmaInfo) -> usize {
        match self {
            Self::Tx => info.tx_ring_offset(),
            Self::Rx => info.rx_ring_offset(),
        }
    }

    #[allow(dead_code)]
    pub fn buff_offset(&self, info: &DmaInfo) -> usize {
        match self {
            Self::Tx => info.tx_buff_offset(),
            Self::Rx => info.rx_buff_offset(),
        }
    }
}

#[derive(Default, Clone)]
#[repr(C, align(16))]
pub struct Descriptor {
    buff_address: u64,
    fields: u64,
}

#[allow(dead_code)]
impl Descriptor {
    pub fn buff_address(&self) -> u64 {
        self.buff_address
    }

    // Tx
    pub fn set_tx_special(&mut self, value: u16) {
        self.fields &= u64::MAX >> 16;
        self.fields |= (value as u64) << 48;
    }

    pub fn set_tx_css(&mut self, value: u8) {
        self.fields &= 0xffff00ff_ffffffff;
        self.fields |= (value as u64) << 40;
    }

    pub fn set_tx_rsv_sta(&mut self, value: u8) {
        self.fields &= 0xffffff00_ffffffff;
        self.fields |= (value as u64) << 32;
    }

    pub fn set_tx_cmd(&mut self, value: u8) {
        self.fields &= 0xffffffff_00ffffff;
        self.fields |= (value as u64) << 24;
    }

    pub fn set_tx_cso(&mut self, value: u8) {
        self.fields &= 0xffffffff_ff00ffff;
        self.fields |= (value as u64) << 16;
    }

    pub fn set_tx_length(&mut self, value: u16) {
        self.fields &= 0xffffffff_ffff0000;
        self.fields |= value as u64;
    }

    pub fn get_tx_special(&self) -> u16 {
        let result = self.fields & 0xffff0000_00000000;
        (result >> 48) as u16
    }

    pub fn get_tx_css(&self) -> u8 {
        let result = self.fields & 0x0000ff00_00000000;
        (result >> 40) as u8
    }

    pub fn get_tx_rsv_sta(&self) -> u8 {
        let result = self.fields & 0x000000ff_00000000;
        (result >> 32) as u8
    }

    pub fn get_tx_cmd(&self) -> u8 {
        let result = self.fields & 0x00000000_ff000000;
        (result >> 24) as u8
    }

    pub fn get_tx_cso(&self) -> u8 {
        let result = self.fields & 0x00000000_00ff0000;
        (result >> 16) as u8
    }

    pub fn get_tx_length(&self) -> u16 {
        let result = self.fields & 0x00000000_0000ffff;
        result as u16
    }

    // Rx

    pub fn set_rx_special(&mut self, value: u16) {
        self.fields &= u64::MAX >> 16;
        self.fields |= (value as u64) << 48;
    }

    pub fn set_rx_errors(&mut self, value: u8) {
        self.fields &= 0xffff00ff_ffffffff;
        self.fields |= (value as u64) << 40;
    }

    pub fn set_rx_status(&mut self, value: u8) {
        self.fields &= 0xffffff00_ffffffff;
        self.fields |= (value as u64) << 32;
    }

    pub fn set_rx_chksum(&mut self, value: u16) {
        self.fields &= 0xffffffff_0000ffff;
        self.fields |= (value as u64) << 24;
    }

    pub fn set_rx_length(&mut self, value: u16) {
        self.fields &= 0xffffffff_ffff0000;
        self.fields |= value as u64;
    }

    pub fn get_rx_special(&self) -> u16 {
        let result = self.fields & 0xffff0000_00000000;
        (result >> 48) as u16
    }

    pub fn get_rx_errors(&self) -> u8 {
        let result = self.fields & 0x0000ff00_00000000;
        (result >> 40) as u8
    }

    pub fn get_rx_status(&self) -> u8 {
        let result = self.fields & 0x000000ff_00000000;
        (result >> 32) as u8
    }

    pub fn get_rx_chksum(&self) -> u16 {
        let result = self.fields & 0x00000000_ffff0000;
        (result >> 24) as u16
    }

    pub fn get_rx_length(&self) -> u16 {
        let result = self.fields & 0x00000000_0000ffff;
        result as u16
    }

    fn from(dma_manager: &DmaManager, dtype: DescType) -> Vec<*mut Descriptor> {
        let info = dma_manager.info();

        let desc_count = dma_manager.info().desc_count() as usize;
        let mut descs: Vec<*mut Descriptor> = Vec::with_capacity(desc_count);

        for i in 0..desc_count {
            let desc: *mut Descriptor =
                (dma_manager.base_vaddr() + dtype.ring_offset(info) + (i * crate::DESCRIPTOR_SIZE))
                    as *mut Descriptor;

            match dtype {
                DescType::Tx => unsafe {
                    (&mut *desc).set_tx_rsv_sta(1); // Set DD = 1
                },
                DescType::Rx => unsafe {
                    let desc_ref = &mut *desc;
                    desc_ref.set_rx_status(0);
                    desc_ref.set_rx_length(0);
                    desc_ref.set_rx_errors(0);
                },
            }
            descs.push(desc);
        }
        descs
    }

    pub fn tx_from(dma_manager: &DmaManager) -> Vec<*mut Descriptor> {
        Self::from(dma_manager, DescType::Tx)
    }

    pub fn rx_from(dma_manager: &DmaManager) -> Vec<*mut Descriptor> {
        Self::from(dma_manager, DescType::Rx)
    }

    #[allow(dead_code)]
    pub unsafe fn read_buff_address(&self) -> u64 {
        self.buff_address
    }

    #[allow(dead_code)]
    pub unsafe fn read_fields(&self) -> u64 {
        self.fields
    }
}
