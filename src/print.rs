use crate::console;
use core::fmt;

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    console::console().write_fmt(args).unwrap();
}

/// Prints without a newline.
///
/// Carbon copy from <https://doc.rust-lang.org/src/std/macros.rs.html>
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::print::_print(format_args!($($arg)*)));
}

/// Prints with a newline.
///
/// Carbon copy from <https://doc.rust-lang.org/src/std/macros.rs.html>
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ({
        $crate::print::_print(format_args_nl!($($arg)*));
    })
}

/// Prints an info, with a newline.
#[macro_export]
macro_rules! info {
    ($string:expr) => ({
        let timestamp = $crate::time::uptime();

        $crate::print::_print(format_args_nl!(
            concat!("[{:#?}] ", $string),
            timestamp,
        ));
    });
    ($format_string:expr, $($arg:tt)*) => ({
        let timestamp = $crate::time::uptime();

        $crate::print::_print(format_args_nl!(
            concat!("[{:#?}] ", $format_string),
            timestamp,
            $($arg)*
        ));
    })
}

/// Prints a warning, with a newline.
#[macro_export]
macro_rules! warn {
    ($string:expr) => ({
        let timestamp = $crate::time::uptime();

        $crate::print::_print(format_args_nl!(
            concat!("[{:#?}] WARN ", $string),
            timestamp,
        ));
    });
    ($format_string:expr, $($arg:tt)*) => ({
        let timestamp = $crate::time::uptime();

        $crate::print::_print(format_args_nl!(
            concat!("[{:#?}] WARN ", $format_string),
            timestamp,
            $($arg)*
        ));
    })
}
