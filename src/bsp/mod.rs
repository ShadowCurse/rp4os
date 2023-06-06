pub mod drivers;
pub mod console;
pub mod cpu;
pub mod driver;
pub mod memory;

pub fn board_name() -> &'static str {
    "Raspberry Pi 4"
}
