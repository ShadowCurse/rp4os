use super::{
    drivers::{bcm2xxx_gpio::GPIO, bcm2xxx_pl011::PL011Uart},
    memory::map::mmio::{GICC_START, GICD_START, GPIO_START, PL011_UART_START},
};
use crate::{
    bsp::{drivers::gicv2::GICv2, execption::PL011_UART_IRQ},
    console,
    driver::DeviceDriverDescriptor,
    driver::DRIVER_MANAGER,
    exception::asynchronous::register_irq_manager,
};
use core::sync::atomic::{AtomicBool, Ordering};

pub static PL011_UART: PL011Uart = unsafe { PL011Uart::new(PL011_UART_START) };
pub static GPIO: GPIO = unsafe { GPIO::new(GPIO_START) };
static INTERRUPT_CONTROLLER: GICv2 = unsafe { GICv2::new(GICD_START, GICC_START) };

/// This must be called only after successful init of the UART driver.
fn post_init_uart() -> Result<(), &'static str> {
    console::register_console(&PL011_UART);
    Ok(())
}

/// This must be called only after successful init of the GPIO driver.
fn post_init_gpio() -> Result<(), &'static str> {
    GPIO.map_pl011_uart();
    Ok(())
}

/// This must be called only after successful init of the interrupt controller driver.
fn post_init_interrupt_controller() -> Result<(), &'static str> {
    register_irq_manager(&INTERRUPT_CONTROLLER);
    Ok(())
}


/// Initialize the driver subsystem.
///
/// # Safety
///
/// See child function calls.
pub unsafe fn init() -> Result<(), &'static str> {
    static INIT_DONE: AtomicBool = AtomicBool::new(false);
    if INIT_DONE.load(Ordering::Relaxed) {
        return Err("Init already done");
    }

    let uart_descriptor = DeviceDriverDescriptor {
        device_driver: &PL011_UART,
        post_init_callback: Some(post_init_uart),
        irq_number: Some(PL011_UART_IRQ),
    };
    DRIVER_MANAGER.register_driver(uart_descriptor);

    let gpio_descriptor = DeviceDriverDescriptor {
        device_driver: &GPIO,
        post_init_callback: Some(post_init_gpio),
        irq_number: None,
    };
    DRIVER_MANAGER.register_driver(gpio_descriptor);

    let interrupt_controller_descriptor = DeviceDriverDescriptor {
        device_driver: &INTERRUPT_CONTROLLER,
        post_init_callback: Some(post_init_interrupt_controller),
        irq_number: None,
    };
    DRIVER_MANAGER.register_driver(interrupt_controller_descriptor);

    INIT_DONE.store(true, Ordering::Relaxed);
    Ok(())
}
