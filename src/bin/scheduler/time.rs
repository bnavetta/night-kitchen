//! Helpers for dealing with time, especially in relation to systemd
use std::convert::TryInto;

use chrono::{DateTime, Utc, TimeZone};
use libc;

/// Converts a timestamp represented as microseconds since the UTC UNIX epoch to a `DateTime`. 
pub fn from_timestamp_usecs(usecs: u64) -> DateTime<Utc> {
    Utc.timestamp_nanos((usecs * 1000) as i64)
}


// Use the same approach as systemd for converting between CLOCK_MONOTONIC and CLOCK_REALTIME timestamps.
// The basic idea is to get the current time with both clocks, and then use the difference as an offset for conversion
// See dual_clock_get in https://github.com/systemd/systemd/blob/master/src/basic/time-util.c#L66 and
// calc_next_elapse in https://github.com/systemd/systemd/blob/master/src/systemctl/systemctl.c#L1295

pub fn monotonic_to_realtime(monotonic: DateTime<Utc>) -> DateTime<Utc> {
    // Could be off by a tiny amount because the two calls don't happen at the same time, but it's probably not enough to notice.
    // These don't need to be recalculated every time but also can't be stored forever because of clock skew / NTP, so it's easier not to cache them
    let monotonic_now = clock_gettime(libc::CLOCK_MONOTONIC);
    let realtime_now = clock_gettime(libc::CLOCK_REALTIME);

    let monotonic_now = Utc.timestamp(monotonic_now.tv_sec, monotonic_now.tv_nsec.try_into().unwrap());
    let realtime_now = Utc.timestamp(realtime_now.tv_sec, realtime_now.tv_nsec.try_into().unwrap());
    monotonic + (realtime_now - monotonic_now)
}

fn clock_gettime(clock: libc::clockid_t) -> libc::timespec {
    let mut timespec = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    let status = unsafe { libc::clock_gettime(clock, &mut timespec as *mut _) };
    if status != 0 {
        panic!("clock_gettime failed!");
    }
    timespec
}