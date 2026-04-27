use ::core::alloc::Layout;
use ::alloc::alloc::{alloc_zeroed, dealloc};
#[allow(unused_imports)]
use ::sys::{kcall::mm, mm::MmioRegionInfo};
use crate::e1000::KernelFunctions;

#[allow(dead_code)]
const E1000_MMIO_TAG: u64 = u64::from_be_bytes(*b"E1000   ");

pub struct NanvixKernelFunctions;

impl KernelFunctions for NanvixKernelFunctions {
	fn dma_alloc(&mut self, pages: usize) -> (usize, usize) {
		// 1. Calculate the total size needed
		let size = pages * Self::PAGE_SIZE;

		// 2. Create a layout that forces Page Alignment (4096 bytes)
		let layout = Layout::from_size_align(size, Self::PAGE_SIZE).unwrap();

		// 3. Allocate zeroed memory (Important for security and preventing garbage data)
		let ptr = unsafe { alloc_zeroed(layout) };

		if ptr.is_null() {
			return (0, 0); // Allocation failed
		}

		// 4. In Nanvix's current simple virtual memory model for the default target, 
		// the virtual address often maps 1:1 to the physical address for these allocations,
		// or the network card uses the virtual address if an IOMMU is not in the way.
		let vaddr = ptr as usize;
		let dma_addr = vaddr; // Assuming 1:1 mapping or no IOMMU translation required

		(vaddr, dma_addr)
	}

	fn dma_free(&mut self, vaddr: usize, pages: usize) {
		if vaddr == 0 { return; }

		let size = pages * Self::PAGE_SIZE;
		let layout = Layout::from_size_align(size, Self::PAGE_SIZE).unwrap();

		unsafe { dealloc(vaddr as *mut u8, layout) };
	}
}
