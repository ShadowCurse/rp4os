#[path = "arch/aarch64/exception_level.rs"]
mod arch_exception_level;

pub use arch_exception_level::*;

/// Kernel privilege levels.
#[derive(Eq, PartialEq)]
pub enum PrivilegeLevel {
    User,
    Kernel,
    Hypervisor,
    Unknown,
}
