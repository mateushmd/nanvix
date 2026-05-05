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
use ::nanvix_net::E1000Device;
use ::nanvix_net::e1000_for_nanvix::NanvixKernelFunctions;

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

    syslog::info!("initializing e1000 device...");
    let mut device = match E1000Device::new(NanvixKernelFunctions, mapped_regs) {
        Ok(d) => d,
        Err(e) => panic!("failed to initialize e1000 device: {:?}", e),
    };

    syslog::info!("e1000 device initialized successfully! MAC: {}", device.hardware_addr());
    
    use ::smoltcp::iface::{Config, Interface, SocketSet};
    use ::smoltcp::socket::udp::{Socket as UdpSocket, PacketBuffer as UdpPacketBuffer, PacketMetadata as UdpPacketMetadata};
    use ::smoltcp::wire::{HardwareAddress, IpAddress, IpCidr};
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
    let udp_rx_buffer = UdpPacketBuffer::new(alloc::vec![UdpPacketMetadata::EMPTY; 10], alloc::vec![0; 1024]);
    let udp_tx_buffer = UdpPacketBuffer::new(alloc::vec![UdpPacketMetadata::EMPTY; 10], alloc::vec![0; 1024]);
    let mut udp_socket = UdpSocket::new(udp_rx_buffer, udp_tx_buffer);
    
    // Bind the socket to port 5555
    if let Err(e) = udp_socket.bind(5555) {
        panic!("failed to bind udp socket to port 5555: {:?}", e);
    }
    
    let udp_handle = sockets.add(udp_socket);

    syslog::info!("starting network event loop (listening on UDP 5555)");

    loop {
        let timestamp = {
            let mut time = match ::sys::time::SystemTime::new(0, 0){
                Some(t) => t,
                None => unreachable!(),
            };
            if let Err(e) = ::sys::kcall::pm::gettime(&mut time) {
                panic!("failed to get current system time: {:?}", e);
            }
            time
        };

        let millis = Instant::from_millis((timestamp.seconds() * 1000 + (timestamp.nanoseconds() as u64) / 1_000_000) as i64);

        let _poll_res = iface.poll(millis, &mut device, &mut sockets);

        // Process UDP packets
        let socket = sockets.get_mut::<UdpSocket>(udp_handle);
        if socket.can_recv() {
            if let Ok((data, meta)) = socket.recv() {
                syslog::info!("received UDP packet from {}: {}", meta.endpoint, core::str::from_utf8(data).unwrap_or("<invalid utf8>"));
                
                // Echo it back
                if socket.can_send() {
                    let reply = b"Packet Received!";
                    if let Err(e) = socket.send_slice(reply, meta.endpoint) {
                        syslog::error!("failed to send UDP reply: {:?}", e);
                    } else {
                        syslog::info!("UDP reply sent.");
                    }
                }
            }
        }

        // Sleep to yield CPU
        let _ = ::sys::kcall::pm::sleep(::core::time::Duration::from_millis(10)); // 10ms
    }
}
