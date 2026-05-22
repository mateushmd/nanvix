#![no_std]
#![no_main]

extern crate alloc;
extern crate libc_string;
extern crate nvx;

const REG_EEPROM: u32 = 0x0014;
const REG_CTL: u32 = 0x0;
const REG_STAT: u32 = 0x008;

const CTL_RST: u32 = 1 << 26;       // Reset
const CTL_SLU: u32 = 0x0040;        // Set Link Up
const CTL_ASDE: u32 = 0x0020;       // Auto Speed Detection Enabled
                                
#[repr(C, align(16))]
#[allow(dead_code)]
pub struct TxDesc {
    base_address: u64,    // Trasmit Descriptor Base Address (low + high)
    len: u16,             // Trasmit Descriptor Base Length
    head: u32,            // Trasmit Descriptor Head
    tail: u32             // Trasmit Descriptor Tail
}

impl TxDesc {
    #[allow(dead_code)]
    fn new (base_address: u64, len: u16, head: u32, tail: u32) -> Self {
        Self {
            base_address,
            len,
            head,
            tail
        }
    }
    #[allow(dead_code)]
    fn empty () -> Self {
        Self {
            base_address: 0,
            len: 0,
            head: 0,
            tail: 0
        }
    }
}

use ::sys::{ 
    kcall:: {
        mm,
        pm
    },
    mm::{
        MmioRegionInfo
    },
    pm::ProcessIdentifier
};

fn init() -> ProcessIdentifier {
    let mypid: ProcessIdentifier = match pm::getpid() {
        Ok(pid) => pid,
        Err(e) => panic!("failed to get pid (error={:?})", e),
    };

    if let Err(e) = pm::capctl(::sys::pm::Capability::IoManagement, true) {
        panic!("failed to acquire I/O management capability (error={:?})", e);
    };

    syslog::info!("netd initialized successfully");

    mypid
}

fn init_e1000() -> MmioRegionInfo {
    let e1000_mmio_tag = u64::from_be_bytes(*b"E1000   ");

    if let Err(e) = mm::mmio_alloc(e1000_mmio_tag) {
        panic!("failed to allocate mmio region for e1000 (error={:?})", e);
    };

    let info: MmioRegionInfo = match mm::mmio_info(e1000_mmio_tag) {
        Ok(info) => info,
        Err(e) => panic!("failed to query mmio info for e1000 (error={:?})", e),
    };

    syslog::info!("mmio for e1000 initialized successfully");
    
    info
}

struct MMIO {
    base_address: u32
}

impl MMIO {
    fn new(base_address: u32) -> Self {
        MMIO { base_address: base_address }
    }

    fn read(&self, address_offset: u32) -> u32 {
        syslog::debug!("reading {:#x}", self.base_address + address_offset);
        unsafe {
            core::ptr::read_volatile(
                (self.base_address + address_offset) as *const u32
            )
        }
    }

    fn write(&self, address_offset: u32, value: u32) {
        syslog::debug!("writing to {:#x}", self.base_address + address_offset);
        unsafe {
            core::ptr::write_volatile(
                (self.base_address + address_offset) as *mut u32,
                value
            );
        }
    }
}

fn detect_eeprom(mmio: &MMIO) -> bool {
    mmio.write(REG_EEPROM, 0x1);

    let mut eeprom_exists = false;

    let mut i: u32 = 0;

    while !eeprom_exists && i < 1000 {
        let val = mmio.read(REG_EEPROM);

        eeprom_exists = val & 0x10 > 0;
        
        i += 1;
    }

    eeprom_exists
}

fn read_eeprom(mmio: &MMIO, address: u8) -> u16 {
    let mut tmp = 0u32;

    mmio.write(REG_EEPROM, 1 | ((address as u32) << 8));
    while tmp & 0b10000 == 0 {
        tmp = mmio.read(REG_EEPROM);
    }

    (tmp >> 16) as u16
}

#[unsafe(no_mangle)]
pub fn main() {
    let _mypid = init();

    let info = init_e1000();
    
    let base = usize::from(info.base());

    let mmio = MMIO::new(base as u32);

    /*
    syslog::info!("testing dma allocation...");
    let test_vaddr = ::sys::mm::VirtualAddress::from_raw_value(0x6000_0000);
    match mm::dma_alloc(test_vaddr, 1) {
        Ok(paddr) => {
            syslog::info!("dma allocation successful: vaddr={:#x}, paddr={:#x}", test_vaddr.into_raw_value(), paddr);
            if let Err(e) = mm::dma_free(test_vaddr, 1) {
                panic!("failed to free dma memory: {:?}", e);
            }
            syslog::info!("dma memory freed successfully!");
        }
        Err(e) => panic!("failed to allocate dma memory: {:?}", e),
    }
    */

    // Reset card
    let ctl = mmio.read(REG_CTL);
    mmio.write(REG_CTL, ctl | CTL_RST);

    match detect_eeprom(&mmio) {
        true => syslog::info!("found eeprom"),
        false => panic!("couldn't found eeprom")
    };

    let temp1 = read_eeprom(&mmio, 0);
    let temp2 = read_eeprom(&mmio, 1);
    let temp3 = read_eeprom(&mmio, 2);

    let mac = [
        temp1 & 0xff,
        temp1 >> 8,
        temp2 & 0xff,
        temp2 >> 8,
        temp3 & 0xff,
        temp3 >> 8,
    ];

    syslog::info!("E1000 MAC Address read directly from MMIO: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}", mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
    
    // Start link & Set ASDE
    let ctl = mmio.read(REG_CTL);
    mmio.write(REG_CTL, ctl | CTL_SLU | CTL_ASDE);

    // Find negotiated speed
    let status = mmio.read(REG_STAT);
    match status & 0b10 != 0 {
        true => {
            syslog::info!("Link is up!");

            let speed = match (status & 0b1100_0000) >> 6 {
                0 => 10,
                1 => 100,
                2 => 1000,
                _ => -1,
            };

            syslog::info!("Auto-negotiated speed: {} Mbps", speed);
        },
        false => { panic!("The link is not up!"); }
    }

//      init_tx();
//      init_rx();

    loop {
        let _ = ::sys::kcall::pm::sleep(::core::time::Duration::from_secs(1));
    }
}
