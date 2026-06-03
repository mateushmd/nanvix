extern crate alloc;
extern crate libc_string;
extern crate nvx;

use crate::{
    dma_info::DmaInfo,
    dma_manager::DmaManager,
    volatile_cell::VolatileCell
};

/*
use core::{
    clone::Clone,
    default::Default,
    prelude::rust_2024::derive,
};
*/

use alloc::vec::Vec;
use ::sys::error::Error;

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

// #[derive(Default, Clone)]
#[repr(C, align(16))]
pub struct Descriptor {
    buff_address: VolatileCell<u64>,
    fields: VolatileCell<u64>,
}

const _: () = assert!(core::mem::size_of::<Descriptor>() == 16);

#[allow(dead_code)]
impl Descriptor {
    pub fn set_buff_address(&mut self, value: u64) {
        self.buff_address.write(value);
    }

    pub fn get_buff_address(&self) -> u64 {
        self.buff_address.read()
    }

    pub fn set_tx_special(&mut self, value: u16) {
        let mut fields = self.fields.read();
        fields &= 0x0000ffff_ffffffff;
        fields |= (value as u64) << 48;
        self.fields.write(fields);
    }

    pub fn set_tx_css(&mut self, value: u8) {
        let mut fields = self.fields.read();
        fields &= 0xffff00ff_ffffffff;
        fields |= (value as u64) << 40;
        self.fields.write(fields);
    }

    pub fn set_tx_rsv_sta(&mut self, value: u8) {
        let mut fields = self.fields.read();
        fields &= 0xffffff00_ffffffff;
        fields |= (value as u64) << 32;
        self.fields.write(fields);
    }

    pub fn set_tx_cmd(&mut self, value: u8) {
        let mut fields = self.fields.read();
        fields &= 0xffffffff_00ffffff;
        fields |= (value as u64) << 24;
        self.fields.write(fields);
    }

    pub fn set_tx_cso(&mut self, value: u8) {
        let mut fields = self.fields.read();
        fields &= 0xffffffff_ff00ffff;
        fields |= (value as u64) << 16;
        self.fields.write(fields);
    }

    pub fn set_tx_length(&mut self, value: u16) {
        let mut fields = self.fields.read();
        fields &= 0xffffffff_ffff0000;
        fields |= value as u64;
        self.fields.write(fields);
    }

    pub fn get_tx_special(&self) -> u16 {
        let fields = self.fields.read();
        ((fields & 0xffff0000_00000000) >> 48) as u16
    }

    pub fn get_tx_css(&self) -> u8 {
        let fields = self.fields.read();
        ((fields & 0x0000ff00_00000000) >> 40) as u8
    }

    pub fn get_tx_rsv_sta(&self) -> u8 {
        let fields = self.fields.read();
        ((fields & 0x000000ff_00000000) >> 32) as u8
    }

    pub fn get_tx_cmd(&self) -> u8 {
        let fields = self.fields.read();
        ((fields & 0x00000000_ff000000) >> 24) as u8
    }

    pub fn get_tx_cso(&self) -> u8 {
        let fields = self.fields.read();
        ((fields & 0x00000000_00ff0000) >> 16) as u8
    }

    pub fn get_tx_length(&self) -> u16 {
        let fields = self.fields.read();
        (fields & 0x00000000_0000ffff) as u16
    }

    pub fn set_rx_special(&mut self, value: u16) {
        let mut fields = self.fields.read();
        fields &= 0x0000ffff_ffffffff;
        fields |= (value as u64) << 48;
        self.fields.write(fields);
    }

    pub fn set_rx_errors(&mut self, value: u8) {
        let mut fields = self.fields.read();
        fields &= 0xffff00ff_ffffffff;
        fields |= (value as u64) << 40;
        self.fields.write(fields);
    }

    pub fn set_rx_status(&mut self, value: u8) {
        let mut fields = self.fields.read();
        fields &= 0xffffff00_ffffffff;
        fields |= (value as u64) << 32;
        self.fields.write(fields);
    }

    pub fn set_rx_chksum(&mut self, value: u16) {
        let mut fields = self.fields.read();
        fields &= 0xffffffff_0000ffff;
        fields |= (value as u64) << 16;
        self.fields.write(fields);
    }

    pub fn set_rx_length(&mut self, value: u16) {
        let mut fields = self.fields.read();
        fields &= 0xffffffff_ffff0000;
        fields |= value as u64;
        self.fields.write(fields);
    }

    pub fn get_rx_special(&self) -> u16 {
        let fields = self.fields.read();
        ((fields & 0xffff0000_00000000) >> 48) as u16
    }

    pub fn get_rx_errors(&self) -> u8 {
        let fields = self.fields.read();
        ((fields & 0x0000ff00_00000000) >> 40) as u8
    }

    pub fn get_rx_status(&self) -> u8 {
        let fields = self.fields.read();
        ((fields & 0x000000ff_00000000) >> 32) as u8
    }

    pub fn get_rx_chksum(&self) -> u16 {
        let fields = self.fields.read();
        ((fields & 0x00000000_ffff0000) >> 16) as u16
    }

    pub fn get_rx_length(&self) -> u16 {
        let fields = self.fields.read();
        (fields & 0x00000000_0000ffff) as u16
    }

    fn from(dma_manager: &DmaManager, dtype: DescType) -> Result<(Vec<*mut Descriptor>, Vec<usize>), Error> {
        let info = dma_manager.info();
        let desc_count = info.desc_count() as usize;
        let mut descs: Vec<*mut Descriptor> = Vec::with_capacity(desc_count);
		let mut bufs: Vec<usize> = Vec::with_capacity(desc_count);

        for i in 0..desc_count {
            let desc_addr = dma_manager.base_vaddr() 
                + dtype.ring_offset(info) 
                + (i * crate::DESCRIPTOR_SIZE);
                
            let desc = desc_addr as *mut Descriptor;

            unsafe {
                let desc_ref = &mut *desc;
                
                desc_ref.set_buff_address(
                    (dma_manager.base_paddr()? + dtype.buff_offset(info)
                    + (i * info.buff_len() as usize)) as u64
                );

                match dtype {
                    DescType::Tx => desc_ref.set_tx_rsv_sta(1),
                    DescType::Rx => {
                        desc_ref.set_rx_status(0);
                        desc_ref.set_rx_length(0);
                        desc_ref.set_rx_errors(0);
                    }
                }
            }

            descs.push(desc);
			bufs.push(
				dma_manager.base_vaddr() + dtype.buff_offset(info)
				+ (i * info.buff_len() as usize)
			);
        }
        Ok((descs, bufs))
    }

    pub fn tx_from(dma_manager: &DmaManager) -> Result<(Vec<*mut Descriptor>, Vec<usize>), Error> {
        Self::from(dma_manager, DescType::Tx)
    }

    pub fn rx_from(dma_manager: &DmaManager) -> Result<(Vec<*mut Descriptor>, Vec<usize>), Error> {
        Self::from(dma_manager, DescType::Rx)
    }

    #[allow(dead_code)]
    pub fn read_fields(&self) -> u64 {
        self.fields.read()
    }
}

