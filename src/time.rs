#[path = "arch/aarch64/time.rs"]
mod arch_time;

use core::{num::NonZeroU64, time::Duration};

const NANOSEC_PER_SEC: NonZeroU64 = NonZeroU64::new(1_000_000_000).unwrap();

pub fn uptime() -> Duration {
    arch_time::uptime()
}

pub fn resolution() -> Duration {
    arch_time::resolution()
}

pub fn spin_for(duration: Duration) -> Result<(), &'static str> {
    arch_time::spin_for(duration)
}
