//! Memory Management Unit.
//!
//! The `BSP` provides such a description through the `bsp::memory::mmu::virt_mem_layout()`
//! function.
//!
//! The `MMU` driver of the `arch` code uses `bsp::memory::mmu::virt_mem_layout()` to compile and
//! install respective translation tables.

#[path = "../../arch/aarch64/mmu/mod.rs"]
mod arch_mmu;

pub mod mapping_record;
pub mod page_alloc;
pub mod translation_table;

pub use arch_mmu::*;

use core::{
    fmt::{Debug, Display},
    iter::Step,
    num::NonZeroUsize,
    ops::Range,
};

use crate::{
    bsp::memory::mmu::{virt_mmio_remap_region, MSKernel, KERNEL_TRANSLATION_TABLES},
    is_aligned,
    memory::{
        mmu::{
            mapping_record::{kernel_add_mapping_record, kernel_try_add_device_record_mmio_user},
            translation_table::TranslationTable,
        },
        Address, AddressType, Physical, Virtual,
    },
    synchronization::{Mutex, ReadWriteExclusive},
    warn,
};

pub static MMU: Aarch64Mmu = arch_mmu::Aarch64Mmu;

pub type MS64KiB = MemorySize<{ 64 * 1024 }>;
pub type MS512MiB = MemorySize<{ 512 * 1024 * 1024 }>;

/// MMU functions.
pub trait MemoryManagementUnit {
    /// Turns on the MMU for the first time and enables data and instruction caching.
    ///
    /// # Safety
    ///
    /// - Changes the HW's global state.
    unsafe fn enable_mmu_and_caching(
        &self,
        phys_tables_base_addr: Address<Physical>,
    ) -> Result<(), MMUEnableError>;

    /// Returns true if the MMU is enabled, false otherwise.
    fn is_enabled(&self) -> bool;
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
pub struct MemorySize<const SIZE: usize>;

impl<const SIZE: usize> MemorySize<SIZE> {
    /// The granule's size.
    pub const SIZE: usize = Self::size_checked();

    /// The granule's mask.
    pub const MASK: usize = Self::SIZE - 1;

    /// The granule's shift, aka log2(size).
    pub const SHIFT: usize = Self::SIZE.trailing_zeros() as usize;

    const fn size_checked() -> usize {
        assert!(SIZE.is_power_of_two());
        SIZE
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

/// Intended to be implemented for [`AddressSpace`].
/// Translation table associated with specific address
/// space size.
pub trait AssociatedTranslationTable {
    /// A translation table whose address range is:
    ///
    /// [AS_SIZE - 1, 0]
    type Table;
}

/// Query the BSP for the reserved virtual addresses for MMIO remapping and initialize the kernel's
/// MMIO VA allocator with it.
pub fn kernel_init_mmio_va_allocator() {
    let region = crate::bsp::memory::mmu::virt_mmio_remap_region();

    page_alloc::KERNEL_MMIO_VA_ALLOCATOR.lock(|allocator| allocator.init(region));
}

/// Map a region in the kernel's translation tables.
///
/// No input checks done, input is passed through to the architectural implementation.
///
/// # Safety
///
/// - See `map_at()`.
/// - Does not prevent aliasing.
unsafe fn kernel_map_at_unchecked(
    name: &'static str,
    virt_region: &MemoryRegion<Virtual>,
    phys_region: &MemoryRegion<Physical>,
    attr: &AttributeFields,
) -> Result<(), &'static str> {
    KERNEL_TRANSLATION_TABLES.write(|tables| tables.map_at(virt_region, phys_region, attr))?;
    if let Err(x) = kernel_add_mapping_record(name, virt_region, phys_region, attr) {
        warn!("{}", x);
    }
    Ok(())
}

/// Raw mapping of a virtual to physical region in the kernel translation tables.
///
/// Prevents mapping into the MMIO range of the tables.
///
/// # Safety
///
/// - See `kernel_map_at_unchecked()`.
/// - Does not prevent aliasing. Currently, the callers must be trusted.
pub unsafe fn kernel_map_at(
    name: &'static str,
    virt_region: &MemoryRegion<Virtual>,
    phys_region: &MemoryRegion<Physical>,
    attr: &AttributeFields,
) -> Result<(), &'static str> {
    if virt_mmio_remap_region().overlaps(virt_region) {
        return Err("Attempt to manually map into MMIO region");
    }

    kernel_map_at_unchecked(name, virt_region, phys_region, attr)?;

