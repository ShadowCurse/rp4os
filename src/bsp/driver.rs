use crate::{
    bsp::execption::PL011_UART_IRQ,
    console,
    driver::DeviceDriverDescriptor,
    driver::DRIVER_MANAGER,
    exception::asynchronous::set_irq_manager,
    memory::mmu::{kernel_map_mmio, MMIODescriptor},
};
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

static mut PL011_UART: MaybeUninit<super::drivers::bcm2xxx_pl011::PL011Uart> =
    MaybeUninit::uninit();
static mut GPIO: MaybeUninit<super::drivers::bcm2xxx_gpio::GPIO> = MaybeUninit::uninit();

static mut INTERRUPT_CONTROLLER: MaybeUninit<super::drivers::gicv2::GICv2> = MaybeUninit::uninit();

/// This must be called only after successful init of the memory subsystem.
unsafe fn instantiate_uart() -> Result<(), &'static str> {
    let mmio_descriptor = MMIODescriptor::new(
        crate::bsp::memory::map::mmio::PL011_UART_START,
        crate::bsp::memory::map::mmio::PL011_UART_SIZE,
    );
    let virt_addr = kernel_map_mmio(
        super::drivers::bcm2xxx_pl011::PL011Uart::COMPATIBLE,
        &mmio_descriptor,
    )?;

    PL011_UART.write(super::drivers::bcm2xxx_pl011::PL011Uart::new(virt_addr));

    Ok(())
}

/// This must be called only after successful init of the UART driver.
unsafe fn post_init_uart() -> Result<(), &'static str> {
    console::register_console(PL011_UART.assume_init_ref());
    Ok(())
}

/// This must be called only after successful init of the memory subsystem.
unsafe fn instantiate_gpio() -> Result<(), &'static str> {
    let mmio_descriptor = MMIODescriptor::new(
        crate::bsp::memory::map::mmio::GPIO_START,
        crate::bsp::memory::map::mmio::GPIO_SIZE,
    );
    let virt_addr = kernel_map_mmio(
        super::drivers::bcm2xxx_gpio::GPIO::COMPATIBLE,
        &mmio_descriptor,
    )?;

    GPIO.write(super::drivers::bcm2xxx_gpio::GPIO::new(virt_addr));

    Ok(())
}

/// This must be called only after successful init of the GPIO driver.
unsafe fn post_init_gpio() -> Result<(), &'static str> {
    GPIO.assume_init_ref().map_pl011_uart();
    Ok(())
}

/// This must be called only after successful init of the memory subsystem.
unsafe fn instantiate_interrupt_controller() -> Result<(), &'static str> {
    let gicd_mmio_descriptor = MMIODescriptor::new(
        crate::bsp::memory::map::mmio::GICD_START,
        crate::bsp::memory::map::mmio::GICD_SIZE,
    );
    let gicd_virt_addr = kernel_map_mmio("GICv2 GICD", &gicd_mmio_descriptor)?;

    let gicc_mmio_descriptor = MMIODescriptor::new(
        crate::bsp::memory::map::mmio::GICC_START,
        crate::bsp::memory::map::mmio::GICC_SIZE,
    );
    let gicc_virt_addr = kernel_map_mmio("GICV2 GICC", &gicc_mmio_descriptor)?;

    INTERRUPT_CONTROLLER.write(super::drivers::gicv2::GICv2::new(
        gicd_virt_addr,
        gicc_virt_addr,
    ));

    Ok(())
}

/// This must be called only after successful init of the interrupt controller driver.
unsafe fn post_init_interrupt_controller() -> Result<(), &'static str> {
    set_irq_manager(INTERRUPT_CONTROLLER.assume_init_ref());
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

    instantiate_uart()?;
    let uart_descriptor = DeviceDriverDescriptor {
        device_driver: PL011_UART.assume_init_ref(),
        post_init_callback: Some(post_init_uart),
        irq_number: Some(PL011_UART_IRQ),
    };
    DRIVER_MANAGER.register_driver(uart_descriptor);

    instantiate_gpio()?;
    let gpio_descriptor = DeviceDriverDescriptor {
        device_driver: GPIO.assume_init_ref(),
        post_init_callback: Some(post_init_gpio),
        irq_number: None,
    };
    DRIVER_MANAGER.register_driver(gpio_descriptor);

    instantiate_interrupt_controller()?;
    let interrupt_controller_descriptor = DeviceDriverDescriptor {
        device_driver: INTERRUPT_CONTROLLER.assume_init_ref(),
        post_init_callback: Some(post_init_interrupt_controller),
        irq_number: None,
    };
    DRIVER_MANAGER.register_driver(interrupt_controller_descriptor);

    INIT_DONE.store(true, Ordering::Relaxed);
    Ok(())
}
