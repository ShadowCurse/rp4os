use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;

use clap::Parser;
use elf::segment::ProgramHeader;
use elf::string_table::StringTable;
use elf::symbol::Symbol;
use elf::ElfBytes;
use elf::{endian::AnyEndian, parse::ParsingTable};
use tock_registers::fields::FieldValue;
use tock_registers::interfaces::{Readable, Writeable};
use tock_registers::register_bitfields;
use tock_registers::registers::InMemoryRegister;

pub type KernelGranule = TranslationGranule<{ 64 * 1024 }>;
pub type Granule64KiB = TranslationGranule<{ 64 * 1024 }>;
pub type Granule512MiB = TranslationGranule<{ 512 * 1024 * 1024 }>;

const NUM_TABLES: usize = 1024 * 1024 * 1024 >> Granule512MiB::SHIFT;

#[derive(Parser)]
struct Cli {
    #[arg(short, long)]
    kernel: PathBuf,
}

fn main() {
    let cli = Cli::parse();

    let file_data = std::fs::read(cli.kernel.clone()).expect("Could not read file.");
    let slice = file_data.as_slice();
    let file = ElfBytes::<AnyEndian>::minimal_parse(slice).expect("Open test1");

    let symbols = file.symbol_table().unwrap().unwrap();

    let kernel_virt_addr_space_size = get_symbol_value("__kernel_virt_addr_space_size", &symbols);
    let virt_addr_of_kernel_translation_tables =
        get_symbol_value("KERNEL_TRANSLATION_TABLES", &symbols);
    let virt_addr_of_phys_kernel_tables_base_addr =
        get_symbol_value("PHYS_KERNEL_TABLES_BASE_ADDR", &symbols);

    println!("kernel_virt_addr_space_size: {kernel_virt_addr_space_size:#x}");
    println!("virt_addr_of_kernel_translation_tables: {virt_addr_of_kernel_translation_tables:#x}");
    println!(
        "virt_addr_of_phys_kernel_tables_base_addr: {virt_addr_of_phys_kernel_tables_base_addr:#x}"
    );

    let descriptors = map_kernel_binary(&file);
    println!("{:#?}", descriptors);

    let mut translation_table = FixedSizeTranslationTable::<NUM_TABLES>::new();
    for descriptor in descriptors {
        translation_table.map_at(descriptor);
    }

    let table_slice = unsafe {
        std::slice::from_raw_parts(
            std::mem::transmute::<_, *const u8>(&translation_table),
            std::mem::size_of::<FixedSizeTranslationTable<NUM_TABLES>>(),
        )
    };

    let kernel_tables_offset_in_file =
        virt_addr_to_file_offset(&file, virt_addr_of_kernel_translation_tables);
    let phys_kernel_tables_base_addr_offset_in_file =
        virt_addr_to_file_offset(&file, virt_addr_of_phys_kernel_tables_base_addr);

    println!("kernel_tables_offset_in_file: {kernel_tables_offset_in_file:#x}");
    println!("phys_kernel_tables_base_addr_offset_in_file: {phys_kernel_tables_base_addr_offset_in_file:#x}");

    let phys_addr_of_kernel_tables = virt_to_phys(&file, virt_addr_of_kernel_translation_tables);
    let lvl2_phys_statrt_addr =
        phys_addr_of_kernel_tables + std::mem::size_of_val(&translation_table.lvl3) as u64;
    println!("phys_addr_of_kernel_tables: {phys_addr_of_kernel_tables:#x}");
    println!("lvl2_phys_statrt_addr: {lvl2_phys_statrt_addr:#x}");

    let mut binary = OpenOptions::new().write(true).open(cli.kernel).unwrap();
    binary
        .seek(SeekFrom::Start(kernel_tables_offset_in_file))
        .unwrap();
    binary.write_all(table_slice);

    binary
        .seek(SeekFrom::Start(phys_kernel_tables_base_addr_offset_in_file))
        .unwrap();
    binary.write_all(&lvl2_phys_statrt_addr.to_le_bytes());
}

fn virt_to_phys(file: &ElfBytes<AnyEndian>, virt_addr: u64) -> u64 {
    let segment = file
        .segments()
        .unwrap()
        .iter()
        .find(|s| s.vma_in(virt_addr))
        .unwrap();

    let translation_offset = segment.p_vaddr - segment.p_paddr;

    virt_addr - translation_offset
}

fn virt_addr_to_file_offset(file: &ElfBytes<AnyEndian>, virt_addr: u64) -> u64 {
    let segment = file
        .segments()
        .unwrap()
        .iter()
        .find(|s| s.vma_in(virt_addr))
        .unwrap();
    segment.vma_to_offset(virt_addr)
}

fn get_symbol_value(
    name: &str,
    (parsing_table, symbol_table): &(ParsingTable<'_, AnyEndian, Symbol>, StringTable<'_>),
) -> u64 {
    for symbol in parsing_table.iter() {
        let s_name = symbol_table.get(symbol.st_name as usize).unwrap();
        if s_name == name {
            return symbol.st_value;
        }
    }
    unreachable!("could not find {name}");
}

