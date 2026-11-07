# Support Prompt Module Specification

## 1. Purpose & Overview
CalcCraft is a free, ad-free, open-source calculator project. To support ongoing development without compromising user trust or cluttering the primary calculation interface, this module provides an unobtrusive support mechanism consisting of:
1. An entry button at the top of the hamburger navigation pane ("Please Support!").
2. A semi-transparent floating card ("Please support us!") over the main application screen.
3. A non-intrusive trigger mechanism that presents the support popup every 5th completed calculation.

The feature is fully gated behind a Cargo feature flag (`support-prompt`, enabled by default) so future paid variants or customized builds can omit it entirely at compile time.

---

## 2. UI Layout & Visual Styling

### 2.1 Navigation Pane Button
- **Position:** Placed at the top of the hamburger drawer, directly to the right of the hamburger toggle button (`Id::new("nav-menu")`) and above the scrollable modes list header (`"CALCULATOR"`).
- **Dimensions:** Width occupies the available header area of the drawer (`~180-220 px`, padding of 8 px on the right), with a standard header button height (`32-36 px`) vertically centered in the 48 px header row.
- **Styling:**
  - Distinct rounded pill/card styling (corner radius 8 px).
  - Amber / warm gold accent background:
    - **Light theme:** Warm amber surface (`#FDE38C` / `#FFE699`) with high-contrast text (`#4D3100`) and a subtle golden border (`#E5C365`).
    - **Dark theme:** Deep amber surface (`#593B00` / `#4A3500`) with warm yellow text (`#FFE699`) and subtle amber border (`#7D5300`).
  - Subtle hover and active feedback matching Fluent design conventions (no harsh text outlines or garish contrast).
- **Accessibility:** Labeled with localized string `CalcCraft.Support.MenuButton` ("Please Support!"), focusable via `Tab` / `Shift+Tab`, activatable via `Enter` or `Space`.
- **Action:** Clicking closes the navigation drawer and opens the support popup on the main screen.

### 2.2 Floating Support Popup
- **Position & Geometry:** Floating panel centered over the main calculator area.
  - Width: ~75% of window width (bounded between 280 px and 520 px to look great both at narrow 480 px mobile/docked sizes and wide desktop windows).
  - Height: ~60% of window height (bounded between 220 px and 380 px).
- **Panel Aesthetics:**
  - Semi-transparent amber panel with rounded corners (12 px corner radius) and a subtle Fluent border (1 px stroke).
  - **Light theme:** Translucent warm amber surface (`rgba(255, 245, 225, 0.90)` / `Color32::from_rgba_unmultiplied(255, 245, 225, 230)`), border `rgba(225, 195, 130, 0.8)`.
  - **Dark theme:** Translucent deep amber surface (`rgba(45, 30, 8, 0.88)` / `Color32::from_rgba_unmultiplied(45, 30, 8, 225)`), border `rgba(130, 90, 25, 0.8)`.
  - The calculator display and keypad remain faintly visible underneath the translucent panel.
- **Header & Dismissal ("X"):**
  - Top-right corner hosts a close button displaying the Fluent dismiss glyph (`ICON_REMOVE`, `\u{ECC9}`).
  - Accessible name: `CalcCraft.Support.Close` ("Close").
  - Subtle hover state with red accent tint on hover (`#D13438` / `#E81123`), consistent with standard Windows close button behavior.
  - Clicking closes the popup.
- **Content:**
  - Centered title in semibold font (18-20 pt): `CalcCraft.Support.Title` ("Please support us!").
  - Brief friendly description text (13-14 pt) in secondary text color:
    `CalcCraft.Support.Body` ("CalcCraft is free and ad-free. Your support helps keep development active.").
- **Action Button ("Donate"):**
  - Located in the lower portion of the panel, centered.
  - Styled as a rounded accent button with Fluent green:
    - Background: Fluent green (`#107C41`), hover `#13894A`, active `#0E6B38`.
    - Text: White (`#FFFFFF`), semibold 14 pt.
  - Accessible name: `CalcCraft.Support.Donate` ("Donate").
  - Action: Invokes the platform URL handler `ctx.open_url(egui::OpenUrl::new_tab(DONATE_URL))`.
  - Constant:
    ```rust
    // TODO: Owner will confirm the real Stripe page URL.
    pub const DONATE_URL: &str = "https://rapalon.com/donate";
    ```

---

## 3. Behaviour & Non-Modality

- **Non-modal Interaction:**
  - The popup does **not** block calculator operation. The calculator controls underneath remain in the Tab order and interactable.
  - Keyboard input directed at the calculator (digits, arithmetic operators, backspace, etc.) continues to function normally while the popup is visible.
- **Dismissal & Stacking:**
  - The popup never stacks; there is only a single instance of `SupportState`. If a trigger fires while the popup is already displayed, it remains open without duplicating.
  - Pressing `Escape` while the popup is open closes the popup and consumes the `Escape` key (preventing accidental clearing of the current calculator input).
  - Clicking the "X" button dismisses the popup.
  - Clicking "Donate" opens the donation webpage in the default browser and leaves or dismisses the popup gracefully.
