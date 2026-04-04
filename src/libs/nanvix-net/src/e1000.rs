// Receive Descriptor
#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct e1000_rx_desc {
    pub addr: u64,
    pub length: u16,
    pub checksum: u16,
    pub status: u8,
    pub errors: u8,
    pub special: u16,
}

// Transmit Descriptor
#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct e1000_tx_desc {
    pub addr: u64,
    pub length: u16,
    pub cso: u8,
    pub cmd: u8,
    pub status: u8, // STA + RSV Fields (Each is u4)
    pub css: u8,
    pub special: u16,
}
