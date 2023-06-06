//! BSP driver support.

use super::{
    drivers::{bcm2xxx_gpio::GPIO, bcm2xxx_pl011::PL011Uart},
    memory::map::mmio,
};
use crate::{console, driver as generic_driver};
use core::sync::atomic::{AtomicBool, Ordering};

pub static PL011_UART: PL011Uart = unsafe { PL011Uart::new(mmio::PL011_UART_START) };
pub static GPIO: GPIO = unsafe { GPIO::new(mmio::GPIO_START) };

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

fn register_driver_uart() -> Result<(), &'static str> {
    let uart_descriptor =
        generic_driver::DeviceDriverDescriptor::new(&PL011_UART, Some(post_init_uart));
    generic_driver::driver_manager().register_driver(uart_descriptor);
    Ok(())
}

fn register_driver_gpio() -> Result<(), &'static str> {
    let gpio_descriptor = generic_driver::DeviceDriverDescriptor::new(&GPIO, Some(post_init_gpio));
    generic_driver::driver_manager().register_driver(gpio_descriptor);
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

    register_driver_uart()?;
    register_driver_gpio()?;

    INIT_DONE.store(true, Ordering::Relaxed);
    Ok(())
}
