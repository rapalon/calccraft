//! The support prompt of the free CalcCraft build (feature `support-prompt`,
//! docs/support-prompt.md).
//!
//! A staged policy: silent for the first launches, then a red dot on the menu button, and only
//! later a popup at a calm moment, under the display. Increasing intervals between reminders,
//! pauses after Donate or "I already supported", and coarse tracking parameters on the donate
//! link. Local storage only: no network, no telemetry, no identifiers.
//!
//! The rest of the app only calls the hooks of [`SupportState`] and [`note_display`].

pub mod clock;
pub mod donate;
pub mod policy;
pub mod stats;
pub mod view;

pub use donate::{DONATE_URL, DonationChannel, ExternalLink, donate_url};
pub use policy::{SupportDecision, SupportEvent, SupportPolicyConfig, Timestamp};
pub use stats::SupportStats;
pub use view::{SupportState, note_display};

#[cfg(test)]
mod tests {
    use super::clock::Clock;
    use super::stats::tests::MemoryStorage;
    use super::view::PopupAction;
    use super::*;
    use crate::engine::Command;
    use eframe::egui::{self, Context, Event, Key, Modifiers, RawInput, pos2, vec2};

    const DAY_MS: u64 = 86_400_000;

    fn at(day: i64, ms: u64) -> Timestamp {
        Timestamp {
            millis: day as u64 * DAY_MS + ms,
            day,
        }
    }

    /// A state at day 0 of a regular user: 4 launches, 3 active days behind.
    fn regular() -> SupportState {
        let mut state = SupportState::with_clock(None, Clock::fixed(at(0, 0)));
        state.stats.launches = 4;
        state.stats.active_days = 3;
        state.stats.last_active_day = Some(-1);
        state
    }

    fn set_time(state: &mut SupportState, now: Timestamp) {
        state.clock = Clock::fixed(now);
    }

