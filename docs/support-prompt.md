# Support Prompt Module Specification (v2)

## 1. Purpose

CalcCraft is free, ad-free and open source. The support prompt asks regular users, rarely and
politely, to support development. v2 replaces v1's temporary "every 5th calculation" rule with a
staged policy: people use the app first, then see a soft red dot, and only later a popup, shown
at a calm moment and never over the result.

Everything lives in `src/support/` behind the Cargo feature `support-prompt` (on by default).
Without the feature, the module, its UI, its strings' use and its stored state disappear at compile
time, and the build and tests pass (`cargo test --no-default-features`).

---

## 2. The policy (state machine)

Time is injected: the policy never reads a clock. A `Timestamp` has `millis` (wall clock, for the
seconds-long timers) and `day` (the local calendar day number, for the day-long ones). All "days"
below are calendar days of the user's local date, so "7 days" means "7 calendar days later",
whatever the hour.

```
            launches <= 3                launches >= 4 and active days >= 3
 [Stage 0: silent] ─────────────────────────────────────────────▶ [Stage 1: red dot]
                                                                     │        │
                         menu opened / popup opened from the menu    │        │ dot shown for
                         ("seen": cycle complete)                    │        │ 7 days unseen
                    ┌────────────────────────────────────────────────┘        ▼
                    │                                         [Stage 2: popup may open at
                    │                                          a calm moment; dot stays]
                    │                                                         │ popup shown
                    ▼                                                         ▼
          [Waiting: interval of the ladder, no dot, no popup] ◀── cycle complete (closed)
                    │ interval over
                    └────────────────────────────────▶ [Stage 1: red dot] (next cycle)

 Donate (popup or menu)  ──▶ pause everything for 180 days, donate_clicked_ever = true
 "I already supported"   ──▶ "Thank you!", pause everything for 365 days,
                             supporter state: the menu shows a green thank-you line (2.9)
```

### 2.1 Stage 0: silent

During the first **3** app launches (those before `min_launches`) nothing support-related appears by
itself: no dot, no popup. The amber "Please Support!" button stays in the navigation menu at all
times (it is on demand), just without a dot.

### 2.2 Eligibility ("regular users")

The soft stage may start only when both hold:

- `launches >= min_launches` (**4**), and
- the app was used on at least `min_active_days` (**3**) distinct calendar days. A day is active
  when at least one successful calculation (section 4) happened on it, by local date.

Only `last_active_day` and an `active_days` counter are stored, not a list of dates.

### 2.3 Stage 1: soft reminder (red dot)

- A small red dot badge on the hamburger (navigation) button, and, while the menu is open, on the
  "Please Support!" button too.
- The day the dot first shows in a cycle is stored (`dot_since_day`).
- Opening the navigation menu while the dot shows counts as **seen**: the cycle is complete (as
  for a shown popup) and the interval starts, even without a Donate click. Both dots stay in the
  open menu, so the user sees what was meant, and go away when it closes.
- Opening the popup from the menu button counts as seen too.

### 2.4 Stage 2: popup

- When the dot has shown for `dot_days_before_popup` = **7** days without being seen, the popup
  may open by itself, but only at a calm moment (2.5). The dot stays until the popup shows.
- At most **one** automatic popup per app session, never at app start (a calm moment needs a
  calculation made in this session).
- Showing the popup completes the cycle and clears the dot.

### 2.5 Calm moment

The owner's key point: never interrupt a calculation in progress. The popup opens by itself only
when all of these hold:

1. a successful calculation just finished (section 4);
2. since then there has been no further input (a key press, typed text, a pointer click or tap
   anywhere in the window, a paste) for `calm_delay_ms` = **3000** ms;
3. the app is visible and focused: egui's `InputState::focused` and the viewport not
   `minimized`. On Android and the web this is the same check (eframe reports a backgrounded
   activity or a hidden tab as unfocused), so the condition is written platform-neutrally;
4. the mode is Standard, Scientific or Programmer (never converters, date calculation, graphing
   or the Settings page), and nothing else is open over the calculator: the navigation menu, a
   history or memory flyout, a context menu, a mode's menu, or the popup itself.

