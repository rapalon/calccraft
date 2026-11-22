//! The Android asks and their queue (docs/support-prompt.md, section 13): the Google Play rating
//! request, the "share with three friends" card and the support popup, one at a time.
//!
//! Everything here is pure: time comes in as a [`Timestamp`] and the stats as values, so the
//! tests can simulate months. It is compiled in every build; only Android switches it on.

use super::policy::{MomentContext, SupportPolicyConfig, Timestamp};
use super::stats::SupportStats;

/// One of the things the app may ask of a user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    /// Google Play's In-App Review dialog.
    Rate,
    /// The card that asks to share CalcCraft with three friends.
    ShareThree,
    /// The support popup (its own policy decides when it is due; the queue only orders it).
    Support,
}

/// Every threshold of the asks. `Default` holds the proposed values (section 13.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskConfig {
    /// The engagement gate: launches, days with a calculation, and days since the first launch.
    pub min_launches: u32,
    pub min_active_days: u32,
    pub min_days_since_first: i64,
    /// Days between any two asks.
    pub ask_gap_days: i64,
    /// Days between two rating requests, and how many there can ever be.
    pub rate_gap_days: i64,
    pub rate_max: u32,
    /// Days between two share asks, and how many there can ever be.
    pub share_gap_days: i64,
    pub share_max: u32,
    /// Days before the next share ask once the user shared from the last one.
    pub share_done_gap_days: i64,
    /// Days since the last rating request that a rating after a share needs.
    pub after_share_rate_gap_days: i64,
    /// How long the thank-you shows before the review is requested.
    pub after_share_delay_ms: u64,
}

impl Default for AskConfig {
    fn default() -> Self {
        Self {
            min_launches: 4,
            min_active_days: 3,
            min_days_since_first: 3,
            ask_gap_days: 7,
            rate_gap_days: 90,
            rate_max: 3,
            share_gap_days: 60,
            share_max: 3,
            share_done_gap_days: 180,
            after_share_rate_gap_days: 30,
            after_share_delay_ms: 1500,
        }
    }
}

impl AskConfig {
    /// The demo mode's thresholds (its days are minutes).
    pub fn fast() -> Self {
        Self {
            min_launches: 2,
            min_active_days: 2,
            min_days_since_first: 1,
            ..Self::default()
        }
    }
}

/// Whether the user has stayed: enough launches, days of use, and days since the first launch.
pub fn gate_met(stats: &SupportStats, cfg: &AskConfig, now: Timestamp) -> bool {
    stats.launches >= cfg.min_launches
        && stats.active_days >= cfg.min_active_days
        && stats
            .first_day
            .is_some_and(|first| now.day - first >= cfg.min_days_since_first)
}

/// The gap since the last ask of any kind has passed.
pub fn gap_passed(stats: &SupportStats, cfg: &AskConfig, now: Timestamp) -> bool {
    stats
        .last_ask_day
        .is_none_or(|day| now.day - day >= cfg.ask_gap_days)
}

fn rate_due(stats: &SupportStats, cfg: &AskConfig, now: Timestamp) -> bool {
    stats.rate_asks < cfg.rate_max
        && stats
            .last_rate_day
            .is_none_or(|day| now.day - day >= cfg.rate_gap_days)
}

fn share_due(stats: &SupportStats, cfg: &AskConfig, now: Timestamp) -> bool {
    let wait = if stats.share_ask_done {
        cfg.share_done_gap_days
    } else {
        cfg.share_gap_days
    };
    stats.share_asks < cfg.share_max
        && stats
            .last_share_ask_day
            .is_none_or(|day| now.day - day >= wait)
}

/// Which ask to show now, if any. `support_due` is whether the support popup's own policy
/// wants to open; `moment` says whether this is a calm moment and whether an ask already showed in
/// this session (it then carries `popup_shown_this_session`). Priority: rating, share, support.
pub fn next_ask(
    stats: &SupportStats,
    cfg: &AskConfig,
    now: Timestamp,
    moment: MomentContext,
    support_due: bool,
    support_cfg: &SupportPolicyConfig,
) -> Option<Ask> {
    if !moment.is_calm(support_cfg, now) || !gap_passed(stats, cfg, now) {
        return None;
    }
    if gate_met(stats, cfg, now) {
        if rate_due(stats, cfg, now) {
            return Some(Ask::Rate);
        }
        if share_due(stats, cfg, now) {
            return Some(Ask::ShareThree);
        }
    }
    support_due.then_some(Ask::Support)
}