    fn frame(ctx: &Context, state: &mut SupportState, events: Vec<Event>, menu_open: bool) {
        let window = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(480.0, 640.0));
        let input = RawInput {
            screen_rect: Some(window),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            state.begin_frame(ui.ctx(), true, false, menu_open);
        });
        output.textures_delta.clear();
    }

    fn key(key: Key) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn launch_is_counted() {
        let state = SupportState::with_clock(None, Clock::fixed(at(5, 0)));
        assert_eq!(state.stats.launches, 1);
        assert_eq!(state.stats.first_day, Some(5));
    }

    #[test]
    fn only_a_successful_equals_is_a_calculation() {
        let mut state = regular();
        state.after_command(Command::Digit(2), false);
        state.after_command(Command::Equals, true);
        assert_eq!(state.stats.calculations, 0);
        assert_eq!(state.calculation_at_ms, None);
        state.after_command(Command::Equals, false);
        assert_eq!(state.stats.calculations, 1);
        assert_eq!(state.stats.active_days, 4);
        assert_eq!(state.calculation_at_ms, Some(0));
    }

    #[test]
    fn dot_then_menu_clears_it_after_it_closes() {
        let ctx = Context::default();
        let mut state = regular();
        frame(&ctx, &mut state, vec![], false);
        assert!(state.dot_visible());
        assert_eq!(state.stats.dot_since_day, Some(0));

        set_time(&mut state, at(1, 0));
        frame(&ctx, &mut state, vec![], true);
        // Seen: the cycle is complete, the dot stays while the menu is open.
        assert_eq!(state.stats.cycles, 1);
        assert!(state.dot_visible());
        frame(&ctx, &mut state, vec![], true);
        assert!(state.dot_visible());
        frame(&ctx, &mut state, vec![], false);
        assert!(!state.dot_visible());
        // Reopening the menu changes nothing more.
        frame(&ctx, &mut state, vec![], true);
        assert_eq!(state.stats.cycles, 1);
        assert!(!state.dot_visible());
    }

    /// Escalates the dot of `regular()` to the popup stage (day 7).
    fn escalated(ctx: &Context) -> SupportState {
        let mut state = regular();
        frame(ctx, &mut state, vec![], false);
        set_time(&mut state, at(7, 0));
        frame(ctx, &mut state, vec![], false);
        assert!(state.dot_visible());
        assert!(!state.is_open());
        state
    }

    #[test]
    fn popup_opens_only_at_a_calm_moment() {
        let ctx = Context::default();
        let mut state = escalated(&ctx);

        state.after_command(Command::Equals, false);
        set_time(&mut state, at(7, 2999));
        frame(&ctx, &mut state, vec![], false);
        assert!(!state.is_open(), "3 s haven't passed");

        // A key cancels the wait.
        frame(&ctx, &mut state, vec![key(Key::Num1)], false);
        set_time(&mut state, at(7, 10_000));
        frame(&ctx, &mut state, vec![], false);
        assert!(!state.is_open(), "input cancelled the moment");

        // The next calculation starts a new wait.
        state.after_command(Command::Equals, false);
        set_time(&mut state, at(7, 12_999));
        frame(&ctx, &mut state, vec![], false);
        assert!(!state.is_open());
        set_time(&mut state, at(7, 13_000));
        frame(&ctx, &mut state, vec![], false);
        assert!(state.is_open());
        assert!(!state.dot_visible());
        assert_eq!(state.stats.cycles, 1);
        assert_eq!(state.stats.popups_shown, 1);
    }

    #[test]
    fn popup_needs_focus_and_the_calculator() {
        let ctx = Context::default();
        let mut state = escalated(&ctx);
        state.after_command(Command::Equals, false);
        set_time(&mut state, at(7, 5000));
        let window = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(480.0, 640.0));
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(window),
                focused: false,
                ..Default::default()
            },
            |ui| state.begin_frame(ui.ctx(), true, false, false),
        );
        output.textures_delta.clear();
        assert!(!state.is_open(), "unfocused");
        let mut output = ctx.run_ui(Default::default(), |ui| {
            state.begin_frame(ui.ctx(), false, false, false);
        });
        output.textures_delta.clear();
        assert!(!state.is_open(), "a converter");
        let mut output = ctx.run_ui(Default::default(), |ui| {
            state.begin_frame(ui.ctx(), true, true, false);
        });
        output.textures_delta.clear();
        assert!(!state.is_open(), "a flyout over the calculator");
        frame(&ctx, &mut state, vec![], false);
        assert!(state.is_open());
    }

    #[test]
    fn one_automatic_popup_per_session_and_quick_close() {
        let ctx = Context::default();
        let mut state = escalated(&ctx);
        state.after_command(Command::Equals, false);
        set_time(&mut state, at(7, 3000));
        frame(&ctx, &mut state, vec![], false);
        assert!(state.is_open());
        // Esc a second later: annoyed, the next wait is 14 days.
        set_time(&mut state, at(7, 4000));
        frame(&ctx, &mut state, vec![key(Key::Escape)], false);
        assert!(!state.is_open());
        assert_eq!(state.stats.next_cycle_day, Some(21));

        // A new cycle in the same session shows the dot but no second popup.
        set_time(&mut state, at(21, 0));
        frame(&ctx, &mut state, vec![], false);
        set_time(&mut state, at(28, 0));
        state.after_command(Command::Equals, false);
        set_time(&mut state, at(28, 5000));
        frame(&ctx, &mut state, vec![], false);
        assert!(state.dot_visible());
        assert!(!state.is_open());
    }

    #[test]
    fn already_supported_shows_thanks_then_closes_and_pauses() {
        let ctx = Context::default();
        let mut state = regular();
        state.stats.donate_clicked_ever = true;
        state.open_on_demand();
        state.handle(&ctx, PopupAction::AlreadySupported);
        assert!(state.is_open());
        assert_eq!(state.stats.pause_until_day, Some(365));
        set_time(&mut state, at(0, 1499));
        frame(&ctx, &mut state, vec![], false);
        assert!(state.is_open(), "the thank-you text shows");
        set_time(&mut state, at(0, 1500));
        frame(&ctx, &mut state, vec![], false);
        assert!(!state.is_open());
    }

    #[test]
    fn closing_the_thank_you_early_is_no_annoyed_close() {
        let ctx = Context::default();
        let mut state = escalated(&ctx);
        state.stats.donate_clicked_ever = true;
        state.after_command(Command::Equals, false);
        set_time(&mut state, at(7, 3000));
        frame(&ctx, &mut state, vec![], false);
        assert!(state.is_open());
        state.handle(&ctx, PopupAction::AlreadySupported);
        let before = state.stats.clone();
        set_time(&mut state, at(7, 3500));
        frame(&ctx, &mut state, vec![key(Key::Escape)], false);
        assert!(!state.is_open());
        assert_eq!(state.stats, before);
        assert_eq!(state.stats.pause_until_day, Some(7 + 365));
    }

    #[test]
    fn donate_from_the_menu_popup_pauses() {
        let ctx = Context::default();
        let mut state = regular();
        state.open_on_demand();
        state.handle(&ctx, PopupAction::Donate);
        assert!(!state.is_open());
        assert!(state.stats.donate_clicked_ever);
        assert_eq!(state.stats.pause_until_day, Some(180));
        let url = ctx.output(|o| {
            o.commands.iter().find_map(|command| match command {
                egui::OutputCommand::OpenUrl(open) => Some(open.url.clone()),
                _ => None,
            })
        });
        let expected = format!(
            "https://rapalon.com/donate?utm_source=calccraft&utm_medium=menu&utm_campaign=support\
             &utm_content={platform}_{version}&client_reference_id=cc_{platform}_{version}_menu_c0_d0-7",
            platform = donate::platform(),
            version = env!("CARGO_PKG_VERSION").replace('.', "-"),
        );
        assert_eq!(url.as_deref(), Some(expected.as_str()));
    }

    #[test]
    fn state_persists() {
        let mut storage = MemoryStorage::default();
        let mut state = regular();
        state.stats.donate_clicked_ever = true;
        state.save(&mut storage);
        let again = SupportState::with_clock(Some(&storage), Clock::fixed(at(1, 0)));
        assert!(again.stats.donate_clicked_ever);
        assert_eq!(again.stats.launches, 5);
        let _ = eframe::Storage::get_string(&storage, stats::STATS_KEY).unwrap();
    }
}
