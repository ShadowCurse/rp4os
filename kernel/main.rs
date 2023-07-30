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
    use rp4os::mmu::interface::MMU;

    if let Err(string) = mmu::mmu().enable_mmu_and_caching() {
        panic!("MMU: {}", string);
    }

    // Initialize the BSP driver subsystem.
    if let Err(x) = bsp::driver::init() {
        panic!("Error initializing BSP driver subsystem: {}", x);
    }

    // Initialize all device drivers.
    driver::driver_manager().init_drivers();
    // println! is usable from here on.

    // Transition from unsafe to safe.
    kernel_main()
}

/// The main function running after the early init.
fn kernel_main() -> ! {
    use console::console;

    info!(
        "[0] {} version {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );
    info!("[1] Booting on: {}", bsp::board_name());

    info!("MMU online. Special regions:");
    bsp::memory::mmu::LAYOUT.print_layout();

    let (_, privilege_level) = priv_level::current_privilege_level();
    info!("Current privilege level: {}", privilege_level);

    info!("Exception handling state:");
    priv_level::print_state();

    info!(
        "Architectural timer resolution: {} ns",
        time::resolution().as_nanos()
    );

    info!("[2] Drivers loaded:");
    driver::driver_manager().enumerate();

    {
        use rp4os::console::interface::Write;
        let remapped_uart = unsafe { bsp::drivers::bcm2xxx_pl011::PL011Uart::new(0x1FFF_1000) };
        writeln!(
            remapped_uart,
            "[     !!!    ] Writing through the remapped UART at 0x1FFF_1000"
        )
        .unwrap();
    }

    info!("[3] Chars written: {}", console().chars_written());
    info!("[4] Echoing input now");

    // Discard any spurious received characters before going into echo mode.
    console().clear_rx();
    loop {
        let c = console().read_char();
        console().write_char(c);
    }
}
