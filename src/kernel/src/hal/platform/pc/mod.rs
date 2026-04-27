// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

//==================================================================================================
// Imports
//==================================================================================================

use crate::hal::{
    arch::x86::{
        self,
        Arch,
    },
    io::{
        IoMemoryAllocator,
        IoPortAllocator,
    },
    mem::{
        AccessPermission,
        Address,
        MemoryRegion,
        MemoryRegionType,
        PageAligned,
        TruncatedMemoryRegion,
    },
    platform::{
        self,
        madt::MadtInfo,
        region_names::VIDEO_MMIO_REGION_NAME,
        region_tags::VIDEO_MMIO_TAG,
    },
};
use ::alloc::collections::linked_list::LinkedList;
use ::arch::{
    cpu::pic,
    mem,
};

use ::sys::{
    error::{
        Error,
        ErrorCode,
    },
    mm::VirtualAddress,
};

#[cfg(feature = "pit")]
use crate::hal::platform::pit::Pit;

//==================================================================================================
// Modules
//==================================================================================================

#[cfg(feature = "qemu-baremetal")]
mod baremetal;

#[cfg(any(feature = "qemu-isapc", feature = "qemu-pc"))]
mod qemu;

#[cfg(feature = "pci")]
pub mod pci;

#[cfg(feature = "bios")]
pub mod bios;

#[cfg(feature = "cmos")]
pub mod cmos;

#[cfg(feature = "mboot")]
pub mod mboot;

//==================================================================================================
// Exports
//==================================================================================================

#[cfg(any(feature = "qemu-isapc", feature = "qemu-pc"))]
pub use qemu::{
    putb,
    shutdown,
};

#[cfg(feature = "qemu-baremetal")]
pub use baremetal::{
    putb,
    shutdown,
};

///
/// # Description
///
/// Start address of application cores.
///
/// # Notes
///
/// This address was carefully chosen to avoid conflicts with the kernel.
///
pub const TRAMPOLINE_ADDRESS: VirtualAddress = VirtualAddress::new(0x00008000);

//==================================================================================================
// Structures
//==================================================================================================

pub struct Platform {
    #[cfg(feature = "cmos")]
    pub _cmos: cmos::Cmos,
    #[cfg(feature = "pit")]
    pub _pit: Pit,
    pub arch: Arch,
    #[cfg(feature = "pci")]
    pub _pci: pci::PciBus
}

//==================================================================================================
// Standalone Functions
//==================================================================================================

///
/// # Description
///
/// Disables all interrupts on the calling core.
///
/// # Safety
///
/// This function is unsafe because it modifies the CPU state.
///
/// It is safe to call this function only when the CPU is in a state where interrupts can be
/// disabled.
///
pub(super) unsafe fn disable_interrupts() {
    ::arch::cpu::cli();
}

///
/// # Description
///
/// Enables all interrupts on the calling core.
///
/// # Safety
///
/// This function is unsafe because it modifies the CPU state.
///
/// It is safe to call this function only when the CPU is in a state where interrupts can be
/// enabled.
///
pub(super) unsafe fn enable_interrupts() {
    ::arch::cpu::sti();
}

///
/// # Description
///
/// Waits for an interrupt to happen.
///
/// # Safety
///
/// This function is unsafe because it modifies the CPU state.
///
/// It is safe to call this function only when the CPU is able to receive interrupts.
///
pub(super) unsafe fn wait_for_interrupt() {
    ::arch::cpu::halt();
}

#[cfg(feature = "bios")]
fn register_bios_data_area(
    memory_regions: &mut LinkedList<MemoryRegion<VirtualAddress>>,
    mem_lower_size: usize,
) -> Result<(), Error> {
    // Check if the memory region for the Bios Data Area fits in the
    // lower memory regions available.
    // NOTE: This is possible because mem_lower_size start at address 0x0.
    if mem_lower_size < bios::BiosDataArea::BASE + mem::PAGE_SIZE {
        let reason: &str = "bios data memory region doesn't fit in lower memory available";
        error!("{:?}", reason);
        return Err(Error::new(ErrorCode::OutOfMemory, reason));
    }

    let bios_data_area: MemoryRegion<VirtualAddress> = MemoryRegion::new(
        "bios data area",
        VirtualAddress::from_raw_value(bios::BiosDataArea::BASE)
            .align_down(::arch::mem::PAGE_ALIGNMENT),
        mem::PAGE_SIZE,
        MemoryRegionType::Reserved,
        AccessPermission::RDWR,
    )?;
    memory_regions.push_back(bios_data_area);

    unsafe {
        // Set warm reset vector.
        // We intentionally shift the address by 4 bits to get correct segmented address.
        let vector: u16 = (TRAMPOLINE_ADDRESS.into_raw_value() & 0xFFFF) as u16 >> 4;
        bios::BiosDataArea::write_reset_vector(vector);
    }
    Ok(())
}

