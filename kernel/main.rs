#![feature(panic_info_message)]
#![feature(format_args_nl)]
#![feature(asm_const)]
#![no_main]
#![no_std]

use rp4os::*;

mod boot;

/// Early init code.
///
/// # Safety
///
/// - Only a single core must be active and running this function.
/// - The init calls in this function must appear in the correct order.
unsafe fn kernel_init() -> ! {
    exception::handling_init();

    memory::init();

    // Initialize the BSP driver subsystem.
    if let Err(x) = bsp::driver::init() {
        panic!("Error initializing BSP driver subsystem: {}", x);
    }

    // Initialize all device drivers.
    driver::DRIVER_MANAGER.init_drivers_and_irqs();
    // println! is usable from here on.
    
    bsp::memory::mmu::kernel_add_mapping_records_for_precomputed();

    // Unmask interrupts on the boot CPU core.
    exception_level::local_irq_unmask();

    // Announce conclusion of the kernel_init() phase.
    state::state_manager().transition_to_single_core_main();

    // Transition from unsafe to safe.
    kernel_main()
}

/// The main function running after the early init.
fn kernel_main() -> ! {
    info!(
        "[0] {} version {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );
    info!("[1] Booting on: {}", bsp::board_name());

    info!("MMU online:");
    memory::mmu::kernel_print_mappings();

    let (_, privilege_level) = exception_level::current_privilege_level();
    info!("Current privilege level: {}", privilege_level);

    info!("Exception handling state:");
    exception_level::print_state();

    info!(
        "Architectural timer resolution: {} ns",
        time::resolution().as_nanos()
    );

    info!("[2] Drivers loaded:");
    driver::DRIVER_MANAGER.enumerate();

    info!("Registered IRQ handlers:");
    exception::asynchronous::irq_manager().print_handler();

    info!("Echoing input now");
    cpu::wait_forever();
}
