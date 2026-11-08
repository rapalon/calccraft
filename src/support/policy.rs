//! The support prompt's policy: when the red dot and the popup show (docs/support-prompt.md).
//!
//! Everything here is pure: time comes in as a [`Timestamp`], and the stats go in and come out as
//! values, so the tests can simulate weeks and months.

use super::stats::SupportStats;

/// A moment in time, as the policy sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timestamp {
    /// Wall clock milliseconds, for the timers of a few seconds.
    pub millis: u64,
    /// The local calendar day number (a minute number in fast mode), for the day-long timers.
    pub day: i64,
}

/// Every threshold of the policy. `Default` holds the owner's values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupportPolicyConfig {
    /// Launches before this one are the silent stage.
    pub min_launches: u32,
    /// Distinct days with a calculation needed before the first dot.
    pub min_active_days: u32,
    /// Days the dot shows unseen before the popup may open by itself.
    pub dot_days_before_popup: i64,
    /// The waits between cycles, by ladder step; the last one repeats.
    pub cycle_intervals_days: Vec<i64>,
    /// Quiet time after a calculation before the popup may open.
    pub calm_delay_ms: u64,
    /// An automatic popup closed faster than this counts as "annoyed".
    pub quick_close_ms: u64,
    /// The pause after a Donate click.
    pub donate_pause_days: i64,
    /// The pause after "I already supported".
    pub already_supported_pause_days: i64,
    /// How long the "Thank you!" text shows before the popup closes.
    pub thank_you_ms: u64,
}

impl Default for SupportPolicyConfig {
    fn default() -> Self {
        Self {
            min_launches: 4,
            min_active_days: 3,
            dot_days_before_popup: 7,
            cycle_intervals_days: vec![7, 14, 30, 60, 90],
            calm_delay_ms: 3000,
            quick_close_ms: 2000,
            donate_pause_days: 180,
            already_supported_pause_days: 365,
            thank_you_ms: 1500,
        }
    }
}

impl SupportPolicyConfig {
    /// The demo mode's thresholds: fewer launches and active days (its days are minutes).
    pub fn fast() -> Self {
        Self {
            min_launches: 2,
            min_active_days: 2,
            ..Self::default()
        }
    }

    /// The wait after a cycle completed at ladder step `step`.
    pub fn interval(&self, step: u32) -> i64 {
        let last = self.cycle_intervals_days.len().saturating_sub(1);
        self.cycle_intervals_days
            .get((step as usize).min(last))
            .copied()
            .unwrap_or(0)
    }
}

/// What the calm-moment rule needs to know about this moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MomentContext {
    /// When the last successful calculation finished, if there was no input since.
    pub calculation_at_ms: Option<u64>,
    /// The app is visible and focused (not minimized, not in the background).
    pub app_focused: bool,
    /// Standard, Scientific or Programmer mode, with nothing open over the calculator.
    pub calculator_idle: bool,
    /// An automatic popup was already shown in this session.
    pub popup_shown_this_session: bool,
}

impl MomentContext {
    /// Whether the popup may open now without interrupting anything.
    pub fn is_calm(&self, cfg: &SupportPolicyConfig, now: Timestamp) -> bool {
        self.app_focused
            && self.calculator_idle
            && !self.popup_shown_this_session
            && self
                .calculation_at_ms
                .is_some_and(|at| now.millis.saturating_sub(at) >= cfg.calm_delay_ms)
    }
}

/// What the UI shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportDecision {
    Nothing,
    /// The red dot on the menu button (and on the support button in the menu).
    ShowDot,
    /// Open the popup now (the dot shows until then).
    ShowPopup,
}

/// What happened, as the UI reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportEvent {
    AppLaunched,
    /// A successful `=` in a calculator mode.
    CalculationDone,
    /// The dot is drawn (stamps the day it first showed in this cycle).
    DotShown,
    /// The navigation menu opened.
    MenuOpened,
    /// The popup opened by itself.
    PopupShown,
    /// The popup opened from the menu's support button.
    PopupOpenedOnDemand,
    PopupClosed {
        after_ms: u64,
        automatic: bool,
    },
    DonateClicked,
    AlreadySupported,
}

