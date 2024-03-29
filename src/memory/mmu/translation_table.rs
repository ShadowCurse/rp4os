#[path = "../../arch/aarch64/mmu/translation_table.rs"]
mod arch_translation_table;

use crate::memory::{
    mmu::{AttributeFields, MemoryRegion, PageAddress},
    Address, Physical, Virtual,
};

/// Translation table operations.
pub trait TranslationTable {
    /// Anything that needs to run before any of the other provided functions can be used.
    ///
    /// # Safety
    ///
    /// - Implementor must ensure that this function can run only once or is harmless if invoked
    ///   multiple times.
    fn init(&mut self);

    /// The translation table's base address to be used for programming the MMU.
    fn phys_base_address(&self) -> Address<Physical>;

    /// Map the given virtual memory region to the given physical memory region.
    ///
    /// # Safety
    ///
    /// - Using wrong attributes can cause multiple issues of different nature in the system.
    /// - It is not required that the architectural implementation prevents aliasing. That is,
    ///   mapping to the same physical memory using multiple virtual addresses, which would
    ///   break Rust's ownership assumptions. This should be protected against in the kernel's
    ///   generic MMU code.
    unsafe fn map_at(
        &mut self,
        virt_region: &MemoryRegion<Virtual>,
        phys_region: &MemoryRegion<Physical>,
        attr: &AttributeFields,
    ) -> Result<(), &'static str>;

    /// Try to get the attributes of a page.
    ///
    /// Will only succeed if there exists a valid mapping for the input page.
    fn try_page_attributes(
        &self,
        virt_page_addr: PageAddress<Virtual>,
    ) -> Result<AttributeFields, &'static str>;
}
