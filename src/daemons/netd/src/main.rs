#![no_std]
#![no_main]

extern crate alloc;
extern crate libc_string;
extern crate nvx;

use ::sys::{ 
    kcall:: {
        mm,
        pm
    },
    mm::{
        Address,
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

#[unsafe(no_mangle)]
pub fn main() {
    let _mypid = init();

    let info = init_e1000();
    
    let mapped_regs = usize::from(info.base());

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

    let ral = unsafe { core::ptr::read_volatile((mapped_regs + 0x5400) as *const u32) };
    let rah = unsafe { core::ptr::read_volatile((mapped_regs + 0x5404) as *const u32) };

    let mac = [
        (ral & 0xff) as u8,
        ((ral >> 8) & 0xff) as u8,
        ((ral >> 16) & 0xff) as u8,
        ((ral >> 24) & 0xff) as u8,
        (rah & 0xff) as u8,
        ((rah >> 8) & 0xff) as u8,
    ];
    syslog::info!("E1000 MAC Address read directly from MMIO: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}", mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);

    syslog::info!("netd is running in a minimal loop.");

    loop {
        let _ = ::sys::kcall::pm::sleep(::core::time::Duration::from_secs(1));
    }
}