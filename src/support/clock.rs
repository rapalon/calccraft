//! The policy's clock: the local calendar day, or minutes in the demo mode
//! (`CALCCRAFT_SUPPORT_FAST=1`, docs/support-prompt.md section 6).

use super::policy::Timestamp;
use chrono::{Datelike, Local, TimeZone};

/// Whether the demo mode is on: `CALCCRAFT_SUPPORT_FAST` is `1` or `true`. The browser has no
/// environment, so the web version never is.
pub fn fast_mode() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("CALCCRAFT_SUPPORT_FAST").is_ok_and(|v| v == "1" || v == "true")
    }
    #[cfg(target_arch = "wasm32")]
    {
        false
    }
}

/// A demo "day" in seconds: a minute, or `CALCCRAFT_SUPPORT_DAY_SECS`.
fn fast_day_secs() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("CALCCRAFT_SUPPORT_DAY_SECS")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .filter(|&secs: &u64| secs > 0)
            .unwrap_or(60)
    }
    #[cfg(target_arch = "wasm32")]
    {
        60
    }
}

/// Real time, or the demo mode's fast days.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    /// `Some(seconds per day)` in the demo mode.
    fast_day: Option<u64>,
    /// The tests' time.
    #[cfg(test)]
    pub fixed: Option<Timestamp>,
}

impl Clock {
    pub fn from_env() -> Self {
        Self {
            fast_day: fast_mode().then(fast_day_secs),
            #[cfg(test)]
            fixed: None,
        }
    }

    /// A clock stopped at `now`, which the tests move.
    #[cfg(test)]
    pub fn fixed(now: Timestamp) -> Self {
        Self {
            fast_day: None,
            fixed: Some(now),
        }
    }

    pub fn is_fast(&self) -> bool {
        self.fast_day.is_some()
    }

    /// The day number of a Unix time in seconds.
    pub fn day_of(&self, secs: u64) -> i64 {
        match self.fast_day {
            Some(day) => (secs / day) as i64,
            None => Local
                .timestamp_opt(secs as i64, 0)
                .single()
                .map_or((secs / 86_400) as i64, |time| {
                    i64::from(time.date_naive().num_days_from_ce())
                }),
        }
    }

    pub fn now(&self) -> Timestamp {
        #[cfg(test)]
        if let Some(now) = self.fixed {
            return now;
        }
        let millis = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        Timestamp {
            millis,
            day: self.day_of(millis / 1000),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_days_are_minutes() {
        let clock = Clock {
            fast_day: Some(60),
            fixed: None,
        };
        assert!(clock.is_fast());
        assert_eq!(clock.day_of(0), 0);
        assert_eq!(clock.day_of(59), 0);
        assert_eq!(clock.day_of(60), 1);
        assert_eq!(clock.day_of(7 * 60), 7);
    }

    #[test]
    fn real_days_are_local_dates() {
        let clock = Clock {
            fast_day: None,
            fixed: None,
        };
        let a = clock.day_of(1_700_000_000);
        // A day later is the next date, a minute later (almost always) the same.
        assert_eq!(clock.day_of(1_700_000_000 + 86_400), a + 1);
        let now = clock.now();
        assert_eq!(
            now.day,
            i64::from(Local::now().date_naive().num_days_from_ce())
        );
    }
}