/// Whether a cycle is running: past the silent stage, eligible, and not waiting or paused.
fn cycle_active(stats: &SupportStats, cfg: &SupportPolicyConfig, now: Timestamp) -> bool {
    stats.launches >= cfg.min_launches
        && stats.active_days >= cfg.min_active_days
        && stats.pause_until_day.is_none_or(|day| now.day >= day)
        && stats.next_cycle_day.is_none_or(|day| now.day >= day)
}

/// What to show now.
pub fn evaluate(
    stats: &SupportStats,
    cfg: &SupportPolicyConfig,
    now: Timestamp,
    moment: MomentContext,
) -> SupportDecision {
    if !cycle_active(stats, cfg, now) {
        return SupportDecision::Nothing;
    }
    let dot_since = stats.dot_since_day.unwrap_or(now.day);
    if now.day - dot_since >= cfg.dot_days_before_popup && moment.is_calm(cfg, now) {
        SupportDecision::ShowPopup
    } else {
        SupportDecision::ShowDot
    }
}

/// Ends the current cycle: the next one waits the ladder's interval.
fn complete_cycle(stats: &mut SupportStats, cfg: &SupportPolicyConfig, now: Timestamp) {
    stats.next_cycle_day = Some(now.day + cfg.interval(stats.ladder_step));
    stats.ladder_step = stats.ladder_step.saturating_add(1);
    stats.cycles = stats.cycles.saturating_add(1);
    stats.last_cycle_day = Some(now.day);
    stats.dot_since_day = None;
}

/// The stats after an event.
pub fn on_event(
    stats: &SupportStats,
    cfg: &SupportPolicyConfig,
    event: SupportEvent,
    now: Timestamp,
) -> SupportStats {
    let mut s = stats.clone();
    match event {
        SupportEvent::AppLaunched => {
            s.launches = s.launches.saturating_add(1);
            s.first_day.get_or_insert(now.day);
        }
        SupportEvent::CalculationDone => {
            s.calculations = s.calculations.saturating_add(1);
            s.first_day.get_or_insert(now.day);
            if s.last_active_day != Some(now.day) {
                s.active_days = s.active_days.saturating_add(1);
                s.last_active_day = Some(now.day);
            }
        }
        SupportEvent::DotShown => {
            if cycle_active(&s, cfg, now) {
                s.dot_since_day.get_or_insert(now.day);
            }
        }
        // Seeing the dot, in the menu or through the popup opened from it, completes the cycle.
        SupportEvent::MenuOpened | SupportEvent::PopupOpenedOnDemand => {
            if s.dot_since_day.is_some() && cycle_active(&s, cfg, now) {
                complete_cycle(&mut s, cfg, now);
            }
        }
        SupportEvent::PopupShown => {
            s.popups_shown = s.popups_shown.saturating_add(1);
            complete_cycle(&mut s, cfg, now);
        }
        SupportEvent::PopupClosed {
            after_ms,
            automatic,
        } => {
            // "Annoyed": one extra step up the ladder, from the day the cycle completed.
            if automatic && after_ms < cfg.quick_close_ms {
                let completed = s.last_cycle_day.unwrap_or(now.day);
                s.next_cycle_day = Some(completed + cfg.interval(s.ladder_step));
                s.ladder_step = s.ladder_step.saturating_add(1);
            }
        }
        // A pause never moves earlier: a supporter donating again from the thank-you line
        // keeps the rest of the 365 days.
        SupportEvent::DonateClicked => {
            s.donate_clicked_ever = true;
            pause_until(&mut s, now.day + cfg.donate_pause_days);
            s.dot_since_day = None;
        }
        SupportEvent::AlreadySupported => {
            let until = now.day + cfg.already_supported_pause_days;
            pause_until(&mut s, until);
            s.supporter_until_day = Some(s.supporter_until_day.map_or(until, |d| d.max(until)));
            s.dot_since_day = None;
        }
    }
    s
}

