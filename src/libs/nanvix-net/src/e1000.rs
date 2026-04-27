use ::alloc::vec::Vec;
use ::core::{
    cmp::min,
    mem::size_of,
    ptr,
    slice,
    sync::atomic::{Ordering, fence},
};
use ::smoltcp::{
    phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken},
    time::Instant,
    wire::EthernetAddress,
};
use ::sys::error::{Error, ErrorCode};

const PAGE_SIZE: usize = 4096;
const TX_RING_SIZE: usize = 256;
const RX_RING_SIZE: usize = 256;
const MBUF_SIZE: usize = 2048;
const ETHERNET_MTU: usize = 1514;
const REG_COUNT: usize = 0x20_000 / size_of::<u32>();

const TX_RING_PAGES: usize = (TX_RING_SIZE * size_of::<TxDesc>()).div_ceil(PAGE_SIZE);
const RX_RING_PAGES: usize = (RX_RING_SIZE * size_of::<RxDesc>()).div_ceil(PAGE_SIZE);
const TX_BUFFER_PAGES: usize = (TX_RING_SIZE * MBUF_SIZE).div_ceil(PAGE_SIZE);
const RX_BUFFER_PAGES: usize = (RX_RING_SIZE * MBUF_SIZE).div_ceil(PAGE_SIZE);

/* Registers */
const E1000_CTL: usize = 0x00000 / 4;
const E1000_STAT: usize = 0x00008 / 4;
const E1000_ITR: usize = 0x000C4 / 4;
const E1000_IMS: usize = 0x000D0 / 4;
const E1000_RCTL: usize = 0x00100 / 4;
const E1000_TCTL: usize = 0x00400 / 4;
const E1000_TIPG: usize = 0x00410 / 4;
const E1000_RDBAL: usize = 0x02800 / 4;
const E1000_RDBAH: usize = 0x02804 / 4;
const E1000_RDLEN: usize = 0x02808 / 4;
const E1000_RDH: usize = 0x02810 / 4;
const E1000_RDT: usize = 0x02818 / 4;
const E1000_RDTR: usize = 0x02820 / 4;
const E1000_RADV: usize = 0x0282C / 4;
const E1000_TDBAL: usize = 0x03800 / 4;
const E1000_TDBAH: usize = 0x03804 / 4;
const E1000_TDLEN: usize = 0x03808 / 4;
const E1000_TDH: usize = 0x03810 / 4;
const E1000_TDT: usize = 0x03818 / 4;
const E1000_TIDV: usize = 0x03820 / 4;
const E1000_TADV: usize = 0x0382C / 4;
const E1000_RFCTL: usize = 0x05008 / 4;
const E1000_MTA: usize = 0x05200 / 4;
const E1000_RA: usize = 0x05400 / 4;

/* Device control */
const E1000_CTL_SLU: u32 = 0x00000040;
const E1000_CTL_RST: u32 = 1 << 26;

/* Transmit control */
const E1000_TCTL_EN: u32 = 0x00000002;
const E1000_TCTL_PSP: u32 = 0x00000008;
const E1000_TCTL_CT_SHIFT: u32 = 4;
const E1000_TCTL_COLD_SHIFT: u32 = 12;

/* Receive control */
const E1000_RCTL_EN: u32 = 0x00000002;
const E1000_RCTL_BAM: u32 = 0x00008000;
const E1000_RCTL_SZ_2048: u32 = 0x00000000;
const E1000_RCTL_SECRC: u32 = 0x04000000;

/* Descriptor bits */
const E1000_TXD_CMD_EOP: u32 = 0x01;
const E1000_TXD_CMD_RS: u32 = 0x08;
const E1000_TXD_STAT_DD: u8 = 0x01;
const E1000_RXD_STAT_DD: u8 = 0x01;
const E1000_RXD_STAT_EOP: u8 = 0x02;

#[derive(Debug, Copy, Clone)]
#[repr(C, align(16))]
pub struct TxDesc {
    pub addr: u64,
    pub length: u16,
    pub cso: u8,
    pub cmd: u8,
    pub status: u8,
    pub css: u8,
    pub special: u16,
}

