#![feature(panic_info_message)]
#![feature(format_args_nl)]
#![feature(asm_const)]
#![no_main]
#![no_std]

use rp4os::*;

mod boot;

const KERNEL_LOAD_START_SIGNAL: u8 = 0x01;
const KERNEL_LOAD_SIZE_ACK_SIGNAL: u8 = 0x02;
const KERNEL_LOAD_ACK_SIGNAL: u8 = 0x03;

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
    driver::DRIVER_MANAGER.init_drivers_and_irqs();
    // println! is usable from here on.

    // Transition from unsafe to safe.
    kernel_main()
}

/// The main function running after the early init.
fn kernel_main() -> ! {
    println!("[Loader] Loaded on {:^37}", bsp::board_name());
    println!("[Loader]  Waiting for ready signal...");

    let console = console::console();
    console.flush();

    // Discard any spurious received characters before starting with the loader protocol.
    console.clear_rx();

    // Wait for ready signal
    while console.read_char() as u8 != KERNEL_LOAD_START_SIGNAL {}

    // Read the binary's size.
    let mut size: u32 = u32::from(console.read_char() as u8);
    size |= u32::from(console.read_char() as u8) << 8;
    size |= u32::from(console.read_char() as u8) << 16;
    size |= u32::from(console.read_char() as u8) << 24;

    // Ack signal
    console.write_char(KERNEL_LOAD_SIZE_ACK_SIGNAL as char);

    let kernel_addr: *mut u8 = bsp::memory::board_default_load_addr() as *mut u8;
    unsafe {
        // Read the kernel byte by byte.
        for i in 0..size {
            core::ptr::write_volatile(kernel_addr.offset(i as isize), console.read_char() as u8)
        }
    }

    // Ack signal
    console.write_char(KERNEL_LOAD_ACK_SIGNAL as char);
    console.flush();

    println!("[Loader]  Loaded! Executing the payload now\n");

    // Use black magic to create a function pointer.
    let kernel: fn() -> ! = unsafe { core::mem::transmute(kernel_addr) };

    // Jump to loaded kernel!
    kernel()
}
