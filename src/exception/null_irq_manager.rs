use crate::{
    bsp::drivers::gicv2::IRQNumber,
    exception::asynchronous::{interface::IRQManager, IRQContext, IRQHandlerDescriptor},
};

pub static NULL_IRQ_MANAGER: NullIRQManager = NullIRQManager {};

pub struct NullIRQManager;

impl IRQManager for NullIRQManager {
    type IRQNumberType = IRQNumber;

    fn register_handler(
        &self,
        _descriptor: IRQHandlerDescriptor<Self::IRQNumberType>,
    ) -> Result<(), &'static str> {
        panic!("No IRQ Manager registered yet");
    }

    fn enable(&self, _irq_number: &Self::IRQNumberType) {
        panic!("No IRQ Manager registered yet");
    }

    fn handle_pending_irqs<'irq_context>(&'irq_context self, _ic: &IRQContext<'irq_context>) {
        panic!("No IRQ Manager registered yet");
    }
}
