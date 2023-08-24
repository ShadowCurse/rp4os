use crate::{
    exception::asynchronous::IRQNumber,
    info,
    synchronization::{InitStateLock, ReadWriteExclusive},
};

const NUM_DRIVERS: usize = 5;
pub static DRIVER_MANAGER: DriverManager<IRQNumber> = DriverManager::new();

/// Tpye to be used as an optional callback after a driver's init() has run.
pub type DeviceDriverPostInitCallback = unsafe fn() -> Result<(), &'static str>;

/// Driver interfaces.
pub mod interface {
    /// Device Driver functions.
    pub trait DeviceDriver {
        /// Different interrupt controllers might use different types for IRQ number.
        type IRQNumberType: core::fmt::Display;

        /// Return a compatibility string for identifying the driver.
        fn compatible(&self) -> &'static str;

        /// Called by the kernel to bring up the device.
        ///
        /// # Safety
        ///
        /// - During init, drivers might do stuff with system-wide impact.
        unsafe fn init(&self) -> Result<(), &'static str> {
            Ok(())
        }

        /// Called by the kernel to register and enable the device's IRQ handler.
        ///
        /// Rust's type system will prevent a call to this function unless the calling instance
        /// itself has static lifetime.
        fn register_and_enable_irq_handler(
            &'static self,
            irq_number: &Self::IRQNumberType,
        ) -> Result<(), &'static str> {
            panic!(
                "Attempt to enable IRQ {} for device {}, but driver does not support this",
                irq_number,
                self.compatible()
            )
        }
    }
}

/// A descriptor for device drivers.
#[derive(Copy, Clone)]
pub struct DeviceDriverDescriptor<T>
where
    T: 'static,
{
    pub device_driver: &'static (dyn interface::DeviceDriver<IRQNumberType = T> + Sync),
    pub post_init_callback: Option<DeviceDriverPostInitCallback>,
    pub irq_number: Option<T>,
}

struct DriverManagerInner<T>
where
    T: 'static,
{
    next_index: usize,
    descriptors: [Option<DeviceDriverDescriptor<T>>; NUM_DRIVERS],
}

impl<T> DriverManagerInner<T>
where
    T: 'static + Copy,
{
    /// Create an instance.
    pub const fn new() -> Self {
        Self {
            next_index: 0,
            descriptors: [None; NUM_DRIVERS],
        }
    }
}

/// Provides device driver management functions.
pub struct DriverManager<T>
where
    T: 'static,
{
    inner: InitStateLock<DriverManagerInner<T>>,
}

impl<T> DriverManager<T>
where
    T: core::fmt::Display + Copy,
{
    /// Create an instance.
    pub const fn new() -> Self {
        Self {
            inner: InitStateLock::new(DriverManagerInner::new()),
        }
    }

    /// Register a device driver with the kernel.
    pub fn register_driver(&self, descriptor: DeviceDriverDescriptor<T>) {
        self.inner.write(|inner| {
            inner.descriptors[inner.next_index] = Some(descriptor);
            inner.next_index += 1;
        })
    }

    /// Helper for iterating over registered drivers.
    fn for_each_descriptor<'a>(&'a self, f: impl FnMut(&'a DeviceDriverDescriptor<T>)) {
        self.inner.read(|inner| {
            inner
                .descriptors
                .iter()
                .filter_map(|x| x.as_ref())
                .for_each(f)
        })
    }

    /// Fully initialize all drivers and their interrupts handlers.
    ///
    /// # Safety
    ///
    /// - During init, drivers might do stuff with system-wide impact.
    pub unsafe fn init_drivers_and_irqs(&self) {
        self.for_each_descriptor(|descriptor| {
            // 1. Initialize driver.
            if let Err(x) = descriptor.device_driver.init() {
                panic!(
                    "Error initializing driver: {}: {}",
                    descriptor.device_driver.compatible(),
                    x
                );
            }

            // 2. Call corresponding post init callback.
            if let Some(callback) = &descriptor.post_init_callback {
                if let Err(x) = callback() {
                    panic!(
                        "Error during driver post-init callback: {}: {}",
                        descriptor.device_driver.compatible(),
                        x
                    );
                }
            }
        });

        // 3. After all post-init callbacks were done, the interrupt controller should be
        //    registered and functional. So let drivers register with it now.
        self.for_each_descriptor(|descriptor| {
            if let Some(irq_number) = &descriptor.irq_number {
                if let Err(x) = descriptor
                    .device_driver
                    .register_and_enable_irq_handler(irq_number)
                {
                    panic!(
                        "Error during driver interrupt handler registration: {}: {}",
                        descriptor.device_driver.compatible(),
                        x
                    );
                }
            }
        });
    }

    /// Enumerate all registered device drivers.
    pub fn enumerate(&self) {
        let mut i: usize = 1;
        self.for_each_descriptor(|descriptor| {
            info!("{}. {}", i, descriptor.device_driver.compatible());

            i += 1;
        });
    }
}
