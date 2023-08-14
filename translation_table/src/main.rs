use std::path::PathBuf;

use clap::Parser;
use elf::segment::ProgramHeader;
use elf::string_table::StringTable;
use elf::symbol::Symbol;
use elf::ElfBytes;
use elf::{endian::AnyEndian, parse::ParsingTable};

#[derive(Parser)]
struct Cli {
    #[arg(short, long)]
    kernel: PathBuf,
}

fn main() {
    let cli = Cli::parse();

    let file_data = std::fs::read(cli.kernel).expect("Could not read file.");
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
        .filter(|segment| true)
        .map(|segment| {
            let size = align_up(segment.p_memsz, 64 * 1024);
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
                end_exclusive: virt_start_addr + size,
            };
            let phys_region = MemoryRegion {
                start: phys_start_addr,
                end_exclusive: phys_start_addr + size,
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
    end_exclusive: u64,
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
