#![feature(panic_info_message)]
#![feature(format_args_nl)]
#![feature(asm_const)]
#![no_main]
#![no_std]

mod bsp;
mod console;
mod cpu;
mod driver;
mod panic;
mod print;
mod synchronization;

/// Early init code.
///
/// # Safety
///
/// - Only a single core must be active and running this function.
/// - The init calls in this function must appear in the correct order.
unsafe fn kernel_init() -> ! {
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

    println!("Loaded on {:^37}", bsp::board_name());
    println!("[ML] Requesting binary");
    console().flush();

    // Discard any spurious received characters before starting with the loader protocol.
    console().clear_rx();

    // Notify `Minipush` to send the binary.
    for _ in 0..3 {
        console().write_char(3 as char);
    }

    // Read the binary's size.
    let mut size: u32 = u32::from(console().read_char() as u8);
    size |= u32::from(console().read_char() as u8) << 8;
    size |= u32::from(console().read_char() as u8) << 16;
    size |= u32::from(console().read_char() as u8) << 24;

    // Trust it's not too big.
    console().write_char('O');
    console().write_char('K');

    let kernel_addr: *mut u8 = bsp::memory::board_default_load_addr() as *mut u8;
    unsafe {
        // Read the kernel byte by byte.
        for i in 0..size {
            core::ptr::write_volatile(kernel_addr.offset(i as isize), console().read_char() as u8)
        }
    }

    println!("[ML] Loaded! Executing the payload now\n");
    console().flush();

    // Use black magic to create a function pointer.
    let kernel: fn() -> ! = unsafe { core::mem::transmute(kernel_addr) };

    // Jump to loaded kernel!
    kernel()
}