fn map_kernel_binary(file: &ElfBytes<AnyEndian>) -> Vec<MappingDescriptor> {
    file.segments()
        .unwrap()
        .iter()
        // Load segments
        .filter(|segment| segment.p_type == 1)
        .map(|segment| {
            let size = align_up(segment.p_memsz, KernelGranule::SIZE as u64);
            let virt_start_addr = segment.p_vaddr;
            let phys_start_addr = segment.p_paddr;
            let acc_perms = match (segment.readable(), segment.writable()) {
                (true, true) => AccessPermissions::ReadWrite,
                (true, false) => AccessPermissions::ReadOnly,
                _ => unreachable!(),
            };
            let execute_never = !segment.executable();

            let virt_region = MemoryRegion {
                start: virt_start_addr,
                size,
            };
            let phys_region = MemoryRegion {
                start: phys_start_addr,
                size,
            };
            let attributes = AttributeFields {
                mem_attributes: MemAttributes::CacheableDRAM,
                acc_perms,
                execute_never,
            };

            MappingDescriptor {
                virt_region,
                phys_region,
                attributes,
            }
        })
        .collect()
}

trait Segment {
    fn readable(&self) -> bool;
    fn writable(&self) -> bool;
    fn executable(&self) -> bool;
    fn vma_in(&self, vma: u64) -> bool;
    fn vma_to_offset(&self, vma: u64) -> u64;
}

impl Segment for ProgramHeader {
    fn readable(&self) -> bool {
        (self.p_flags & 4) == 4
    }

    fn writable(&self) -> bool {
        (self.p_flags & 2) == 2
    }

    fn executable(&self) -> bool {
        (self.p_flags & 1) == 1
    }

    fn vma_in(&self, vma: u64) -> bool {
        vma >= (self.p_vaddr & !self.p_align) && vma <= (self.p_vaddr + self.p_memsz)
    }

    fn vma_to_offset(&self, vma: u64) -> u64 {
        vma - self.p_vaddr + self.p_offset
    }
}

/// Align up.
#[inline(always)]
pub const fn align_up(ptr: u64, alignment: u64) -> u64 {
    assert!(alignment.is_power_of_two());

    (ptr + alignment - 1) & !(alignment - 1)
}

#[derive(Copy, Clone, Debug, Eq, PartialOrd, PartialEq)]
pub enum AccessPermissions {
    ReadOnly,
    ReadWrite,
}

/// A type that describes a region of memory in quantities of pages.
#[derive(Copy, Clone, Debug, Eq, PartialOrd, PartialEq)]
pub struct MemoryRegion {
    start: u64,
    size: u64,
}

impl MemoryRegion {
    fn iter(&self) -> impl Iterator<Item = u64> + '_ {
        let num_pages = self.size / KernelGranule::SIZE as u64;
        (0..num_pages)
            .into_iter()
            .map(|i| self.start + i * KernelGranule::SIZE as u64)
    }

    fn is_empty(&self) -> bool {
        self.size == 0
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialOrd, PartialEq)]
struct MappingDescriptor {
    virt_region: MemoryRegion,
    phys_region: MemoryRegion,
    attributes: AttributeFields,
}

#[derive(Copy, Clone, Debug, Eq, PartialOrd, PartialEq)]
pub struct AttributeFields {
    pub mem_attributes: MemAttributes,
    pub acc_perms: AccessPermissions,
    pub execute_never: bool,
}

