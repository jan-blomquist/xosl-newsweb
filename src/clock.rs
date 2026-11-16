//! The current time, read through a trait so that tests set it.

/// A source of the current time.
pub trait Clock {
    /// The current time as nanoseconds since 1970-01-01 UTC.
    fn now_ns(&self) -> i64;
}

/// The system's clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ns(&self) -> i64 {
        // i64 nanoseconds reach the year 2262.
        i64::try_from(jiff::Timestamp::now().as_nanosecond()).unwrap_or(i64::MAX)
    }
}
