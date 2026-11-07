//! Local calculation statistics and persistence for the support prompt module.

use eframe::Storage;

/// Storage keys for local persistence via `eframe::Storage`.
pub const CALCULATION_COUNT_KEY: &str = "SupportCalculationCount";
pub const FIRST_USED_KEY: &str = "SupportFirstUsed";
pub const POPUP_SHOWN_COUNT_KEY: &str = "SupportPopupShownCount";

/// Local calculation and interaction counters.
///
/// Kept strictly on the user's local machine; no data is ever transmitted externally.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SupportStats {
    /// Total number of successful evaluations completed in Standard, Scientific,
    /// or Programmer mode.
    pub calculation_count: u64,
    /// Unix timestamp (seconds) of the first recorded calculation / app use.
    pub first_used: Option<u64>,
    /// Number of times the support popup has been presented.
    pub popup_shown_count: u64,
}

impl SupportStats {
    /// Creates a blank stats container with zeroed counters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads persisted statistics from local storage, falling back to defaults if absent.
    pub fn load(storage: Option<&dyn Storage>) -> Self {
        let Some(storage) = storage else {
            return Self::new();
        };

        let calculation_count = storage
            .get_string(CALCULATION_COUNT_KEY)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let first_used = storage
            .get_string(FIRST_USED_KEY)
            .and_then(|s| s.parse().ok());
        let popup_shown_count = storage
            .get_string(POPUP_SHOWN_COUNT_KEY)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        Self {
            calculation_count,
            first_used,
            popup_shown_count,
        }
    }

    /// Saves the current statistics to local storage.
    pub fn save(&self, storage: &mut dyn Storage) {
        storage.set_string(CALCULATION_COUNT_KEY, self.calculation_count.to_string());
        if let Some(first_used) = self.first_used {
            storage.set_string(FIRST_USED_KEY, first_used.to_string());
        }
        storage.set_string(POPUP_SHOWN_COUNT_KEY, self.popup_shown_count.to_string());
    }

    /// Records a successful calculation evaluation.
    ///
    /// Sets `first_used` on the initial calculation if not already recorded.
    pub fn record_calculation(&mut self, now: Option<u64>) {
        self.calculation_count = self.calculation_count.saturating_add(1);
        if self.first_used.is_none() {
            self.first_used = now;
        }
    }

    /// Records that the support popup was displayed.
    pub fn record_popup_shown(&mut self) {
        self.popup_shown_count = self.popup_shown_count.saturating_add(1);
    }
}
