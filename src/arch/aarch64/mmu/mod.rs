//! Only 64 KiB granule is supported.

use crate::{
    bsp,
    memory::{mmu::MS512MiB, Address, Physical},
};
use aarch64_cpu::{asm::barrier, registers::*};
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

use super::{AddressSpace, MMUEnableError, MemoryManagementUnit};

impl<const AS_SIZE: usize> AddressSpace<AS_SIZE> {
    /// Checks for architectural restrictions.
    pub const fn arch_address_space_size_sanity_checks() {
        // Size must be at least one full 512 MiB table.
        assert!((AS_SIZE % MS512MiB::SIZE) == 0);

        // Check for 48 bit virtual address size as maximum, which is supported by any ARMv8
        // version.
        assert!(AS_SIZE <= (1 << 48));
    }
}

/// Memory Management Unit type.
pub struct Aarch64Mmu;

impl Aarch64Mmu {
    /// Setup function for the MAIR_EL1 register.
    fn set_up_mair(&self) {
        // Define the memory types being mapped.
        MAIR_EL1.write(
            // Attribute 1 - Cacheable normal DRAM.
            MAIR_EL1::Attr1_Normal_Outer::WriteBack_NonTransient_ReadWriteAlloc +
            MAIR_EL1::Attr1_Normal_Inner::WriteBack_NonTransient_ReadWriteAlloc +

            // Attribute 0 - Device.
            MAIR_EL1::Attr0_Device::nonGathering_nonReordering_EarlyWriteAck,
        );
    }

    /// Configure various settings of stage 1 of the EL1 translation regime.
    fn configure_translation_control(&self) {
        let t0sz = (64 - bsp::memory::mmu::KernelVirtAddrSpace::SIZE_SHIFT) as u64;

        TCR_EL1.write(
            TCR_EL1::TBI0::Used
                + TCR_EL1::IPS::Bits_40
                + TCR_EL1::TG0::KiB_64
                + TCR_EL1::SH0::Inner
                + TCR_EL1::ORGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
                + TCR_EL1::IRGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
                + TCR_EL1::EPD0::EnableTTBR0Walks
                + TCR_EL1::A1::TTBR0
                + TCR_EL1::T0SZ.val(t0sz)
                + TCR_EL1::EPD1::DisableTTBR1Walks,
        );
    }
}

impl MemoryManagementUnit for Aarch64Mmu {
    unsafe fn enable_mmu_and_caching(
        &self,
        phys_tables_base_addr: Address<Physical>,
    ) -> Result<(), MMUEnableError> {
        if self.is_enabled() {
            return Err(MMUEnableError::AlreadyEnabled);
        }

        // Fail early if translation granule is not supported.
        if !ID_AA64MMFR0_EL1.matches_all(ID_AA64MMFR0_EL1::TGran64::Supported) {
            return Err(MMUEnableError::Other(
                "Translation granule not supported in HW",
            ));
        }

        // Prepare the memory attribute indirection register.
        self.set_up_mair();

        // Set the "Translation Table Base Register".
        TTBR0_EL1.set_baddr(phys_tables_base_addr.as_usize() as u64);

        self.configure_translation_control();

        // Switch the MMU on.
        //
        // First, force all previous changes to be seen before the MMU is enabled.
        barrier::isb(barrier::SY);

        // Enable the MMU and turn on data and instruction caching.
        SCTLR_EL1.modify(SCTLR_EL1::M::Enable + SCTLR_EL1::C::Cacheable + SCTLR_EL1::I::Cacheable);

        // Force MMU init to complete before next instruction.
        barrier::isb(barrier::SY);

        Ok(())
    }

    #[inline(always)]
    fn is_enabled(&self) -> bool {
        SCTLR_EL1.matches_all(SCTLR_EL1::M::Enable)
    }
}