    Ok(())
}

/// MMIO remapping in the kernel translation tables.
///
/// Typically used by device drivers.
///
/// # Safety
///
/// - Same as `kernel_map_at_unchecked()`, minus the aliasing part.
pub unsafe fn kernel_map_mmio(
    name: &'static str,
    mmio_descriptor: &MMIODescriptor,
) -> Result<Address<Virtual>, &'static str> {
    let phys_region = MemoryRegion::from(*mmio_descriptor);
    let offset_into_start_page = mmio_descriptor.start_addr.offset_into_page();

    // Check if an identical region has been mapped for another driver. If so, reuse it.
    let virt_addr =
        if let Some(addr) = kernel_try_add_device_record_mmio_user(name, mmio_descriptor) {
            addr
        // Otherwise, allocate a new region and map it.
        } else {
            let num_pages = match NonZeroUsize::new(phys_region.num_pages()) {
                None => return Err("Requested 0 pages"),
                Some(x) => x,
            };

            let virt_region = page_alloc::KERNEL_MMIO_VA_ALLOCATOR
                .lock(|allocator| allocator.alloc(num_pages))?;

            kernel_map_at_unchecked(
                name,
                &virt_region,
                &phys_region,
                &AttributeFields {
                    mem_attributes: MemAttributes::Device,
                    acc_perms: AccessPermissions::ReadWrite,
                    execute_never: true,
                },
            )?;

            virt_region.start_page.address()
        };

    Ok(virt_addr + offset_into_start_page)
}

/// Map the kernel's binary. Returns the translation table's base address.
///
/// # Safety
///
/// - See [`bsp::memory::mmu::kernel_map_binary()`].
pub unsafe fn kernel_map_binary() -> Result<Address<Physical>, &'static str> {
    let phys_kernel_tables_base_addr = KERNEL_TRANSLATION_TABLES.write(|tables| {
        tables.init();
        tables.phys_base_address()
    });

    crate::bsp::memory::mmu::kernel_map_binary()?;

    Ok(phys_kernel_tables_base_addr)
}

/// Enable the MMU and data + instruction caching.
///
/// # Safety
///
/// - Crucial function during kernel init. Changes the the complete memory view of the processor.
pub unsafe fn enable_mmu_and_caching(
    phys_tables_base_addr: Address<Physical>,
) -> Result<(), MMUEnableError> {
    MMU.enable_mmu_and_caching(phys_tables_base_addr)
}

/// Human-readable print of all recorded kernel mappings.
pub fn kernel_print_mappings() {
    mapping_record::kernel_print()
}

/// Architecture agnostic memory attributes.
#[allow(missing_docs)]
#[derive(Copy, Clone, Debug, Eq, PartialOrd, PartialEq)]
pub enum MemAttributes {
    CacheableDRAM,
    Device,
}

/// Architecture agnostic access permissions.
#[allow(missing_docs)]
#[derive(Copy, Clone, Debug, Eq, PartialOrd, PartialEq)]
pub enum AccessPermissions {
    ReadOnly,
    ReadWrite,
}

/// Collection of memory attributes.
#[allow(missing_docs)]
#[derive(Copy, Clone, Debug, Eq, PartialOrd, PartialEq)]
pub struct AttributeFields {
    pub mem_attributes: MemAttributes,
    pub acc_perms: AccessPermissions,
    pub execute_never: bool,
}

/// A wrapper type around [Address] that ensures page alignment.
#[derive(Copy, Clone, Debug, Eq, PartialOrd, PartialEq)]
pub struct PageAddress<T: AddressType> {
    inner: Address<T>,
}

impl<T: AddressType> PageAddress<T> {
    /// Internal address.
    pub fn address(self) -> Address<T> {
        self.inner
    }

    /// Calculates the offset from the page address.
    ///
    /// `count` is in units of [PageAddress]. For example, a count of 2 means `result = self + 2 *
    /// page_size`.
    pub fn checked_offset(self, count: isize) -> Option<Self> {
        if count == 0 {
            return Some(self);
        }

        let delta = count.unsigned_abs().checked_mul(MSKernel::SIZE)?;
        let result = if count.is_positive() {
            self.inner.as_usize().checked_add(delta)?
        } else {
            self.inner.as_usize().checked_sub(delta)?
        };

        Some(Self {
            inner: Address::new(result),
        })
    }
}

impl<T: AddressType> From<usize> for PageAddress<T> {
    fn from(addr: usize) -> Self {
        assert!(
            is_aligned(addr, MSKernel::SIZE),
            "Input usize not page aligned"
        );

        Self {
            inner: Address::new(addr),
        }
    }
}

impl<T: AddressType> From<Address<T>> for PageAddress<T> {
    fn from(addr: Address<T>) -> Self {
        assert!(addr.is_page_aligned(), "Input Address not page aligned");

        Self { inner: addr }
    }
}

