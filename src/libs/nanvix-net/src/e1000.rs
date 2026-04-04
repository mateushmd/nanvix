const PAGE_SIZE:usize = 4096;
const RING_SIZE: usize = 256;
const MBUF_SIZE: usize = 2048;

const alloc_tx_ring_pages: usize =
    ((RING_SIZE * size_of::<Tx_desc>()) + (PAGE_SIZE - 1)) / PAGE_SIZE;
const alloc_rx_ring_pages: usize =
    ((RING_SIZE * size_of::<Rx_desc>()) + (PAGE_SIZE - 1)) / PAGE_SIZE;

const alloc_tx_buffer_pages: usize = 
    ((RING_SIZE * MBUF_SIZE) + (PAGE_SIZE - 1)) / PAGE_SIZE;
const alloc_rx_buffer_pages: usize = 
    ((RING_SIZE * MBUF_SIZE) + (PAGE_SIZE - 1)) / PAGE_SIZE;


// Receive Descriptor
#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct Tx_desc {
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
pub struct Rx_desc {
    pub addr: u64,
    pub length: u16,
    pub cso: u8,
    pub cmd: u8,
    pub status: u8, // STA + RSV Fields (Each is u4)
    pub css: u8,
    pub special: u16,
}

pub trait KernelFunctions {
    const PAGE_SIZE: usize = 4096;
    fn dma_alloc(&mut self, pages: usize) -> (usize, usize);
    fn dma_free(&mut self, vaddr: usize, pages: usize);
}

pub struct E1000Repr<'a, K: KernelFunctions> {
    regs: &'static mut [todo!("Volatile pointers?")],
    rx_ring_dma: usize,
    tx_ring_dma: usize,
    rx_ring: &'a mut [Rx_desc],
    tx_ring: &'a mut [Tx_desc],
    rx_mbufs: Vec<usize>, // Can use vec??
    tx_mbufs: Vec<usize>,
    mbuf_size: usize,
    kfn: K,
}

impl<'a, K: KernelFunctions> E1000Repr<'a, K> {

    pub fn new(mut kfn: K, mapped_regs: usize) -> Result<Self, i32> {
        todo!("Create and allocate an E1000Repr")
    }

    pub fn init(&mut self) {
        todo!("Initialize driver");
    }

    pub fn transmit(&mut self, packet: &[u8]) -> i32 {
        todo!("Transmit function")
    }

    pub fn receive(&mut self) -> Option<Vec<Vec<u8>>> {
        // Can we use Vec???
        todo!("Receive Function")
    }

    // TODO! (Interrupts functions)
}

// Drop DMA memory
impl <'a, K: KernelFunctions> Drop for E1000Repr<'a, K> {
    fn drop(&mut self) {
        self.kfn.dma_free(self.tx_ring.as_ptr() as usize, alloc_tx_ring_pages);
        self.kfn.dma_free(self.rx_ring.as_ptr() as usize, alloc_rx_ring_pages);
        self.kfn.dma_free(self.tx_mbufs[0], alloc_tx_buffer_pages);
        self.kfn.dma_free(self.rx_mbufs[0], alloc_rx_buffer_pages);
    }
}
