use ::core::sync::atomic::{AtomicUsize, Ordering};
#[allow(unused_imports)]
use ::sys::{kcall::mm, mm::MmioRegionInfo, mm::VirtualAddress};
use crate::e1000::KernelFunctions;

#[allow(dead_code)]
const E1000_MMIO_TAG: u64 = u64::from_be_bytes(*b"E1000   ");

static DMA_VADDR_BASE: AtomicUsize = AtomicUsize::new(0x5000_0000);

pub struct NanvixKernelFunctions;

impl KernelFunctions for NanvixKernelFunctions {
	fn dma_alloc(&mut self, pages: usize) -> (usize, usize) {
		let vaddr_raw = DMA_VADDR_BASE.fetch_add(pages * Self::PAGE_SIZE, Ordering::SeqCst);
		let vaddr = VirtualAddress::from_raw_value(vaddr_raw);

		match mm::dma_alloc(vaddr, pages) {
			Ok(paddr) => (vaddr_raw, paddr),
			Err(e) => {
				::syslog::error!("dma_alloc failed: {:?}", e);
				(0, 0)
			},
		}
	}

	fn dma_free(&mut self, vaddr: usize, pages: usize) {
		if vaddr == 0 { return; }
		let vaddr = VirtualAddress::from_raw_value(vaddr);
		let _ = mm::dma_free(vaddr, pages);
	}
}
