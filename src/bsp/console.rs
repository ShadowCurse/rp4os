use crate::console;

pub fn console() -> &'static impl console::interface::Console {
    &super::driver::PL011_UART
}
