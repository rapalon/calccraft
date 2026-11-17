//! The support prompt's stored state and its persistence (docs/support-prompt.md, section 5).

use eframe::Storage;

/// The v2 record's storage key.
pub const STATS_KEY: &str = "SupportStatsV2";
/// v1's keys, read once for the migration and removed when v2 is saved.
pub const V1_CALCULATION_COUNT_KEY: &str = "SupportCalculationCount";
pub const V1_FIRST_USED_KEY: &str = "SupportFirstUsed";
pub const V1_POPUP_SHOWN_COUNT_KEY: &str = "SupportPopupShownCount";

const VERSION: &str = "2";

/// What the policy remembers between launches. Local only: counters and day numbers, nothing
/// that identifies a person or an installation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SupportStats {
    pub launches: u32,
    /// Distinct days with a successful calculation.
    pub active_days: u32,
    pub last_active_day: Option<i64>,
    /// The day of the first launch (for the coarse days-since-install bucket of the link).
    pub first_day: Option<i64>,
    /// Complete reminder cycles.
    pub cycles: u32,
    /// Index into the interval ladder (one per cycle, plus one per "annoyed" close).
    pub ladder_step: u32,
    pub last_cycle_day: Option<i64>,
    /// No new cycle before this day.
    pub next_cycle_day: Option<i64>,
    /// The day the current cycle's dot first showed.
    pub dot_since_day: Option<i64>,
    /// No automatic reminder before this day.
    pub pause_until_day: Option<i64>,
    /// The supporter state ("I already supported") lasts until this day.
    pub supporter_until_day: Option<i64>,
    /// Ever clicked Donate (never reset).
    pub donate_clicked_ever: bool,
    /// Share clicks (informational).
    pub shares: u32,
    /// `cycles` at the last Share that moved the ladder: one extra step per cycle.
    pub last_share_cycle: Option<u32>,
    pub calculations: u64,
    pub popups_shown: u32,
    /// Written in fast mode, whose day numbers are minutes.
    pub fast: bool,
}

/// One `name=value` field per `;`; an empty value is `None`.
fn field<T: std::str::FromStr>(fields: &[(&str, &str)], name: &str) -> Option<T> {
    fields
        .iter()
        .find(|(key, _)| *key == name)
        .and_then(|(_, value)| value.parse().ok())
}

impl SupportStats {
    /// Reads the record from `text`; unknown fields are ignored, missing ones take defaults.
    pub fn parse(text: &str) -> Option<Self> {
        let fields: Vec<(&str, &str)> = text
            .split(';')
            .filter_map(|part| part.split_once('='))
            .map(|(key, value)| (key.trim(), value.trim()))
            .collect();
        if field::<String>(&fields, "v").as_deref() != Some(VERSION) {
            return None;
        }
        Some(Self {
            launches: field(&fields, "launches").unwrap_or(0),
            active_days: field(&fields, "active_days").unwrap_or(0),
            last_active_day: field(&fields, "last_active_day"),
            first_day: field(&fields, "first_day"),
            cycles: field(&fields, "cycles").unwrap_or(0),
            ladder_step: field(&fields, "ladder_step").unwrap_or(0),
            last_cycle_day: field(&fields, "last_cycle_day"),
            next_cycle_day: field(&fields, "next_cycle_day"),
            dot_since_day: field(&fields, "dot_since_day"),
            pause_until_day: field(&fields, "pause_until_day"),
            supporter_until_day: field(&fields, "supporter_until_day"),
            donate_clicked_ever: field(&fields, "donate_clicked_ever").unwrap_or(false),
            shares: field(&fields, "shares").unwrap_or(0),
            last_share_cycle: field(&fields, "last_share_cycle"),
            calculations: field(&fields, "calculations").unwrap_or(0),
            popups_shown: field(&fields, "popups_shown").unwrap_or(0),
            fast: field(&fields, "fast").unwrap_or(false),
        })
    }

    /// The record as stored.
    pub fn serialize(&self) -> String {
        fn day(value: Option<i64>) -> String {
            value.map(|v| v.to_string()).unwrap_or_default()
        }
        format!(
            "v={VERSION};launches={};active_days={};last_active_day={};first_day={};cycles={};\
             ladder_step={};last_cycle_day={};next_cycle_day={};dot_since_day={};\
             pause_until_day={};supporter_until_day={};donate_clicked_ever={};calculations={};\
             popups_shown={};fast={};shares={};last_share_cycle={}",
            self.launches,
            self.active_days,
            day(self.last_active_day),
            day(self.first_day),
            self.cycles,
            self.ladder_step,
            day(self.last_cycle_day),
            day(self.next_cycle_day),
            day(self.dot_since_day),
            day(self.pause_until_day),
            day(self.supporter_until_day),
            self.donate_clicked_ever,
            self.calculations,
            self.popups_shown,
            self.fast,
            self.shares,
            day(self.last_share_cycle.map(i64::from)),
        )
    }

    /// v1's counters mapped to v2: its calculations count, a user who calculated counts as one
    /// active day, and launches start at 0, so v1 users go through the silent stage once.
    /// `day_of` turns v1's Unix seconds into a day number.
    pub fn from_v1(storage: &dyn Storage, day_of: impl Fn(u64) -> i64) -> Self {
        let number = |key: &str| storage.get_string(key).and_then(|s| s.trim().parse().ok());
        let calculations: u64 = number(V1_CALCULATION_COUNT_KEY).unwrap_or(0);
        let first_day = number(V1_FIRST_USED_KEY).map(&day_of);
        let popups_shown = number(V1_POPUP_SHOWN_COUNT_KEY)
            .unwrap_or(0u64)
            .min(u32::MAX as u64) as u32;
        Self {
            calculations,
            active_days: u32::from(calculations > 0),
            last_active_day: first_day.filter(|_| calculations > 0),
            first_day,
            popups_shown,
            ..Self::default()
        }
    }

