#![no_std]
#![no_main]
#![allow(clippy::all)]
#[allow(unused_imports)]

extern crate alloc;
extern crate libc_string;
extern crate nvx;

mod descriptor;
mod dma_info;
mod dma_manager;
mod volatile_cell;

const DESCRIPTOR_SIZE: usize = 16;
use crate::{
    descriptor::Descriptor,
    dma_info::DmaInfo,
    dma_manager::DmaManager,
};
use syscall::safe::time::Time;

#[allow(unused_imports)]
use ::alloc::{
    borrow::ToOwned,
	boxed::Box,
    vec,
    vec::Vec,
};
use ::sys::{
    error::{
        Error,
        ErrorCode,
    },
    kcall::{
        mm,
        pm,
    },
    mm::MmioRegionInfo,
    pm::ProcessIdentifier,
};

#[allow(unused_imports)]
use smoltcp::{
    iface::{
        Config,
        Interface,
        SocketSet,
    },
    phy::{
        Device,
        DeviceCapabilities,
        Medium,
        RxToken,
        TxToken,
    },
    socket::tcp,
    time::Instant,
    wire::{
        EthernetAddress,
        IpAddress,
        IpCidr,
        Ipv4Address,
    },
};

#[allow(unused_imports)]
use ::core::{
    convert::From,
	hint::spin_loop,
    module_path,
    option::{
        Option,
        Option::*,
    },
    panic,
	ptr::write_bytes,
    result::{
        Result,
        Result::*,
    },
    slice,
	str::FromStr,
    sync::atomic::{
        fence,
        Ordering,
    },
    writeln,
};

const REG_EEPROM: u32 = 0x0014;
const REG_CTL: u32 = 0x0;
const REG_STAT: u32 = 0x008;
const REG_RAL: u32 = 0x5400;
const REG_RAH: u32 = 0x5404;
#[allow(dead_code)]
const REG_IMS: u32 = 0x00D0;
const REG_RDTR: u32 = 0x2820;
const REG_RADV: u32 = 0x282C;
const REG_ICR: u32 = 0x00C0;
const REG_IMC: u32 = 0x00D8;

const CTL_RST: u32 = 1 << 26; // Reset
const CTL_SLU: u32 = 0x0040; // Set Link Up
const CTL_ASDE: u32 = 0x0020; // Auto Speed Detection Enabled

const DMA_BASE_ADDRESS: usize = 0x6000_0000;

const REG_MTA: u32 = 0x5200; // Multicast table start

// Transmit registers
const REG_TDBAL: u32 = 0x3800;
const REG_TDBAH: u32 = 0x3804;
const REG_TDLEN: u32 = 0x3808;
const REG_TDH: u32 = 0x3810;
const REG_TDT: u32 = 0x3818;
const REG_TCTL: u32 = 0x0400;
const REG_TIPG: u32 = 0x0410;

// Receive registers
const REG_RDBAL: u32 = 0x2800;
const REG_RDBAH: u32 = 0x2804;
const REG_RDLEN: u32 = 0x2808;
const REG_RDH: u32 = 0x2810;
const REG_RDT: u32 = 0x2818;
const REG_RCTL: u32 = 0x0100;