Any input during the wait cancels this opportunity; the next successful calculation starts a new
wait. The module calls `ctx.request_repaint_after(remaining)` so that the timer fires without
input.

### 2.6 Placement

The popup never covers the result or the expression line. Every mode's display reports its
rectangle to the module (`support::note_display`), and the card is anchored below it, at the
bottom of the window over the memory row and keypad. It stays fully inside the window from the
320 px minimum and ~480 px up to wide windows (where it keeps its 480 px maximum width, centered
over the calculator part, left of a docked history panel). If the space under the display is too
short for the card, its contents scroll inside it rather than move up over the display.

The look is v1's: semi-transparent amber card, green Donate button, Fluent close X. It stays
non-modal (the calculator keeps working under it), never stacks, and closes with Esc or X.

### 2.7 Cycles and backoff

After a complete cycle (dot seen, or popup shown), the next cycle waits an interval, then starts
again at Stage 1 (dot first, popup after 7 unseen days). The interval comes from a ladder indexed
by `ladder_step`, which goes up by one per complete cycle:

`cycle_intervals_days` = **7, 14, 30, 60, 90**, then **90** for every later cycle (the cap).

### 2.8 Reactions

| Reaction | Effect |
| --- | --- |
| Automatic popup closed (X or Esc) within `quick_close_ms` = **2000** ms of appearing ("annoyed") | One extra step up the ladder: the wait becomes the next interval (7 → 14 days, …). |
| Donate clicked (popup or menu popup) | All automatic reminders pause for `donate_pause_days` = **180** days; `donate_clicked_ever = true`, never reset. The popup closes. |
| "I already supported" | Shown in the popup only when `donate_clicked_ever` is true (until then the popup has just Donate and Close). Secondary style, under Donate. Pressing it shows "Thank you!" for `thank_you_ms` = **1500** ms, closes the popup, and pauses automatic reminders for `already_supported_pause_days` = **365** days. |
| Share ("Tell a friend" or the supporter line's share glyph, section 12) | A supporter-friendly reaction: completes the running cycle like "seen" and adds one extra ladder step (once per cycle). Not a Donate: `donate_clicked_ever` stays, no pause, no supporter state. |
| Popup opened on demand from the menu | Counts as seen: completes the cycle if the dot was active, no penalty. Closing it quickly is no penalty either. Outside an active cycle it changes nothing. |

The on-demand menu button keeps working during every pause (in the supporter state it is the
thank-you line, 2.9).

### 2.9 Supporter state

While the pause of "I already supported" runs (`already_supported_pause_days` = **365** days from
the press, stored as `supporter_until_day`), the user is a **supporter**:

- The amber "Please Support!" button is not in the navigation menu. In its place, at the top of
  the menu, is one quiet line in green text: **"Supported by you"** (`CalcCraft.Support.Thanks`),
  followed by a small filled heart in the same green from the app's Fluent icon font
  (`ICON_HEART`, U+EB52, Segoe Fluent Icons' HeartFill), the owner's "Supported by you 💚". The
  heart is the icon-font glyph, not the 💚 emoji: egui draws emoji monochrome or not at all. No
  fill, no outline; a light hover/pressed tint and the usual focus visual.
- Green: Windows' success green (`SystemFillColorSuccess`), `#0F7B0F` in the light theme and
  `#6CCB5F` in the dark theme, both above 4.5:1 against the menu's background.
- The text wraps (never clips) in the narrow menu: the line takes the header's height, and a very
  long translation gets a smaller font. Without room for any text (under 90 px) only the heart
  shows, with "Supported by you" as its tooltip. The menu has no icons-only mode today, so that fallback is
  defensive.
- It is a focusable button (`access::control`, accessible name = the plain text "Supported by you", no heart). Click, Enter
  or Space opens the usual support popup, so a supporter can donate again. Opening it never
  resets or shortens the 365 days and never counts as a cycle (no cycle runs during the pause). A
  Donate click from it keeps the longer of the two pauses (`pause_until_day` never moves
  earlier) and keeps the supporter state.
- No red dot and no automatic popup (the pause already holds them back).
- A Donate click alone does **not** make a supporter (the user may not have paid): only
  "I already supported" does.
- When the 365 days are over, the amber button is back and the normal cycle resumes at Stage 1
  (the dot first).

Pure API: `policy::menu_entry(stats, cfg, now) -> MenuEntry` (`SupportButton` or
`SupporterThanks`).

---

## 3. Configuration: `SupportPolicyConfig`

All thresholds are fields of one struct in `src/support/policy.rs`, with `Default` holding the
values above, so they can be tuned in one place:

| Field | Default | Fast mode (6) |
| --- | --- | --- |
| `min_launches` | 4 | 2 |
| `min_active_days` | 3 | 2 |
| `dot_days_before_popup` | 7 | 7 |
| `cycle_intervals_days` | [7, 14, 30, 60, 90] (last one repeats) | same |
| `calm_delay_ms` | 3000 | 3000 |
| `quick_close_ms` | 2000 | 2000 |
| `donate_pause_days` | 180 | 180 |
| `already_supported_pause_days` | 365 | 365 |
| `thank_you_ms` | 1500 | 1500 |

Launches before `min_launches` are the silent stage: launches 1–3, so the 4th may show the dot.

### 3.1 The pure API (`src/support/policy.rs`)

```rust
pub fn evaluate(stats: &SupportStats, cfg: &SupportPolicyConfig, now: Timestamp,
                moment: MomentContext) -> SupportDecision;          // Nothing | ShowDot | ShowPopup
pub fn on_event(stats: &SupportStats, cfg: &SupportPolicyConfig, event: SupportEvent,
                now: Timestamp) -> SupportStats;
pub fn show_already_supported_button(stats: &SupportStats) -> bool;
pub fn menu_entry(stats: &SupportStats, cfg: &SupportPolicyConfig,
                  now: Timestamp) -> MenuEntry;                     // SupportButton | SupporterThanks
```

`SupportEvent`: `AppLaunched`, `CalculationDone`, `DotShown`, `MenuOpened`, `PopupShown`,
`PopupOpenedOnDemand`, `PopupClosed { after_ms, automatic }`, `DonateClicked`,
`AlreadySupported`. `MomentContext` holds what the calm-moment rule needs: the time of the last
successful calculation with no input since, whether the app is visible and focused, whether the
mode is a calculator mode with nothing open over it, and whether an automatic popup was already
shown this session. The UI only feeds events and draws decisions.

---

## 4. Definition of a "calculation" (unchanged from v1)

A successful evaluation triggered by `=` or `Enter` (keypad button or keyboard) in Standard,
Scientific or Programmer mode: `Command::Equals` executed and `!has_error()` afterwards. Typing,
intermediate operators, converters, date calculation, graphing and pastes that don't end in `=`
don't count. The check is `SupportState::after_command(command, has_error)` in the module; the app
only calls it after executing a command.

---

## 5. Storage and privacy

Local only, through the existing `eframe::Storage` (`%APPDATA%\CalcCraft\data\app.ron` on
Windows, `localStorage` in the browser). No network, no telemetry, no unique or per-install IDs.

One key, `SupportStatsV2`, holds a versioned text record (`v=2;launches=5;active_days=3;…`):
unknown fields are ignored and missing ones take their defaults, so later versions can add
fields.

| Field | Meaning |
| --- | --- |
| `launches` | app launches counted |
| `active_days`, `last_active_day` | distinct days with a calculation, and the last one |
| `first_day` | the day of the first launch (for the coarse days-since-install bucket) |
| `cycles` | complete cycles |
| `ladder_step` | index into the interval ladder |
| `last_cycle_day` | the day the last cycle completed |
| `next_cycle_day` | the earliest day the next cycle may start |
| `dot_since_day` | the day the current cycle's dot first showed (empty: no dot) |
| `pause_until_day` | no automatic reminder before this day (Donate, "I already supported") |
| `supporter_until_day` | the supporter state (2.9) lasts until this day ("I already supported" + 365; empty: none) |
| `donate_clicked_ever` | ever clicked Donate (never reset) |
| `shares` | Share clicks counted (informational) |
| `last_share_cycle` | the value of `cycles` at the last Share that moved the ladder (one extra step per cycle; empty: never) |
| `calculations` | successful calculations counted (kept from v1, informational) |
| `fast` | the record was written in fast mode (6), whose day numbers are minutes |

### 5.1 Migration from v1

v1 stored `SupportCalculationCount`, `SupportFirstUsed` (Unix seconds) and
`SupportPopupShownCount`. When `SupportStatsV2` is missing, they are read (unparsable values
are ignored, nothing panics) and mapped: the count goes to `calculations`; a user who already
calculated counts as having used the app on 1 active day; `SupportFirstUsed` becomes `first_day`;
launches start at 0 (v1 didn't count them). So v1 users go through the silent stage once. The
v1 keys are removed when v2 is saved.

A record written in fast mode is reset when read in normal mode and the other way around, since
their day numbers mean different things.

---

## 6. Debug / demo mode (fast time)

So the owner can see the flow without waiting weeks:

```powershell
$env:CALCCRAFT_SUPPORT_FAST = "1"; .\target\release\calculator.exe
```

- Active only while `CALCCRAFT_SUPPORT_FAST` is `1` or `true` (desktop and Android; the web
  version has no environment and never runs in fast mode).
- One "day" is one minute: day numbers are Unix minutes, so 7 days of dot take 7 minutes and the
  intervals take 7, 14, 30 … minutes. `CALCCRAFT_SUPPORT_DAY_SECS=10` shortens a day further
  (to 10 s) for quick checks.
- The launch thresholds drop (1 silent launch, 2 launches and 2 active "days" for eligibility).
- The seconds-long timers (calm 3 s, quick close 2 s) stay real.
- Each Donate prints the final URL to stderr (`support: donate URL …`).
- Since the stored record is marked `fast`, normal runs start fresh afterwards (5.1).

To jump straight to a stage (fast mode only), `CALCCRAFT_SUPPORT_DEMO` makes the user eligible
and starts in it: `dot`, `menu` (the menu open over the dot), `popup` (opened by itself),
`popup-supported` (after a Donate click: with "I already supported"), `thanks`, `supporter`
(the supporter state of 2.9, with the menu open on the thank-you line) or `supporter-popup` (the
supporter state with the popup as the thank-you line opens it).

Developer-only extra variables: `CALCCRAFT_SUPPORT_OPEN=1` opens the popup at start (v1),
`CALCCRAFT_SUPPORT_LONG_TEXT=1` triples the body text (layout checks), and the app's
`CALCCRAFT_THEME=Light|Dark`, `CALCCRAFT_WINDOW_SIZE=480x640` and `CALCCRAFT_SCREENSHOT_PATH`
take screenshots, e.g.:

```powershell
$env:CALCCRAFT_SUPPORT_FAST = "1"; $env:CALCCRAFT_SUPPORT_DEMO = "popup"; $env:CALCCRAFT_THEME = "Dark"
$env:CALCCRAFT_WINDOW_SIZE = "480x640"; $env:CALCCRAFT_SCREENSHOT_PATH = "$PWD\popup.png"
.\target\release\calculator.exe
```

---

## 7. Donate link and Stripe tracking

`DONATE_URL` is CalcCraft's own page on rapalon.com (support links are per product). The site
redirects it to CalcCraft's Stripe payment link, so the target changes in one place without an
app release, and campaigns stay trackable:

```rust
pub const DONATE_URL: &str = "https://rapalon.com/calccraft/donate";
```

The opened URL is `DONATE_URL` plus query parameters built by the pure function
`donate_url(base, &DonateContext)` (`src/support/donate.rs`):

| Parameter | Value |
| --- | --- |
| `utm_source` | `calccraft` |
| `utm_medium` | `popup` or `menu` (where Donate was clicked: an automatic popup, or one opened from the menu) |
| `utm_campaign` | `support` |
| `utm_content` | `<platform>_<version with dots as dashes>`, e.g. `win_1-3-2` |
| `client_reference_id` | `cc_<platform>_<version>_<source>_c<cycles>_<bucket>`, e.g. `cc_win_1-3-2_popup_c3_d30-90` |

- Platform: `win`, `mac`, `linux`, `android`, `web`, else `other`. Version: `CARGO_PKG_VERSION`.
- `c<cycles>`: complete reminder cycles so far, capped at `c10`.
- Days-since-install bucket from `first_day`: `d0-7`, `d7-30`, `d30-90`, `d90-365`, `d365p`.
- Every value is sanitized to Stripe's allowed characters (ASCII letters, digits, `-`, `_`) and
  cut to its limit (150 for UTM values, 200 for `client_reference_id`).
- Nothing in it identifies a person or an installation: many users share every value.

Example:
`https://rapalon.com/calccraft/donate?utm_source=calccraft&utm_medium=popup&utm_campaign=support&utm_content=win_1-3-2&client_reference_id=cc_win_1-3-2_popup_c3_d30-90`

Notes for the owner's setup:

- Stripe Payment Links accept these UTM parameters and `client_reference_id`, which arrives in
  the `checkout.session.completed` webhook and in the dashboard.
- `rapalon.com/calccraft/donate` redirects to the Stripe Payment Link, the redirect **must forward the
  query string**, or the parameters are lost.
- The UTM codes reach the page shown after payment only when the Payment Link's confirmation
  behaviour is set to "redirect to your website".

---

## 8. Store builds: `DonationChannel`

Google Play and the App Store require their own in-app purchase for tips and paid versions.
`src/support/donate.rs` defines:

```rust
pub trait DonationChannel {
    /// Starts a donation for a Donate click from `source`.
    fn donate(&self, ctx: &egui::Context, request: &DonateContext);
}
pub struct ExternalLink; // opens donate_url(DONATE_URL, request) with ctx.open_url
```

`SupportState` holds a `Box<dyn DonationChannel>`, `ExternalLink` today. A store build would add
a `PlayBilling` (Google Play Billing through JNI, next to `src/app/android.rs`) or `StoreKit`
implementation and pick it in `SupportState::new` under its `cfg`; a "buy the paid version" action
would be another method of the trait (or a second channel) shown as a popup button. The policy,
stats and UI stay as they are. Nothing of this is implemented yet: no SDKs.

---

## 9. Integration (module boundary)

All logic, state, UI and strings handling live in `src/support/`. The rest of the app has only
thin hook calls, each behind `#[cfg(feature = "support-prompt")]`:

| Hook | Where |
| --- | --- |
| `SupportState::new(storage)` / `save(storage)` | app creation and `save` |
| `support.begin_frame(ctx, calculator_mode, covered)` | start of `draw_frame` (input watch, calm timer, Esc, auto open) |
| `support.after_command(command, has_error)` | after the calculator executes a key |
| `support::note_display(ctx, rect)` | where a mode draws its display |
| `support.draw_menu_badge(ui, rect)` | on the hamburger button |
| `support.on_menu_opened()` | when the navigation menu opens |
| `support.draw_menu_button(ui, rect, p)` | in the navigation header (the amber button, or the thank-you line in the supporter state); returns true to close the menu |
| `support.draw_overlay(ui, window, p)` | last, over the page |

Files: `policy.rs` (config, decisions, events: pure), `stats.rs` (record and persistence,
migration), `donate.rs` (URL, `DonationChannel`), `clock.rs` (real and fast time),
`view.rs` (`SupportState`, drawing), `mod.rs` (exports and hooks).

---

## 10. Strings and accessibility

`locales/calccraft.txt`, `[en-US]` (other languages fall back to English):

- `CalcCraft.Support.MenuButton=Please Support!`, `.Title`, `.Body`, `.Donate`, `.Close` (v1)
- `CalcCraft.Support.Reminder=Support reminder` — the dot's accessible description on the
  hamburger and menu buttons
- `CalcCraft.Support.AlreadySupported=I already supported`
- `CalcCraft.Support.ThankYou=Thank you!`
- `CalcCraft.Support.Thanks=Supported by you` — the supporter state's menu line
- `CalcCraft.Support.Share=Tell a friend`, `.ShareTooltip=Share CalcCraft`,
  `.ShareMessage=CalcCraft — a free, precise calculator:`, `.LinkCopied=Link copied` (section 12)

The new button goes through `access::control` (Tab, Enter, Space). The popup is non-modal and
traps no focus.

---

## 11. Verification scope

Built, run and verified on Windows desktop (`x86_64-pc-windows-msvc`) only: `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings` (with and without default features), `cargo test`
(with and without), `cargo build --release`, and a walk through the whole flow in fast mode. The
code uses only cross-platform pieces (`eframe::Storage`, `egui::OpenUrl`, `web_time`, chrono's
local date), but macOS, Linux, Android and the web version are not run in this task.

---

## 12. Share with a friend

Sharing is a free way to support CalcCraft, for people who won't donate. It is a quiet secondary
action, never competing with Donate. No network, no telemetry, no identifiers.

### 12.1 The link

One universal URL for every platform (the page picks the right download, and can change without
an app release):

```rust
pub const SHARE_URL: &str = "https://rapalon.com/calccraft";
```

`share_url(base, &ShareContext)` (`src/support/share.rs`, pure, tested) adds
`utm_source=calccraft&utm_medium=share&utm_content=<platform>_<version>` (same sanitizing as the
donate link), e.g. `https://rapalon.com/calccraft?utm_source=calccraft&utm_medium=share&utm_content=win_1-3-2`.
It carries no cycle count, no age bucket and no `client_reference_id`: whoever opens a shared link
is somebody else, and the sender is not identified.

### 12.2 Where it shows

- **Popup:** under Donate (and under "I already supported" when that shows), a text-style action
  with the share glyph and **"Tell a friend"**: no fill, no outline, secondary text color. It is in
  every variant of the popup (not in the 1.5 s "Thank you!" state).
- **Supporter state:** a small 32 px share glyph button (Fluent `ICON_SHARE`) at the right end of
  the "Supported by you" line, tooltip and accessible name **"Share CalcCraft"**. The line keeps
  wrapping in the remaining width.
- Both are `access::control`s: reachable with Tab, pressed with Enter or Space, no focus trap.

### 12.3 What it does, per platform

The text is `"<ShareMessage> <share_url>"`. `ShareChannel` (like `DonationChannel`) is the seam.

| Platform | Today |
| --- | --- |
| Windows, macOS, Linux | Copies the text to the clipboard (`Context::copy_text`) and shows "Link copied" for 2 s. |
| Web | `navigator.share` where the browser has it (`WebShare`): its sheet is the feedback, so no note (`ShareOutcome::Sheet`). Otherwise the same clipboard copy and note. A failed clipboard write has no manual-copy fallback yet (egui's `copy_text` can't report it): follow-up. |
| Android | The same clipboard copy. A system share sheet (`ACTION_SEND` through JNI, next to `src/app/android.rs`) isn't implemented yet. |

The existing `src/app/share.rs` is the Graphing mode's Windows-only share sheet for an image and
HTML, so it isn't reused. `ShareChannel::share` returns a `ShareOutcome` (`Copied`, `Sheet`,
`Failed`); only `Copied` shows the note. "Link copied" is a small non-modal note at the bottom of
the window, announced politely to screen readers, that disappears by itself after 2 s. It is
drawn by `SupportState::draw_toast` on its own top layer (`Order::Tooltip`) at the end of every
frame, so it shows in every mode and over the navigation menu, where the supporter line's share
glyph lives (drawn in the popup's overlay it was hidden under the menu). It has no fade, so
reduced motion needs nothing special.

### 12.4 Policy effect

`SupportEvent::ShareClicked`: if a cycle is running (the dot shows) it completes like "seen";
then, once per cycle (`last_share_cycle`), the ladder moves one extra step and the next cycle
starts no earlier than the longer interval (the same arithmetic as an "annoyed" close). It sets
neither `donate_clicked_ever` nor the supporter state, and starts no pause. `shares` counts the
clicks.

### 12.5 Demo

`CALCCRAFT_SUPPORT_DEMO=popup` and `popup-supported` show "Tell a friend"; `supporter` shows the
glyph on the thank-you line. `CALCCRAFT_SUPPORT_DEMO=share` is the popup with the "Link copied"
note already showing (for screenshots).
