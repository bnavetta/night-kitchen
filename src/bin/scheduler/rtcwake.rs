//! Functions to configure the RTC wake alarm

use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind};
use std::mem::MaybeUninit;
use std::os::unix::io::AsRawFd;

use anyhow::{anyhow, Context, Error, Result};
use chrono::{
    DateTime, Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Timelike, Utc,
};
use libc::{c_int, c_uchar};

// These constants are based on <linux/rtc.h>
const RTC_IOCTL_IDENTIFIER: u8 = b'p';
const RTC_WKALRM_SET: u8 = 0x0f;
const RTC_WKALRM_RD: u8 = 0x10;

#[repr(C)]
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
struct RtcTime {
    /// Seconds (0-60)
    tm_sec: c_int,
    /// Minutes (0-59)
    tm_min: c_int,
    /// Hours (0-23)
    tm_hour: c_int,
    /// Day of the month (1-31)
    tm_mday: c_int,
    /// Month (0-11)
    tm_mon: c_int,
    /// Year - 1900
    tm_year: c_int,
    /// Day of the week (0-6, Sunday = 0)
    /// This is unused
    tm_wday: c_int,
    /// Day in the year (0-365, January 1st = 0)
    /// This is unused
    tm_yday: c_int,
    /// Daylight savings time
    /// This is unused
    tm_isdst: c_int,
}

impl RtcTime {
    /// Converts a RTC time to a Chrono time. This does not include timezone information, because the RTC could be set to either UTC or
    /// the local timezone.
    pub fn to_chrono(&self) -> NaiveDateTime {
        // See https://en.wikipedia.org/wiki/ISO_8601#Dates and man:gmtime(3) for the conversion
        let date = NaiveDate::from_ymd(
            self.tm_year + 1900,
            self.tm_mon as u32 + 1,
            self.tm_mday as u32,
        );
        // Linux handles leap seconds by setting tm_sec to 60, but Chrono handles them with large fractional seconds.
        let time = if self.tm_sec == 60 {
            NaiveTime::from_hms_milli(self.tm_hour as u32, self.tm_min as u32, 59, 1999)
        } else {
            NaiveTime::from_hms(self.tm_hour as u32, self.tm_min as u32, self.tm_sec as u32)
        };
        NaiveDateTime::new(date, time)
    }

    /// Converts a Chrono time to a RTC time.
    pub fn from_chrono(dt: &NaiveDateTime) -> RtcTime {
        RtcTime {
            tm_sec: if dt.timestamp_subsec_millis() > 999 {
                60
            } else {
                dt.second() as c_int
            },
            tm_min: dt.minute() as c_int,
            tm_hour: dt.hour() as c_int,
            tm_mday: dt.day() as c_int,
            tm_mon: dt.month0() as c_int,
            tm_year: dt.year() - 1900,
            tm_wday: 0,
            tm_yday: 0,
            tm_isdst: 0,
        }
    }
}

/// RTC wake alarm configuration
#[repr(C)]
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct RtcWakeAlarm {
    enabled: c_uchar,
    pending: c_uchar,
    time: RtcTime,
}

impl RtcWakeAlarm {
    /// Is the wake alarm enabled?
    pub fn enabled(&self) -> bool {
        self.enabled != 0
    }

    /// When will the alarm go off?
    pub fn time(&self) -> NaiveDateTime {
        self.time.to_chrono()
    }

    /// Set whether the alarm should be enabled or disabled
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = if enabled { 1 } else { 0 }
    }

    /// Sets the time at which the alarm should go off
    pub fn set_time(&mut self, time: &NaiveDateTime) {
        self.time = RtcTime::from_chrono(time)
    }
}

impl fmt::Display for RtcWakeAlarm {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "RTC alarm at {} ({})",
            self.time(),
            if self.enabled() {
                "enabled"
            } else {
                "disabled"
            }
        )
    }
}

