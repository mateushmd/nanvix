extern crate alloc;
extern crate libc_string;
extern crate nvx;

use crate::{ 
	dma_info::DmaInfo, 
	dma_manager::DmaManager 
};

use core::{
	clone::Clone,
	convert::Into,
	default::Default,
	iter::Iterator,
	mem::size_of,
	ptr::{
		read_volatile,
		write_volatile
	},
	prelude::rust_2024::derive,
	result::{
		Result,
		Result::*
	}
};

use alloc::{vec, vec::Vec };

use ::sys::error::Error;

enum DescType {
	Tx,
	Rx
}

impl DescType {
	pub fn ring_offset(&self, info: &DmaInfo) -> usize {
		match self {
			Self::Tx => info.tx_ring_offset(),
			Self::Rx => info.rx_ring_offset()
		}        
	}

	pub fn buff_offset(&self, info: &DmaInfo) -> usize {
		match self {
			Self::Tx => info.tx_buff_offset(),
			Self::Rx => info.rx_buff_offset()
		}
	}
}

#[derive(Default, Clone)]
pub struct Descriptor {
    buff_address: *mut u64,
    fields: *mut u64
}

impl Descriptor {
	fn from(dma_manager: &DmaManager, dtype: DescType) -> Result<Vec<Descriptor>, Error> {
		let info = dma_manager.info();
		let mut descs = vec![Self::default(); info.desc_count().into()];
		
		for (i, desc) in descs.iter_mut().enumerate() {
			desc.buff_address = (dma_manager.base_vaddr() + dtype.ring_offset(info)
				+ i * crate::DESCRIPTOR_SIZE) as *mut u64;

			unsafe {
				*desc.buff_address =
					(dma_manager.base_paddr()? + dtype.buff_offset(info)
					+ i * (info.buff_len() as usize)) as u64;
			};

			desc.fields = unsafe { desc.buff_address.add(size_of::<*mut u64>()) };

			unsafe { *desc.fields = 0; }
		}

		Ok(descs)
	}

	pub fn tx_from(dma_manager: &DmaManager) -> Result<Vec<Descriptor>, Error> {
		Self::from(dma_manager, DescType::Tx)
	}

	pub fn rx_from(dma_manager: &DmaManager) -> Result<Vec<Descriptor>, Error> {
		Self::from(dma_manager, DescType::Rx)
	}

	#[allow(dead_code)]
	pub unsafe fn read_buff_address(&self) -> u64 {
		unsafe { read_volatile(self.buff_address) }
	}

	#[allow(dead_code)]
	pub unsafe fn read_fields(&self) -> u64 {
		unsafe { read_volatile(self.fields) }
	}

	#[allow(dead_code)]
	pub unsafe fn write_fields(&self, value: u64) {
		unsafe { write_volatile(self.fields, value); } 
	}
}
