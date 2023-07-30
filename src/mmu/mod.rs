//! Memory Management Unit.
//!
//! The `BSP` provides such a description through the `bsp::memory::mmu::virt_mem_layout()`
//! function.
//!
//! The `MMU` driver of the `arch` code uses `bsp::memory::mmu::virt_mem_layout()` to compile and
//! install respective translation tables.

#[path = "../arch/aarch64/mmu/mod.rs"]
mod arch_mmu;
mod translation_table;

pub use arch_mmu::*;
use core::{
    fmt::{Debug, Display},
    ops::RangeInclusive,
};

use crate::info;

/// Memory Management interfaces.
pub mod interface {
    use super::*;

    /// MMU functions.
    pub trait MMU {
        /// Called by the kernel during early init. Supposed to take the translation tables from the
        /// `BSP`-supplied `virt_mem_layout()` and install/activate them for the respective MMU.
        ///
        /// # Safety
        ///
        /// - Changes the HW's global state.
        unsafe fn enable_mmu_and_caching(&self) -> Result<(), MMUEnableError>;

        /// Returns true if the MMU is enabled, false otherwise.
        fn is_enabled(&self) -> bool;
    }
}

/// MMU enable errors variants.
#[derive(Debug)]
pub enum MMUEnableError {
    AlreadyEnabled,
    Other(&'static str),
}

impl Display for MMUEnableError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MMUEnableError::AlreadyEnabled => write!(f, "MMU is already enabled"),
            MMUEnableError::Other(x) => write!(f, "{}", x),
        }
    }
}

/// Describes the characteristics of a translation granule.
pub struct TranslationGranule<const GRANULE_SIZE: usize>;

impl<const GRANULE_SIZE: usize> TranslationGranule<GRANULE_SIZE> {
    /// The granule's size.
    pub const SIZE: usize = Self::size_checked();

    /// The granule's shift, aka log2(size).
    pub const SHIFT: usize = Self::SIZE.trailing_zeros() as usize;

    const fn size_checked() -> usize {
        assert!(GRANULE_SIZE.is_power_of_two());
        GRANULE_SIZE
    }
}

/// Describes properties of an address space.
pub struct AddressSpace<const AS_SIZE: usize>;

impl<const AS_SIZE: usize> AddressSpace<AS_SIZE> {
    /// The address space size.
    pub const SIZE: usize = Self::size_checked();

    /// The address space shift, aka log2(size).
    pub const SIZE_SHIFT: usize = Self::SIZE.trailing_zeros() as usize;

    const fn size_checked() -> usize {
        assert!(AS_SIZE.is_power_of_two());

        // Check for architectural restrictions as well.
        Self::arch_address_space_size_sanity_checks();

        AS_SIZE
    }
}

/// Architecture agnostic translation types.
#[derive(Copy, Clone)]
pub enum Translation {
    Identity,
    Offset(usize),
}

/// Architecture agnostic memory attributes.
#[derive(Copy, Clone)]
pub enum MemAttributes {
    CacheableDRAM,
    Device,
}

/// Architecture agnostic access permissions.
#[derive(Copy, Clone)]
pub enum AccessPermissions {
    ReadOnly,
    ReadWrite,
}

/// Collection of memory attributes.
#[derive(Copy, Clone)]
pub struct AttributeFields {
    pub mem_attributes: MemAttributes,
    pub acc_perms: AccessPermissions,
    pub execute_never: bool,
}

impl Default for AttributeFields {
    fn default() -> AttributeFields {
        AttributeFields {
            mem_attributes: MemAttributes::CacheableDRAM,
            acc_perms: AccessPermissions::ReadWrite,
            execute_never: true,
        }
    }
}

/// Architecture agnostic descriptor for a memory range.
pub struct TranslationDescriptor {
    pub name: &'static str,
    pub virtual_range: fn() -> RangeInclusive<usize>,
    pub physical_range_translation: Translation,
    pub attribute_fields: AttributeFields,
}

/// Human-readable output of a TranslationDescriptor.
impl Display for TranslationDescriptor {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        // Call the function to which self.range points, and dereference the result, which causes
        // Rust to copy the value.
        let start = *(self.virtual_range)().start();
        let end = *(self.virtual_range)().end();
        let size = end - start + 1;

        let (size, unit) = crate::size_human_readable_ceil(size);

        let attr = match self.attribute_fields.mem_attributes {
            MemAttributes::CacheableDRAM => "C",
            MemAttributes::Device => "Dev",
        };

        let acc_p = match self.attribute_fields.acc_perms {
            AccessPermissions::ReadOnly => "RO",
            AccessPermissions::ReadWrite => "RW",
        };

        let xn = if self.attribute_fields.execute_never {
            "PXN"
        } else {
            "PX"
        };

        write!(
            f,
            "{:#010x} - {:#010x} | {: >3} {} | {: <3} {} {: <3} | {}",
            start, end, size, unit, attr, acc_p, xn, self.name
        )
    }
}

/// Type for expressing the kernel's virtual memory layout.
pub struct KernelVirtualLayout<const NUM_SPECIAL_RANGES: usize> {
    /// The last (inclusive) address of the address space.
    max_virt_addr_inclusive: usize,
    /// Array of descriptors for non-standard (normal cacheable DRAM) memory regions.
    descriptors: [TranslationDescriptor; NUM_SPECIAL_RANGES],
}

impl<const NUM_SPECIAL_RANGES: usize> KernelVirtualLayout<{ NUM_SPECIAL_RANGES }> {
    /// Create a new instance.
    pub const fn new(max: usize, layout: [TranslationDescriptor; NUM_SPECIAL_RANGES]) -> Self {
        Self {
            max_virt_addr_inclusive: max,
            descriptors: layout,
        }
    }

    /// For a virtual address, find and return the physical output address and corresponding
    /// attributes.
    ///
    /// If the address is not found in `inner`, return an identity mapped default with normal
    /// cacheable DRAM attributes.
    pub fn virt_addr_properties(
        &self,
        virt_addr: usize,
    ) -> Result<(usize, AttributeFields), &'static str> {
        if virt_addr > self.max_virt_addr_inclusive {
            return Err("Address out of range");
        }

        for i in self.descriptors.iter() {
            let virtual_range = (i.virtual_range)();
            if virtual_range.contains(&virt_addr) {
                let output_addr = match i.physical_range_translation {
                    Translation::Identity => virt_addr,
                    Translation::Offset(a) => a + (virt_addr - virtual_range.start()),
                };

                return Ok((output_addr, i.attribute_fields));
            }
        }

        Ok((virt_addr, AttributeFields::default()))
    }

    /// Print the memory layout.
    pub fn print_layout(&self) {
        for i in self.descriptors.iter() {
            info!("{}", i);
        }
    }
}
