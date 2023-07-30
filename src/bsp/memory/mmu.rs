//! BSP Memory Management Unit.

use crate::bsp::memory::{code_end_exclusive, code_start, map};
use crate::mmu::{
    AccessPermissions, AddressSpace, AttributeFields, KernelVirtualLayout, MemAttributes,
    Translation, TranslationDescriptor,
};
use core::ops::RangeInclusive;

const NUM_MEM_RANGES: usize = 3;

/// The kernel's address space defined by this BSP.
pub type KernelAddrSpace = AddressSpace<{ map::END_INCLUSIVE + 1 }>;

/// The virtual memory layout.
///
/// The layout must contain only special ranges, aka anything that is _not_ normal cacheable DRAM.
/// It is agnostic of the paging granularity that the architecture's MMU will use.
pub static LAYOUT: KernelVirtualLayout<NUM_MEM_RANGES> = KernelVirtualLayout::new(
    map::END_INCLUSIVE,
    [
        TranslationDescriptor {
            name: "Kernel code and RO data",
            virtual_range: code_range_inclusive,
            physical_range_translation: Translation::Identity,
            attribute_fields: AttributeFields {
                mem_attributes: MemAttributes::CacheableDRAM,
                acc_perms: AccessPermissions::ReadOnly,
                execute_never: false,
            },
        },
        TranslationDescriptor {
            name: "Remapped Device MMIO",
            virtual_range: remapped_mmio_range_inclusive,
            physical_range_translation: Translation::Offset(map::mmio::START + 0x20_0000),
            attribute_fields: AttributeFields {
                mem_attributes: MemAttributes::Device,
                acc_perms: AccessPermissions::ReadWrite,
                execute_never: true,
            },
        },
        TranslationDescriptor {
            name: "Device MMIO",
            virtual_range: mmio_range_inclusive,
            physical_range_translation: Translation::Identity,
            attribute_fields: AttributeFields {
                mem_attributes: MemAttributes::Device,
                acc_perms: AccessPermissions::ReadWrite,
                execute_never: true,
            },
        },
    ],
);

fn code_range_inclusive() -> RangeInclusive<usize> {
    // Notice the subtraction to turn the exclusive end into an inclusive end.
    #[allow(clippy::range_minus_one)]
    RangeInclusive::new(code_start(), code_end_exclusive() - 1)
}

fn remapped_mmio_range_inclusive() -> RangeInclusive<usize> {
    // The last 64 KiB slot in the first 512 MiB
    RangeInclusive::new(0x1FFF_0000, 0x1FFF_FFFF)
}

fn mmio_range_inclusive() -> RangeInclusive<usize> {
    RangeInclusive::new(map::mmio::START, map::mmio::END_INCLUSIVE)
}