    /// Loads the record, or migrates v1's keys, or starts fresh. A record of the other time
    /// mode (fast or not) starts fresh: its day numbers mean something else.
    pub fn load(storage: Option<&dyn Storage>, fast: bool, day_of: impl Fn(u64) -> i64) -> Self {
        let Some(storage) = storage else {
            return Self {
                fast,
                ..Self::default()
            };
        };
        let stats = match storage.get_string(STATS_KEY) {
            Some(text) => Self::parse(&text).unwrap_or_default(),
            None => Self::from_v1(storage, day_of),
        };
        if stats.fast == fast {
            stats
        } else {
            Self {
                fast,
                ..Self::default()
            }
        }
    }

    /// Saves the record and removes v1's keys.
    pub fn save(&self, storage: &mut dyn Storage) {
        storage.set_string(STATS_KEY, self.serialize());
        for key in [
            V1_CALCULATION_COUNT_KEY,
            V1_FIRST_USED_KEY,
            V1_POPUP_SHOWN_COUNT_KEY,
        ] {
            storage.remove_string(key);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    pub(crate) struct MemoryStorage(pub BTreeMap<String, String>);

    impl Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }

        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_owned(), value);
        }

        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }

        fn flush(&mut self) {}
    }

    fn day_of(secs: u64) -> i64 {
        (secs / 86_400) as i64
    }

    #[test]
    fn round_trip() {
        let stats = SupportStats {
            launches: 7,
            active_days: 5,
            last_active_day: Some(20_400),
            first_day: Some(20_300),
            cycles: 3,
            ladder_step: 4,
            last_cycle_day: Some(20_390),
            next_cycle_day: Some(20_450),
            dot_since_day: None,
            pause_until_day: Some(20_570),
            supporter_until_day: Some(20_600),
            donate_clicked_ever: true,
            shares: 2,
            last_share_cycle: Some(3),
            calculations: 123,
            popups_shown: 2,
            fast: false,
        };
        let mut storage = MemoryStorage::default();
        stats.save(&mut storage);
        let loaded = SupportStats::load(Some(&storage), false, day_of);
        assert_eq!(loaded, stats);
        assert!(loaded.donate_clicked_ever);
        // Negative day numbers (and empty fields) survive too.
        let odd = SupportStats {
            dot_since_day: Some(-3),
            ..SupportStats::default()
        };
        assert_eq!(SupportStats::parse(&odd.serialize()), Some(odd));
    }

    #[test]
    fn missing_and_unknown_fields() {
        let stats = SupportStats::parse("v=2;launches=4;future_field=abc;cycles=x").unwrap();
        assert_eq!(stats.launches, 4);
        assert_eq!(stats.cycles, 0);
        assert!(!stats.donate_clicked_ever);
        // A record from before the share action has no share fields.
        assert_eq!((stats.shares, stats.last_share_cycle), (0, None));
        assert_eq!(SupportStats::parse("v=3;launches=4"), None);
        assert_eq!(SupportStats::parse("garbage"), None);
        let mut storage = MemoryStorage::default();
        storage.set_string(STATS_KEY, "garbage".into());
        assert_eq!(
            SupportStats::load(Some(&storage), false, day_of),
            SupportStats::default()
        );
    }

    #[test]
    fn migration_from_v1() {
        let mut storage = MemoryStorage::default();
        storage.set_string(V1_CALCULATION_COUNT_KEY, "42".into());
        storage.set_string(V1_FIRST_USED_KEY, "1700000000".into());
        storage.set_string(V1_POPUP_SHOWN_COUNT_KEY, "8".into());
        let stats = SupportStats::load(Some(&storage), false, day_of);
        assert_eq!(stats.calculations, 42);
        assert_eq!(stats.popups_shown, 8);
        assert_eq!(stats.first_day, Some(19_675));
        assert_eq!(stats.last_active_day, Some(19_675));
        assert_eq!(stats.active_days, 1);
        assert_eq!(stats.launches, 0);
        assert!(!stats.donate_clicked_ever);

        stats.save(&mut storage);
        assert!(storage.get_string(V1_CALCULATION_COUNT_KEY).is_none());
        assert!(storage.get_string(V1_FIRST_USED_KEY).is_none());
        assert!(storage.get_string(V1_POPUP_SHOWN_COUNT_KEY).is_none());
        assert_eq!(SupportStats::load(Some(&storage), false, day_of), stats);
    }

    #[test]
    fn migration_from_broken_v1_values() {
        let mut storage = MemoryStorage::default();
        storage.set_string(V1_CALCULATION_COUNT_KEY, "-5".into());
        storage.set_string(V1_FIRST_USED_KEY, "yesterday".into());
        storage.set_string(V1_POPUP_SHOWN_COUNT_KEY, "99999999999999".into());
        let stats = SupportStats::load(Some(&storage), false, day_of);
        assert_eq!(stats.calculations, 0);
        assert_eq!(stats.active_days, 0);
        assert_eq!(stats.first_day, None);
        assert_eq!(stats.popups_shown, u32::MAX);
        assert_eq!(
            SupportStats::load(None, false, day_of),
            SupportStats::default()
        );
    }

    #[test]
    fn fast_and_normal_records_dont_mix() {
        let fast = SupportStats {
            launches: 9,
            fast: true,
            ..SupportStats::default()
        };
        let mut storage = MemoryStorage::default();
        fast.save(&mut storage);
        assert_eq!(SupportStats::load(Some(&storage), true, day_of), fast);
        let normal = SupportStats::load(Some(&storage), false, day_of);
        assert_eq!(normal, SupportStats::default());
    }
}
