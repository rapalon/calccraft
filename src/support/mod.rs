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
pub mod share;
pub mod stats;
pub mod view;

pub use donate::{DONATE_URL, DonationChannel, ExternalLink, donate_url};
pub use policy::{MenuEntry, SupportDecision, SupportEvent, SupportPolicyConfig, Timestamp};
pub use share::{ClipboardShare, SHARE_URL, ShareChannel, ShareOutcome, share_text, share_url};
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
            "https://rapalon.com/calccraft/donate?utm_source=calccraft&utm_medium=menu&utm_campaign=support\
             &utm_content={platform}_{version}&client_reference_id=cc_{platform}_{version}_menu_c0_d0-7",
            platform = donate::platform(),
            version = env!("CARGO_PKG_VERSION").replace('.', "-"),
        );
        assert_eq!(url.as_deref(), Some(expected.as_str()));
    }

    /// A supporter at day 10: "I already supported" was pressed on day 10.
    fn supporter() -> SupportState {
        let mut state = regular();
        state.stats.donate_clicked_ever = true;
        set_time(&mut state, at(10, 0));
        state.open_on_demand();
        state.handle(&Context::default(), PopupAction::AlreadySupported);
        state.popup = None;
        state
    }

    /// A context with the app's fonts (the thank-you line draws an icon).
    fn fonts_context() -> Context {
        let ctx = Context::default();
        crate::app::setup_fonts(&ctx, &[]);
        ctx
    }

    /// The menu's support entry drawn at the header's place in a 480 px window, with `events`.
    fn menu_frame(ctx: &Context, state: &mut SupportState, events: Vec<Event>) -> bool {
        let window = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(480.0, 640.0));
        let mut clicked = false;
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(window),
                events,
                ..Default::default()
            },
            |ui| {
                state.begin_frame(ui.ctx(), false, true, true);
                let rect = egui::Rect::from_min_max(pos2(52.0, 8.0), pos2(270.0, 40.0));
                clicked |= state.draw_menu_button(ui, rect, &crate::app::palette::LIGHT);
            },
        );
        output.textures_delta.clear();
        clicked
    }

    fn click_at(pos: egui::Pos2) -> [Vec<Event>; 2] {
        let button = |pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        [
            vec![Event::PointerMoved(pos), button(true)],
            vec![button(false)],
        ]
    }

    #[test]
    fn supporter_menu_shows_thanks_and_opens_the_popup_without_touching_the_pause() {
        let ctx = fonts_context();
        let mut state = supporter();
        assert_eq!(state.stats.supporter_until_day, Some(375));
        assert_eq!(state.menu_entry(), MenuEntry::SupporterThanks);
        let before = state.stats.clone();

        menu_frame(&ctx, &mut state, vec![]);
        assert!(!state.dot_visible());
        let [press, release] = click_at(pos2(120.0, 24.0));
        menu_frame(&ctx, &mut state, press);
        assert!(menu_frame(&ctx, &mut state, release));
        assert!(state.is_open());
        assert!(!state.popup.unwrap().automatic);
        assert_eq!(state.stats, before, "no cycle, the pause unchanged");

        // Donating again from it keeps the supporter state and the 365 days.
        state.handle(&ctx, PopupAction::Donate);
        assert!(!state.is_open());
        assert_eq!(state.stats.pause_until_day, Some(375));
        assert_eq!(state.menu_entry(), MenuEntry::SupporterThanks);

        // No dot and no automatic popup during the state, even at a calm moment.
        for day in [30, 200, 374] {
            set_time(&mut state, at(day, 0));
            state.after_command(Command::Equals, false);
            set_time(&mut state, at(day, 5000));
            frame(&ctx, &mut state, vec![], false);
            assert!(!state.dot_visible() && !state.is_open(), "day {day}");
            assert_eq!(state.menu_entry(), MenuEntry::SupporterThanks);
        }
        set_time(&mut state, at(375, 0));
        frame(&ctx, &mut state, vec![], false);
        assert_eq!(state.menu_entry(), MenuEntry::SupportButton);
        assert!(state.dot_visible());
    }

    #[test]
    fn supporter_thanks_line_presses_with_the_keyboard() {
        let ctx = fonts_context();
        let mut state = supporter();
        menu_frame(&ctx, &mut state, vec![]);
        ctx.memory_mut(|m| m.request_focus(egui::Id::new("support-thanks")));
        menu_frame(&ctx, &mut state, vec![]);
        assert!(menu_frame(&ctx, &mut state, vec![key(Key::Enter)]));
        assert!(state.is_open());
    }

    #[test]
    fn demo_supporter_opens_the_menu_on_the_thanks_line() {
        let mut state = SupportState::with_clock(None, Clock::fixed(at(0, 0)));
        state.start_demo("supporter");
        assert!(state.demo_opens_menu());
        assert_eq!(state.menu_entry(), MenuEntry::SupporterThanks);
        assert!(state.stats.donate_clicked_ever);
        assert!(!state.is_open());

        let mut state = SupportState::with_clock(None, Clock::fixed(at(0, 0)));
        state.start_demo("supporter-popup");
        assert!(!state.demo_opens_menu());
        assert_eq!(state.menu_entry(), MenuEntry::SupporterThanks);
        assert!(state.is_open() && !state.popup.unwrap().automatic);
    }

    fn copied_text(ctx: &Context) -> Option<String> {
        ctx.output(|o| {
            o.commands.iter().find_map(|command| match command {
                egui::OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            })
        })
    }

    #[test]
    fn share_copies_the_message_and_link_and_shows_the_note_for_two_seconds() {
        let ctx = Context::default();
        let mut state = regular();
        state.open_on_demand();
        state.handle(&ctx, PopupAction::Share);
        let expected = format!(
            "CalcCraft — a free, precise calculator: https://rapalon.com/calccraft?utm_source=calccraft\
             &utm_medium=share&utm_content={}_{}",
            donate::platform(),
            env!("CARGO_PKG_VERSION").replace('.', "-"),
        );
        assert_eq!(copied_text(&ctx).as_deref(), Some(expected.as_str()));
        // The popup stays (Donate is still there); the note shows, then goes by itself.
        assert!(state.is_open());
        assert_eq!(state.copied_until_ms, Some(2000));
        set_time(&mut state, at(0, 1999));
        frame(&ctx, &mut state, vec![], false);
        assert!(state.copied_until_ms.is_some());
        set_time(&mut state, at(0, 2000));
        frame(&ctx, &mut state, vec![], false);
        assert_eq!(state.copied_until_ms, None);
    }

    #[test]
    fn share_is_no_donate() {
        let ctx = Context::default();
        let mut state = escalated(&ctx);
        state.open_on_demand();
        // Opening it completed the cycle; the share doesn't complete another.
        let cycles = state.stats.cycles;
        assert_eq!(cycles, 1);
        state.handle(&ctx, PopupAction::Share);
        assert_eq!(state.stats.shares, 1);
        assert_eq!(state.stats.cycles, cycles);
        assert!(!state.stats.donate_clicked_ever);
        assert_eq!(state.stats.pause_until_day, None);
        assert_eq!(state.menu_entry(), MenuEntry::SupportButton);
        assert!(state.is_open());
    }

    #[test]
    fn supporter_share_glyph_presses_with_the_keyboard_and_keeps_the_state() {
        let ctx = fonts_context();
        let mut state = supporter();
        let before = state.stats.clone();
        menu_frame(&ctx, &mut state, vec![]);
        ctx.memory_mut(|m| m.request_focus(egui::Id::new("support-share-glyph")));
        menu_frame(&ctx, &mut state, vec![]);
        let closed = menu_frame(&ctx, &mut state, vec![key(Key::Enter)]);
        // The menu stays open (nothing is returned for it) and the popup doesn't open.
        assert!(!closed && !state.is_open());
        assert!(state.copied_until_ms.is_some());
        assert_eq!(state.stats.shares, 1);
        assert_eq!(state.stats.supporter_until_day, before.supporter_until_day);
        assert_eq!(state.stats.pause_until_day, before.pause_until_day);
        assert_eq!(state.menu_entry(), MenuEntry::SupporterThanks);
    }

    #[test]
    fn a_system_share_sheet_shows_no_note_but_counts_as_a_share() {
        struct Sheet;
        impl ShareChannel for Sheet {
            fn share(&self, _: &Context, _: &str) -> ShareOutcome {
                ShareOutcome::Sheet
            }
        }
        let ctx = Context::default();
        let mut state = regular();
        state.share_channel = &Sheet;
        state.open_on_demand();
        state.handle(&ctx, PopupAction::Share);
        assert_eq!(state.copied_until_ms, None);
        assert_eq!(state.stats.shares, 1);
    }

    #[test]
    fn the_toast_draws_over_the_menu_and_is_a_live_region() {
        let ctx = fonts_context();
        let mut state = supporter();
        ctx.memory_mut(|m| m.request_focus(egui::Id::new("support-share-glyph")));
        menu_frame(&ctx, &mut state, vec![key(Key::Enter)]);
        assert!(state.copied_until_ms.is_some());
        let mut output = ctx.run_ui(Default::default(), |ui| {
            state.draw_toast(ui.ctx(), &crate::app::palette::DARK);
        });
        output.textures_delta.clear();
        let area = egui::Id::new("support-link-copied-area");
        assert!(ctx.memory(|m| m.area_rect(area)).is_some());
    }

    #[test]
    fn demo_share_shows_the_popup_with_the_note() {
        let mut state = SupportState::with_clock(None, Clock::fixed(at(0, 0)));
        state.start_demo("share");
        assert!(state.is_open() && state.copied_until_ms.is_some());
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
