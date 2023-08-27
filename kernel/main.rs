#![feature(panic_info_message)]
#![feature(format_args_nl)]
#![feature(asm_const)]
#![no_main]
#![no_std]

extern crate alloc;

use alloc::vec;
use rp4os::*;

mod boot;

/// Early init code.
///
/// # Safety
///
/// - Only a single core must be active and running this function.
/// - The init calls in this function must appear in the correct order.
unsafe fn kernel_init() -> ! {
    exception::set_exception_vector();

    let phys_kernel_tables_base_addr = match memory::mmu::kernel_map_binary() {
        Err(string) => panic!("Error mapping kernel binary: {}", string),
        Ok(addr) => addr,
    };

    if let Err(e) = memory::mmu::enable_mmu_and_caching(phys_kernel_tables_base_addr) {
        panic!("Enabling MMU failed: {}", e);
    }

    memory::post_enable_init();

    // Initialize the BSP driver subsystem.
    if let Err(x) = bsp::driver::init() {
        panic!("Error initializing BSP driver subsystem: {}", x);
    }

    // Initialize all device drivers.
    driver::DRIVER_MANAGER.init_drivers_and_irqs();
    // println! is usable from here on.

    // Unmask interrupts on the boot CPU core.
    exception::local_irq_unmask();

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

    let exception_level = exception::ExceptionLevel::current_level();
    info!("Current privilege level: {:?}", exception_level);

    info!("Exception handling state:");
    exception::print_exception_state();

    info!(
        "Architectural timer resolution: {} ns",
        time::resolution().as_nanos()
    );

    info!("[2] Drivers loaded:");
    driver::DRIVER_MANAGER.enumerate();

    info!("Registered IRQ handlers:");
    exception::asynchronous::irq_manager().print_handler();

    info!("Kernel heap:");
    memory::heap_alloc::KERNEL_HEAP_ALLOCATOR.print_usage();

    {
        let _numbers = vec![1, 2, 3, 4];

        info!("Kernel heap:");
        memory::heap_alloc::KERNEL_HEAP_ALLOCATOR.print_usage();
    }

    info!("Kernel heap:");
    memory::heap_alloc::KERNEL_HEAP_ALLOCATOR.print_usage();

    info!("Echoing input now");
    cpu::wait_forever();
}
