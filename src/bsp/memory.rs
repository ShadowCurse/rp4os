/// The board's physical memory map.
#[rustfmt::skip]
pub(super) mod map {
    pub const BOARD_DEFAULT_LOAD_ADDRESS: usize =        0x8_0000;

    pub const GPIO_OFFSET:         usize = 0x0020_0000;
    pub const UART_OFFSET:         usize = 0x0020_1000;

    /// Physical devices.
    pub mod mmio {
        use super::*;

        pub const START:            usize =         0xFE00_0000;
        pub const GPIO_START:       usize = START + GPIO_OFFSET;
        pub const PL011_UART_START: usize = START + UART_OFFSET;
    }
}

/// The address on which the Raspberry firmware loads every binary by default.
#[inline(always)]
pub fn board_default_load_addr() -> *const u64 {
    map::BOARD_DEFAULT_LOAD_ADDRESS as _
}