impl TxDesc {
    const fn empty() -> Self {
        Self {
            addr: 0,
            length: 0,
            cso: 0,
            cmd: 0,
            status: 0,
            css: 0,
            special: 0,
        }
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C, align(16))]
pub struct RxDesc {
    pub addr: u64,
    pub length: u16,
    pub csum: u16,
    pub status: u8,
    pub errors: u8,
    pub special: u16,
}

impl RxDesc {
    const fn empty() -> Self {
        Self {
            addr: 0,
            length: 0,
            csum: 0,
            status: 0,
            errors: 0,
            special: 0,
        }
    }
}

pub trait KernelFunctions {
    const PAGE_SIZE: usize = PAGE_SIZE;

    /// Allocates physically contiguous DMA memory.
    ///
    /// Returns `(virtual_address, dma_address)`.
    fn dma_alloc(&mut self, pages: usize) -> (usize, usize);

    fn dma_free(&mut self, vaddr: usize, pages: usize);
}

#[derive(Debug, Copy, Clone)]
struct DmaRegion {
    vaddr: usize,
    dma_addr: usize,
    pages: usize,
}

impl DmaRegion {
    fn new(vaddr: usize, dma_addr: usize, pages: usize) -> Self {
        Self {
            vaddr,
            dma_addr,
            pages,
        }
    }
}

pub struct E1000Device<K: KernelFunctions> {
    regs: *mut u32,
    rx_ring: DmaRegion,
    tx_ring: DmaRegion,
    rx_buffers: DmaRegion,
    tx_buffers: DmaRegion,
    mac_address: EthernetAddress,
    kfn: K,
}

impl<K: KernelFunctions> E1000Device<K> {
    pub fn new(mut kfn: K, mapped_regs: usize) -> Result<Self, Error> {
        let tx_ring = Self::alloc_dma(&mut kfn, TX_RING_PAGES, "tx descriptor ring")?;
        let rx_ring = Self::alloc_dma(&mut kfn, RX_RING_PAGES, "rx descriptor ring")?;
        let tx_buffers = Self::alloc_dma(&mut kfn, TX_BUFFER_PAGES, "tx buffers")?;
        let rx_buffers = Self::alloc_dma(&mut kfn, RX_BUFFER_PAGES, "rx buffers")?;

        let mut device = Self {
            regs: mapped_regs as *mut u32,
            rx_ring,
            tx_ring,
            rx_buffers,
            tx_buffers,
            mac_address: EthernetAddress([0, 0, 0, 0, 0, 0]),
            kfn,
        };

        device.reset_rings();
        device.init()?;
        Ok(device)
    }

    pub fn init(&mut self) -> Result<(), Error> {
        if !self.ring_len_is_valid() {
            return Err(Error::new(
                ErrorCode::InvalidArgument,
                "descriptor ring length must be 128-byte aligned",
            ));
        }

        let ctl = self.read_reg(E1000_CTL);
        self.write_reg(E1000_IMS, 0);
        self.write_reg(E1000_CTL, ctl | E1000_CTL_RST);
        self.write_flush();

        self.write_reg(E1000_CTL, self.read_reg(E1000_CTL) | E1000_CTL_SLU);
        self.init_tx();
        self.init_rx();
        self.mac_address = self.read_mac_address();

        Ok(())
    }

    pub fn hardware_addr(&self) -> EthernetAddress {
        self.mac_address
    }

    pub fn set_hardware_addr(&mut self, address: EthernetAddress) {
        let [a, b, c, d, e, f] = address.0;
        let ral = u32::from(a)
            | (u32::from(b) << 8)
            | (u32::from(c) << 16)
            | (u32::from(d) << 24);
        let rah = u32::from(e) | (u32::from(f) << 8) | (1 << 31);

        self.write_reg(E1000_RA, ral);
        self.write_reg(E1000_RA + 1, rah);
        self.mac_address = address;
        self.write_flush();
    }