#[cfg(feature = "cmos")]
fn register_cmos(ioports: &mut IoPortAllocator) -> Result<cmos::Cmos, Error> {
    // Register ports for the CMOS.
    ioports.register_read_write(cmos::Cmos::DATA)?;
    ioports.register_read_write(cmos::Cmos::INDEX)?;

    // Enable warm reset. It allows the INIT signal to be asserted without actually causing the
    // processor to run through its entire BIOS initialization procedure (POST).
    let mut cmos: cmos::Cmos = cmos::Cmos::init(ioports)?;
    cmos.write_shutdown_status(cmos::ShutdownStatus::JmpDwordRequestWithoutIntInit);

    Ok(cmos)
}

#[cfg(feature = "pit")]
fn register_pit(ioports: &mut IoPortAllocator) -> Result<Pit, Error> {
    // Register ports for the PIT.
    ioports.register_read_write(::arch::cpu::pit::PIT_CTRL)?;
    ioports.register_read_write(::arch::cpu::pit::PIT_DATA)?;

    Pit::new(ioports, ::config::kernel::TIMER_FREQ)
}

#[cfg(feature = "pci")]
fn register_pci_devices(
    ioports: &mut IoPortAllocator,
    ioaddresses: &mut IoMemoryAllocator,
    mmio_regions: &mut LinkedList<TruncatedMemoryRegion<VirtualAddress>>,
) -> Result<pci::PciBus, Error> {
    let mut pci = pci::PciBus::new(ioports)?;

    let mut found_nic = false;

    let bus: u8 = 0;
    
    for slot in 0..32 {
        // Always looking at bus 0, its unlikely to find the NIC
        // in a different bus
        let vendor_device = pci.read_config(0, slot, 0, 0x00);

        // Found e1000
        if !found_nic && vendor_device == 0x100E8086 {
            found_nic = true;

            info!("PCI: found e1000 at Bus {bus}, Slot {slot}");

            // TODO: figure out the right way to define the base address
            let e1000_hardcoded_base: u32 = 0xFE800000;

            let e1000_size = {
                // Writing 0xFFFFFFFF to BAR0 probes the hardware for the required memory size. 
                // The NIC enforces a mask by hardwiring the bits that define the alignment 
                // to zero. By clearing the PCI flags (bits 0-3) and calculating the 
                // two's complement of this mask, we determine the address space size.
                pci.write_config(bus, slot, 0, pci::HeaderType0::BAR0, 0xFFFFFFFF);
                let dirty_mask = pci.read_config(bus, slot, 0, pci::HeaderType0::BAR0);
                let mask = dirty_mask & !0xF;
                debug!("Returned mask is {:#x}", mask);
                (!mask + 1) as usize
            };

            debug!("Size is {:#x}", e1000_size);

            pci.write_config(bus, slot, 0, pci::HeaderType0::BAR0, e1000_hardcoded_base);

            // Reads the Status and Command registers at offset 0x04,
            // sets bits 1 (Memory Space) and 2 (Bus Master) to 1
            // and write back
            let mut cmd = pci.read_config(bus, slot, 0, pci::HeaderType0::StatusCommand);
            cmd |= 0x00000006;
            pci.write_config(bus, slot, 0, pci::HeaderType0::StatusCommand, cmd);

            let region = TruncatedMemoryRegion::new(
                "e1000",
                PageAligned::from_raw_value(e1000_hardcoded_base as usize)?,
                e1000_size,
                MemoryRegionType::Mmio,
                AccessPermission::RDWR
            )?;

            ioaddresses.register(crate::hal::platform::region_tags::E1000_MMIO_TAG, region.clone())?;
            
            mmio_regions.push_back(region);

            #[cfg(feature = "debug")]
            pci.print_config(bus, slot);
        }
    }

    Ok(pci)
}