/// The stats after `ask` showed.
pub fn on_ask_shown(stats: &SupportStats, ask: Ask, now: Timestamp) -> SupportStats {
    let mut s = stats.clone();
    s.last_ask_day = Some(now.day);
    match ask {
        Ask::Rate => {
            s.rate_asks = s.rate_asks.saturating_add(1);
            s.last_rate_day = Some(now.day);
        }
        Ask::ShareThree => {
            s.share_asks = s.share_asks.saturating_add(1);
            s.last_share_ask_day = Some(now.day);
            s.share_ask_done = false;
        }
        // The support policy counts its own popup.
        Ask::Support => {}
    }
    s
}

/// Whether the rating may be requested after a share: not on the first day, and the rating's
/// own cool-downs hold. `rated_this_session` is a request already made in this session.
pub fn rate_after_share(
    stats: &SupportStats,
    cfg: &AskConfig,
    now: Timestamp,
    rated_this_session: bool,
) -> bool {
    !rated_this_session
        && stats.first_day.is_some_and(|first| now.day > first)
        && stats.rate_asks < cfg.rate_max
        && stats
            .last_rate_day
            .is_none_or(|day| now.day - day >= cfg.after_share_rate_gap_days)
}

/// The stats after a rating request that followed a share. It doesn't count as the day's ask
/// slot for the share card, but the gap holds for the asks after it.
pub fn on_rate_after_share(stats: &SupportStats, now: Timestamp) -> SupportStats {
    on_ask_shown(stats, Ask::Rate, now)
}

/// The share card's Share button was pressed.
pub fn on_share_ask_done(stats: &SupportStats) -> SupportStats {
    SupportStats {
        share_ask_done: true,
        ..stats.clone()
    }
}

/// Asks the Play In-App Review API for its dialog. The API decides whether the dialog shows and
/// returns nothing the app reads.
pub trait ReviewRequester {
    /// Sends the request. Whether it reached anything is of no interest to the policy.
    fn request(&self);
}

/// No review library in this build (everything but the Play channels on Android).
pub struct NoReview;

impl ReviewRequester for NoReview {
    fn request(&self) {}
}