    pub fn transmit_frame(&mut self, packet: &[u8]) -> Result<usize, Error> {
        if packet.len() > MBUF_SIZE {
            return Err(Error::new(
                ErrorCode::InvalidArgument,
                "packet exceeds e1000 buffer size",
            ));
        }

        let index = self.read_reg(E1000_TDT) as usize;
        let tx_buffer_ptr = self.tx_buffer_ptr(index);
        unsafe {
            let descriptor = &mut self.tx_ring_mut()[index];
            if descriptor.status & E1000_TXD_STAT_DD == 0 {
                return Err(Error::new(
                    ErrorCode::TryAgain,
                    "tx descriptor still owned by device",
                ));
            }

            let buffer = slice::from_raw_parts_mut(tx_buffer_ptr, packet.len());
            buffer.copy_from_slice(packet);

            descriptor.length = packet.len() as u16;
            descriptor.cso = 0;
            descriptor.cmd = (E1000_TXD_CMD_EOP | E1000_TXD_CMD_RS) as u8;
            descriptor.status = 0;
            descriptor.css = 0;
            descriptor.special = 0;
        }

        fence(Ordering::SeqCst);
        self.write_reg(E1000_TDT, ((index + 1) % TX_RING_SIZE) as u32);
        self.write_flush();

        Ok(packet.len())
    }

    pub fn receive_frame(&mut self) -> Option<Vec<u8>> {
        let index = (self.read_reg(E1000_RDT) as usize + 1) % RX_RING_SIZE;
        let rx_buffer_ptr = self.rx_buffer_ptr(index);
        let frame = {
            let descriptor = &mut self.rx_ring_mut()[index];

            if descriptor.status & E1000_RXD_STAT_DD == 0 {
                return None;
            }

            let frame = if descriptor.status & E1000_RXD_STAT_EOP == 0 {
                None
            } else {
                let length = min(descriptor.length as usize, MBUF_SIZE);
                Some(unsafe { slice::from_raw_parts(rx_buffer_ptr, length).to_vec() })
            };

            descriptor.length = 0;
            descriptor.csum = 0;
            descriptor.status = 0;
            descriptor.errors = 0;
            descriptor.special = 0;

            frame
        };

        fence(Ordering::SeqCst);
        self.write_reg(E1000_RDT, index as u32);
        self.write_flush();

        frame
    }

    fn alloc_dma(kfn: &mut K, pages: usize, reason: &'static str) -> Result<DmaRegion, Error> {
        let (vaddr, dma_addr) = kfn.dma_alloc(pages);
        if vaddr == 0 || dma_addr == 0 {
            return Err(Error::new(ErrorCode::OutOfMemory, reason));
        }

        Ok(DmaRegion::new(vaddr, dma_addr, pages))
    }

    fn ring_len_is_valid(&self) -> bool {
        (TX_RING_SIZE * size_of::<TxDesc>()) % 128 == 0
            && (RX_RING_SIZE * size_of::<RxDesc>()) % 128 == 0
    }

    fn init_tx(&mut self) {
        self.write_reg(
            E1000_TCTL,
            E1000_TCTL_EN
                | E1000_TCTL_PSP
                | (0x10 << E1000_TCTL_CT_SHIFT)
                | (0x40 << E1000_TCTL_COLD_SHIFT),
        );
        self.write_reg(E1000_TIPG, 10 | (8 << 10) | (6 << 20));
        self.write_reg(E1000_TDBAL, self.tx_ring.dma_addr as u32);
        self.write_reg(E1000_TDBAH, self.tx_ring.dma_addr.wrapping_shr(32) as u32);
        self.write_reg(E1000_TDLEN, (TX_RING_SIZE * size_of::<TxDesc>()) as u32);
        self.write_reg(E1000_TDH, 0);
        self.write_reg(E1000_TDT, 0);
        self.write_reg(E1000_TIDV, 0);
        self.write_reg(E1000_TADV, 0);
    }

