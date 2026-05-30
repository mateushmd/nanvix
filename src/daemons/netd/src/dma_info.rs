pub struct DmaInfo {
	desc_count: u8,
	buff_len: u16
}

impl DmaInfo {
	pub fn new(desc_count: u8, buff_len: u16) -> Self {
		Self {
			desc_count: desc_count,
			buff_len: buff_len
		}
	}

    /// Number of descriptors of the same type
	pub fn desc_count(&self) -> u8 {
		self.desc_count
	}

	pub fn buff_len(&self) -> u16 {
		self.buff_len
	}

	pub fn dma_len(&self) -> usize {
		2 * (self.desc_count() as usize) *
			(crate::DESCRIPTOR_SIZE + (self.buff_len() as usize))
	}

	pub fn ring_len(&self) -> usize {
		(self.desc_count() as usize) * crate::DESCRIPTOR_SIZE
	}

	pub fn tx_ring_offset(&self) -> usize {
		0
	}

	pub fn rx_ring_offset(&self) -> usize {
		self.tx_ring_offset() + self.ring_len()
	}

	pub fn tx_buff_offset(&self) -> usize {
		self.rx_ring_offset() + self.ring_len()
	}

	pub fn rx_buff_offset(&self) -> usize {
		self.tx_buff_offset() + (self.buff_len() as usize) * (self.desc_count() as usize)
	}
}
