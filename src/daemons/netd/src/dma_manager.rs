extern crate alloc;
extern crate libc_string;
extern crate nvx;

use crate::{
	dma_info::DmaInfo
};

use core::{
	matches,
	module_path,
	result::{
		Result,
		Result::*
	},
	writeln
};

use ::sys::{
	error::{
		Error,
		ErrorCode
	},
	kcall::mm::dma_alloc,
	mm::VirtualAddress
};

pub struct DmaManager {
	info: DmaInfo,
	base_vaddr: usize,
	base_paddr: Result<usize, Error>
}

impl DmaManager {
	pub fn new(info: DmaInfo, vaddr: usize) -> Self {
		Self {
			info: info,
			base_vaddr: vaddr,
			base_paddr: Err(Error::new(
				ErrorCode::OperationNotPermitted,
				"dma region not allocated"
			))
		}
	}

	pub fn alloc(&mut self) -> Result<(), Error> {
		if matches!(self.base_paddr, Err(_)) {
			let paddr = dma_alloc(
				VirtualAddress::from_raw_value(self.base_vaddr), self.info().dma_len()
			)?;

			syslog::info!(
				"DMA allocation successful: vaddr={:#x}, paddr={:#x}",
				self.base_vaddr,
				paddr
			);

			self.base_paddr = Ok(paddr);
		}

		Ok(())
	}

	pub fn info(&self) -> &DmaInfo {
		&self.info
	}

	pub fn base_vaddr(&self) -> usize {
		self.base_vaddr
	}

	pub fn base_paddr(&self) -> Result<usize, Error> {
		match &self.base_paddr {
			Ok(paddr) => Ok(*paddr),
			Err(err) => Err(Error::new(err.code, err.reason))
		}
	}

	#[allow(dead_code)]
	pub fn tx_start_paddr(&self) -> Result<usize, Error> {
		self.base_paddr()
	}

	#[allow(dead_code)]
	pub fn rx_start_paddr(&self) -> Result<usize, Error> {
		Ok(
			self.base_paddr()? + self.info().rx_ring_offset()
		)
	}
}