#[derive(Copy, Clone, Debug, Eq, PartialOrd, PartialEq)]
pub enum MemAttributes {
    CacheableDRAM,
    Device,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct PageDescriptor {
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
    pub fn from_output_page_addr(phys_output_addr: u64, attributes: AttributeFields) -> Self {
        let val = InMemoryRegister::<u64, STAGE1_PAGE_DESCRIPTOR::Register>::new(0);

        let shifted = phys_output_addr as usize >> Granule64KiB::SHIFT;
        val.write(
            STAGE1_PAGE_DESCRIPTOR::OUTPUT_ADDR_64KiB.val(shifted as u64)
                + STAGE1_PAGE_DESCRIPTOR::AF::True
                + STAGE1_PAGE_DESCRIPTOR::TYPE::Page
                + STAGE1_PAGE_DESCRIPTOR::VALID::True
                + attributes.into(),
        );

        Self { value: val.get() }
    }

    /// Returns the valid bit.
    fn is_valid(&self) -> bool {
        InMemoryRegister::<u64, STAGE1_PAGE_DESCRIPTOR::Register>::new(self.value)
            .is_set(STAGE1_PAGE_DESCRIPTOR::VALID)
    }

    /// Returns the output page.
    fn output_page_addr(&self) -> u64 {
        let shifted = InMemoryRegister::<u64, STAGE1_PAGE_DESCRIPTOR::Register>::new(self.value)
            .read(STAGE1_PAGE_DESCRIPTOR::OUTPUT_ADDR_64KiB) as usize;

        (shifted << Granule64KiB::SHIFT) as u64
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct TableDescriptor {
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
    pub fn from_next_lvl_table_addr(phys_next_lvl_table_addr: u64) -> Self {
        let val = InMemoryRegister::<u64, STAGE1_TABLE_DESCRIPTOR::Register>::new(0);

        let shifted = phys_next_lvl_table_addr as usize >> Granule64KiB::SHIFT;
        val.write(
            STAGE1_TABLE_DESCRIPTOR::NEXT_LEVEL_TABLE_ADDR_64KiB.val(shifted as u64)
                + STAGE1_TABLE_DESCRIPTOR::TYPE::Table
                + STAGE1_TABLE_DESCRIPTOR::VALID::True,
        );

        TableDescriptor { value: val.get() }
    }
}

/// Big monolithic struct for storing the translation tables. Individual levels must be 64 KiB
/// aligned, so the lvl3 is put first.
#[repr(C)]
#[repr(align(65536))]
pub struct FixedSizeTranslationTable<const NUM_TABLES: usize> {
    /// Page descriptors, covering 64 KiB windows per entry.
    pub lvl3: [[PageDescriptor; 8192]; NUM_TABLES],

    /// Table descriptors, covering 512 MiB windows.
    pub lvl2: [TableDescriptor; NUM_TABLES],
}

impl<const NUM_TABLES: usize> FixedSizeTranslationTable<NUM_TABLES> {
    pub const fn new() -> Self {
        assert!(KernelGranule::SIZE == Granule64KiB::SIZE);

        // Can't have a zero-sized address space.
        assert!(NUM_TABLES > 0);

        Self {
            lvl3: [[PageDescriptor::new_zeroed(); 8192]; NUM_TABLES],
            lvl2: [TableDescriptor::new_zeroed(); NUM_TABLES],
        }
    }

    fn map_at(&mut self, descriptor: MappingDescriptor) -> Result<(), &'static str> {
        let MappingDescriptor {
            virt_region,
            phys_region,
            attributes,
        } = descriptor;
        if descriptor.virt_region.size != phys_region.size {
            return Err("Tried to map memory regions with unequal sizes");
        }

        for (phys_page_addr, virt_page_addr) in phys_region.iter().zip(virt_region.iter()) {
            let new_desc = PageDescriptor::from_output_page_addr(phys_page_addr, attributes);
            let virt_page = virt_page_addr;

            self.set_page_descriptor_from_page_addr(virt_page, &new_desc)?;
        }

        Ok(())
    }

    /// Helper to calculate the lvl2 and lvl3 indices from an address.
    #[inline(always)]
    fn lvl2_lvl3_index_from_page_addr(
        &self,
        virt_page_addr: u64,
    ) -> Result<(usize, usize), &'static str> {
        let addr = virt_page_addr as usize;
        let lvl2_index = addr >> Granule512MiB::SHIFT;
        let lvl3_index = (addr & Granule512MiB::MASK) >> Granule64KiB::SHIFT;
        Ok((lvl2_index as usize, lvl3_index as usize))
    }

    /// Returns the PageDescriptor corresponding to the supplied page address.
    #[inline(always)]
    fn page_descriptor_from_page_addr(
        &self,
        virt_page_addr: u64,
    ) -> Result<&PageDescriptor, &'static str> {
        let (lvl2_index, lvl3_index) = self.lvl2_lvl3_index_from_page_addr(virt_page_addr)?;
        let desc = &self.lvl3[lvl2_index][lvl3_index];

        Ok(desc)
    }

    /// Sets the PageDescriptor corresponding to the supplied page address.
    ///
    /// Doesn't allow overriding an already valid page.
    #[inline(always)]
    fn set_page_descriptor_from_page_addr(
        &mut self,
        virt_page_addr: u64,
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
}

/// Describes the characteristics of a translation granule.
pub struct TranslationGranule<const GRANULE_SIZE: usize>;

impl<const GRANULE_SIZE: usize> TranslationGranule<GRANULE_SIZE> {
    /// The granule's size.
    pub const SIZE: usize = Self::size_checked();

    /// The granule's mask.
    pub const MASK: usize = Self::SIZE - 1;

    /// The granule's shift, aka log2(size).
    pub const SHIFT: usize = Self::SIZE.trailing_zeros() as usize;

    const fn size_checked() -> usize {
        assert!(GRANULE_SIZE.is_power_of_two());
        GRANULE_SIZE
    }
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
        desc += if attribute_fields.execute_never {
            STAGE1_PAGE_DESCRIPTOR::PXN::True
        } else {
            STAGE1_PAGE_DESCRIPTOR::PXN::False
        };

        // Always set unprivileged exectue-never as long as userspace is not implemented yet.
        desc += STAGE1_PAGE_DESCRIPTOR::UXN::True;

        desc
    }
}

/// Constants for indexing the MAIR_EL1.
#[allow(dead_code)]
pub mod mair {
    pub const DEVICE: u64 = 0;
    pub const NORMAL: u64 = 1;
}
