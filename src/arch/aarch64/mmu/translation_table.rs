//! Architectural translation table.
//!
//! Only 64 KiB granule is supported.

use tock_registers::{
    fields::FieldValue,
    interfaces::{Readable, Writeable},
    register_bitfields,
    registers::InMemoryRegister,
};

use crate::memory::{
    mmu::{
        AccessPermissions, AddressSpace, AssociatedTranslationTable, AttributeFields, MS512MiB,
        MS64KiB, MemAttributes, MemoryRegion, PageAddress,
    },
    Address, Physical, Virtual,
};

use super::TranslationTable;

/// Constants for indexing the MAIR_EL1.
#[allow(dead_code)]
mod mair {
    pub const DEVICE: u64 = 0;
    pub const NORMAL: u64 = 1;
}

// A table descriptor, as per ARMv8-A Architecture Reference Manual Figure D5-15.
register_bitfields! {u64,
    STAGE1_TABLE_DESCRIPTOR [
        /// Physical address of the next descriptor.
        NEXT_LEVEL_TABLE_ADDR_64KiB OFFSET(16) NUMBITS(32) [], // [47:16]

        TYPE  OFFSET(1) NUMBITS(1) [
            Block = 0,
            Table = 1
        ],

        VALID OFFSET(0) NUMBITS(1) [
            False = 0,
            True = 1
        ]
    ]
}

// A level 3 page descriptor, as per ARMv8-A Architecture Reference Manual Figure D5-17.
register_bitfields! {u64,
    STAGE1_PAGE_DESCRIPTOR [
        /// Unprivileged execute-never.
        UXN      OFFSET(54) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        /// Privileged execute-never.
        PXN      OFFSET(53) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        /// Physical address of the next table descriptor (lvl2) or the page descriptor (lvl3).
        OUTPUT_ADDR_64KiB OFFSET(16) NUMBITS(32) [], // [47:16]

        /// Access flag.
        AF       OFFSET(10) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        /// Shareability field.
        SH       OFFSET(8) NUMBITS(2) [
            OuterShareable = 0b10,
            InnerShareable = 0b11
        ],

        /// Access Permissions.
        AP       OFFSET(6) NUMBITS(2) [
            RW_EL1 = 0b00,
            RW_EL1_EL0 = 0b01,
            RO_EL1 = 0b10,
            RO_EL1_EL0 = 0b11
        ],

        /// Memory attributes index into the MAIR_EL1 register.
        AttrIndx OFFSET(2) NUMBITS(3) [],

        TYPE     OFFSET(1) NUMBITS(1) [
            Reserved_Invalid = 0,
            Page = 1
        ],

        VALID    OFFSET(0) NUMBITS(1) [
            False = 0,
            True = 1
        ]
    ]
}

/// Big monolithic struct for storing the translation tables. Individual levels must be 64 KiB
/// aligned, so the lvl3 is put first.
#[repr(C)]
#[repr(align(65536))]
pub struct FixedSizeTranslationTable<const NUM_TABLES: usize> {
    /// Page descriptors, covering 64 KiB windows per entry.
    lvl3: [[PageDescriptor; 8192]; NUM_TABLES],

    /// Table descriptors, covering 512 MiB windows.
    lvl2: [TableDescriptor; NUM_TABLES],

    /// Have the tables been initialized?
    initialized: bool,
}

impl<const NUM_TABLES: usize> FixedSizeTranslationTable<NUM_TABLES> {
    /// Create an instance.
    #[allow(clippy::assertions_on_constants)]
    pub const fn new() -> Self {
        assert!(crate::bsp::memory::mmu::MSKernel::SIZE == MS64KiB::SIZE);

        // Can't have a zero-sized address space.
        assert!(NUM_TABLES > 0);

        Self {
            lvl3: [[PageDescriptor::new_zeroed(); 8192]; NUM_TABLES],
            lvl2: [TableDescriptor::new_zeroed(); NUM_TABLES],
            initialized: false,
        }
    }

    /// Helper to calculate the lvl2 and lvl3 indices from an address.
    #[inline(always)]
    fn lvl2_lvl3_index_from_page_addr(
        &self,
        virt_page_addr: PageAddress<Virtual>,
    ) -> Result<(usize, usize), &'static str> {
        let addr = virt_page_addr.address().as_usize();
        let lvl2_index = addr >> MS512MiB::SHIFT;
        let lvl3_index = (addr & MS512MiB::MASK) >> MS64KiB::SHIFT;

        if lvl2_index > (NUM_TABLES - 1) {
            return Err("Virtual page is out of bounds of translation table");
        }