    fn init_rx(&mut self) {
        self.write_reg(
            E1000_RCTL,
            (E1000_RCTL_EN | E1000_RCTL_BAM | E1000_RCTL_SZ_2048 | E1000_RCTL_SECRC)
                & !(0b11 << 10),
        );
        self.write_reg(E1000_RFCTL, 0);
        self.write_reg(E1000_RDBAL, self.rx_ring.dma_addr as u32);
        self.write_reg(E1000_RDBAH, self.rx_ring.dma_addr.wrapping_shr(32) as u32);
        self.write_reg(E1000_RDLEN, (RX_RING_SIZE * size_of::<RxDesc>()) as u32);
        self.write_reg(E1000_RDH, 0);
        self.write_reg(E1000_RDT, (RX_RING_SIZE - 1) as u32);
        self.write_reg(E1000_RDTR, 0);
        self.write_reg(E1000_RADV, 0);
        self.write_reg(E1000_ITR, 0);

        for index in 0..(4096 / 32) {
            self.write_reg(E1000_MTA + index, 0);
        }
    }

    fn reset_rings(&mut self) {
        let tx_buffers_dma = self.tx_buffers.dma_addr;
        let rx_buffers_dma = self.rx_buffers.dma_addr;

        for (index, descriptor) in self.tx_ring_mut().iter_mut().enumerate() {
            *descriptor = TxDesc::empty();
            descriptor.addr = (tx_buffers_dma + (index * MBUF_SIZE)) as u64;
            descriptor.status = E1000_TXD_STAT_DD;
        }

        for (index, descriptor) in self.rx_ring_mut().iter_mut().enumerate() {
            *descriptor = RxDesc::empty();
            descriptor.addr = (rx_buffers_dma + (index * MBUF_SIZE)) as u64;
        }

        fence(Ordering::SeqCst);
    }

    fn read_mac_address(&self) -> EthernetAddress {
        let ral = self.read_reg(E1000_RA);
        let rah = self.read_reg(E1000_RA + 1);

        EthernetAddress([
            (ral & 0xff) as u8,
            ((ral >> 8) & 0xff) as u8,
            ((ral >> 16) & 0xff) as u8,
            ((ral >> 24) & 0xff) as u8,
            (rah & 0xff) as u8,
            ((rah >> 8) & 0xff) as u8,
        ])
    }

    fn tx_ring_mut(&mut self) -> &mut [TxDesc] {
        unsafe { slice::from_raw_parts_mut(self.tx_ring.vaddr as *mut TxDesc, TX_RING_SIZE) }
    }

    fn rx_ring_mut(&mut self) -> &mut [RxDesc] {
        unsafe { slice::from_raw_parts_mut(self.rx_ring.vaddr as *mut RxDesc, RX_RING_SIZE) }
    }

    fn tx_buffer_ptr(&self, index: usize) -> *mut u8 {
        (self.tx_buffers.vaddr + (index * MBUF_SIZE)) as *mut u8
    }

    fn rx_buffer_ptr(&self, index: usize) -> *const u8 {
        (self.rx_buffers.vaddr + (index * MBUF_SIZE)) as *const u8
    }

    fn read_reg(&self, index: usize) -> u32 {
        debug_assert!(index < REG_COUNT);
        unsafe { ptr::read_volatile(self.regs.add(index)) }
    }

    fn write_reg(&mut self, index: usize, value: u32) {
        debug_assert!(index < REG_COUNT);
        unsafe { ptr::write_volatile(self.regs.add(index), value) };
    }

    fn write_flush(&self) {
        let _ = self.read_reg(E1000_STAT);
    }
}

impl<K: KernelFunctions> Drop for E1000Device<K> {
    fn drop(&mut self) {
        self.kfn.dma_free(self.tx_ring.vaddr, self.tx_ring.pages);
        self.kfn.dma_free(self.rx_ring.vaddr, self.rx_ring.pages);
        self.kfn.dma_free(self.tx_buffers.vaddr, self.tx_buffers.pages);
        self.kfn.dma_free(self.rx_buffers.vaddr, self.rx_buffers.pages);
    }
}

