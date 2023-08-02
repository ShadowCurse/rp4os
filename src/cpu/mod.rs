#[path = "../arch/aarch64/cpu/mod.rs"]
mod arch_cpu;
pub mod smp;

pub use arch_cpu::*;
