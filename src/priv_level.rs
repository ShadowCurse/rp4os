#[path = "arch/aarch64/priv_level.rs"]
mod arch_priv_level;

pub use arch_priv_level::*;

/// Kernel privilege levels.
#[derive(Eq, PartialEq)]
pub enum PrivilegeLevel {
    User,
    Kernel,
    Hypervisor,
    Unknown,
}
