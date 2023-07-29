use crate::info;
use aarch64_cpu::registers::*;
use tock_registers::interfaces::Readable;

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