pub fn init(
    ioports: &mut IoPortAllocator,
    ioaddresses: &mut IoMemoryAllocator,
    memory_regions: &mut LinkedList<MemoryRegion<VirtualAddress>>,
    mmio_regions: &mut LinkedList<TruncatedMemoryRegion<VirtualAddress>>,
    madt: &Option<MadtInfo>,
    mem_lower: Option<usize>,
) -> Result<Platform, Error> {
    
    unsafe {
        ::arch::io::out32(0xCF8, 0x80000000);
        let raw_val = ::arch::io::in32(0xCFC);
        debug!("RAW PCI TEST: {:#x}", raw_val);
    }

    // Register I/O ports for 8259 PIC.
    ioports.register_read_write(pic::PIC_CTRL_MASTER as u16)?;
    ioports.register_read_write(pic::PIC_DATA_MASTER as u16)?;
    ioports.register_read_write(pic::PIC_CTRL_SLAVE as u16)?;
    ioports.register_read_write(pic::PIC_DATA_SLAVE as u16)?;

    // Register I/O ports from 0x3f8 to 0x3fc as read/write.
    for base in [0x3F8, 0x2F8, 0x3E8, 0x2E8, 0x3E0, 0x2E0, 0x3F0, 0x2F0].iter() {
        for p in [0, 1, 2, 3, 4, 7].iter() {
            ioports.register_read_write(base + p)?;
        }

        // Register read-only ports.
        for p in [5, 6].iter() {
            ioports.register_read_only(base + p)?;
        }
    }

    // Register BIOS data area.
    #[cfg(feature = "bios")]
    let mem_lower_size = match mem_lower {
        Some(mem_lower_size) => mem_lower_size,
        None => {
            let reason: &str = "availability of lower memory is not known";
            error!("{reason}");
            return Err(Error::new(ErrorCode::InvalidArgument, reason));
        },
    };

    register_bios_data_area(memory_regions, mem_lower_size)?;

    // Check if the memory region for the Trampoline fits in the
    // lower memory regions available.
    // NOTE: This is possible because mem_lower_size start at address 0x0.
    if mem_lower_size < platform::TRAMPOLINE_ADDRESS.into_raw_value() + mem::PAGE_SIZE {
        let reason: &str = "Trampoline memory region doesn't fit in lower memory available";
        error!("{reason}");
        return Err(Error::new(ErrorCode::OutOfMemory, reason));
    }
    // Trampoline.
    let trampoline: MemoryRegion<VirtualAddress> = MemoryRegion::new(
        "trampoline",
        VirtualAddress::from_raw_value(platform::TRAMPOLINE_ADDRESS.into_raw_value()),
        mem::PAGE_SIZE,
        MemoryRegionType::Reserved,
        AccessPermission::RDWR,
    )?;
    memory_regions.push_back(trampoline);

    // Register video display memory.
    // FIXME: https://github.com/nanvix/kernel/issues/435
    let video_display_memory: TruncatedMemoryRegion<VirtualAddress> = TruncatedMemoryRegion::new(
        VIDEO_MMIO_REGION_NAME,
        PageAligned::from_raw_value(0x000a0000)?,
        32 * mem::PAGE_SIZE,
        MemoryRegionType::Mmio,
        AccessPermission::RDWR,
    )?;
    ioaddresses.register(VIDEO_MMIO_TAG, video_display_memory.clone())?;
    mmio_regions.push_back(video_display_memory);

    // Bios memory.
    // FIXME: https://github.com/nanvix/kernel/issues/435
    let bios: MemoryRegion<VirtualAddress> = MemoryRegion::new(
        "bios memory",
        VirtualAddress::from_raw_value(0x000c0000),
        48 * mem::PAGE_SIZE,
        MemoryRegionType::Reserved,
        AccessPermission::RDONLY,
    )?;
    memory_regions.push_back(bios);

    Ok(Platform {
        arch: x86::init(ioports, ioaddresses, madt)?,
        #[cfg(feature = "pit")]
        _pit: register_pit(ioports)?,
        #[cfg(feature = "cmos")]
        _cmos: register_cmos(ioports)?,
        #[cfg(feature = "pci")]
        _pci: register_pci_devices(ioports, ioaddresses, mmio_regions)?
    })
}
