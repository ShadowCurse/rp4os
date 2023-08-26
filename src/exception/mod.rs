#[path = "../arch/aarch64/exception/mod.rs"]
mod arch_exception;

#[path = "../arch/aarch64/exception/vector.rs"]
mod arch_exception_vector;

pub mod asynchronous;
pub mod null_irq_manager;

use core::fmt::Debug;

/// Init exception handling by setting the exception vector base address register.
///
/// # Safety
///
/// - Changes the HW state of the executing core.
pub unsafe fn set_exception_vector() {
    arch_exception_vector::set_exception_vector()
}

/// Kernel privilege levels.
#[derive(Eq, PartialEq)]
pub enum ExceptionLevel {
    User,
    Kernel,
    Hypervisor,
    Unknown,
}

impl ExceptionLevel {
    pub fn current_level() -> ExceptionLevel {
        arch_exception::current_exception_level()
    }
}

impl Debug for ExceptionLevel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ExceptionLevel::Hypervisor => f.write_str("EL2"),
            ExceptionLevel::Kernel => f.write_str("EL1"),
            ExceptionLevel::User => f.write_str("EL0"),
            ExceptionLevel::Unknown => f.write_str("Unknown"),
        }
    }
}

/// Prints exception status.
#[inline(always)]
pub fn print_exception_state() {
    arch_exception::print_exception_state()
}

/// Returns whether IRQs are masked on the executing core.
#[inline(always)]
pub fn local_irq_enabled() -> bool {
    arch_exception::local_irq_enabled()
}

/// Unmask IRQs on the executing core.
#[inline(always)]
pub fn local_irq_unmask() {
    arch_exception::local_irq_unmask()
}

/// Mask IRQs on the executing core.
#[inline(always)]
pub fn local_irq_mask() {
    arch_exception::local_irq_mask()
}

/// Mask IRQs on the executing core and return the previously saved interrupt mask bits (DAIF).
#[inline(always)]
pub fn local_irq_mask_and_save() -> u64 {
    arch_exception::local_irq_mask_and_save()
}

/// Restore the interrupt mask bits (DAIF) using the callee's argument.
///
/// # Invariant
///
/// - No sanity checks on the input.
#[inline(always)]
pub fn local_irq_restore(saved: u64) {
    arch_exception::local_irq_restore(saved)
}
