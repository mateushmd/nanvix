use core::slice::from_raw_parts_mut;
use volatile::Volatile;

/// E1000 registers which were mapped
pub const E1000_REGS: u32 = 0x40000000;
/// Qemu virt PCIe config space

pub fn pci_init(ecam : usize) {
    
    let mut dev = 0x00;
    let mut found = false;

    // Look at each PCI device at bus 0
    while dev <= 0x20 && !found {
        let off: u32 = dev << 11;
        let base = off as usize + ecam;
        let pci_base = (base + off as usize) as *mut Volatile<u32>;
        let device_id = unsafe {
            (*pci_base).read()
        };

        // E1000 ID = 100e8086
        if device_id == 0x100e8086 {
            let pci_config = unsafe { 
                from_raw_parts_mut(base as *mut Volatile<u32>, 0xff >> 2)
            };

            // Enable I/O access, memory access, mastering
            pci_config[1].write(0x7);
            pci_config[4 + 0].write(E1000_REGS);
            found = true;
        }
        dev += 1;
    }
}