fn pause_until(stats: &mut SupportStats, day: i64) {
    stats.pause_until_day = Some(stats.pause_until_day.map_or(day, |d| d.max(day)));
}

/// What the top of the navigation menu shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuEntry {
    /// The amber "Please Support!" button.
    SupportButton,
    /// The supporter state's green thank-you line ("I already supported", for 365 days).
    SupporterThanks,
}

/// The menu's support entry now. Only "I already supported" makes a supporter: a Donate click
/// alone may not have ended in a payment.
pub fn menu_entry(stats: &SupportStats, _cfg: &SupportPolicyConfig, now: Timestamp) -> MenuEntry {
    if stats.supporter_until_day.is_some_and(|day| now.day < day) {
        MenuEntry::SupporterThanks
    } else {
        MenuEntry::SupportButton
    }
}

/// "I already supported" shows only after a Donate click.
pub fn show_already_supported_button(stats: &SupportStats) -> bool {
    stats.donate_clicked_ever
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(day: i64) -> Timestamp {
        Timestamp {
            millis: day as u64 * 86_400_000,
            day,
        }
    }

    fn cfg() -> SupportPolicyConfig {
        SupportPolicyConfig::default()
    }

    fn apply(stats: &SupportStats, event: SupportEvent, now: Timestamp) -> SupportStats {
        on_event(stats, &cfg(), event, now)
    }

    /// A calm moment: a calculation 3 s ago, focused, in a calculator mode.
    fn calm(now: Timestamp) -> MomentContext {
        MomentContext {
            calculation_at_ms: Some(now.millis.saturating_sub(3000)),
            app_focused: true,
            calculator_idle: true,
            popup_shown_this_session: false,
        }
    }

    fn decide(stats: &SupportStats, now: Timestamp) -> SupportDecision {
        evaluate(stats, &cfg(), now, calm(now))
    }

    /// A user with `launches` launches who calculated on days 0, 1 and 2.
    fn regular(launches: u32) -> SupportStats {
        let mut s = SupportStats::default();
        for day in 0..3 {
            s = apply(&s, SupportEvent::CalculationDone, at(day));
        }
        for _ in 0..launches {
            s = apply(&s, SupportEvent::AppLaunched, at(2));
        }
        s
    }

    /// Shows the dot on `day` (as the UI does when it draws it).
    fn dot(stats: &SupportStats, day: i64) -> SupportStats {
        assert_eq!(
            decide(stats, at(day)),
            SupportDecision::ShowDot,
            "day {day}"
        );
        apply(stats, SupportEvent::DotShown, at(day))
    }

    #[test]
    fn silent_during_the_first_three_launches() {
        let mut s = SupportStats::default();
        for day in 0..10 {
            s = apply(&s, SupportEvent::CalculationDone, at(day));
        }
        for launch in 1..=3 {
            s = apply(&s, SupportEvent::AppLaunched, at(10));
            assert_eq!(s.launches, launch);
            assert_eq!(decide(&s, at(10)), SupportDecision::Nothing);
        }
        s = apply(&s, SupportEvent::AppLaunched, at(10));
        assert_eq!(decide(&s, at(10)), SupportDecision::ShowDot);
    }

    #[test]
    fn eligibility_needs_three_distinct_active_days() {
        let mut s = SupportStats::default();
        for _ in 0..5 {
            s = apply(&s, SupportEvent::AppLaunched, at(0));
        }
        // Many calculations on one day are one active day.
        for _ in 0..20 {
            s = apply(&s, SupportEvent::CalculationDone, at(0));
        }
        assert_eq!(s.active_days, 1);
        assert_eq!(s.last_active_day, Some(0));
        assert_eq!(decide(&s, at(0)), SupportDecision::Nothing);
        // A day without a calculation doesn't count.
        s = apply(&s, SupportEvent::CalculationDone, at(4));
        assert_eq!(s.active_days, 2);
        assert_eq!(decide(&s, at(9)), SupportDecision::Nothing);
        s = apply(&s, SupportEvent::CalculationDone, at(9));
        assert_eq!(s.active_days, 3);
        assert_eq!(decide(&s, at(9)), SupportDecision::ShowDot);
        assert_eq!(s.calculations, 22);
    }

    #[test]
    fn dot_appears_and_opening_the_menu_clears_it_and_starts_the_interval() {
        let s = dot(&regular(4), 2);
        assert_eq!(s.dot_since_day, Some(2));
        // Drawing it again later keeps the first day.
        let s = dot(&s, 4);
        assert_eq!(s.dot_since_day, Some(2));

        let s = apply(&s, SupportEvent::MenuOpened, at(5));
        assert_eq!(s.dot_since_day, None);
        assert_eq!(s.cycles, 1);
        assert_eq!(s.next_cycle_day, Some(12));
        for day in 5..12 {
            assert_eq!(decide(&s, at(day)), SupportDecision::Nothing, "day {day}");
        }
        // The next cycle starts with the dot again.
        assert_eq!(decide(&s, at(12)), SupportDecision::ShowDot);
    }

    #[test]
    fn opening_the_menu_without_a_dot_changes_nothing() {
        let s = regular(2);
        assert_eq!(apply(&s, SupportEvent::MenuOpened, at(3)), s);
        let s = regular(4);
        assert_eq!(apply(&s, SupportEvent::MenuOpened, at(3)), s);
        let s = apply(&s, SupportEvent::PopupOpenedOnDemand, at(3));
        assert_eq!(s.cycles, 0);
    }

    #[test]
    fn popup_opened_from_the_menu_counts_as_seen() {
        let s = dot(&regular(4), 2);
        let s = apply(&s, SupportEvent::PopupOpenedOnDemand, at(3));
        assert_eq!(
            (s.cycles, s.dot_since_day, s.next_cycle_day),
            (1, None, Some(10))
        );
        // Closing it quickly is no penalty.
        let closed = SupportEvent::PopupClosed {
            after_ms: 200,
            automatic: false,
        };
        assert_eq!(apply(&s, closed, at(3)), s);
    }

    #[test]
    fn dot_escalates_to_the_popup_after_seven_days() {
        let s = dot(&regular(4), 2);
        for day in 2..9 {
            assert_eq!(decide(&s, at(day)), SupportDecision::ShowDot, "day {day}");
        }
        assert_eq!(decide(&s, at(9)), SupportDecision::ShowPopup);
        // Not calm: the dot stays.
        let busy = MomentContext {
            calculation_at_ms: None,
            ..calm(at(9))
        };
        assert_eq!(evaluate(&s, &cfg(), at(9), busy), SupportDecision::ShowDot);

        let s = apply(&s, SupportEvent::PopupShown, at(9));
        assert_eq!((s.cycles, s.popups_shown, s.dot_since_day), (1, 1, None));
        assert_eq!(decide(&s, at(9)), SupportDecision::Nothing);
        assert_eq!(s.next_cycle_day, Some(16));
    }

    #[test]
    fn calm_moment_rule() {
        let c = cfg();
        let now = at(100);
        let base = calm(now);
        assert!(base.is_calm(&c, now));
        // Needs 3 s after the calculation.
        let early = MomentContext {
            calculation_at_ms: Some(now.millis - 2999),
            ..base
        };
        assert!(!early.is_calm(&c, now));
        // Input since the calculation cancels it (the UI clears the time).
        let typed = MomentContext {
            calculation_at_ms: None,
            ..base
        };
        assert!(!typed.is_calm(&c, now));
        let unfocused = MomentContext {
            app_focused: false,
            ..base
        };
        assert!(!unfocused.is_calm(&c, now));
        let converter = MomentContext {
            calculator_idle: false,
            ..base
        };
        assert!(!converter.is_calm(&c, now));
    }

    #[test]
    fn one_automatic_popup_per_session() {
        let s = dot(&regular(4), 2);
        let shown = MomentContext {
            popup_shown_this_session: true,
            ..calm(at(9))
        };
        assert_eq!(evaluate(&s, &cfg(), at(9), shown), SupportDecision::ShowDot);
    }

    #[test]
    fn backoff_ladder_and_cap() {
        let mut s = regular(4);
        let mut day = 2;
        let mut waits = Vec::new();
        for _ in 0..8 {
            s = dot(&s, day);
            s = apply(&s, SupportEvent::MenuOpened, at(day));
            let next = s.next_cycle_day.unwrap();
            waits.push(next - day);
            assert_eq!(decide(&s, at(next - 1)), SupportDecision::Nothing);
            day = next;
        }
        assert_eq!(waits, [7, 14, 30, 60, 90, 90, 90, 90]);
        assert_eq!(s.cycles, 8);
    }

    #[test]
    fn quick_close_moves_one_extra_step() {
        let s = dot(&regular(4), 2);
        let s = apply(&s, SupportEvent::PopupShown, at(9));
        let quick = SupportEvent::PopupClosed {
            after_ms: 1500,
            automatic: true,
        };
        let slow = SupportEvent::PopupClosed {
            after_ms: 2000,
            automatic: true,
        };
        let annoyed = apply(&s, quick, at(9));
        assert_eq!(annoyed.next_cycle_day, Some(9 + 14));
        assert_eq!(annoyed.ladder_step, 2);
        let calm = apply(&s, slow, at(9));
        assert_eq!(calm.next_cycle_day, Some(9 + 7));
        assert_eq!(calm.ladder_step, 1);
        // The cycle after an annoyed close waits 30 days.
        let s = dot(&annoyed, 23);
        let s = apply(&s, SupportEvent::MenuOpened, at(23));
        assert_eq!(s.next_cycle_day, Some(23 + 30));
    }

    #[test]
    fn donate_pauses_for_180_days() {
        let s = dot(&regular(4), 2);
        assert!(!show_already_supported_button(&s));
        let s = apply(&s, SupportEvent::DonateClicked, at(3));
        assert!(s.donate_clicked_ever);
        assert!(show_already_supported_button(&s));
        assert_eq!(s.dot_since_day, None);
        assert_eq!(decide(&s, at(182)), SupportDecision::Nothing);
        assert_eq!(decide(&s, at(183)), SupportDecision::ShowDot);
        // The flag is never reset.
        let s = apply(&s, SupportEvent::AlreadySupported, at(200));
        let s = apply(&s, SupportEvent::MenuOpened, at(600));
        assert!(show_already_supported_button(&s));
    }

    #[test]
    fn already_supported_button_only_after_a_donate_click() {
        let mut s = regular(4);
        for day in 2..400 {
            s = apply(&s, SupportEvent::CalculationDone, at(day));
            if decide(&s, at(day)) != SupportDecision::Nothing {
                s = apply(&s, SupportEvent::DotShown, at(day));
            }
            if decide(&s, at(day)) == SupportDecision::ShowPopup {
                s = apply(&s, SupportEvent::PopupShown, at(day));
            }
            assert!(!show_already_supported_button(&s));
        }
        assert!(s.popups_shown > 0);
        let s = apply(&s, SupportEvent::DonateClicked, at(400));
        assert!(show_already_supported_button(&s));
    }

    #[test]
    fn already_supported_pauses_for_365_days() {
        let s = dot(&regular(4), 2);
        let s = apply(&s, SupportEvent::DonateClicked, at(3));
        let s = dot(&s, 183);
        let s = apply(&s, SupportEvent::AlreadySupported, at(190));
        assert_eq!(s.dot_since_day, None);
        assert_eq!(decide(&s, at(190 + 364)), SupportDecision::Nothing);
        assert_eq!(decide(&s, at(190 + 365)), SupportDecision::ShowDot);
    }

    fn entry(stats: &SupportStats, day: i64) -> MenuEntry {
        menu_entry(stats, &cfg(), at(day))
    }

    #[test]
    fn supporter_thanks_for_365_days_after_already_supported() {
        let s = dot(&regular(4), 2);
        assert_eq!(entry(&SupportStats::default(), 0), MenuEntry::SupportButton);
        assert_eq!(entry(&s, 2), MenuEntry::SupportButton);
        // A Donate click alone is no proof of support.
        let s = apply(&s, SupportEvent::DonateClicked, at(3));
        for day in [3, 100, 182, 183, 1000] {
            assert_eq!(entry(&s, day), MenuEntry::SupportButton, "day {day}");
        }
        let s = apply(&s, SupportEvent::AlreadySupported, at(10));
        assert_eq!(s.supporter_until_day, Some(375));
        for day in [10, 11, 200, 374] {
            assert_eq!(entry(&s, day), MenuEntry::SupporterThanks, "day {day}");
            assert_eq!(decide(&s, at(day)), SupportDecision::Nothing, "day {day}");
        }
        // Then the button is back and the cycle starts again at Stage 1, the dot.
        assert_eq!(entry(&s, 375), MenuEntry::SupportButton);
        assert_eq!(decide(&s, at(375)), SupportDecision::ShowDot);
        let s = dot(&s, 375);
        assert_eq!(decide(&s, at(381)), SupportDecision::ShowDot);
        assert_eq!(decide(&s, at(382)), SupportDecision::ShowPopup);
    }

    #[test]
    fn opening_and_donating_from_the_thanks_line_keep_the_pause() {
        let s = apply(&regular(4), SupportEvent::DonateClicked, at(2));
        let s = apply(&s, SupportEvent::AlreadySupported, at(10));
        let before = s.clone();
        // Opening the popup on demand is no cycle.
        let s = apply(&s, SupportEvent::PopupOpenedOnDemand, at(20));
        assert_eq!(s, before);
        let closed = SupportEvent::PopupClosed {
            after_ms: 100,
            automatic: false,
        };
        assert_eq!(apply(&s, closed, at(20)), before);
        // Donating again doesn't shorten the 365 days to 180 from now.
        let s = apply(&s, SupportEvent::DonateClicked, at(20));
        assert_eq!(s.pause_until_day, Some(375));
        assert_eq!(s.supporter_until_day, Some(375));
        assert_eq!(s.cycles, before.cycles);
        assert_eq!(entry(&s, 374), MenuEntry::SupporterThanks);
        // A late Donate still pauses its own 180 days past the supporter state.
        let s = apply(&s, SupportEvent::DonateClicked, at(300));
        assert_eq!(s.pause_until_day, Some(480));
        assert_eq!(entry(&s, 375), MenuEntry::SupportButton);
        assert_eq!(decide(&s, at(479)), SupportDecision::Nothing);
    }

    #[test]
    fn interval_ladder_from_config() {
        let c = cfg();
        let steps: Vec<i64> = (0..7).map(|step| c.interval(step)).collect();
        assert_eq!(steps, [7, 14, 30, 60, 90, 90, 90]);
        assert_eq!(c.interval(u32::MAX), 90);
        let empty = SupportPolicyConfig {
            cycle_intervals_days: Vec::new(),
            ..cfg()
        };
        assert_eq!(empty.interval(3), 0);
    }

    #[test]
    fn fast_config_lowers_the_launch_thresholds() {
        let c = SupportPolicyConfig::fast();
        let mut s = SupportStats::default();
        s = on_event(&s, &c, SupportEvent::AppLaunched, at(0));
        s = on_event(&s, &c, SupportEvent::CalculationDone, at(0));
        s = on_event(&s, &c, SupportEvent::CalculationDone, at(1));
        assert_eq!(
            evaluate(&s, &c, at(1), calm(at(1))),
            SupportDecision::Nothing
        );
        s = on_event(&s, &c, SupportEvent::AppLaunched, at(1));
        assert_eq!(
            evaluate(&s, &c, at(1), calm(at(1))),
            SupportDecision::ShowDot
        );
    }
}