ioctl_read! {
    /// Read the RTC's wake alarm time. Note that not all RTCs support this interface, some use the less-powerful
    /// `RTC_ALM_READ` ioctl instead.
    ///
    /// See [`man:rtc(4)`](http://man7.org/linux/man-pages/man4/rtc.4.html) for more information.
    rtc_read_wake_alarm, RTC_IOCTL_IDENTIFIER, RTC_WKALRM_RD, RtcWakeAlarm
}

ioctl_write_ptr! {
    /// Configure the RTC's wake alarm. Note that not all RTCs support this interface, some use `RTC_ALM_SET`
    /// and `RTC_AIE_ON/OFF` instead.
    /// See [`man:rtc(4)`](http://man7.org/linux/man-pages/man4/rtc.4.html) for more information.
    rtc_set_wake_alarm, RTC_IOCTL_IDENTIFIER, RTC_WKALRM_SET, RtcWakeAlarm
}

/// Linux RTC driver
///
/// See [`man:rtc(4)`](http://man7.org/linux/man-pages/man4/rtc.4.html) for details
pub struct Rtc {
    device_file: File,
}

impl Rtc {
    /// Creates a new RTC driver
    pub fn new() -> Result<Rtc> {
        let file = File::open("/dev/rtc0").context("Could not open RTC device file /dev/rtc0")?;
        Ok(Rtc { device_file: file })
    }

    /// Read the current RTC wake alarm configuration
    pub fn alarm_configuration(&self) -> Result<RtcWakeAlarm> {
        let mut alarm = MaybeUninit::<RtcWakeAlarm>::uninit();
        unsafe {
            rtc_read_wake_alarm(self.device_file.as_raw_fd(), alarm.as_mut_ptr())
                .context("RTC_WKALRM_RD ioctl failed")?;
            Ok(alarm.assume_init())
        }
    }

    /// Configure the RTC wake alarm
    pub fn set_alarm_configuration(&self, alarm: &RtcWakeAlarm) -> Result<()> {
        unsafe {
            rtc_set_wake_alarm(self.device_file.as_raw_fd(), alarm as *const RtcWakeAlarm)
                .context("RTC_WKALRM_SET ioctl failed")?;
        }
        Ok(())
    }

    /// Gets the hardware clock time. This is determined by `/etc/adjtime`, not the RTC itself.
    ///
    /// See [`man:adjtime(5)`](http://man7.org/linux/man-pages/man5/adjtime.5.html).
    pub fn read_clock_mode() -> Result<ClockMode> {
        match File::open("/etc/adjtime") {
            Ok(file) => {
                let reader = BufReader::new(file);
                let mut lines = reader.lines();
                let mode_line = lines
                    .nth(2)
                    .ok_or_else(|| anyhow!("Invalid /etc/adjtime"))??;
                match mode_line.trim() {
                    "UTC" => Ok(ClockMode::Utc),
                    "LOCAL" => Ok(ClockMode::Local),
                    other => Err(anyhow!("Invalid clock mode: {}", other)),
                }
            }
            // If /etc/adjtime does not exist, the default is UTC
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(ClockMode::Utc),
            Err(e) => Err(Error::from(e)).context("Could not access /etc/adjtime"),
        }
    }
}

/// Hardware clock mode (which timezone the clock uses)
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum ClockMode {
    /// The clock is in UTC
    Utc,
    /// The clock is in the local timezone
    Local,
}

impl ClockMode {
    /// Converts a NaiveDateTime from the hardware clock to a UTC DateTime
    pub fn to_datetime(self, hardware_time: &NaiveDateTime) -> DateTime<Utc> {
        match self {
            ClockMode::Utc => Utc.from_utc_datetime(hardware_time),
            ClockMode::Local => Local
                .from_local_datetime(hardware_time)
                .unwrap()
                .with_timezone(&Utc),
        }
    }

    /// Converts a UTC DateTime to a NaiveDateTime in the hardware clock timezone
    pub fn to_hardware(self, dt: &DateTime<Utc>) -> NaiveDateTime {
        match self {
            ClockMode::Utc => dt.naive_utc(),
            ClockMode::Local => dt.with_timezone(&Local).naive_local(),
        }
    }
}
