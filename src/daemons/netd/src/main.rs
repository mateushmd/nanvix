#![no_std]
#![no_main]

extern crate alloc;
extern crate libc_string;
extern crate nvx;

use ::sys::kcall::{ 
    mm,
    pm
};
use ::sys::{ 
    mm::MmioRegionInfo,
    pm::ProcessIdentifier
};
use ::nanvix_net::E1000Device;
use ::nanvix_net::e1000_for_nanvix::NanvixKernelFunctions;

fn init() -> ProcessIdentifier {
    let mypid: ProcessIdentifier = match pm::getpid() {
        Ok(pid) => pid,
        Err(e) => panic!("failed to get pid (error={:?})", e),
    };

    if let Err(e) = pm::capctl(pm::Capability::IoManagement, true) {
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
    let mypid = init();

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

    syslog::info!("initializing e1000 device...");
    let mut device = match E1000Device::new(NanvixKernelFunctions, mapped_regs) {
        Ok(d) => d,
        Err(e) => panic!("failed to initialize e1000 device: {:?}", e),
    };

    syslog::info!("e1000 device initialized successfully! MAC: {}", device.hardware_addr());
    
    use ::smoltcp::iface::{Config, Interface, SocketSet};
    use ::smoltcp::socket::icmp::{Socket as IcmpSocket, PacketBuffer as IcmpPacketBuffer};
    use ::smoltcp::wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr, Ipv4Address};
    use ::smoltcp::time::Instant;

    let mac_addr = device.hardware_addr();
    let hw_addr = HardwareAddress::Ethernet(mac_addr);
    
    let mut config = Config::new(hw_addr);
    config.random_seed = 0; // Simple seed
    let mut iface = Interface::new(config, &mut device, Instant::from_millis(0));
    
    iface.update_ip_addrs(|ip_addrs| {
        ip_addrs.push(IpCidr::new(IpAddress::v4(10, 0, 2, 15), 24)).unwrap();
    });

    let mut sockets = SocketSet::new(alloc::vec![]);
    let icmp_rx_buffer = IcmpPacketBuffer::new(alloc::vec![smoltcp::socket::icmp::PacketMetadata::EMPTY; 1], alloc::vec![0; 256]);
    let icmp_tx_buffer = IcmpPacketBuffer::new(alloc::vec![smoltcp::socket::icmp::PacketMetadata::EMPTY; 1], alloc::vec![0; 256]);
    let icmp_socket = IcmpSocket::new(icmp_rx_buffer, icmp_tx_buffer);
    sockets.add(icmp_socket);

    syslog::info!("starting network event loop");

    loop {
        let timestamp = {
            let mut sec: u64 = 0;
            let mut nsec: u64 = 0;
            let _ = ::sys::kcall::pm::gettime(&mut sec, &mut nsec);
            let millis = sec * 1000 + nsec / 1_000_000;
            Instant::from_millis(millis as i64)
        };

        iface.poll(timestamp, &mut device, &mut sockets);

        // Sleep to yield CPU
        let _ = ::sys::kcall::pm::sleep(0, 10_000_000); // 10ms
    }
}