//// TX/RX Device Control
/* Transmit Control */
//  const TCTL_RST: u32 = 0x00000001;    /* software reset */
const TCTL_EN: u32 = 0x00000002; /* enable tx */
//  const TCTL_BCE: u32 = 0x00000004;    /* busy check enable */
const TCTL_PSP: u32 = 0x00000008; /* pad short packets */
//  const TCTL_CT: u32 = 0x00000ff0;    /* collision threshold */
const TCTL_CT_SHIFT: u32 = 4;
//  const TCTL_COLD: u32 = 0x003ff000;    /* collision distance */
const TCTL_COLD_SHIFT: u32 = 12;
//  const TCTL_SWXOFF: u32 = 0x00400000;    /* SW Xoff transmission */
//  const TCTL_PBE: u32 = 0x00800000;    /* Packet Burst Enable */
//  const TCTL_RTLC: u32 = 0x01000000;    /* Re-transmit on late collision */
//  const TCTL_NRTU: u32 = 0x02000000;    /* No Re-transmit on underrun */
//  const TCTL_MULR: u32 = 0x10000000;    /* Multiple request support */
/* Receive Control */
//  const RCTL_RST: u32 = 0x00000001;    /* Software reset */
const RCTL_EN: u32 = 0x00000002; /* enable */
//  const RCTL_SBP: u32 = 0x00000004;    /* store bad packet */
//  const RCTL_UPE: u32 = 0x00000008;    /* unicast promiscuous enable */
//  const RCTL_MPE: u32 = 0x00000010;    /* multicast promiscuous enab */
//  const RCTL_LPE: u32 = 0x00000020;    /* long packet enable */
//  const RCTL_LBM_NO: u32 = 0x00000000;    /* no loopback mode */
//  const RCTL_LBM_MAC: u32 = 0x00000040;    /* MAC loopback mode */
//  const RCTL_LBM_SLP: u32 = 0x00000080;    /* serial link loopback mode */
//  const RCTL_LBM_TCVR: u32 = 0x000000C0;    /* tcvr loopback mode */
//  const RCTL_DTYP_MASK: u32 = 0x00000C00;    /* Descriptor type mask */
//  const RCTL_DTYP_PS: u32 = 0x00000400;    /* Packet Split descriptor */
//  const RCTL_RDMTS_HALF: u32 = 0x00000000;    /* rx desc min threshold size */
//  const RCTL_RDMTS_QUAT: u32 = 0x00000100;    /* rx desc min threshold size */
//  const RCTL_RDMTS_EIGTH: u32 = 0x00000200;    /* rx desc min threshold size */
//  const RCTL_MO_SHIFT: u32 = 12;            /* multicast offset shift */
//  const RCTL_MO_0: u32 = 0x00000000;    /* multicast offset 11:0 */
//  const RCTL_MO_1: u32 = 0x00001000;    /* multicast offset 12:1 */
//  const RCTL_MO_2: u32 = 0x00002000;    /* multicast offset 13:2 */
//  const RCTL_MO_3: u32 = 0x00003000;    /* multicast offset 15:4 */
//  const RCTL_MDR: u32 = 0x00004000;    /* multicast desc ring 0 */
const RCTL_BAM: u32 = 0x00008000; /* broadcast enable */
/* these buffer sizes are valid if E1000_RCTL_BSEX is 0 */
//  const RCTL_SZ_2048: u32 = 0x00000000;    /* rx buffer size 2048 */
//  const RCTL_SZ_1024: u32 = 0x00010000;    /* rx buffer size 1024 */
//  const RCTL_SZ_512: u32 = 0x00020000;    /* rx buffer size 512 */
//  const RCTL_SZ_256: u32 = 0x00030000;    /* rx buffer size 256 */
/* these buffer sizes are valid if E1000_RCTL_BSEX is 1 */
//  const RCTL_SZ_16384: u32 = 0x00010000;    /* rx buffer size 16384 */
//  const RCTL_SZ_8192: u32 = 0x00020000;    /* rx buffer size 8192 */
const RCTL_SZ_4096: u32 = 0x00030000; /* rx buffer size 4096 */
//  const RCTL_VFE: u32 = 0x00040000;    /* vlan filter enable */
//  const RCTL_CFIEN: u32 = 0x00080000;    /* canonical form enable */
//  const RCTL_CFI: u32 = 0x00100000;    /* canonical form indicator */
//  const RCTL_DPF: u32 = 0x00400000;    /* discard pause frames */
//  const RCTL_PMCF: u32 = 0x00800000;    /* pass MAC control frames */
const RCTL_BSEX: u32 = 0x02000000; /* Buffer size extension */
const RCTL_SECRC: u32 = 0x04000000; /* Strip Ethernet CRC */
//  const RCTL_FLXBUF_MASK: u32 = 0x78000000;    /* Flexible buffer size */
//  const RCTL_FLXBUF_SHIFT: u32 = 27;            /* Flexible buffer shift */
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
    base_address: u32,
}

impl MMIO {
    fn new(base_address: u32) -> Self {
        MMIO {
            base_address: base_address,
        }
    }

    fn read(&self, address_offset: u32) -> u32 {
        //syslog::debug!("reading {:#x}", self.base_address + address_offset);
        unsafe { core::ptr::read_volatile((self.base_address + address_offset) as *const u32) }
    }