- **Open On-Demand:**
  - Activating the "Please Support!" button in the hamburger menu immediately opens the popup and closes the menu pane.

---

## 4. Definition of a "Calculation"

A calculation is defined strictly and unambiguously as:
> **A successful evaluation triggered by `=` or `Enter` in Standard, Scientific, or Programmer modes.**

Specifically:
1. **Trigger source:** The user presses the `=` button on the keypad or presses `=` or `Enter` on the keyboard while in `Mode::Standard`, `Mode::Scientific`, or `Mode::Programmer`.
2. **Success criterion:** The calculation engine evaluates the command without error (`!calc.has_error()`). If an evaluation results in an error (such as division by zero, overflow, or invalid input), it does **not** count.
3. **Excluded interactions:**
   - Raw key presses (typing numbers, intermediate operators like `+`, `-`, `*`, `(`, `)`).
   - Conversions in unit converters.
   - Date calculations.
   - Equation plotting / evaluation in Graphing mode.
   - Clipboard paste operations that do not submit `=`.

---

## 5. Storage & Privacy

All stats are stored locally on the user's machine through the app's existing `eframe::Storage` interface (which maps to the standard application data location on Windows and local storage on web).

- **No telemetry / No network:** No network requests, analytics, or external calls are ever made by this module.
- **Keys persisted:**
  - `SupportCalculationCount`: Total number of successful evaluations (stored as decimal string representing `u64`).
  - `SupportFirstUsed`: Timestamp of initial launch / first recorded calculation (stored as decimal string representing seconds since Unix epoch).
  - `SupportPopupShownCount`: Total number of times the support popup has been shown (stored as decimal string representing `u64`).

---

## 6. Architecture & Separation of Concerns

The module is housed under `src/support/`:
- `src/support/stats.rs`: Definition of `SupportStats`, loading from and saving to `eframe::Storage`, and tracking calculation events and first-use timestamps.
- `src/support/policy.rs`: Pure decision policy defining when to trigger the popup.
  - Trait `SupportPolicy` with method `fn should_show(&self, stats: &SupportStats, now: Option<u64>) -> bool`.
  - Default implementation `EveryNCalculations { interval: 5 }`: returns `true` when `stats.calculation_count > 0 && stats.calculation_count % 5 == 0`.
  - Free pure function `pub fn should_show(stats: &SupportStats, now: Option<u64>) -> bool` allowing drop-in replacement by future formulas without touching UI code.
- `src/support/view.rs`: State struct `SupportState`, layout metrics, Fluent color palette mapping for amber/green components, and egui drawing routines for the menu button and floating dialog.
- `src/support/mod.rs`: Module exports and public interface.

---

## 7. Feature Flag (`support-prompt`)

- Configured in `Cargo.toml`:
  ```toml
  [features]
  default = ["support-prompt"]
  support-prompt = []
  ```
- **When enabled (default, free build):** The menu button, floating dialog, calculation counter, and storage hooks are compiled and active.
- **When disabled (e.g. `cargo check --no-default-features` for a future paid variant):**
  - The `support` module, UI buttons, popup rendering, calculation tracking, and support strings disappear completely from compilation.
  - The project compiles cleanly and passes all tests without dead code or compiler warnings.

---

## 8. Localization & Fallback Strategy

- Strings are integrated into CalcCraft's localized strings file (`locales/calccraft.txt`) under `[en-US]`:
  - `CalcCraft.Support.MenuButton=Please Support!`
  - `CalcCraft.Support.Title=Please support us!`
  - `CalcCraft.Support.Body=CalcCraft is free and ad-free. Your support helps keep development active.`
  - `CalcCraft.Support.Donate=Donate`
  - `CalcCraft.Support.Close=Close`
- **Fallback behavior:** The string system's `Strings::get` checks the active language table and falls back to `self.english` if a key is not present in the current locale. Because all 5 keys are present in `[en-US]`, any non-English locale (e.g., German, French, Japanese) will display the English text gracefully without missing key assertions or panics.

---

## 9. Future Extension Points (Out of Scope for This Task)

The architecture is deliberately prepared for future owner enhancements:
1. **Smarter Frequency Formula:** The `SupportPolicy` trait allows replacing the `n % 5 == 0` rule with exponential backoff (e.g. 5, 20, 50, 100), time-decay formulas, or grace periods based on `stats.first_used`.
2. **Grace Period After Installation:** Policy check using `stats.first_used` to suppress prompts during the first N days of use.
3. **Prompt Frequency Ceiling:** Maximum number of automatic prompts per day or per lifetime.
4. **"Already Supported" Setting:** A future setting or dialog option to disable the automatic trigger permanently for donors.

*(Note: These extensions are documented as future design targets and are explicitly not implemented in this initial task).*

---

## 10. Scope & Verification Target

- **Verified Target:** Windows desktop build (`x86_64-pc-windows-msvc`).
- **Platforms Not Verified:** Web (WASM), Android, macOS, and Linux targets are deliberately not built or executed in this task per project scope instructions. All code is written using cross-platform abstractions (`eframe::Storage`, `egui::OpenUrl`) to ensure future compatibility across all supported platforms.
