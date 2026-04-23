use crate::hal::io::{ IoPortAllocator, ReadWriteIoPort };
use ::sys::error::Error;

// CONFIG_ADDRESS register address
const PCI_CONFIG_ADDRESS: u16 = 0xCF8;

// CONFIG_DATA register address
const PCI_CONFIG_DATA: u16 = 0xCFC;

// Every address has bit 31 (Enable bit) set to 1
const BASE_ADDRESS: u32 = 0x80000000;  
                                      
// Shift ammount required for each part of the address
const BUS_SHIFT: u32 = 16; // 23-16
const SLOT_SHIFT: u32 = 11; // 15-11
const FUNC_SHIFT: u32 = 8; // 10-8

// The register offset has to point to consecutive DWORDs (4 bytes), meaning 
// that bits 0 and 1 must always be 0 
const OFFSET_MASK: u32 = 0xFC; 

///
/// Provides read and write operations for PMIO communication with the PCI
///
pub struct PciBus { 
    /// 
    /// Specifies the configuration address that is required to be accessed
    ///
    config_address: ReadWriteIoPort,

    /// 
    /// Generate the configuration access and tranfer the configuration data to 
    /// or from the CONFIG_DATA register
    ///
    config_data: ReadWriteIoPort
}

#[allow(dead_code)]
impl PciBus {
    pub fn new(ioports: &mut IoPortAllocator) -> Result<Self, Error> {
        ioports.register_read_write(PCI_CONFIG_ADDRESS)?;
        ioports.register_read_write(PCI_CONFIG_ADDRESS + 1)?;
        ioports.register_read_write(PCI_CONFIG_ADDRESS + 2)?;
        ioports.register_read_write(PCI_CONFIG_ADDRESS + 3)?;
        
        ioports.register_read_write(PCI_CONFIG_DATA)?;
        ioports.register_read_write(PCI_CONFIG_DATA + 1)?;
        ioports.register_read_write(PCI_CONFIG_DATA + 2)?;
        ioports.register_read_write(PCI_CONFIG_DATA + 3)?;

        let config_address = ioports.allocate_read_write(PCI_CONFIG_ADDRESS)?;
        let config_data = ioports.allocate_read_write(PCI_CONFIG_DATA)?;

        let pci_bus = PciBus {
            config_address: config_address,
            config_data: config_data
        };

        Ok(pci_bus)
    }

    /// 
    /// # Description
    /// 
    /// Reads the CONFIG_DATA register value
    ///
    pub fn read_config(&mut self, bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
        let address = BASE_ADDRESS
            | (bus as u32) << BUS_SHIFT
            | (slot as u32) << SLOT_SHIFT
            | (func as u32) << FUNC_SHIFT
            | (offset as u32) & OFFSET_MASK;

        self.config_address.write32(address); 
        self.config_data.read32()
    }

    ///
    /// # Description
    ///
    /// Writes a value to the CONFIG_DATA register
    ///
    pub fn write_config(
        &mut self, bus: u8, slot: u8, func: u8, offset: u8, value: u32
    ) {
        let address = BASE_ADDRESS
            | (bus as u32) << BUS_SHIFT
            | (slot as u32) << SLOT_SHIFT
            | (func as u32) << FUNC_SHIFT
            | (offset as u32) & OFFSET_MASK;

        self.config_address.write32(address); 
        self.config_data.write32(value);
    }
}
