use crate::console::interface::Console;
use crate::synchronization::interface::ReadWriteEx;
use crate::synchronization::InitStateLock;

pub mod null_console;

static CUR_CONSOLE: InitStateLock<&'static (dyn Console + Sync)> =
    InitStateLock::new(&null_console::NULL_CONSOLE);

/// Console interfaces.
pub mod interface {
    use core::fmt;

    /// Console write functions.
    pub trait Write {
        /// Write a single character.
        fn write_char(&self, c: char);

        /// Write a Rust format string.
        fn write_fmt(&self, args: fmt::Arguments) -> fmt::Result;

        /// Block until the last buffered character has been physically put on the TX wire.
        fn flush(&self);
    }

    /// Console read functions.
    pub trait Read {
        /// Read a single character.
        fn read_char(&self) -> char {
            ' '
        }

        /// Clear RX buffers, if any.
        fn clear_rx(&self);
    }

    /// Console statistics.
    pub trait Statistics {
        /// Return the number of characters written.
        fn chars_written(&self) -> usize {
            0
        }

        /// Return the number of characters read.
        fn chars_read(&self) -> usize {
            0
        }
    }

    /// Trait alias for a full-fledged console.
    pub trait Console: Write + Read + Statistics {}
}

/// Register a new console.
pub fn register_console(new_console: &'static (impl Console + Sync)) {
    CUR_CONSOLE.write(|con| *con = new_console);
}

/// Return a reference to the currently registered console.
///
/// This is the global console used by all printing macros.
pub fn console() -> &'static (dyn Console + Sync) {
    CUR_CONSOLE.read(|con| *con)
}