impl<T: AddressType> Step for PageAddress<T> {
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        if start > end {
            return None;
        }

        // Since start <= end, do unchecked arithmetic.
        Some((end.inner.as_usize() - start.inner.as_usize()) >> MSKernel::SHIFT)
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        start.checked_offset(count as isize)
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        start.checked_offset(-(count as isize))
    }
}

/// A type that describes a region of memory in quantities of pages.
#[derive(Copy, Clone, Debug, Eq, PartialOrd, PartialEq)]
pub struct MemoryRegion<T: AddressType> {
    pub start_page: PageAddress<T>,
    pub end_page_exclusive: PageAddress<T>,
}

impl<T: AddressType> MemoryRegion<T> {
    /// Create an instance.
    pub fn new(start_addr: PageAddress<T>, end_addr_exclusive: PageAddress<T>) -> Self {
        assert!(start_addr <= end_addr_exclusive);

        Self {
            start_page: start_addr,
            end_page_exclusive: end_addr_exclusive,
        }
    }

    fn as_range(&self) -> Range<PageAddress<T>> {
        self.into_iter()
    }

    /// Returns the exclusive end page address.
    pub fn end_inclusive_page_addr(&self) -> PageAddress<T> {
        self.end_page_exclusive.checked_offset(-1).unwrap()
    }

    /// Checks if self contains an address.
    pub fn contains(&self, addr: Address<T>) -> bool {
        let page_addr = PageAddress::from(addr.align_down_page());
        self.as_range().contains(&page_addr)
    }

    /// Checks if there is an overlap with another memory region.
    pub fn overlaps(&self, other_region: &Self) -> bool {
        let self_range = self.as_range();

        self_range.contains(&other_region.start_page)
            || self_range.contains(&other_region.end_inclusive_page_addr())
    }

    /// Returns the number of pages contained in this region.
    pub fn num_pages(&self) -> usize {
        PageAddress::steps_between(&self.start_page, &self.end_page_exclusive).unwrap()
    }

    /// Returns the size in bytes of this region.
    pub fn size(&self) -> usize {
        // Invariant: start <= end_exclusive, so do unchecked arithmetic.
        let end_exclusive = self.end_page_exclusive.address().as_usize();
        let start = self.start_page.address().as_usize();

        end_exclusive - start
    }

    /// Splits the MemoryRegion like:
    ///
    /// --------------------------------------------------------------------------------
    /// |   |   |   |   |   |   |   |   |   |   |   |   |   |   |   |   |   |   |
    /// --------------------------------------------------------------------------------
    ///   ^                               ^                                       ^
    ///   |                               |                                       |
    ///   left_start     left_end_exclusive                                       |
    ///                                                                           |
    ///                                   ^                                       |
    ///                                   |                                       |
    ///                                   right_start           right_end_exclusive
    ///
    /// Left region is returned to the caller. Right region is the new region for this struct.
    pub fn take_first_n_pages(&mut self, num_pages: NonZeroUsize) -> Result<Self, &'static str> {
        let count: usize = num_pages.into();

        let left_end_exclusive = match self.start_page.checked_offset(count as isize) {
            None => return Err("Overflow while calculating left_end_exclusive"),
            Some(x) => x,
        };

        if left_end_exclusive > self.end_page_exclusive {
            return Err("Not enough free pages");
        }

        let allocation = Self {
            start_page: self.start_page,
            end_page_exclusive: left_end_exclusive,
        };
        self.start_page = left_end_exclusive;

        Ok(allocation)
    }
}

impl<ATYPE: AddressType> IntoIterator for MemoryRegion<ATYPE> {
    type Item = PageAddress<ATYPE>;
    type IntoIter = Range<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        Range {
            start: self.start_page,
            end: self.end_page_exclusive,
        }
    }
}

impl From<MMIODescriptor> for MemoryRegion<Physical> {
    fn from(desc: MMIODescriptor) -> Self {
        let start = PageAddress::from(desc.start_addr.align_down_page());
        let end_exclusive = PageAddress::from(desc.end_addr_exclusive.align_up_page());

        Self {
            start_page: start,
            end_page_exclusive: end_exclusive,
        }
    }
}

/// An MMIO descriptor for use in device drivers.
#[derive(Copy, Clone)]
pub struct MMIODescriptor {
    pub start_addr: Address<Physical>,
    pub end_addr_exclusive: Address<Physical>,
}

impl MMIODescriptor {
    /// Create an instance.
    pub const fn new(start_addr: Address<Physical>, size: usize) -> Self {
        assert!(size > 0);
        let end_addr_exclusive = Address::new(start_addr.as_usize() + size);

        Self {
            start_addr,
            end_addr_exclusive,
        }
    }
}
