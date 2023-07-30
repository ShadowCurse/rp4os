#![feature(panic_info_message)]
#![feature(unchecked_math)]
#![feature(format_args_nl)]
#![feature(int_roundings)]
#![feature(const_option)]
#![feature(asm_const)]
#![no_main]
#![no_std]

pub mod bsp;
pub mod console;
pub mod cpu;
pub mod mmu;
pub mod driver;
pub mod panic;
pub mod print;
pub mod priv_level;
pub mod synchronization;
pub mod time;

/// Convert a size into human readable format.
pub const fn size_human_readable_ceil(size: usize) -> (usize, &'static str) {
    const KIB: usize = 1024;
    const MIB: usize = 1024 * 1024;
    const GIB: usize = 1024 * 1024 * 1024;

    if (size / GIB) > 0 {
        (size.div_ceil(GIB), "GiB")
    } else if (size / MIB) > 0 {
        (size.div_ceil(MIB), "MiB")
    } else if (size / KIB) > 0 {
        (size.div_ceil(KIB), "KiB")
    } else {
        (size, "Byte")
    }
}
