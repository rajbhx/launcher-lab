//! Wall-clock time for the status bar and the OnePlus clock/weather widget.
//!
//! On Android the local time comes straight from `localtime_r` (bionic), so
//! the demo follows the device time zone without pulling in a calendar crate.
//! On the host the same structures are filled from a pure UTC civil-date
//! algorithm (which is also unit-tested below).

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimeParts {
    pub hour: u32,
    pub minute: u32,
    /// 0 = Sunday .. 6 = Saturday
    pub wday: u32,
    /// 1..=12
    pub mon: u32,
    pub mday: u32,
}

/// Current local wall-clock time.
#[cfg(target_os = "android")]
pub fn now() -> TimeParts {
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Tm {
        tm_sec: i32,
        tm_min: i32,
        tm_hour: i32,
        tm_mday: i32,
        tm_mon: i32,
        tm_year: i32,
        tm_wday: i32,
        tm_yday: i32,
        tm_isdst: i32,
        tm_gmtoff: i64,
        tm_zone: *const core::ffi::c_char,
    }

    extern "C" {
        fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
    }

    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let mut tm: Tm = unsafe { core::mem::zeroed() };
    let ok = unsafe { !localtime_r(&secs, &mut tm).is_null() };
    if ok {
        TimeParts {
            hour: tm.tm_hour.clamp(0, 23) as u32,
            minute: tm.tm_min.clamp(0, 59) as u32,
            wday: tm.tm_wday.clamp(0, 6) as u32,
            mon: (tm.tm_mon + 1).clamp(1, 12) as u32,
            mday: tm.tm_mday.clamp(1, 31) as u32,
        }
    } else {
        utc_from_epoch(secs)
    }
}

/// Desktop/dev fallback: UTC.
#[cfg(not(target_os = "android"))]
pub fn now() -> TimeParts {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    utc_from_epoch(secs)
}

/// Howard Hinnant's `civil_from_days`, plus clock time. Pure and testable.
pub fn utc_from_epoch(secs: i64) -> TimeParts {
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let hour = (tod / 3600) as u32;
    let minute = ((tod % 3600) / 60) as u32;

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { year + 1 } else { year };
    let _ = year;

    // 1970-01-01 was a Thursday (wday = 4 with 0 = Sunday).
    let wday = (days.rem_euclid(7) + 4) % 7;

    TimeParts {
        hour,
        minute,
        wday: wday as u32,
        mon: m as u32,
        mday: d as u32,
    }
}

pub const WDAY: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
pub const MON: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// `"MON, SEP 23"` — the widget's upper line.
pub fn date_label(t: TimeParts) -> String {
    let w = WDAY[(t.wday as usize).min(6)];
    let m = MON[(t.mon.max(1) as usize - 1).min(11)];
    format!("{w}, {m} {}", t.mday)
}

/// `"12:04"` — 24h, zero-padded, like the review-unit shots.
pub fn clock_label(t: TimeParts) -> String {
    format!("{:02}:{:02}", t.hour, t.minute)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_is_thursday_jan_1970() {
        let t = utc_from_epoch(0);
        assert_eq!((t.mon, t.mday, t.wday, t.hour, t.minute), (1, 1, 4, 0, 0));
        assert_eq!(date_label(t), "THU, JAN 1");
        assert_eq!(clock_label(t), "00:00");
    }

    #[test]
    fn known_instant_2023_11_14() {
        // 1700000000 = 2023-11-14T22:13:20Z (a Tuesday).
        let t = utc_from_epoch(1_700_000_000);
        assert_eq!((t.mon, t.mday, t.wday), (11, 14, 2));
        assert_eq!(clock_label(t), "22:13");
        assert_eq!(date_label(t), "TUE, NOV 14");
    }

    #[test]
    fn leap_day_2020() {
        // 2020-02-29T07:05:00Z = 1582959900
        let t = utc_from_epoch(1_582_959_900);
        assert_eq!((t.mon, t.mday, t.wday), (2, 29, 6));
        assert_eq!(clock_label(t), "07:05");
    }
}