impl<K: KernelFunctions> Device for E1000Device<K> {
    type RxToken<'a>
        = E1000RxToken
    where
        Self: 'a;
    type TxToken<'a>
        = E1000TxToken<'a, K>
    where
        Self: 'a;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let packet = self.receive_frame()?;
        Some((E1000RxToken { buffer: packet }, E1000TxToken { device: self }))
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(E1000TxToken { device: self })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut capabilities = DeviceCapabilities::default();
        capabilities.medium = Medium::Ethernet;
        capabilities.max_transmission_unit = ETHERNET_MTU;
        capabilities.max_burst_size = Some(1);
        capabilities.checksum = Default::default();
        capabilities
    }
}

pub struct E1000RxToken {
    buffer: Vec<u8>,
}

impl RxToken for E1000RxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(&self.buffer)
    }
}

pub struct E1000TxToken<'a, K: KernelFunctions> {
    device: &'a mut E1000Device<K>,
}

impl<K: KernelFunctions> TxToken for E1000TxToken<'_, K> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut packet = ::alloc::vec![0u8; len];
        let result = f(&mut packet);
        let _ = self.device.transmit_frame(&packet);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::core::alloc::Layout;
    use ::std::{
        alloc::{alloc_zeroed, dealloc},
        collections::BTreeMap,
    };

    struct FakeKernel {
        allocations: BTreeMap<usize, Layout>,
    }

    impl FakeKernel {
        fn new() -> Self {
            Self {
                allocations: BTreeMap::new(),
            }
        }
    }

    impl KernelFunctions for FakeKernel {
        fn dma_alloc(&mut self, pages: usize) -> (usize, usize) {
            let layout = Layout::from_size_align(pages * PAGE_SIZE, PAGE_SIZE).unwrap();
            let ptr = unsafe { alloc_zeroed(layout) };
            self.allocations.insert(ptr as usize, layout);
            (ptr as usize, ptr as usize)
        }

        fn dma_free(&mut self, vaddr: usize, _pages: usize) {
            if let Some(layout) = self.allocations.remove(&vaddr) {
                unsafe { dealloc(vaddr as *mut u8, layout) };
            }
        }
    }

    #[test]
    fn transmit_writes_descriptor_and_buffer() {
        let mut regs = ::std::vec![0u32; REG_COUNT];
        let mut device = E1000Device::new(FakeKernel::new(), regs.as_mut_ptr() as usize).unwrap();

        let frame = [0xde, 0xad, 0xbe, 0xef];
        let written = device.transmit_frame(&frame).unwrap();

        assert_eq!(written, frame.len());
        assert_eq!(regs[E1000_TDT], 1);

        let desc = device.tx_ring_mut()[0];
        assert_eq!(desc.length, frame.len() as u16);
        assert_eq!(desc.cmd, (E1000_TXD_CMD_EOP | E1000_TXD_CMD_RS) as u8);

        let bytes = unsafe { slice::from_raw_parts(device.tx_buffer_ptr(0), frame.len()) };
        assert_eq!(bytes, frame);
    }

    #[test]
    fn receive_returns_completed_frame_and_reclaims_descriptor() {
        let mut regs = ::std::vec![0u32; REG_COUNT];
        let mut device = E1000Device::new(FakeKernel::new(), regs.as_mut_ptr() as usize).unwrap();

        let payload = [1u8, 2, 3, 4, 5];
        unsafe {
            let buffer = slice::from_raw_parts_mut(device.rx_buffer_ptr(0) as *mut u8, payload.len());
            buffer.copy_from_slice(&payload);
        }

        let desc = &mut device.rx_ring_mut()[0];
        desc.length = payload.len() as u16;
        desc.status = E1000_RXD_STAT_DD | E1000_RXD_STAT_EOP;
        regs[E1000_RDT] = (RX_RING_SIZE - 1) as u32;

        let frame = device.receive_frame().unwrap();
        assert_eq!(frame, payload);
        assert_eq!(regs[E1000_RDT], 0);
        assert_eq!(device.rx_ring_mut()[0].status, 0);
    }
}
