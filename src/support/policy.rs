//! Policies defining when to trigger the support prompt.
//!
//! Separating the trigger decision behind a pure function and trait allows the owner
//! to easily substitute smarter frequency rules (e.g. exponential intervals, grace periods,
//! daily caps) in the future without touching any UI or persistence code.

use super::stats::SupportStats;

/// Trait implemented by policies that evaluate whether the support popup should appear.
pub trait SupportPolicy {
    /// Determines whether the prompt should be shown given the current stats and time.
    fn should_show(&self, stats: &SupportStats, now: Option<u64>) -> bool;
}

/// Simple rule triggering the popup on every Nth calculation (default: 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EveryNCalculations {
    pub interval: u64,
}

impl Default for EveryNCalculations {
    fn default() -> Self {
        Self { interval: 5 }
    }
}

impl EveryNCalculations {
    pub const fn new(interval: u64) -> Self {
        Self { interval }
    }
}

impl SupportPolicy for EveryNCalculations {
    fn should_show(&self, stats: &SupportStats, _now: Option<u64>) -> bool {
        self.interval > 0
            && stats.calculation_count > 0
            && stats.calculation_count.is_multiple_of(self.interval)
    }
}

/// Pure decision function: determines if the support prompt should be shown.
///
/// Uses the active policy (trigger on every 5th calculation).
pub fn should_show(stats: &SupportStats, now: Option<u64>) -> bool {
    EveryNCalculations::default().should_show(stats, now)
}
