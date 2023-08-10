use super::MemoryRegion;
use crate::{
    memory::{AddressType, Virtual},
    synchronization::IRQSafeNullLock,
    warn,
};
use core::num::NonZeroUsize;

pub static KERNEL_MMIO_VA_ALLOCATOR: IRQSafeNullLock<PageAllocator<Virtual>> =
    IRQSafeNullLock::new(PageAllocator::new());

/// A page allocator that can be lazyily initialized.
pub struct PageAllocator<ATYPE: AddressType> {
    pool: Option<MemoryRegion<ATYPE>>,
}

impl<ATYPE: AddressType> PageAllocator<ATYPE> {
    /// Create an instance.
    pub const fn new() -> Self {
        Self { pool: None }
    }

    /// Initialize the allocator.
    pub fn init(&mut self, pool: MemoryRegion<ATYPE>) {
        if self.pool.is_some() {
            warn!("Already initialized");
            return;
        }

        self.pool = Some(pool);
    }

    /// Allocate a number of pages.
    pub fn alloc(
        &mut self,
        num_requested_pages: NonZeroUsize,
    ) -> Result<MemoryRegion<ATYPE>, &'static str> {
        if self.pool.is_none() {
            return Err("Allocator not initialized");
        }

        self.pool
            .as_mut()
            .unwrap()
            .take_first_n_pages(num_requested_pages)
    }
}
