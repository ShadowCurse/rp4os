use std::path::PathBuf;

use clap::Parser;
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