        Ok((lvl2_index, lvl3_index))
    }

    /// Sets the PageDescriptor corresponding to the supplied page address.
    ///
    /// Doesn't allow overriding an already valid page.
    #[inline(always)]
    fn set_descriptor(
        &mut self,
        virt_page_addr: PageAddress<Virtual>,
        new_desc: &PageDescriptor,
    ) -> Result<(), &'static str> {
        let (lvl2_index, lvl3_index) = self.lvl2_lvl3_index_from_page_addr(virt_page_addr)?;
        let desc = &mut self.lvl3[lvl2_index][lvl3_index];

        if desc.is_valid() {
            return Err("Virtual page is already mapped");
        }

        *desc = *new_desc;
        Ok(())
    }

    /// Returns the PageDescriptor corresponding to the supplied page address.
    #[inline(always)]
    fn get_descriptor(
        &self,
        virt_page_addr: PageAddress<Virtual>,
    ) -> Result<&PageDescriptor, &'static str> {
        let (lvl2_index, lvl3_index) = self.lvl2_lvl3_index_from_page_addr(virt_page_addr)?;
        let desc = &self.lvl3[lvl2_index][lvl3_index];

        Ok(desc)
    }
}

impl<const NUM_TABLES: usize> TranslationTable for FixedSizeTranslationTable<NUM_TABLES> {
    fn init(&mut self) {
        if self.initialized {
            return;
        }

        // Populate the l2 entries.
        for (i, lvl2_entry) in self.lvl2.iter_mut().enumerate() {
            let phys_table_addr = self.lvl3[i].phys_start_addr();

            let new_desc = TableDescriptor::from_phys_addr(phys_table_addr);
            *lvl2_entry = new_desc;
        }

        self.initialized = true;
    }

    fn phys_base_address(&self) -> Address<Physical> {
        self.lvl2.phys_start_addr()
    }

    unsafe fn map_at(
        &mut self,
        virt_region: &MemoryRegion<Virtual>,
        phys_region: &MemoryRegion<Physical>,
        attr: &AttributeFields,
    ) -> Result<(), &'static str> {
        assert!(self.initialized, "Translation tables not initialized");

        if virt_region.size() != phys_region.size() {
            return Err("Tried to map memory regions with unequal sizes");
        }

        if phys_region.end_page_exclusive > crate::bsp::memory::phys_addr_space_end_exclusive_addr()
        {
            return Err("Tried to map outside of physical address space");
        }

        for (phys_page_addr, virt_page_addr) in phys_region.as_range().zip(virt_region.as_range()) {
            let new_desc = PageDescriptor::new(phys_page_addr, attr);
            self.set_descriptor(virt_page_addr, &new_desc)?;
        }

        Ok(())
    }

    fn try_page_attributes(
        &self,
        virt_page_addr: PageAddress<Virtual>,
    ) -> Result<AttributeFields, &'static str> {
        let page_desc = self.get_descriptor(virt_page_addr)?;

        if !page_desc.is_valid() {
            return Err("Page marked invalid");
        }

        page_desc.try_attributes()
    }
}

trait StartAddr {
    fn phys_start_addr(&self) -> Address<Physical>;
}

// The binary is still identity mapped, so we don't need to convert here.
impl<T, const N: usize> StartAddr for [T; N] {
    fn phys_start_addr(&self) -> Address<Physical> {
        Address::new(self as *const _ as usize)
    }
}

/// A table descriptor for 64 KiB aperture.
///
/// The output points to the next table.
#[derive(Copy, Clone)]
#[repr(C)]
struct TableDescriptor {
    value: u64,
}

impl TableDescriptor {
    /// Create an instance.
    ///
    /// Descriptor is invalid by default.
    pub const fn new_zeroed() -> Self {
        Self { value: 0 }
    }

    /// Create an instance pointing to the supplied address.
    pub fn from_phys_addr(phys_next_lvl_table_addr: Address<Physical>) -> Self {
        let val = InMemoryRegister::<u64, STAGE1_TABLE_DESCRIPTOR::Register>::new(0);

        let shifted = phys_next_lvl_table_addr.as_usize() >> MS64KiB::SHIFT;
        val.write(
            STAGE1_TABLE_DESCRIPTOR::NEXT_LEVEL_TABLE_ADDR_64KiB.val(shifted as u64)
                + STAGE1_TABLE_DESCRIPTOR::TYPE::Table
                + STAGE1_TABLE_DESCRIPTOR::VALID::True,
        );

        TableDescriptor { value: val.get() }
    }
}

/// A page descriptor with 64 KiB aperture.
///
/// The output points to physical memory.
#[derive(Copy, Clone)]
#[repr(C)]
struct PageDescriptor {
    value: u64,
}

