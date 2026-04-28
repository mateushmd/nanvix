#![no_std]
#![no_main]

extern crate libc_string;
extern crate nvx;

use ::sys::kcall::mm;
use ::sys::mm::MmioRegionInfo;
use ::nanvix_net::E1000Device;
use ::nanvix_net::e1000_for_nanvix::NanvixKernelFunctions;
use sys::pm::ProcessIdentifier;
use ::syslog;

#[unsafe(no_mangle)]
pub fn main() {
    let mypid: ProcessIdentifier = match ::sys::kcall::pm::getpid() {
        Ok(pid) => pid,
        Err(e) => panic!("failed to get pid (error={:?})", e),
    };
    let myname: &str = "netd";

    ::syslog::info!("running network daemon (pid={:?})...", mypid);

    let e1000_mmio_tag = u64::from_be_bytes(*b"E1000   ");

    syslog::info!("allocating e1000 mmio...");
    if let Err(e) = mm::mmio_alloc(e1000_mmio_tag) {
        panic!("failed to allocate mmio: {:?}", e);
    }

    let info: MmioRegionInfo = match mm::mmio_info(e1000_mmio_tag) {
        Ok(info) => info,
        Err(e) => panic!("failed to query mmio info: {:?}", e),
    };
    
    let mapped_regs = usize::from(info.base());

    syslog::info!("e1000 mmio mapped at {:#x}", mapped_regs);

    syslog::info!("initializing e1000 device...");
    let _device = match E1000Device::new(NanvixKernelFunctions, mapped_regs) {
        Ok(d) => d,
        Err(e) => panic!("failed to initialize e1000 device: {:?}", e),
    };

    syslog::info!("e1000 device initialized successfully!");
    
    // Shut down gracefully
    let _ = ::sys::kcall::pm::exit(0);
}
