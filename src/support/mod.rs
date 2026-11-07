//! Support the Project prompt module for the free CalcCraft build.
//!
//! Provides:
//! - An amber button at the top of the hamburger navigation menu.
//! - A non-modal semi-transparent floating support card over the main screen.
//! - A trigger policy opening the prompt on every 5th completed calculation.
//! - Local-only counter persistence with zero network or telemetry access.

pub mod policy;
pub mod stats;
pub mod view;

pub use policy::{EveryNCalculations, SupportPolicy, should_show};
pub use stats::SupportStats;
pub use view::{DONATE_URL, SupportAction, SupportState};

/// Returns the current Unix timestamp in seconds, using `web_time` for cross-platform safety.
pub fn current_time_secs() -> Option<u64> {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::Storage;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct MemoryStorage(BTreeMap<String, String>);

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

    #[test]
    fn stats_persistence_roundtrip() {
        let mut storage = MemoryStorage::default();
        let mut stats = SupportStats::new();
        stats.record_calculation(Some(1700000000));
        stats.record_calculation(Some(1700000005));
        stats.record_popup_shown();

        assert_eq!(stats.calculation_count, 2);
        assert_eq!(stats.first_used, Some(1700000000));
        assert_eq!(stats.popup_shown_count, 1);

        stats.save(&mut storage);

        let loaded = SupportStats::load(Some(&storage));
        assert_eq!(loaded.calculation_count, 2);
        assert_eq!(loaded.first_used, Some(1700000000));
        assert_eq!(loaded.popup_shown_count, 1);
    }

    #[test]
    fn every_5th_calculation_rule() {
        let mut state = SupportState::default();
        let now = Some(1700000000);

        for i in 1..=15 {
            let triggered = state.on_calculation_completed(now);
            if i % 5 == 0 {
                assert!(triggered, "calculation {i} should trigger popup");
                assert!(state.is_open, "popup should be open at {i}");
                // Dismiss popup to test the next interval
                state.is_open = false;
            } else {
                assert!(!triggered, "calculation {i} should not trigger popup");
            }
        }
        assert_eq!(state.stats.calculation_count, 15);
        assert_eq!(state.stats.popup_shown_count, 3);
    }

    #[test]
    fn no_double_popup_while_open() {
        let mut state = SupportState::default();
        let now = Some(1700000000);

        // Advance to 5
        for _ in 0..5 {
            state.on_calculation_completed(now);
        }
        assert!(state.is_open);
        assert_eq!(state.stats.popup_shown_count, 1);

        // Advance to 10 while popup remains open
        for _ in 5..10 {
            let triggered = state.on_calculation_completed(now);
            assert!(!triggered, "should not re-trigger while already open");
        }
        assert!(state.is_open);
        // Popup shown count should not have increased while open
        assert_eq!(state.stats.popup_shown_count, 1);
    }

    #[test]
    fn custom_interval_policy() {
        let mut stats = SupportStats::new();
        let policy_3 = EveryNCalculations::new(3);

        for _ in 0..2 {
            stats.record_calculation(None);
            assert!(!policy_3.should_show(&stats, None));
        }
        stats.record_calculation(None);
        assert!(policy_3.should_show(&stats, None));
    }
}
