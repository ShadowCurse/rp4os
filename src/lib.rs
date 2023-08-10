#![feature(generic_const_exprs)]
#![feature(panic_info_message)]
#![feature(unchecked_math)]
#![feature(format_args_nl)]
#![feature(int_roundings)]
#![feature(const_option)]
#![feature(step_trait)]
#![feature(is_sorted)]
#![feature(asm_const)]
#![no_main]
#![no_std]

pub mod bsp;
pub mod console;
pub mod cpu;
pub mod driver;
pub mod exception;
pub mod exception_level;
pub mod memory;
pub mod panic;
pub mod print;
pub mod state;
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

/// Check if a value is aligned to a given size.
#[inline(always)]
pub const fn is_aligned(ptr: usize, alignment: usize) -> bool {
    assert!(alignment.is_power_of_two());

    (ptr & (alignment - 1)) == 0
}

/// Align down.
#[inline(always)]
pub const fn align_down(ptr: usize, alignment: usize) -> usize {
    assert!(alignment.is_power_of_two());

    ptr & !(alignment - 1)
}

/// Align up.
#[inline(always)]
pub const fn align_up(ptr: usize, alignment: usize) -> usize {
    assert!(alignment.is_power_of_two());

    (ptr + alignment - 1) & !(alignment - 1)
}