impl PageDescriptor {
    /// Create an instance.
    ///
    /// Descriptor is invalid by default.
    pub const fn new_zeroed() -> Self {
        Self { value: 0 }
    }

    /// Create an instance.
    pub fn new(
        phys_output_addr: PageAddress<Physical>,
        attribute_fields: &AttributeFields,
    ) -> Self {
        let val = InMemoryRegister::<u64, STAGE1_PAGE_DESCRIPTOR::Register>::new(0);

        let shifted = phys_output_addr.address().as_usize() as u64 >> MS64KiB::SHIFT;
        val.write(
            STAGE1_PAGE_DESCRIPTOR::OUTPUT_ADDR_64KiB.val(shifted)
                + STAGE1_PAGE_DESCRIPTOR::AF::True
                + STAGE1_PAGE_DESCRIPTOR::TYPE::Page
                + STAGE1_PAGE_DESCRIPTOR::VALID::True
                + (*attribute_fields).into(),
        );

        Self { value: val.get() }
    }

    /// Returns the valid bit.
    fn is_valid(&self) -> bool {
        InMemoryRegister::<u64, STAGE1_PAGE_DESCRIPTOR::Register>::new(self.value)
            .is_set(STAGE1_PAGE_DESCRIPTOR::VALID)
    }

    /// Returns the attributes.
    fn try_attributes(&self) -> Result<AttributeFields, &'static str> {
        InMemoryRegister::<u64, STAGE1_PAGE_DESCRIPTOR::Register>::new(self.value).try_into()
    }
}

impl<const S: usize> AssociatedTranslationTable for AddressSpace<S>
where
    [u8; Self::SIZE >> MS512MiB::SHIFT]: Sized,
{
    type Table = FixedSizeTranslationTable<{ Self::SIZE >> MS512MiB::SHIFT }>;
}

/// Convert the HW-specific attributes of the MMU to kernel's generic memory attributes.
impl TryFrom<InMemoryRegister<u64, STAGE1_PAGE_DESCRIPTOR::Register>> for AttributeFields {
    type Error = &'static str;

    fn try_from(
        desc: InMemoryRegister<u64, STAGE1_PAGE_DESCRIPTOR::Register>,
    ) -> Result<AttributeFields, Self::Error> {
        let mem_attributes = match desc.read(STAGE1_PAGE_DESCRIPTOR::AttrIndx) {
            mair::NORMAL => MemAttributes::CacheableDRAM,
            mair::DEVICE => MemAttributes::Device,
            _ => return Err("Unexpected memory attribute"),
        };

        let acc_perms = match desc.read_as_enum(STAGE1_PAGE_DESCRIPTOR::AP) {
            Some(STAGE1_PAGE_DESCRIPTOR::AP::Value::RO_EL1) => AccessPermissions::ReadOnly,
            Some(STAGE1_PAGE_DESCRIPTOR::AP::Value::RW_EL1) => AccessPermissions::ReadWrite,
            _ => return Err("Unexpected access permission"),
        };

        let executable = desc.read(STAGE1_PAGE_DESCRIPTOR::PXN) > 0;

        Ok(AttributeFields {
            mem_attributes,
            acc_perms,
            executable,
        })
    }
}

/// Convert the kernel's generic memory attributes to HW-specific attributes of the MMU.
impl From<AttributeFields> for FieldValue<u64, STAGE1_PAGE_DESCRIPTOR::Register> {
    fn from(attribute_fields: AttributeFields) -> Self {
        // Memory attributes.
        let mut desc = match attribute_fields.mem_attributes {
            MemAttributes::CacheableDRAM => {
                STAGE1_PAGE_DESCRIPTOR::SH::InnerShareable
                    + STAGE1_PAGE_DESCRIPTOR::AttrIndx.val(mair::NORMAL)
            }
            MemAttributes::Device => {
                STAGE1_PAGE_DESCRIPTOR::SH::OuterShareable
                    + STAGE1_PAGE_DESCRIPTOR::AttrIndx.val(mair::DEVICE)
            }
        };

        // Access Permissions.
        desc += match attribute_fields.acc_perms {
            AccessPermissions::ReadOnly => STAGE1_PAGE_DESCRIPTOR::AP::RO_EL1,
            AccessPermissions::ReadWrite => STAGE1_PAGE_DESCRIPTOR::AP::RW_EL1,
        };

        // The execute-never attribute is mapped to PXN in AArch64.
        desc += if attribute_fields.executable {
            STAGE1_PAGE_DESCRIPTOR::PXN::False
        } else {
            STAGE1_PAGE_DESCRIPTOR::PXN::True
        };

        // Always set unprivileged exectue-never as long as userspace is not implemented yet.
        desc += STAGE1_PAGE_DESCRIPTOR::UXN::True;

        desc
    }
}