/// The requester of this build.
pub fn platform_requester() -> &'static dyn ReviewRequester {
    #[cfg(target_os = "android")]
    {
        &crate::app::android::PlayReview
    }
    #[cfg(not(target_os = "android"))]
    {
        &NoReview
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY_MS: u64 = 86_400_000;

    fn at(day: i64) -> Timestamp {
        Timestamp {
            millis: day as u64 * DAY_MS + 50_000,
            day,
        }
    }

    fn calm() -> MomentContext {
        MomentContext {
            calculation_at_ms: Some(0),
            app_focused: true,
            calculator_idle: true,
            popup_shown_this_session: false,
        }
    }

    /// A regular user, first launch on day 0.
    fn regular() -> SupportStats {
        SupportStats {
            launches: 4,
            active_days: 3,
            first_day: Some(0),
            ..SupportStats::default()
        }
    }

    fn next(stats: &SupportStats, day: i64, moment: MomentContext, support: bool) -> Option<Ask> {
        next_ask(
            stats,
            &AskConfig::default(),
            at(day),
            moment,
            support,
            &SupportPolicyConfig::default(),
        )
    }

    #[test]
    fn nobody_is_asked_on_the_first_days() {
        let stats = regular();
        for day in 0..3 {
            assert_eq!(next(&stats, day, calm(), false), None, "day {day}");
        }
        assert_eq!(next(&stats, 3, calm(), false), Some(Ask::Rate));
    }

    #[test]
    fn the_gate_needs_launches_days_of_use_and_age() {
        let cfg = AskConfig::default();
        let mut s = regular();
        assert!(gate_met(&s, &cfg, at(3)));
        s.launches = 3;
        assert!(!gate_met(&s, &cfg, at(3)));
        s = regular();
        s.active_days = 2;
        assert!(!gate_met(&s, &cfg, at(30)));
        s = regular();
        assert!(!gate_met(&s, &cfg, at(2)));
        s.first_day = None;
        assert!(!gate_met(&s, &cfg, at(30)));
    }

    #[test]
    fn a_calm_moment_is_needed_and_one_ask_per_session() {
        let stats = regular();
        let mut moment = calm();
        moment.calculation_at_ms = None;
        assert_eq!(next(&stats, 5, moment, true), None, "mid-input");
        moment = calm();
        moment.app_focused = false;
        assert_eq!(next(&stats, 5, moment, true), None);
        moment = calm();
        moment.popup_shown_this_session = true;
        assert_eq!(next(&stats, 5, moment, true), None, "one ask a session");
        // A moment whose calculation is 3 s ago.
        moment = calm();
        moment.calculation_at_ms = Some(at(5).millis - 2999);
        assert_eq!(next(&stats, 5, moment, true), None, "3 s haven't passed");
    }

    #[test]
    fn the_queue_goes_rate_then_share_then_support_a_week_apart() {
        let cfg = AskConfig::default();
        let mut stats = regular();
        let mut order = Vec::new();
        for day in 3..400 {
            if let Some(ask) = next(&stats, day, calm(), true) {
                order.push((day, ask));
                stats = on_ask_shown(&stats, ask, at(day));
                if ask == Ask::Support {
                    break;
                }
            }
        }
        assert_eq!(
            order,
            vec![(3, Ask::Rate), (10, Ask::ShareThree), (17, Ask::Support)]
        );
        assert!(stats.last_ask_day.is_some());
        let _ = cfg;
    }

    #[test]
    fn the_gap_holds_for_every_kind() {
        let mut stats = regular();
        stats = on_ask_shown(&stats, Ask::Support, at(10));
        assert_eq!(next(&stats, 16, calm(), true), None);
        assert_eq!(next(&stats, 17, calm(), false), Some(Ask::Rate));
    }

    #[test]
    fn the_rating_repeats_after_90_days_at_most_three_times() {
        let mut stats = regular();
        let mut days = Vec::new();
        for day in 3..1000 {
            // Only the rating: the share card is used up.
            stats.share_asks = 3;
            if next(&stats, day, calm(), false) == Some(Ask::Rate) {
                days.push(day);
                stats = on_ask_shown(&stats, Ask::Rate, at(day));
            }
        }
        assert_eq!(days, vec![3, 93, 183]);
    }

    #[test]
    fn the_share_card_waits_longer_after_it_was_used() {
        let mut stats = regular();
        stats.rate_asks = 3;
        stats = on_ask_shown(&stats, Ask::ShareThree, at(10));
        assert_eq!(next(&stats, 69, calm(), false), None);
        assert_eq!(next(&stats, 70, calm(), false), Some(Ask::ShareThree));
        stats = on_share_ask_done(&stats);
        assert_eq!(next(&stats, 189, calm(), false), None);
        assert_eq!(next(&stats, 190, calm(), false), Some(Ask::ShareThree));
        // A new card is a new ask: its done flag resets.
        stats = on_ask_shown(&stats, Ask::ShareThree, at(190));
        assert!(!stats.share_ask_done);
        assert_eq!(stats.share_asks, 2);
    }

    #[test]
    fn support_shows_when_the_rest_is_done_or_not_due() {
        let mut stats = regular();
        stats.rate_asks = 3;
        stats.share_asks = 3;
        assert_eq!(next(&stats, 20, calm(), true), Some(Ask::Support));
        assert_eq!(next(&stats, 20, calm(), false), None);
        // Before the gate only the support popup's own policy decides.
        let young = SupportStats {
            launches: 1,
            first_day: Some(0),
            ..SupportStats::default()
        };
        assert_eq!(next(&young, 20, calm(), true), Some(Ask::Support));
        assert_eq!(next(&young, 20, calm(), false), None);
    }

    #[test]
    fn a_rating_after_a_share_respects_the_first_day_and_the_cool_downs() {
        let cfg = AskConfig::default();
        let mut stats = SupportStats {
            first_day: Some(5),
            ..SupportStats::default()
        };
        assert!(!rate_after_share(&stats, &cfg, at(5), false), "first day");
        assert!(rate_after_share(&stats, &cfg, at(6), false));
        assert!(
            !rate_after_share(&stats, &cfg, at(6), true),
            "once a session"
        );
        stats = on_rate_after_share(&stats, at(6));
        assert_eq!(stats.last_ask_day, Some(6));
        assert!(!rate_after_share(&stats, &cfg, at(35), false));
        assert!(rate_after_share(&stats, &cfg, at(36), false));
        stats.rate_asks = 3;
        assert!(
            !rate_after_share(&stats, &cfg, at(500), false),
            "lifetime limit"
        );
    }

    #[test]
    fn fast_mode_lowers_the_gate_only() {
        let fast = AskConfig::fast();
        assert_eq!((fast.min_launches, fast.min_active_days), (2, 2));
        assert_eq!(fast.ask_gap_days, AskConfig::default().ask_gap_days);
    }
}
