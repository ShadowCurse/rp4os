pub mod heap_alloc;
pub mod mmu;

use crate::{align_down, align_up, bsp::memory::mmu::MSKernel, is_aligned};
use core::{
    marker::PhantomData,
    ops::{Add, Sub},
};

use self::{heap_alloc::kernel_init_heap_allocator, mmu::kernel_init_mmio_va_allocator};

/// Finish initialization of the MMU subsystem.
pub fn post_enable_init() {
    kernel_init_mmio_va_allocator();
    kernel_init_heap_allocator();
}

/// Metadata trait for marking the type of an address.
pub trait AddressType: Copy + Clone + PartialOrd + PartialEq + Ord + Eq {}

/// Zero-sized type to mark a physical address.
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq, Ord, Eq)]
pub struct Physical {}

impl AddressType for Physical {}

/// Zero-sized type to mark a virtual address.
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq, Ord, Eq)]
pub struct Virtual {}

impl AddressType for Virtual {}

/// Generic address type.
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq, Ord, Eq)]
pub struct Address<T: AddressType> {
    value: usize,
    _address_type: PhantomData<fn() -> T>,
}

impl<T: AddressType> Address<T> {
    /// Create an instance.
    pub const fn new(value: usize) -> Self {
        Self {
            value,
            _address_type: PhantomData,
        }
    }

    /// Convert to usize.
    pub const fn as_usize(self) -> usize {
        self.value
    }

    /// Align down to page size.
    #[must_use]
    pub const fn align_down_page(self) -> Self {
        let aligned = align_down(self.value, MSKernel::SIZE);

        Self::new(aligned)
    }

    /// Align up to page size.
    #[must_use]
    pub const fn align_up_page(self) -> Self {
        let aligned = align_up(self.value, MSKernel::SIZE);

        Self::new(aligned)
    }

    /// Checks if the address is page aligned.
    pub const fn is_page_aligned(&self) -> bool {
        is_aligned(self.value, MSKernel::SIZE)
    }

    /// Return the address' offset into the corresponding page.
    pub const fn offset_into_page(&self) -> usize {
        self.value & MSKernel::MASK
    }
}

impl<T: AddressType> Add<usize> for Address<T> {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: usize) -> Self::Output {
        match self.value.checked_add(rhs) {
            None => panic!("Overflow on Address::add"),
            Some(x) => Self::new(x),
        }
    }
}

impl<T: AddressType> Sub<Address<T>> for Address<T> {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Address<T>) -> Self::Output {
        match self.value.checked_sub(rhs.value) {
            None => panic!("Overflow on Address::sub"),
            Some(x) => Self::new(x),
        }
    }
}

impl core::fmt::Display for Address<Physical> {
    // Don't expect to see physical addresses greater than 40 bit.
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let q3: u8 = ((self.value >> 32) & 0xff) as u8;
        let q2: u16 = ((self.value >> 16) & 0xffff) as u16;
        let q1: u16 = (self.value & 0xffff) as u16;

        write!(f, "0x")?;
        write!(f, "{:02x}_", q3)?;
        write!(f, "{:04x}_", q2)?;
        write!(f, "{:04x}", q1)
    }
}

impl core::fmt::Display for Address<Virtual> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let q4: u16 = ((self.value >> 48) & 0xffff) as u16;
        let q3: u16 = ((self.value >> 32) & 0xffff) as u16;
        let q2: u16 = ((self.value >> 16) & 0xffff) as u16;
        let q1: u16 = (self.value & 0xffff) as u16;

        write!(f, "0x")?;
        write!(f, "{:04x}_", q4)?;
        write!(f, "{:04x}_", q3)?;
        write!(f, "{:04x}_", q2)?;
        write!(f, "{:04x}", q1)
    }
}
