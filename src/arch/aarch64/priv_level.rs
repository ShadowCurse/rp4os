use crate::info;
use aarch64_cpu::registers::*;
use core::arch::asm;
use tock_registers::interfaces::{Readable, Writeable};

use crate::priv_level::PrivilegeLevel;

/// The processing element's current privilege level.
pub fn current_privilege_level() -> (PrivilegeLevel, &'static str) {
    let el = CurrentEL.read_as_enum(CurrentEL::EL);
    match el {
        Some(CurrentEL::EL::Value::EL2) => (PrivilegeLevel::Hypervisor, "EL2"),
        Some(CurrentEL::EL::Value::EL1) => (PrivilegeLevel::Kernel, "EL1"),
        Some(CurrentEL::EL::Value::EL0) => (PrivilegeLevel::User, "EL0"),
        _ => (PrivilegeLevel::Unknown, "Unknown"),
    }
}

/// Print the AArch64 exceptions status.
#[rustfmt::skip]
pub fn print_state() {

    info!("Debug:  masked: {}", Debug::is_set());
    info!("SError: masked: {}", SError::is_set());
    info!("IRQ:    masked: {}", Irq::is_set());
    info!("FIQ:    masked: {}", Fiq::is_set());
}

trait DaifField {
    fn daif_field() -> tock_registers::fields::Field<u64, DAIF::Register>;
}

struct Debug;
struct SError;
struct Irq;
struct Fiq;

macro_rules! impl_daif {
    ($t:ty, $daif_field:ident) => {
        impl $t {
            pub fn is_set() -> bool {
                DAIF.is_set(DAIF::$daif_field)
            }
        }
    };
}

impl_daif!(Debug, D);
impl_daif!(SError, A);
impl_daif!(Irq, I);
impl_daif!(Fiq, F);

/// Returns whether IRQs are masked on the executing core.
pub fn is_local_irq_masked() -> bool {
    !Irq::is_set()
}


mod daif_bits {
    pub const IRQ: u8 = 0b0010;
}

/// Unmask IRQs on the executing core.
///
/// It is not needed to place an explicit instruction synchronization barrier after the `msr`.
/// Quoting the Architecture Reference Manual for ARMv8-A, section C5.1.3:
///
/// "Writes to PSTATE.{PAN, D, A, I, F} occur in program order without the need for additional
/// synchronization."
#[inline(always)]
pub fn local_irq_unmask() {
    unsafe {
        asm!(
            "msr DAIFClr, {arg}",
            arg = const daif_bits::IRQ,
            options(nomem, nostack, preserves_flags)
        );
    }
}

/// Mask IRQs on the executing core.
#[inline(always)]
pub fn local_irq_mask() {
    unsafe {
        asm!(
            "msr DAIFSet, {arg}",
            arg = const daif_bits::IRQ,
            options(nomem, nostack, preserves_flags)
        );
    }
}

/// Mask IRQs on the executing core and return the previously saved interrupt mask bits (DAIF).
#[inline(always)]
pub fn local_irq_mask_save() -> u64 {
    let saved = DAIF.get();
    local_irq_mask();

    saved
}

/// Restore the interrupt mask bits (DAIF) using the callee's argument.
///
/// # Invariant
///
/// - No sanity checks on the input.
#[inline(always)]
pub fn local_irq_restore(saved: u64) {
    DAIF.set(saved);
}
