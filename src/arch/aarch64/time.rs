use aarch64_cpu::{asm::barrier, registers::*};
use core::{
    num::{NonZeroU128, NonZeroU32, NonZeroU64},
    ops::{Add, Div},
    time::Duration,
};
use tock_registers::interfaces::Readable;

const NANOSEC_PER_SEC: NonZeroU64 = NonZeroU64::new(1_000_000_000).unwrap();

/// Boot assembly code overwrites this value with the value of CNTFRQ_EL0 before any Rust code is
/// executed. This given value here is just a (safe) dummy.
#[no_mangle]
static ARCH_TIMER_COUNTER_FREQUENCY: NonZeroU32 = NonZeroU32::MIN;

/// The timer's resolution.
pub fn resolution() -> Duration {
    Duration::from(TimerCounter(1))
}

/// The uptime since power-on of the device.
///
/// This includes time consumed by firmware and bootloaders.
pub fn uptime() -> Duration {
    TimerCounter::from_cntpct().into()
}

/// Spin for a given duration.
pub fn spin_for(duration: Duration) -> Result<(), &'static str> {
    let curr_timer = TimerCounter::from_cntpct();

    let duration: TimerCounter = duration.try_into()?;
    let timer_target = curr_timer + duration;

    while TimerCounter::from_cntpct_direct() < timer_target {}

    Ok(())
}

fn arch_timer_counter_frequency() -> NonZeroU32 {
    // Read volatile is needed here to prevent the compiler from optimizing
    // ARCH_TIMER_COUNTER_FREQUENCY away.
    //
    // This is safe, because all the safety requirements as stated in read_volatile()'s
    // documentation are fulfilled.
    unsafe { core::ptr::read_volatile(&ARCH_TIMER_COUNTER_FREQUENCY) }
}

#[derive(Copy, Clone, PartialOrd, PartialEq)]
pub struct TimerCounter(u64);

impl TimerCounter {
    pub const MAX: Self = Self(u64::MAX);

    /// Reads CNTPCT_EL0 register.
    /// Waits for barier to prevent read ahead of time.
    #[inline(always)]
    pub fn from_cntpct() -> Self {
        // Prevent that the counter is read ahead of time due to out-of-order execution.
        barrier::isb(barrier::SY);
        let cnt = CNTPCT_EL0.get();
        Self(cnt)
    }

    /// Reads CNTPCT_EL0 register.
    /// Does not wait for a barier.
    #[inline(always)]
    pub fn from_cntpct_direct() -> Self {
        let cnt = CNTPCT_EL0.get();
        Self(cnt)
    }
}

impl Add for TimerCounter {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        TimerCounter(self.0.wrapping_add(other.0))
    }
}

impl From<TimerCounter> for Duration {
    fn from(timer: TimerCounter) -> Self {
        if timer.0 == 0 {
            return Duration::ZERO;
        }

        let frequency: NonZeroU64 = arch_timer_counter_frequency().into();

        // Div<NonZeroU64> implementation for u64 cannot panic.
        let secs = timer.0.div(frequency);

        // This is safe, because frequency can never be greater than u32::MAX, which means the
        // largest theoretical value for sub_second_counter_value is (u32::MAX - 1). Therefore,
        // (sub_second_counter_value * NANOSEC_PER_SEC) cannot overflow an u64.
        //
        // The subsequent division ensures the result fits into u32, since the max result is smaller
        // than NANOSEC_PER_SEC. Therefore, just cast it to u32 using `as`.
        let sub_second_counter_value = timer.0 % frequency;
        let nanos = unsafe { sub_second_counter_value.unchecked_mul(u64::from(NANOSEC_PER_SEC)) }
            .div(frequency) as u32;

        Duration::new(secs, nanos)
    }
}

impl TryFrom<Duration> for TimerCounter {
    type Error = &'static str;

    fn try_from(duration: Duration) -> Result<Self, Self::Error> {
        if duration < resolution() {
            return Ok(TimerCounter(0));
        }

        if duration > Duration::from(TimerCounter::MAX) {
            return Err("Conversion error. Duration too big");
        }

        let frequency: u128 = u32::from(arch_timer_counter_frequency()) as u128;
        let duration: u128 = duration.as_nanos();

        // This is safe, because frequency can never be greater than u32::MAX, and
        // (Duration::MAX.as_nanos() * u32::MAX) < u128::MAX.
        let counter_value =
            unsafe { duration.unchecked_mul(frequency) }.div(NonZeroU128::from(NANOSEC_PER_SEC));

        // Since we checked above that we are <= max_duration(), just cast to u64.
        Ok(TimerCounter(counter_value as u64))
    }
}