    fn write(&self, address_offset: u32, value: u32) {
        //syslog::debug!("writing {:#x} to {:#x}", value, self.base_address + address_offset);
        unsafe {
            core::ptr::write_volatile((self.base_address + address_offset) as *mut u32, value);
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

#[allow(dead_code)]
pub struct E1000Device {
    mmio: MMIO,
    dma_man: DmaManager,
    mac_addr: [u16; 6],
    tx_ring: Vec<*mut Descriptor>,
    rx_ring: Vec<*mut Descriptor>,
	tx_bufs: Vec<usize>,
	rx_bufs: Vec<usize>
}

#[allow(dead_code)]
impl E1000Device {
    pub fn init() -> Self {
        let _mypid = init();

        let info = init_e1000();

        let base = usize::from(info.base());

        let mmio = MMIO::new(base as u32);

        // Reset card
        let ctl = mmio.read(REG_CTL);
        mmio.write(REG_CTL, ctl | CTL_RST);
		while mmio.read(REG_CTL) & CTL_RST != 0 {
			spin_loop();			
		}

        fence(Ordering::Release);

		syslog::info!("E1000 reseted successfully!");

        match detect_eeprom(&mmio) {
            true => syslog::info!("found eeprom"),
            false => panic!("couldn't found eeprom"),
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

        syslog::info!(
            "E1000 MAC Address read directly from MMIO: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            mac[0],
            mac[1],
            mac[2],
            mac[3],
            mac[4],
            mac[5]
        );

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
            false => {
                panic!("The link is not up!");
            },
        }

        let mut dma_man = DmaManager::new(DmaInfo::new(8, 4096), DMA_BASE_ADDRESS);

        if let Err(e) = dma_man.alloc() {
            panic!("Failed to allocate DMA memory: {:?}", e);
        }
        syslog::trace!("DMA Memory successfully allocated!");

		unsafe {
			write_bytes(
				dma_man.base_vaddr() as *mut u8,
				0,
				dma_man.info().ring_len() * 2
			);	
		}

        let (tx_ring, tx_bufs) = Descriptor::tx_from(&dma_man).expect("DMA region does not exist");
        let (rx_ring, rx_bufs) = Descriptor::rx_from(&dma_man).expect("DMA region does not exist");

        // Write TX ring info to e1000 registers
        let tx_addr = dma_man.info().tx_ring_offset()
            + dma_man
                .base_paddr()
                .expect("Failed to get DMA physical addr");
        mmio.write(REG_TDBAH, 0);
        mmio.write(REG_TDBAL, tx_addr as u32);
        mmio.write(REG_TDLEN, dma_man.info().ring_len() as u32);
        mmio.write(REG_TDT, 0);
        mmio.write(REG_TDH, 0);

        // Write RX ring info to e1000 registers
        let rx_addr = dma_man.info().rx_ring_offset()
            + dma_man
                .base_paddr()
                .expect("Failed to get DMA physical addr");
        mmio.write(REG_RDBAH, 0);
        mmio.write(REG_RDBAL, rx_addr as u32);
        mmio.write(REG_RDLEN, dma_man.info().ring_len() as u32);
        mmio.write(REG_RDH, 0);
        mmio.write(REG_RDT, (dma_man.info().desc_count() as u32) - 1);

        // Write MAC to Receive Address
        let ral: u32 = ((temp2 as u32) << 16) | temp1 as u32;
        let rah: u32 = temp3 as u32;
        mmio.write(REG_RAL, ral);
        mmio.write(REG_RAH, rah | (1 << 31));

        // Setup multicast table array
        for i in 0..128 {
            mmio.write(REG_MTA + (i * 4), 0);
        }

        mmio.write(
            REG_TCTL,
            TCTL_EN | TCTL_PSP | (0x10 << TCTL_CT_SHIFT) | (0x40 << TCTL_COLD_SHIFT), // enable | padShortPackets | collision stuff
        );
        mmio.write(
            REG_TIPG,
            10 | (8 << 10) | (6 << 20), // Intel magic number
        );

        mmio.write(
            REG_RCTL,
            RCTL_EN | RCTL_BAM | RCTL_SZ_4096 | RCTL_BSEX | RCTL_SECRC, // enable | broadcast | 4096byte rx buffer
        );

		// Intel docs recommends not setting certain bit ranges of these registers
		// so we forcefully reset them as a safety measure
		mmio.write(REG_RDTR, 0);
		mmio.write(REG_RADV, 0);

		// Receiver descriptor write back
		mmio.write(REG_IMS, 1 << 7);

		// Disabling interrupts
		mmio.write(REG_IMC, u32::MAX);
		_ = mmio.read(REG_ICR);

        syslog::trace!("Register setup done");

        Self {
            mmio,
            dma_man,
            mac_addr: mac,
            tx_ring,
            rx_ring,
			tx_bufs,
			rx_bufs
        }
    }

    pub fn transmit_frame(&mut self, packet: &[u8]) -> Result<usize, Error> {
		if packet.len() > self.dma_man.info().buff_len() as usize {
			return Err(Error::new(ErrorCode::InvalidArgument, "packet exceeds e1000 buffer size"));
		}

		let index = self.mmio.read(REG_TDT) as usize;

		unsafe {
			let desc_ref = &mut *self.tx_ring[index];
			let buff_address = self.tx_bufs[index];

			// Check [Descriptor Done]
			if desc_ref.get_tx_rsv_sta() & 1 == 0 {
				return Err(Error::new(ErrorCode::TryAgain, "tx descriptor still owned by device"));
			}

			let buffer = slice::from_raw_parts_mut(buff_address as *mut u8, packet.len());
			buffer.copy_from_slice(packet);

			desc_ref.set_tx_length(packet.len() as u16);
			desc_ref.set_tx_cso(0);
			desc_ref.set_tx_cmd(0b1001);
			desc_ref.set_tx_rsv_sta(0);
			desc_ref.set_tx_css(0);
			desc_ref.set_tx_special(0);

			fence(Ordering::SeqCst);

			self.mmio
				.write(REG_TDT, (index as u32 + 1) % self.dma_man.info().desc_count() as u32);
			let _ = self.mmio.read(REG_STAT); // Flush

			Ok(packet.len())
		}
    }

    pub fn receive_frame(&mut self) -> Option<Vec<u8>> {
        let index = (self.mmio.read(REG_RDT) + 1) % self.dma_man.info().desc_count() as u32;
		unsafe {
			let desc_ref = &mut *self.rx_ring[index as usize];
			let buff_address = self.rx_bufs[index as usize];

			let frame = {
				let status = desc_ref.get_rx_status();

				if status & 1 == 0 {
					// Check [Descriptor Done]
					return None;
				}

				let frame = if status & 0b10 == 0 {
					// Check [End Of Packet]
					None
				} else {
					let rx_length = desc_ref.get_rx_length();

					let length = core::cmp::min(rx_length, self.dma_man.info().buff_len()) as usize;

					Some(slice::from_raw_parts(buff_address as *const u8, length).to_vec())
				};

				desc_ref.set_rx_length(0);
				desc_ref.set_rx_chksum(0);
				desc_ref.set_rx_status(0);
				desc_ref.set_rx_errors(0);
				desc_ref.set_rx_special(0);

				frame
			};

			fence(Ordering::SeqCst);
			self.mmio.write(REG_RDT, index);
			let _ = self.mmio.read(REG_STAT); // Flush

			frame
		}
    }

    pub fn mac_address(&self) -> EthernetAddress {
        return EthernetAddress([
            self.mac_addr[0] as u8,
            self.mac_addr[1] as u8,
            self.mac_addr[2] as u8,
            self.mac_addr[3] as u8,
            self.mac_addr[4] as u8,
            self.mac_addr[5] as u8,
        ]);
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

pub struct E1000TxToken<'a> {
    device: &'a mut E1000Device,
}

impl TxToken for E1000TxToken<'_> {
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

impl Device for E1000Device {
    type RxToken<'a>
        = E1000RxToken
    where
        Self: 'a;

    type TxToken<'a>
        = E1000TxToken<'a>
    where
        Self: 'a;

    fn receive(
        &mut self,
        _timestamp: smoltcp::time::Instant,
    ) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let packet = self.receive_frame()?;
        syslog::info!("E1000 recebeu um frame de {} bytes!", packet.len());
        Some((E1000RxToken { buffer: packet }, E1000TxToken { device: self }))
    }

    fn transmit(&mut self, _timestamp: smoltcp::time::Instant) -> Option<Self::TxToken<'_>> {
        Some(E1000TxToken { device: self })
    }

    fn capabilities(&self) -> smoltcp::phy::DeviceCapabilities {
        let mut capabilities = DeviceCapabilities::default();
        capabilities.medium = Medium::Ethernet;
        capabilities.max_transmission_unit = 1514;
        capabilities.max_burst_size = Some(1);
        capabilities.checksum = Default::default();
        capabilities
    }
}

#[allow(dead_code)]
fn get_instant() -> Instant {
    // Pega o tempo do kernel do Nanvix
    let now = Time::now().expect("Falha ao obter o tempo do sistema");

    // Converte segundos e nanosegundos para milissegundos totais
    let millis = (now.seconds() * 1000) + (now.nanoseconds() as u64 / 1_000_000);

    // Cria o Instant do smoltcp a partir dos milissegundos.
    // Nota: Dependendo da versão do smoltcp, from_millis pede um i64 ou u64.
    // Fazemos o cast para i64 (o padrão na maioria das versões recentes).
    Instant::from_millis(millis as i64)
}

#[allow(dead_code)]
const IP: &str = "10.0.2.15";
#[allow(dead_code)]
const GATEWAY: &str = "10.0.2.2"; // QEMU user networking gateway
#[allow(dead_code)]
const PORT: u16 = 5555;

pub fn print_hex_dump(buf: &[u8], len: usize) {
    for i in 0..((len - (len % 4)) / 4) {
		let value =
			((buf[3 + i * 4] as u32) << 24) |
			((buf[2 + i * 4] as u32) << 16) |
			((buf[1 + i * 4] as u32) << 8) |
			(buf[0 + i * 4] as u32);
		syslog::info!("0x{:08x}", value);
	}
}

#[no_mangle]
pub fn main() {
	let mut e1000 = E1000Device::init();
	
	let ping_frame: Box<[u8]> = Box::new([
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x52, 0x54, 0x00, 0x12, 0x34, 0x56, 0x08, 0x06, 0x00,
        0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x01, 0x52, 0x54, 0x00, 0x12, 0x34, 0x56, 0x0a, 0x00,
        0x02, 0x0f, //10.0.2.15
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x02, 0x02,
	]);

	let _ = ::sys::kcall::pm::sleep(::core::time::Duration::from_millis(500));

	e1000.transmit_frame(&ping_frame).unwrap();
	e1000.transmit_frame(&ping_frame).unwrap();
	e1000.transmit_frame(&ping_frame).unwrap();
	e1000.transmit_frame(&ping_frame).unwrap();

	let mut c = 12;
	loop {
		if let Some(data) = e1000.receive_frame() {
			syslog::info!("rx check: received package!");
			print_hex_dump(&data, data.len());
		} else {
			syslog::info!("rx check: no packages!");
		}

		c -= 1;
		if c <= 0 {
			break;
		}

		let _ = ::sys::kcall::pm::sleep(::core::time::Duration::from_millis(100));
	}
}

/*
#[no_mangle]
#[allow(unused_mut, unused_variables)]
pub fn main() {
    let mut e1000 = E1000Device::init();

    let mut config = Config::new(e1000.mac_address().into());
    config.random_seed = 0x1234;

    let mut iface = Interface::new(config, &mut e1000, get_instant());

    iface.update_ip_addrs(|ip_addrs| {
        ip_addrs
            .push(IpCidr::new(IpAddress::from_str(IP).unwrap(), 24))
            .unwrap();
    });

    iface
        .routes_mut()
        .add_default_ipv4_route(Ipv4Address::from_str(GATEWAY).unwrap())
        .unwrap();

    let tcp_rx_buffer = tcp::SocketBuffer::new(vec![0; 1024]);
    let tcp_tx_buffer = tcp::SocketBuffer::new(vec![0; 1024]);
    let tcp_socket = tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer);

    let mut sockets = SocketSet::new(vec![]);
    let tcp_handle = sockets.add(tcp_socket);

    let mut tcp_active = false;

    syslog::trace!("Entering polling loop...");

    loop {
        let timestamp = get_instant();
        iface.poll(timestamp, &mut e1000, &mut sockets);

        let socket = sockets.get_mut::<tcp::Socket>(tcp_handle);
        if !socket.is_open() {
            syslog::info!("listening on port {}...", PORT);
            socket.listen(PORT).unwrap();
        }

        if socket.is_active() && !tcp_active {
            syslog::info!("tcp:{} connected", PORT);
        } else if !socket.is_active() && tcp_active {
            syslog::info!("tcp:{} disconnected", PORT);
        }
        tcp_active = socket.is_active();
        if socket.may_recv() {
            let data = socket
                .recv(|buffer| {
                    let recvd_len = buffer.len();
                    if !buffer.is_empty() {
                        syslog::info!("tcp:{} recv {} bytes: {:?}", PORT, recvd_len, buffer);
                        let mut lines = buffer
                            .split(|&b| b == b'\n')
                            .map(ToOwned::to_owned)
                            .collect::<Vec<_>>();
                        for line in lines.iter_mut() {
                            line.reverse();
                        }
                        let data = lines.join(&b'\n');
                        (recvd_len, data)
                    } else {
                        (0, vec![])
                    }
                })
                .unwrap();
            if socket.can_send() && !data.is_empty() {
                syslog::info!("tcp:{} send data: {:?}", PORT, data);
                socket.send_slice(&data[..]).unwrap();
            }
        } else if socket.may_send() {
            syslog::info!("tcp:{} close", PORT);
            socket.close();
            break;
        }
    }
}
*/
