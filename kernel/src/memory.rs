use core::convert::TryFrom;
use core::fmt::Display;

#[derive(Debug, Copy, Clone)]
pub enum MemoryTypeName {
    Reserved = 0,
    LoaderCode = 1,
    LoaderData = 2,
    BootServicesCode = 3,
    BootServicesData = 4,
    RuntimeServicesCode = 5,
    RuntimeServicesData = 6,
    Conventional = 7,
    Unusable = 8,
    AcpiReclaim = 9,
    AcpiNonVolatile = 10,
    Mmio = 11,
    MmioPortSpace = 12,
    PalCode = 13,
    PersistentMemory = 14,
}

impl TryFrom<u32> for MemoryTypeName {
    type Error = u32;

    fn try_from(v: u32) -> Result<Self, Self::Error> {
        match v {
            v if v == MemoryTypeName::Reserved as u32 => Ok(MemoryTypeName::Reserved),
            v if v == MemoryTypeName::LoaderCode as u32 => Ok(MemoryTypeName::LoaderCode),
            v if v == MemoryTypeName::LoaderData as u32 => Ok(MemoryTypeName::LoaderData),
            v if v == MemoryTypeName::BootServicesCode as u32 => {
                Ok(MemoryTypeName::BootServicesCode)
            }
            v if v == MemoryTypeName::BootServicesData as u32 => {
                Ok(MemoryTypeName::BootServicesData)
            }
            v if v == MemoryTypeName::RuntimeServicesCode as u32 => {
                Ok(MemoryTypeName::RuntimeServicesCode)
            }
            v if v == MemoryTypeName::RuntimeServicesData as u32 => {
                Ok(MemoryTypeName::RuntimeServicesData)
            }
            v if v == MemoryTypeName::Conventional as u32 => Ok(MemoryTypeName::Conventional),
            v if v == MemoryTypeName::Unusable as u32 => Ok(MemoryTypeName::Unusable),
            v if v == MemoryTypeName::AcpiReclaim as u32 => Ok(MemoryTypeName::AcpiReclaim),
            v if v == MemoryTypeName::AcpiNonVolatile as u32 => Ok(MemoryTypeName::AcpiNonVolatile),
            v if v == MemoryTypeName::Mmio as u32 => Ok(MemoryTypeName::Mmio),
            v if v == MemoryTypeName::MmioPortSpace as u32 => Ok(MemoryTypeName::MmioPortSpace),
            v if v == MemoryTypeName::PalCode as u32 => Ok(MemoryTypeName::PalCode),
            v if v == MemoryTypeName::PersistentMemory as u32 => {
                Ok(MemoryTypeName::PersistentMemory)
            }
            v => Err(v),
        }
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct MemoryDescriptor {
    pub memory_type: u32,
    pub padding: u32,
    pub phys_start: u64,
    pub virt_start: u64,
    pub page_count: u64,
    pub att: u64,
}

impl Display for MemoryDescriptor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match MemoryTypeName::try_from(self.memory_type) {
            Ok(t) => write!(f, "memory_type: {:?}, ", t)?,
            Err(v) => write!(f, "memory_type: {}, ", v)?,
        };
        write!(
            f,
            "padding: 0x{:x} phys_start: 0x{:x}, virt_start: 0x{:x}, page_count: {}, att: {:x}",
            self.padding, self.phys_start, self.virt_start, self.page_count, self.att
        )
    }
}
