//! Time manipulation.
pub use std::time::Duration;
use super::sample::{SampleRate,NSamples};


/// Time base rational used in stream to calculate frame timestamps (see
/// ffmpeg's `AVStream.time_base`)
pub struct TimeBase {
    pub num: u32,
    pub den: u32,
}

impl TimeBase {
    /// Return a duration from provided timestamp (unit in rational time_base)
    pub fn ts_to_duration(&self, timestamp: i64) -> Duration {
        Duration::from_micros(timestamp as u64 * self.num as u64 * 1000000 / self.den as u64)
    }

    /// Return a timestamp from the provided duration (unit in time_base)
    pub fn duration_to_ts(&self, duration: Duration) -> i64 {
        duration.as_micros() as i64 * self.den as i64 / (self.num as i64  * 1000000)
    }
}

impl From<(u32,u32)> for TimeBase {
    fn from(value: (u32, u32)) -> TimeBase {
        Self {
            num: value.0,
            den: value.1,
        }
    }
}


impl From<(i32,i32)> for TimeBase {
    fn from(value: (i32, i32)) -> TimeBase {
        Self {
            num: value.0 as u32,
            den: value.1 as u32,
        }
    }
}


/// Time to NSamples
pub fn ts_to_samples(duration: Duration, rate: SampleRate) -> NSamples {
    (duration.as_micros() as NSamples ) * (rate as NSamples) / 1000000
}

/// NSamples to time
pub fn samples_to_ts(n_samples: NSamples, rate: SampleRate) -> Duration {
    Duration::from_micros(n_samples as u64  * 1000000 / rate as u64)
}

