# AGENTS.md

Guidance for AI coding agents (Claude Code, Codex, Copilot, Cursor, …) and human contributors.
Read this before changing code. Agent-specific files (such as `CLAUDE.md`) only add to it.

## What this project is

A clone of the Windows 11 Calculator's **Standard**, **Scientific**, **Graphing**, **Programmer**
and **Date calculation** modes and its unit **converters** (all but Currency), written in Rust
with eframe/egui.
**The real Windows Calculator is the specification.** Visual style and behavior should match it,
and the user-facing changes that are welcome are the ones that bring the app closer to the original.

## Commands

| Task | Command |
| --- | --- |
| Build | `cargo build` |
| Run (with a console for logs) | `cargo run` |
| Run (like an end user) | `cargo run --release` |
| Tests | `cargo test` |
| Format | `cargo fmt` |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| Lint the web version | `cargo clippy --target wasm32-unknown-unknown -- -D warnings` |
| Check the dependencies' licenses | `cargo deny check licenses` (`cargo install cargo-deny --locked`) |
| Run the web version | `trunk serve` (http://127.0.0.1:8080; `trunk build --release` writes `dist/`) |

**Definition of done:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test` and the web version's `cargo clippy --target wasm32-unknown-unknown -- -D warnings`
all pass, and UI changes have been checked by actually running the app (see "Verifying UI
changes"). CI (`.github/workflows/ci.yml`) runs the first three on Windows, Linux and macOS
for every push and pull request and builds the web version with Trunk, and a pull request
isn't done until all of that passes. Code for Windows alone goes under `#[cfg(windows)]`, and
what only it uses gets `#[cfg_attr(not(windows), allow(dead_code))]`, or clippy fails on the
others; code for the browser alone goes under `#[cfg(target_arch = "wasm32")]` the same way, and
code for the Android app alone under `#[cfg(target_os = "android")]` (see `android/README.md`).

## Architecture

```
src/main.rs              starts the app: `run` on desktops, `start_web` in the browser
src/lib.rs               the modules; window options and eframe::run_native, the web version's
                         eframe::WebRunner, and the Android app's `android_main`
index.html, Trunk.toml   the web version's page and its Trunk build (deployed by pages.yml)
android/                 the Android app (prototype): manifest, icons, themes, build-apk.ps1
assets/web/              the web version's icons, app manifest and service worker (sw.js): a PWA
src/icon.rs              the app icon's images for the window and the .ico, std only (also compiled
                         into build.rs); tools/app-icon.py scales them, the Android, Play and
                         web icons from Konst's PNGs in android/icon-source/
src/brand.rs             the company credit: "CalcCraft by Rapalon Labs", the company's name and URL
build.rs                 embeds the icon and version info into calculator.exe (Windows only)
src/engine.rs            calculator logic, no UI dependencies, unit tests at the bottom
src/engine/math.rs       the Scientific mode's functions (a child module of engine.rs), unit tests
src/engine/integer.rs    the Programmer mode's integer mode (a child module of engine.rs), unit tests
src/engine/caret.rs      the caret in the number being typed (CalcCraft's own), unit tests
src/engine/tape.rs       the tape: replaying a calculation with a number edited, the history's
                         Edit (CalcCraft's own), unit and property tests
src/dates.rs             date calculation logic (chrono), no UI dependencies, unit tests
src/converter.rs         unit converter logic and unit data, no UI dependencies, unit tests
src/locale.rs            regional number formats and date patterns, no UI dependencies, unit tests
src/graphing.rs          the Graphing mode's equations (y = f(x), x = g(y), implicit), no UI, tests
src/graphing/parse.rs    the equation parser and Windows' syntax errors
src/graphing/expr.rs     expressions: f64 evaluation, angle units, symbolic derivative
src/graphing/plot.rs     views, grid steps, plotting, inequality regions, best fit, tracing
src/graphing/analysis.rs the key graph features (function analysis)
src/graphing/exact.rs    exact values (fractions, multiples of π, square roots) for the analysis
src/app.rs               egui user interface (custom painting), modes, menu, keyboard, fonts, title bar
src/app/access.rs        keyboard focus, focus visuals and screen reader names of every control
src/app/caret.rs         where the engine's caret is in the text shown (grouping, padding), pure
src/app/icons.rs         the icons: code points of the embedded icon font, and a test that it has them
src/app/strings.rs       the interface's text in the user's language (from locales/)
src/app/region.rs        the user's regional formats, read from Windows, the browser's Intl or
                         Java (docs/regional-formats.md); en-US on Linux and macOS
src/app/region/java.rs   Java's locale data as `Formats` (the Android app): separators and grouping
                         off a formatted number, first day, names, a Java date pattern formatter (pure)
src/app/web.rs           the web version's ties to the browser: URL parameters, languages, Intl,
                         clipboard, downloads, browser shortcuts (wasm32 only)
src/app/android.rs       the Android app's ties to the system (JNI): the system bars' insets as
                         egui's safe area, the back button, languages, the regional formats
                         (`region_source`, `display_name`), the keys' vibration (Vibrator, the
                         touch feedback setting), whether the system keyboard is showing
                         (`ime_visible`) (Android only)
src/app/vibration.rs     the Android key vibration: mappings, the Auto style rule, the card rows (pure)
src/app/orientation.rs   the Android screen orientation setting: choices, stored keys, the
                         `setRequestedOrientation` values, where Android honors them (pure)
src/app/layout_tests.rs  the layouts at the Android screen sizes, and a software rasterizer that
                         paints them into target/shots/ (`write_layout_shots`, ignored)
src/app/a11y_tests.rs    the accessibility tree through AccessKit's consumer: bounds and names of every
                         node, at most 50 children a node (Marionet's limit), popup menus, the
                         expression line's parts, a focus that is always in the tree, and the
                         device scenarios run on the host (`device_scenarios_pass_on_the_host`)
tests/device/            device tests by accessible name: scenarios/*.json, run.ps1 (adb and
                         uiautomator, `-SelfTest` in CI), README.md (the steps, the Marionet prompt)
src/app/palette.rs       the color palettes: Classic's `DARK` and `LIGHT`, and every colour theme's
                         (`THEMES`, `ThemeSpec`; Gold's `Metal` key), with contrast tests
src/app/color_theme.rs   the Colour theme setting: themes, stored keys, gating by channel (pure)
src/channel.rs           the distribution channel (Cargo features `play-free`, `play-paid`) and
                         the paid app's ID (`CALCCRAFT_PAID_ID`), in every build
src/app/math_label.rs    the keys' formula labels (x², ²√x, xʸ …): measured layout, vector radical
src/app/share.rs         the graph's share: Windows' share sheet (`IDataTransferManagerInterop`),
                         Android's (one cached PNG for android/java's ShareProvider), or a
                         PNG download in the browser
src/app/date_view.rs     the date calculation mode's UI (a child module of app.rs)
src/app/converter_view.rs  the converters' UI (a child module of app.rs)
src/app/scientific_view.rs  the Scientific mode's UI (a child module of app.rs)
src/app/programmer_view.rs  the Programmer mode's UI (a child module of app.rs)
src/app/graphing_view.rs    the Graphing mode's UI: layout, equations, variables, analysis, flyouts
src/app/graph_canvas.rs     the Graphing mode's graph and the buttons over it
src/app/graphing_keypad.rs  the Graphing mode's keypad and its menus
src/support/             the support prompt module (feature `support-prompt`): the pure staged policy,
                         stored stats, donate link and channel, clock, red dots, menu button, popup;
                         the distribution channel (`play-free`, `play-paid`: Google Play)
locales/<language>.txt   the strings, generated by tools/import-strings.ps1 from Windows' resources
assets/fonts/            the fonts compiled into the executable (icons, Inter) and their licenses
vendor/accesskit_winit   a copy of accesskit_winit 0.32.2 (Apache-2.0) that `[patch.crates-io]` puts in
                         place of the release, with one change: its Android adapter hooks
                         NativeActivity's content view, from Android 8.0 (android/README.md,
                         "Accessibility"; re-apply it when eframe moves to a new accesskit_winit)
tools/icon-font.py       builds assets/fonts/CalcCraftIcons.ttf from Fluent System Icons
PRIVACY.md, TERMS.md     the privacy policy and the Terms of Service (rapalon.com/calccraft/privacy/
                         and /terms/)
website/                 CalcCraft's page on rapalon.com (calccraft.md, screenshots) and sync.mjs,
                         which copies it and the two legal texts into rapalon/website (README.md)
```

### `engine.rs`

- `Calculator` is the whole state. The UI only calls `execute(Command)`, the memory and
  history methods, `paste`, and the read-only getters (`display`, `expression`, `is_enabled`,
  `memory`, `history`, `copy_text`).
- The engine is a port of the Windows Calculator's `CCalcEngine::ProcessCommandWorker`
  (`CalcManager/CEngine/scicomm.cpp`) and `CHistoryCollector` (`History.cpp`). The fields keep
  Windows' meaning, and their comments give the Windows names (`record` is `m_bRecord`,
  `no_prev_equ` is `m_bNoPrevEqu`, …). To answer "what does this key do here", read the
  matching branch of `ProcessCommandWorker`. Every command starts with `begin`, which finishes
  the number being typed and remembers the last two commands (`last_key`, `this_key`).
- `Calculator::default()` is the Standard mode (immediate execution, 16 digits).
  `Calculator::with_precedence()` is the Scientific mode: Windows' precedence flag (operator
  precedence, a stack of pending operations and parentheses, 25 levels each), 32 digits
  (`digits`, `m_precision`), the angle unit (`angle`, `m_angletype`) and F-E (`notation`,
  `m_nFE`). `Calculator::programmer()` is the Programmer mode: precedence plus Windows' integer
  mode (`integer: Option<IntegerMode>`, `m_fIntegerMode`) with a radix and a word size, and no
  history (Windows gives that engine no history display). `is_enabled` hides each mode's keys
  from the others (`exp` stays in the Standard mode, which paste uses).
- The typed number (`Input`, `CalcInput`) can have an exponent after `exp` (`Exponent`, at most 4
  digits). A finished number whose exponent goes beyond ±9999 is `Overflow`, and the command
  goes on, as in Windows.
- `HistoryEntry::text` keeps the result as the display showed it (digits and F-E), like Windows'
  history string; the lists show it.
- The expression line is a token list (`HistoryLine`). As in Windows, the text shown
  (`shown`) only refreshes at certain steps, which is why `1 ÷ 0 =` keeps showing `1 ÷`.
- `paste` checks the text with the rules of Windows' `CopyPasteManager` (`paste_is_valid`),
  then types it in key by key.
- Numbers are `BigRational` (`Number`). `normalize` checks every result for overflow and
  rounds results with huge denominators to 40 significant digits. `sqrt` is exact for
  perfect squares.
- `format_with(v, grouping, digits, scientific)` ports `NumberToString`: display text with
  grouping, expression-line text without, with 16 or 32 digits, and with F-E always in
  scientific notation (`5.e+0`, `0.e+0`). `Calculator::format_value` uses the engine's settings;
  the tests' `format_number` is the Standard mode's.
- Pasted keys go straight to the engine, as Windows' `OnPaste` sends them: an error stops the
  rest of the paste, a `0` never takes the pending sign, and the opening CE leaves F-E on.
- Tests use the `run("2+3*4=")` helper, a mini key language described above `press` in the
  test module. It returns `(display, expression)`. Scientific tests use
  `run_scientific("30 sin + 2 ^ 3 =")` and Programmer tests `run_programmer("hex FF and 3C =")`,
  whose words are described above `press_scientific` and `press_programmer`.

### `engine/math.rs` (Scientific mode)

- The functions take and return exact rationals (`Number`). `BigRational` has no `sin` or `ln`,
  so they compute on `Float`, a binary floating point number with a `BigInt` mantissa, with
  about 400 bits (`PREC`), more for small arguments (`extra_bits`) and for radians of large
  angles, and `finish` rounds the result to `INTERNAL_DIGITS` (40) significant digits. That's why
  exact results stay exact (`sin(30°)` is 0.5, `log(1000)` is 3): don't skip `finish`.
- The series: `exp_float` (argument reduced by ln 2 and halved), `ln_float` (atanh series near 1,
  which keeps its precision there), `sin_cos_sums`, `atan_small`, Machin's formula for π, and
  Stirling's series with Bernoulli numbers for the gamma function (`factorial` of fractions).
- Degrees and gradians are reduced to a quarter turn exactly in rationals, so multiples of 90°
  give exact 0 and ±1; then Windows' rule makes |sin|, |cos| ≤ 10^-32 zero (`rat_smallest`),
  which is what makes `tan(90°)` and `tan(π/2)` `Invalid input`.
- The domain checks follow `ratpak`, function by function: `exprat`'s ±100000, `powratcomp`'s
  odd denominators for negative bases and "within 10^-32 of an integer", `factrat`'s 3249 and
  −1000 (`Overflow`), `IsValidForHypFunc`, the reciprocal functions dividing by zero, and
  `IsCurrentTooBigForTrig` (10^100, positive values only) in `Calculator::unary`.
- `power` keeps integer powers exact while the numbers stay below some 100000 bits.

### `engine/integer.rs` (Programmer mode)

- The integer mode keeps Windows' design: values stay `Number` rationals, and
  `Calculator::display_num` truncates `current` to an unsigned integer of the word size
  (`IntegerMode::truncate`, Windows' `TruncateNumForIntMath`) at every place where Windows calls
  `DisplayNum`. Don't truncate anywhere else: a quotient or a right shift inside parentheses keeps
  its fraction for the next operation, as in Windows (`(255 AND 10 − 7 ÷ 2)` is 6, not 7).
- `IntegerMode::apply` ports `DoOperation`'s integer branches: `boolean` does `ratpak`'s
  `boolrat` (on magnitudes, with the sign of the right operand), division and `%` are signed via
  the highest bit, and a shift by the word size or more is `CalcError::NoResult`.
  `rotate`, `not` and `toggle_bit` port `SciCalcFunctions` and `TryToggleBit`.
- `format` writes a value in a radix (signed only in decimal, `GetStringForDisplay`), `group`
  adds the digit groups (`GroupDigitsPerRadix`) and `pad_binary` the view model's leading zeros.
  The display and `radix_values` combine them; the expression line has no groups.
- The expression line's operands keep their values (`Token::value`), so a new radix rewrites
  them (`UpdateHistoryExpression`), from the unsigned value, like Windows.
- `digit_fits` and `sign_toggles` port `CalcInput`'s integer limits; `paste_operand_is_valid`
  ports `CopyPasteManager`'s Programmer patterns and limits (no regex crate: `operand_matches`).

### `engine/caret.rs` and `engine/tape.rs` (editing, CalcCraft's own)

Windows has no caret and no editable expression line; CalcCraft adds them, always on, and
nothing changes until the display or the line is touched. The spec and every choice made are in
`docs/research/editable-expression.md` ("Decisions").

- **A, the caret** (`caret.rs`): `Input::caret` is a position in the typed characters (digits and
  point, then the exponent's digits as a second segment), never in the text shown. Every edit is
  checked by typing the edited number again from scratch (`Input::retype`), so the typing limits
  hold at the caret. A hidden caret, or one at the end, is exactly the old typing.
- **B, the tape** (`tape.rs`): `execute` records every command since the last `C`, a typed number
  as one `Item::Operand` with the keys that typed it. `Token::source` links an operand of the
  expression line to its tape item (`current_source`; every assignment to `current` clears it),
  and `expression_parts` gives those numbers' places in the line. An edit replaces the item and
  replays the tape into a fresh engine (`replayed`), which is what typing the edited keys fresh
  gives: the property tests `replaying_an_edit_is_typing_it_fresh` and
  `caret_edits_replay_the_same` check it in all three modes. Keep these rules when the engine
  changes: a replay (`replaying`) adds no history and doesn't touch the memory; MR, `rand` and a
  a history entry without a tape replay the value they gave (`Item::Recall`, `Random`,
  `HistoryRecall`); a history entry with a tape is recalled by replaying it, then `Recalled`
  leaves the engine as Windows' recall does (`recall_tape`, `settle_recall`); a new
  public entry point that changes the state has to record itself on the tape; and the state
  after `C` must equal a fresh engine with the tape's settings.
- A number whose keys were made from its digits (`Input::keys`, after the caret edited it) also
  remembers how its typing ended, its last key and whether the line was hidden
  (`Operand::finish`), since those keys can end otherwise and the next key depends on it.
- While a number of the line is edited (`Edit::Line`), `display` and `error` show what `=` would
  give (`preview`), and the line shows the edited number (`expression_parts`). `=`, any other key
  and a tap elsewhere commit it (`commit_edit`): an edited finished calculation adds a new
  history entry. Esc (`hide_entry_caret`) puts the original engine back. A tap on a computed
  display value edits a typed copy (`Edit::Display`) that replaces its operand once a key
  changes it (`display_copy_start` says where that operand starts on the tape).
- The line's parts (`ExpressionPart`, `PartKind`) are typed numbers, computed numbers and binary
  operators; the UI edits any of them with `edit_part`. A computed number (`Token::value`
  without a `source`) is first typed as a literal in place of the steps that computed it
  (`materialize`): the first range of steps from `PartKind::Computed { floor }` whose
  replacement keeps a `Signature` (line, display, settings, the next two `=`) is taken, and
  none means it can't be edited. A commit that changed nothing puts the original tape back. A
  selected operator (`Target::Operator`, `selected_operator`, a token with `operator` set whose
  item is an `Operator` command) takes the next operator key, which replaces that tape item
  and replays (`edit_key`); `is_enabled` keeps the operator keys on while one is selected, even
  in the error state. Editing never recomputes other history entries.
- **C, Edit in the history**: `HistoryEntry` keeps the tape that logged it (`entry_tape`, with
  `=` for a Standard step an operator logged); `edit_history` replays it and puts the caret
  into its last number.

### `app.rs`

- Everything is painted by hand. `button_flash` draws every key, and the layout is computed
  from the window rect in `App::ui`. The only stock egui widgets are the `ScrollArea`s of the
  lists and the `TextEdit`s of the Graphing mode (its equation, variable and range boxes).
- All colors come from `Palette` in `app/palette.rs`, which has a `DARK` and a `LIGHT` constant.
  `App::ui` picks one from `ctx.theme()` at the start of every frame. Drawing methods read it
  with `let p = self.palette;`, and free functions take `p: &Palette`. Don't put color
  literals in drawing code: add a field to `Palette` and fill it in both themes.
- Keypad colors follow the opacity table in the Windows Calculator's `App.xaml`: white at a fixed
  opacity over the background, computed with `white()` / `black()` / `over()`. Examples: number
  keys are 12.5 % in dark and 100 % in light; operators are 8.52 % in dark and 50 % in light.
  A unit test checks that the dark values still match the colors sampled from the reference
  screenshot.
- The App theme choice (`Theme`) maps to egui's `ThemePreference`. With `System`, egui follows
  the Windows app mode, including live changes. `TitleBar::apply` recolors the system title bar
  whenever the effective theme changes. The choice is saved through eframe storage under
  Windows' own key and values (`SelectedAppTheme` = `Light` / `Dark` / `Default`).
- The Language and Regional format cards (`ComboCard`, `draw_combo_card`, `draw_settings_list`)
  are CalcCraft's own; Windows has no such settings. They switch `strings::set_language` and
  `region::set_region` at runtime and save `CalcCraftLanguage` and `CalcCraftRegionalFormat`
  (a tag, or `Default` for the Windows setting). Their lists show names in their own scripts, so
  opening one loads `strings::LIST_FONTS` (`all_scripts`), which go again with the page.
  The Regional format card is left out where `region::regions()` is empty (Linux, macOS: only
  "Use system setting", which changes nothing there; `show_region_card`). The web version and
  the Android app list the translations' regions; `docs/regional-formats.md` has every
  platform's sources and the Linux and macOS proposal.
- The Colour theme card (CalcCraft's own, right under App theme; `docs/color-themes.md`) picks a
  `ColorTheme` (`app/color_theme.rs`): Classic, and the Pro themes Gold, Night, Retro LCD,
  Paper, AMOLED Black and High Contrast, saved as `CalcCraftColorTheme`. Each theme is data: a `ThemeSpec` in
  `palette::THEMES` with its dark and light `Palette` (most built with struct update from
  `DARK` / `LIGHT`) and the egui theme it fixes (Night, AMOLED Black and High Contrast dark;
  Paper and Retro LCD light), which
  overrides App theme's preference while it is on. Gold's `=` is a `Metal` gradient
  (`paint_metal_key`); High Contrast sets `key_border` (2 px) and `bold_keys`. A new theme is a palette, a `ThemeSpec` and a label string; the palette
  tests check its contrast. **The rule:** the default channel (GitHub, F-Droid, desktops, web;
  open source with Donate) unlocks every theme, default Classic; `play-paid` (CalcCraft Pro)
  unlocks every theme, default Gold; `play-free` shows the Pro themes locked with a PRO pill
  that opens Pro's Play listing (`open_pro_listing`), or hides them without
  `CALCCRAFT_PAID_ID`; a stored theme the build doesn't unlock loads as Classic. No in-app
  purchase, licence check or network. The channel is `crate::channel` (compiled in every
  build; `support::channel` re-exports it).
- The Expression size card (`SettingsList::ExpressionSize`, `app/expression_size.rs`,
  CalcCraft's own) comes right after App theme in every build: Small (Windows' 14 px, the
  default, exactly the old look), Medium, Large and Extra large, saved as
  `CalcCraftExpressionSize` and kept in `CalculatorApp::expression_size`.
  `draw_expression_line` draws the line at that size (8/7 of it while a part is edited), with a
  48 px tap area at Large and up; `draw_display_sized` moves the line up into the room above
  it, then the result down (shrinking it if it must), so the two never overlap.
- The Android app alone has a Key vibration card after Language (`draw_vibration_card`): a
  strength slider from 1 to 100, a Style drop-down (`SettingsList::VibrationStyle`: Off,
  Auto, Click, Pulse, Heavy click; Off hides the sliders, and a stored strength of 0 from
  older builds loads as Off, `vibration::load`) and, for an explicit Pulse only, a Duration slider (5–12 ms),
  saved as `CalcCraftKeyVibration`, `CalcCraftKeyVibrationStyle` and
  `CalcCraftKeyVibrationDuration`. `app/vibration.rs` holds the pure parts, tested on every
  platform: the mappings (`click_scale`, `amplitude`, `duration_ms`), the Auto rule over the
  vibrator's `Capabilities` (`auto`, `tap`), and the card's rows (`card_rows`). The JNI calls
  (capabilities, playing a `Tap`) are in `android.rs`. The rule comes from the vibration lab's
  results by feel (`docs/vibration-lab-results.md`): don't change it without measuring again.
- The Android app alone has a Screen orientation card right after Language (`SettingsList::
  Orientation`, `app/orientation.rs`): Auto, Portrait, Landscape, saved as
  `CalcCraftScreenOrientation` and handed to `Activity.setRequestedOrientation`
  (`android::set_orientation`) in `new`, before the first frame, and at once when chosen. Auto
  is `SCREEN_ORIENTATION_USER` (the system's auto-rotate and rotation lock); Portrait and
  Landscape are `SENSOR_PORTRAIT` and `SENSOR_LANDSCAPE`, which ignore the system's setting but
  use both sides. The manifest fixes no orientation, and its `configChanges` keep a rotation
  from restarting the activity. The card is left out where Android 16 ignores an app's
  orientation (large screens, `orientation::honored`). Its icon is drawn in code
  (`paint_orientation_icon`; `CardIcon`).
- A phone on its side (lower than `LANDSCAPE_HEIGHT`, 500, and `LANDSCAPE_RATIO` times as wide;
  desktop and web windows are never that low) gets the landscape layout (`Landscape`,
  `draw_landscape`): the Standard, Scientific and Programmer modes put the header, the display
  and their rows above the keys (`draw_scientific_controls`, `draw_programmer_controls_column`)
  in a left column, with the docked history and memory under them where there is room, and
  their keys alone in the right column (`*_keypad_only`); a flyout covers the right column. The
  Graphing mode keeps its narrow layout there, and its equations view splits the same way
  (`graphing_split`, `draw_graphing_keypad_split`). `layout_tests` checks every mode at the
  Android sizes (keys at least 40 px, inside the safe area); `cargo test write_layout_shots --
  --ignored` paints them into `target/shots/` (`CALCCRAFT_SHOTS=<name part>` picks some). On the
  desktop, `CALCCRAFT_WINDOW_SIZE=780x320` opens a window that low (it lowers the minimum size)
  and `CALCCRAFT_MODE=<Mode::serialization_id>` opens a mode.
- Icons come from the icon font compiled into the app (`assets/fonts/CalcCraftIcons.ttf`), a
  subset of Microsoft's Fluent System Icons that `tools/icon-font.py` puts at the code points of
  the Segoe Fluent Icons glyphs Windows uses. The code points are `ICON_*` and `category_icon`
  in `app/icons.rs`, drawn with `icon_label`. A new icon needs a line in the tool's `ICONS` and a
  run of `uv run tools/icon-font.py`; a test checks that the font has every glyph `icons.rs` names.
- Text is Segoe UI from `%WINDIR%\Fonts` (regular, semibold and light). Where a Segoe UI file is
  missing, `setup_fonts` uses the embedded Inter (`assets/fonts/Inter.ttf`, a variable font) at
  the same weight instead.
- Keyboard: `char_command` handles text input (digits, operators), and `handle_key` handles
  named keys and `Ctrl` shortcuts; in the Programmer mode `ProgrammerState::char_command` and
  `key_command` take their place, and in the Scientific mode `ScientificState::char_command`
  and `key_command` (letters arrive as keys there, `Shift`, `Ctrl` or both picking the inverse
  and hyperbolic functions, so letter text is ignored). On a layout without Latin letters the
  letter keys type their own script (`к` on the R key of a Russian keyboard), and
  `latin_letters` turns that text into the key's letter, which Windows' shortcuts see: they're
  virtual keys. A symbol typed on such a letter key (`;` on the Greek Q key) is its letter only
  where `layout_letter` can ask Windows (with `Ctrl` too, when egui sends no text) and the key
  types it without `AltGr` (a Czech keyboard's `AltGr+Q`, `\`, stays). Keys pressed with `Alt`
  stay as they are, and with `Shift` only the Programmer mode's letters are converted (Windows has `Shift` chords for `A`–`F`, not for `R` and `Q`). The date mode and the converters have their own
  `handle_date_keyboard` and `handle_converter_keyboard`, so calculator keys do nothing there, and
  the Graphing mode `handle_graphing_keyboard` (its text boxes take the typing).
  `handle_mode_keys` (`Alt+1` … `Alt+5`, `Ctrl+E`, from Windows' `NavCategory`) runs first in
  every mode, and takes the text of every `Alt` chord (Windows types nothing for them; on a Mac
  only the mode keys' text, since Option types characters there, like `@` on German keyboards).
  With the
  menu or a context menu open, only `Esc` reaches the calculator (the converters too); an open
  history or memory flyout disables the keypad and memory keys (`keypad_disabled`, Windows'
  `EnableControls(false)`), and keep-on-top turns the `Ctrl` shortcuts off. egui's own
  `Ctrl` + plus, minus and 0 zoom is off (`zoom_with_keyboard`): Windows has none, and the graph
  zooms with those keys. A letter key pressed with `AltGr` is no key press for the calculator,
  only the character it typed is (`remove_alt_gr_keys`; `@` on the Q key of a German keyboard):
  Windows sees `AltGr` as `Ctrl+Alt`, which has no shortcuts, but winit reports it without
  modifiers. The right `Alt` key's own events tell when it's down, in order with the other
  keys; a letter key that typed only other characters than its letter (diacritics aside, so a
  dead key's `é` and `´q` stay `E` and `Q`) counts as pressed with `AltGr` too.
- The display and the expression line sense taps (`draw_display_sized`,
  `draw_expression_line`) to place the caret (`app/caret.rs` maps it onto the localized text by
  finding the typed characters in it). While a caret is shown, `caret_key` takes Left, Right,
  Home, End, Ctrl+Left/Right, Delete (which is otherwise CE) and Esc (otherwise C), and cancels
  egui's focus move; with no caret they do what they did. The keypad needs nothing: the engine's
  `execute` sends the typing keys to the edited number.
- Every interactive control goes through `access::control`, which makes it clickable and
  focusable (`Tab`, `Shift+Tab`, arrow keys: egui's own focus handling), gives it a screen
  reader description (`access::button`, `item`, `combo_box`), draws WinUI's focus visual and
  scrolls it into view when it has the focus. Use it for any new control, with the Windows
  `AutomationProperties.Name` as its name. Results that Narrator should read when they change
  are `access::text(…, live: true)`. `Enter` presses the focused control, except on the keypad
  and the memory row (`is_calculator_key`), where it stays `=` like Windows' `CalculatorButton`.
  The same tree reaches TalkBack and Android's accessibility services (automation agents,
  `uiautomator dump`) through eframe's `accesskit` feature and `vendor/accesskit_winit`, on
  Android 8.0 and later: a control's name is what they click it by, so keep the names stable
  (`tests/device/scenarios` use them). Marionet keeps a node's first 50 children, and egui's tree
  is flat, so keypads and popup menus are groups (`access::group`; a new keypad or menu needs
  one); the expression line's numbers and operators are child nodes that a click edits
  (`expression_part_nodes`); a control its scroll area hides leaves the tree, and one it shows in
  part keeps only that part as its bounds (`access::interact`). `src/app/a11y_tests.rs` checks
  all of it on the Android screen sizes.
- Popups: `popup_covers` is true while the menu, a context menu, a drop-down or the calendar is
  open; the controls drawn under it are "covered": they leave the Tab order and sense only
  hovers, so neither a screen reader's click (egui takes AccessKit's `Click` for a real one) nor
  a drag on the graph or a slider (the popup's backdrop senses only clicks) reaches them. A
  scroll area under it doesn't scroll (`access::scroll_source`: egui scrolls the area under the
  pointer whatever covers it in its layer). Text boxes (`TextEdit`) aren't covered: a screen
  reader or a drag can still focus one under a popup. The graph options' and line options' own
  drop-downs cover the rest of their flyout but not their combo, which keeps the keyboard
  focus. A click elsewhere on the flyout or Esc closes only the drop-down (the repeats of a
  held Esc close nothing more until its key-up, `swallow_held_escape`), and its combo gets the
  focus back if the focus was in it (`drop_down_closed`), as in WinUI. The Graphing mode's
  zoom keys (Ctrl + plus, minus and 0) still work under a popup or the menu: Windows' are
  unscoped `KeyboardAccelerator`s, which WinUI runs whatever has the focus. The popup calls
  `access::set_covered(ctx, false)` before drawing its own. A popup opened with the keyboard
  (`opened_from`) focuses its selected item (`take_popup_focus`) and the opener gets the focus
  back when it closes (`focus_return`).
- Text: don't write English UI strings in code. `strings::tr("HistoryLabel.Text")` looks up a
  Windows resource key in the user's language (`tr_format` fills in `%1`); unit names use
  `strings::unit_name(unit.key)`. A new string needs its key in `tools/import-strings.ps1`,
  then a run of the script. Only a string Windows doesn't have goes into
  `locales/calccraft.txt` (`CalcCraft.*` keys, every language translated by hand, as the tests
  check). Numbers shown to the user go through `numbers().localize` (the
  engine and the converter write en-US text), pasted text through `numbers().delocalize`, and
  dates through `region::formats()`.
- Right-clicking a history or memory item opens its context menu (`draw_context_menu`, state in
  `context_menu`). It's drawn last in the panel over a full-window backdrop that closes it, like
  the date mode's popups, and uses their `popup_*` colors and `popup_frame`. A history item's
  menu also closes if the history changes under it (`history_key`): its index would then name
  another entry.
- `Mode` (`Standard`, `Scientific`, `Graphing`, `Programmer`, `DateCalculation`,
  `Converter(Category)`) is picked in the ☰ menu (`draw_navigation`), which lists
  `Mode::CALCULATORS` and `Category::ALL` under two headings and scrolls when the window is
  short. `App::ui` hands the Graphing mode, the date mode and the converters over to
  `draw_graphing_mode`, `draw_date_mode` and `draw_converter_mode` and returns early.
  `Page::Settings` replaces any mode, and going back returns to the mode that was open.
  Like Windows, the app opens in the mode it was closed in: `save` stores it under Windows' key
  `Mode` as Windows' id (`Mode::serialization_id`, `NavCategory`'s `SerializationId`), and `new`
  switches to it with `set_mode`. Settings is a page, not a mode: the mode under it is saved.
- The Standard, Scientific and Programmer modes each have their engine (`calc`,
  `scientific.calc`, `programmer.calc`). `engine()` / `engine_mut()` return the one of
  `last_calculator`, and the shared code (display, keyboard, memory row and list, history)
  goes through them: the Standard and Scientific modes have a history each, as in Windows. Like
  Windows' view model, `set_mode` moves the memory to the other engine and resets it
  (`Calculator::reset`) when switching between them, but not when the date mode or a converter
  comes in between.
- From 560 px wide (`DOCK_WIDTH`, Windows' `DockVisible` state), the Standard, Scientific and
  Programmer modes dock history and memory on the right (`draw_dock`: Pivot-style tabs over
  `draw_list`, the same list the flyouts use; the Programmer mode has only the Memory tab). The
  header's history button and `M˅` disappear, and `Ctrl+H` does nothing then. The lists wrap
  long expressions and numbers (`layout_wrapped_right`), as Windows' `TextWrapping="Wrap"` does.
- Narrower, history and memory open as flyouts over the keys only, as tall as Windows'
  `NumpadPanel` (`RowHeight = NumpadPanel.ActualHeight`): the rows above them (the memory row,
  the Scientific mode's unit and F-E row, the Programmer mode's radix list and the row under it)
  stay in view with the memory buttons disabled, and a click there closes the flyout. In the
  Standard and Programmer modes the flyout lies over the whole window, as Windows' does
  (`Placement="Full"`): the header and those rows are drawn covered (`access::draw_covered`),
  out of the Tab order and not clickable, not even by a screen reader, and a click anywhere
  above the list closes it.

### `app/scientific_view.rs`

- `ScientificState` holds the engine, `2nd` (`shift`), the Trigonometry menu's `2nd` and `hyp`
  and the open menu. `draw_scientific` lays out `Calculator.xaml`'s `Scientific` rows with
  `star_rows`: the result font is 26, 46 or 72 px by window height (the 544 px trigger of
  `ResultsM`), then the unit and F-E row, the memory row and the keys. The keys' sizes follow the
  `Small`, `Medium` and `Large` states of `CalculatorScientificOperators.xaml` (`Sizes`).
- `2nd` swaps the first column for its second functions (`Kind::Emphasized`, which turns to the
  accent color on hover), and using one turns `2nd` off (`ShiftButton_Uncheck`). A function from
  a menu closes it and turns the menu's toggles off (`FlyoutButton_Clicked`).
- Windows draws most key labels from the Calculator's own font; here the formulas (`x²`, `xʸ`,
  `10ˣ`, `¹/x`, `²√x`, `|x|`, `logᵧx` …, in the Standard, Scientific and Graphing keypads) are
  `math_label::Formula`s: pieces of text laid out by their measured ink, not one line of glyphs.
  The variables are Noto Sans Math's real italic letters (`𝑥`, U+1D465; egui's `italics` only
  skews the upright glyph), an exponent or index is 0.6× the size with a small gap, raised so
  its bottom is at about half the base's x-height and its middle no lower than the base's top,
  and the radical is a vector path whose vinculum spans the radicand, in the key's text color.
  `⌊ ⌋ ⌈ ⌉` still come from Segoe UI Symbol. The screen reader names don't change.
- The history and memory flyouts cover only the keys, as tall as Windows' `NumpadPanel`: the
  unit, F-E and memory rows stay in view, the memory buttons disabled. The flyout lies over the
  whole window, as Windows' does (`Placement="Full"`): the header and these rows are drawn
  covered (`access::draw_covered`), out of the Tab order and not clickable, not even by a screen
  reader, and a click anywhere above the list closes the flyout.

### `app/programmer_view.rs`

- `ProgrammerState` holds the engine, the bit toggling keypad switch (`bit_flip`), the Bit shift
  menu's choice (`ShiftMode`, which also decides what `<` and `>` do) and the open menu.
- `draw_programmer` lays out `Calculator.xaml`'s `Programmer` rows with `star_rows`; the result
  font is 26, 46 or 72 px by window height (`results_size`, Windows' `ResultsS/M/L` with the
  Programmer mode's 640 px trigger). The keys' sizes follow the `Small`, `Medium` and `Large`
  states of `CalculatorProgrammerRadixOperators.xaml` (`keypad_sizes`).
- The menus and the radix list's Copy menu are drawn last (`draw_programmer_popups`) over a
  backdrop, like the other popups. The keypad switch, Bitwise and Bit shift icons come from the
  Calculator's own font in Windows, so they are drawn in code (`paint_*_icon`); `«` and `»` are
  two chevron icons close together.

### `graphing.rs` and `graphing/` (Graphing mode)

- Windows graphs with a proprietary engine (`GraphingImpl` is only a mock in its source), so
  this one is CalcCraft's own, to the same specification: `GraphingCalculator.xaml(.cs)`,
  `GraphControl/Control/Grapher.cpp`, `RenderMain.cpp`, the view models in
  `Calculator.ViewModels/GraphingCalculator/` and `docs/ManualTests.md`. Keep it free of egui.
- `parse` reads the linear text the keypad writes and people type: implicit multiplication
  (`2x`, `mx`, `(x+1)(x−1)`), function names without parentheses (`sin 2x` is sin(2x), `sin x
  cos x` is sin x · cos x), `|x|`, `x!`, `²`, `√`, `π` and the Unicode operators, `log(b, x)` and
  `root(x, n)` with the region's separator (`;` with a decimal comma). Letter runs split into the
  longest function names and one-letter variables (`word_parts`); `e` is Euler's number and `i`
  an error. Errors are Windows' `SyntaxErrorCode`s, whose `key` names their message.
- Input has limits, so that no equation can overflow the stack or freeze the UI: 2048
  characters (the box's `MaxLength`, also for the menu's Paste), 64 nesting levels and an
  expression tree 100 levels deep (`MAX_DEPTH`), all `InvalidEquationSyntax`. `derivative`
  gives up (`None`) beyond 1000 nodes, and the analysis then calls those features too complex.
- `Equation::parse` sorts a line into `Shape::Function` (y = f(x), a bare f(x), or `f(x) = …`
  written with its parentheses: `ax = 3` is an implicit equation with a slider for a),
  `Shape::Inverse` (x = g(y)) or `Shape::Implicit` (F(x, y) = left − right), each with a
  `Relation`. Other letters are free variables (`variables`).
- `expr` evaluates in `f64` (Windows' graphs use doubles too): `real_pow` gives negative bases
  real odd roots (`(−8)^(1/3)` is −2), trigonometry follows the angle unit, and what isn't real is
  NaN. `factorial` is exact for whole numbers up to 170! (rounded once beyond 22!), the gamma
  function otherwise. `derivative` is symbolic (the analysis needs f′ and f″); factorials have none.
- `plot` works in pixels of the graph. Functions are sampled about once a pixel and subdivided
  where they bend; a jump of more than a quarter of the height at the last subdivision breaks the
  line (tan's asymptotes), and a domain edge is found by bisection (√x starts at 0). Implicit
  curves are marching squares on 4 px cells, refined by a few Illinois steps, rejecting the sign
  changes of poles. Inequalities fill Windows' 100 × 100 lattice of dots. `best_fit` is the
  automatic view, `grid_step` the labeled and minor grid lines, `trace` the curve point nearest
  the pointer with x rounded to `View::precision` (`RenderMain::GetPrecision`).
- `analysis` finds the features numerically on a grid from −1000 to 1000 (`grid`), with
  `sign_changes` (bisection; through poles for cusps when asked), touching zeros from f′, the
  ends of flat pieces where f′ is exactly 0 (`flat_ends`: |x − 1| + |x + 1| on [−1, 1]),
  limits from far-away and nearby values (`limit_of`: converged, geometric, or growing), and the
  domain from the zeros of `boundary_functions` (denominators, radicands, cos of tan's argument …)
  checked by evaluating f. A function built from trigonometric functions of linear arguments
  has a `period` (verified, and shrunk to the smallest one); it's analyzed over one period and
  written as families with `families` and `family_term`. One that is constant (sin²x + cos²x)
  has no period and is analyzed like y = c. Values go through `exact`: integers
  and fractions, else a short decimal (`1.05`, to the `ROUNDOFF` only), a multiple of π or
  (p/q)√c + r/s, with tight tolerances (`TIGHT` for roots, `LOOSE` for limits), else 6
  significant digits. Some long surd is within the tolerance of almost any number, so those
  have to take `SAVED_DIGITS` fewer digits to write than a number to that tolerance (or be
  exact to the `ROUNDOFF`), and the closest wins. Large counts (hundreds of zeros of sin(x)/x,
  or more than 20 in a period) are "too complex" before any exact value is looked for:
  `recognize` is slow. A domain boundary is moved to the exact value it is near only if a
  boundary function is zero there but for roundoff, or, for a pole, if f is still a pole there
  (`snap_x`).

### `app/graphing_view.rs`, `app/graph_canvas.rs` and `app/graphing_keypad.rs`

- `GraphingState` keeps the boxes (`Row`: the typed `text`, the graphed `submitted`, number,
  color index, style, visibility, parsed equation), the variables, the view and whether it was
  moved by hand (`manual`, Windows' `IsManualAdjustment`), the options, the open `Popup` and the
  tracer. Rows follow `EquationInputArea.xaml.cs`: at most 14, the next color in
  `COLOR_ORDER`, the last box never removed and renumbered after a removal.
- An equation is graphed when submitted (`submit`: Enter, ↵, or leaving the box after a change,
  but not for a click on the keypad), as in Windows. `equations_changed` updates the variables
  (keeping the sliders that stay) and refits the view unless it's manual.
- The equation boxes are `TextEdit`s with `math_job` as their layouter: the text stays what was
  typed (so the cursor and the keypad's insertions keep working), only laid out like math
  (italic variables, a hidden `^` before a raised exponent, spaced relations); `*` and `-`
  become `·` and `−` as they're typed (`normalize`). The keypad types into the box that had the
  focus last (`active`) through its stored `TextEditState` (`insert_text`, Windows'
  `InsertText(text, cursor, selection)`), then gives it the focus back.
- On Android (touch, no hover) the equation boxes differ, and only there: a tap focuses a
  box and places the cursor but doesn't raise the system keyboard, which would cover the
  keypad (egui-winit raises it whenever a focused `TextEdit` reports IME output, so
  `SoftKeyboard::allows` drops that output for the equation boxes). Every box always shows its
  buttons, 40 dp wide: function analysis, line options and remove as on Windows, plus a
  keyboard toggle (`CalcCraft.Keyboard`, a painted icon) that focuses the box and opens the
  system keyboard for it; tapping it again, focusing another box, or closing the keyboard
  (Back; `android::ime_visible`) turns it off. The × is left out there (the keypad's `C` clears
  the box). Windows, macOS, Linux and the web keep the hover buttons and have no toggle.
- `graph_canvas` draws the graph in the graph theme's colors (`GraphPalette`, always light unless
  "Match app theme"), caches the plots (`PlotCache`) until the view, the size, the equations or
  the variables change, and takes a picture of the graph by drawing it alone (`Capture`) and
  cropping a viewport screenshot. The context menu's Copy puts it on the clipboard; the share
  button (`Purpose::Share`) hands it to `app/share.rs`.
- `app/share.rs` opens Windows' share sheet (`IDataTransferManagerInterop`, the `windows`
  crate) with `OnDataRequested`'s content: the graph as `graph.png` (encoded with `png`), the
  equations and the variables as HTML (`html`, tested), and the bitmap. The sheet asks for the
  data after it opens, so `PENDING` keeps the last share. When it can't open, the mode shows
  `ShowShareError`'s dialog (`Popup::ShareError`). The share's title is "CalcCraft": Windows'
  `ShareActionTitle` names the Windows Calculator. On Android it sends the PNG alone:
  `write_single_file` (tested) makes it the one file of `cache/share/` (everything else there
  is deleted, the picture overwritten: shares never pile up), and `android::share_image` starts
  an `ACTION_SEND` chooser with `content://<application id>.share/graph.png`, which the app's
  only Java class, `android/java/.../ShareProvider.java`, serves read-only (android/README.md).
- The keypad reuses the Scientific mode's labels and `Sizes`; `KeypadMenu` popups are drawn last
  with the other popups (`draw_graphing_popups`), over a backdrop.

### `dates.rs` and `app/date_view.rs`

- `dates.rs` is a port of `DateCalculationEngine` and `DateCalculatorViewModel` from the
  Windows Calculator source: `difference`, `describe_difference`, `add`, `subtract`, the date
  formats, and `month_page_start` for the calendar.
- `date_view.rs` adds `impl CalculatorApp` methods and keeps its state in `DateState`. Only one
  popup is open at a time (`Popup`: the page list, the calendar, or a 0–999 list). It's drawn
  last, over a full-window "light dismiss" backdrop that closes it when clicked.
- Its colors are `Palette` fields too (`popup_*`, `calendar_*`, `control_*`, `other_month`).
  The Add/Subtract choice uses the same `radio_button` as the Settings page.
- Sizes come from `DateCalculator.xaml`: 16 px page margin, 32 px controls, 12 px captions,
  14 px values, results at 20 px from a 480 px window width and 14 px below that, and 84 px
  wide offset boxes.

### `converter.rs` and `app/converter_view.rs`

- `converter.rs` is a port of `UnitConverter` (`CalcManager/UnitConverter.cpp`) with the unit
  tables of `UnitConverterDataLoader.cs` and the field logic of `UnitConverterViewModel.cs`.
  `Converter` keeps Windows' state as strings: `current` (`m_currentDisplay`, what is typed),
  `result` (`m_returnDisplay`), `switched_active`, and `value1_is_source` (which field is typed into).
  The UI calls `execute(ConverterCommand)`, `activate`, `set_unit1` / `set_unit2`,
  `set_category`, `paste`, and the getters (`value1`, `value2`, `unit1`, `unit2`, `units`,
  `suggestions`).
- `suggestions` ports `CalculateSuggested` for the "About equal to" row. The everyday
  comparison units (Windows' whimsical units) live in separate `*_COMPARISONS` tables with
  their Segoe UI Symbol pictures from `DelighterUnitStyles.xaml`, so they never reach the
  drop-down lists.
- It converts with `f64`, not `BigRational`: Windows converts with `double` and rounds with
  `RoundSignificantDigits` and friends, and only the same arithmetic gives the same digits.
  Keep the factors exactly as `UnitConverterDataLoader.cs` writes them (hence the
  `clippy::excessive_precision` allows) and look the units up by name in tests.
- The default units depend on the Windows region (`Region`, from `GetUserDefaultGeoName` in
  `converter_view.rs`), like Windows' flags in `LoadData`. `preferences` / `restore_preferences`
  save the last category and units under Windows' key `UnitConverterPreferences`.
- `converter_view.rs` adds `impl CalculatorApp` methods and keeps its state in
  `ConverterState`. Rows follow `UnitConverter.xaml` (`star_rows` sizes them like XAML star
  rows with minimums); a landscape window moves the keypad to the right half, and from 640 px
  the values, unit pickers and keys get larger. `draw_suggestions` fills the row under the
  values like `HorizontalNoOverflowStackPanel`. The unit drop-down reuses the date view's
  `list_item` and `popup_frame`, and the unit pickers' hover and pressed text colors are
  `Palette::unit_hover` / `unit_pressed`.

### `support/` module and `support-prompt` feature

- The `support-prompt` Cargo feature is on by default in free builds. Turning it off
  (`--no-default-features`) disables the module completely for future paid builds, omitting the menu
  button, the floating popup, the calculation counter and support prompt strings at compile time.
- The spec is `docs/support-prompt.md` (v2): silent for the first 3 launches, then a red dot on the
  menu button for regular users (4 launches, 3 active days), a popup after 7 unseen days at a calm
  moment only (3 s without input after a successful `=`, focused, calculator mode, nothing open),
  under the display, an interval ladder (7, 14, 30, 60, 90 days), and pauses after Donate (180
  days) or "I already supported" (365 days, shown only after a Donate click, and never in
  `play-free`, which has no self-declared supporter state: `self_declared_support`).
- Everything lives in `src/support/`; the rest of the app has only thin hooks behind
  `#[cfg(feature = "support-prompt")]` (`begin_frame`, `after_command`, `note_display`,
  `draw_menu_badge`, `draw_menu_button`, `draw_overlay`, `new`/`save`). Don't put support logic
  elsewhere.
- `policy.rs` is pure: `evaluate`, `on_event` and `show_already_supported_button` take the time as
  a `Timestamp` (wall milliseconds and a local day number) and every threshold from
  `SupportPolicyConfig`. Its tests simulate months; keep it free of clocks and egui.
- `stats.rs` stores one versioned record under `SupportStatsV2` (unknown fields ignored, missing
  ones default) and migrates v1's `SupportCalculationCount` / `SupportFirstUsed` /
  `SupportPopupShownCount`. Local only: no network, telemetry or identifiers.
- `donate.rs` builds the donate URL with Stripe's UTM parameters and a coarse
  `client_reference_id` (`donate_url`, tested), and has the `DonationChannel` trait (`ExternalLink`
  today; a store build's in-app purchase would be another implementation).
- The distribution channel is a Cargo feature too (`channel.rs`, docs/support-prompt.md,
  section 8): the default has the donate link; the Google Play builds have none, as Play's
  Payments policy requires. `play-free` leads the popup's button ("Get CalcCraft Pro") to the
  paid app's Play listing, named by `CALCCRAFT_PAID_ID` at build time (`option_env!`), and
  hides the whole prompt without it, and never has "I already supported" or "Supported by
  you" (a user there bought Pro or didn't); `play-paid` shows no prompt, only "Supported by
  you". `SupportPolicyConfig::prompt` and `self_declared_support` carry this into the pure
  policy. Keep `DONATE_URL` and
  anything Stripe under `#[cfg(not(any(feature = "play-free", feature = "play-paid")))]`:
  `build-apk.ps1` fails a Play build whose library contains them. `android/README.md` has the
  commands (`-Channel`, `-PaidApplicationId`).
- `CALCCRAFT_SUPPORT_FAST=1` makes a day a minute (`CALCCRAFT_SUPPORT_DAY_SECS` shortens it) and
  `CALCCRAFT_SUPPORT_DEMO` starts in a stage; a fast-mode record is reset in normal mode.

## Windows behaviors the tests pin down

All of these were checked against the real Windows 11 Calculator. Don't "fix" them:

- Immediate execution with no operator precedence: `2 + 3 × 4 =` → `20`.
- Chained operators collapse in the expression line: `2 + 3 +` shows `5 +`.
- Pressing another operator replaces the pending one.
- `=` with no right operand reuses the display: `5 + =` → `5 + 5 =`.
- Repeated `=` repeats the last operation. A new number followed by `=` also does:
  `2 + 3 = 7 =` → `7 + 3 =` → `10`.
- `%`: after `+` or `−` it gives `left × value / 100`; after `×` or `÷` it gives `value / 100`;
  with no pending operator it gives `0`.
- Functions appear as `sqr(x)`, `√(x)`, `1/(x)` and `negate(x)`, and nest.
- `±` while typing flips the sign silently; on a finished number it shows `negate(x)`.
- The display uses 16 significant digits with en-US grouping (`1,234,567`). The expression
  line has no grouping (`1234567 +`).
- Numbers ≥ 1e16 use scientific notation: `1.e+16`, `1.234567890123456e+18`.
- Numbers < 1 use plain notation if there are at most 2 zeros after the point (then they are
  rounded to 16 decimals, as in `0.0033333333333333`) or if they fit exactly in 16 decimals
  (`0.000125`). Otherwise they use scientific notation (`3.333333333333333e-4`).
- Errors:
  - `Cannot divide by zero`: the expression shows `1 ÷` after `=`, and `8 ÷ 0 +` after an operator.
  - `Result is undefined`: `0 ÷ 0`.
  - `Invalid input`: the square root of a negative number.
  - `Overflow`: the decimal exponent goes beyond ±9999.

  In the error state only digits, `C`, `CE`, `⌫` and `=` work (`=` only clears the error);
  the number pad's `ErrorLayout` disables `.`.
- `CE` after `=` clears the display and the expression line. `⌫` after `=` clears only the
  expression line.

These follow from the Windows source and are pinned by tests too:

- With precedence on: `2 + 3 × 4 =` → `14` with `2 + 3 × 4 =` in the expression line, and
  `2 + 3 × 4 +` shows `14`. Changing `−` to `×` in `1 + 2 −` gives `(1 + 2) ×`. `=` closes all
  open parentheses, and a number next to a parenthesis multiplies: `2(3)` → `6`.
- The expression line writes subtraction as a hyphen: `9 - 4 =`.
- `5 =` adds `5 =` to the history, and a finished function followed by a new number or MR adds
  the function with the result it showed (`√(4)` = `2`, not the recalled value).
- After `=`, `CE` keeps the last operation: `2 + 3 =`, `CE`, `=` gives `0 + 3 =` → `3`.
- Pasted text is checked like in `CopyPasteManager`: at most 512 characters, 100 operands and
  16 digits per operand (32 in the Scientific mode), exponents of up to 4 digits (`1.5e-3`).
  Invalid text shows `Invalid input`. Valid text is typed in after a `CE`, so `12 + 5 =` gives
  `17`; `e` is `exp`, and a sign after it goes to the exponent.

When you change or add behavior, check it against the real calculator (`calc.exe`) or its
source first, then add or update a test in `engine.rs`.

### Scientific mode

These were checked against the real Windows 11 Calculator or follow from its source, and are
pinned by the `scientific_*`, `exponent_entry`, `angle_units` and `degrees_*` tests in
`engine.rs` and the tests in `engine/math.rs`:

- 32 significant digits on the display and in the expression line (`1 ÷ 3 =` →
  `0.33333333333333333333333333333333`), and 32 typed digits.
- `xʸ`, `ʸ√x` (`yroot`) and `logᵧx` (`log base`) bind tighter than `×` and `÷`; `mod` (`Mod`)
  ranks with `×` and `÷`, has the sign of the divisor (`-10 mod 3` is 2), and `x mod 0` is x.
- The expression line writes the functions as Windows' engine strings: `sin₀(30)` in degrees,
  `sinᵣ` in radians, `sin₉` in gradians, `sin₀⁻¹`, `sinh⁻¹`, `sec₀`, `sqr`, `cube`, `cuberoot`,
  `10^`, `2^`, `e^`, `log`, `ln`, `fact`, `abs`, `floor`, `ceil`, `dms`, `degrees`.
- `π`, `e` and `rand` replace the typed number (`2π` is π, and `π(2)` is 2).
- Errors: `tan(90°)` and `ln(0)` are `Invalid input`, `csc(0)` and `atanh(1)` `Cannot divide by
  zero`, `3250!` `Overflow`; trigonometry of 10^100 or more is `Invalid input`, hyperbolic too.
  In the error state the unit and F-E buttons stay enabled (Windows' code sends them to visual
  states that don't exist); here they clear the error and apply.
- `exp` works only while typing and adds a decimal point (`5.e+0`); `±` then flips the
  exponent's sign; `⌫` removes the exponent's digits, then the exponent. `99 exp 9999 +` is
  `Overflow` with `9.9e+10000 +`.
- F-E writes every finished number in scientific notation (`5.e+0 +`, `0.e+0`), history included;
  `C` and `CE` turn it off, and so does leaving the mode.
- `F4`, `F5` and `F3` pick radians, gradians and degrees: each is the key of the unit button that
  shows before it, and that button switches to the next unit.
- `→deg` after `=` hides the expression line (`2 + 3 =` then `→deg` shows only `5`), and it
  doesn't count as a function: a new number replaces it instead of logging it.

### Programmer mode

These were checked against the real Windows 11 Calculator or follow from its source, and are
pinned by the `programmer_*` tests in `engine.rs` and the tests in `engine/integer.rs`:

- Precedence: `OR` and `XOR` bind loosest, then `AND`, `NAND`, `NOR`, then `+ −`, then
  `× ÷ %` and the shifts: `4 OR 2 + 2 =` → `4`. Parentheses work as in the Scientific engine.
- Every displayed result is truncated to the word: `5 ÷ 2 × 2 =` → `4`. Inside parentheses a
  quotient keeps its fraction until they close: `(255 AND 10 − 7 ÷ 2) =` → `6`.
- Division and `%` are signed: `-7 ÷ 2` → `-3`, `-7 % 3` → `-1`; `x % 0` is `Result is
  undefined`. A shift by the word size or more is `Result not defined`, showing `1 Lsh`.
- The expression line writes `AND`, `OR`, `XOR`, `NAND`, `NOR`, `Lsh`, `Rsh` (logical too), `%`,
  `NOT(x)`, `RoL(x)`, `RoR(x)` and `negate(x)`, with operands in the current radix (`-5 +` in
  DEC). A new radix rewrites them from their unsigned values: `-5 +` becomes `FFFFFFFFFFFFFFFB +`
  in HEX and `18446744073709551611 +` back in DEC. After `=`, a new radix clears the line.
- The display groups HEX and BIN digits by 4 and OCT digits by 3 with spaces, DEC by the region;
  a binary number gets leading zeros up to a whole group (`0010`). In the error state the radix
  list is empty.
- Typing is limited by the word: 16 HEX digits in a QWORD, 22 OCT digits only after a leading
  `1`, and DEC up to `9223372036854775807` (one more for a negative number). A typed negative
  number that wouldn't fit as a positive one keeps its sign (`-128` in a byte).
- `±` outside DEC finishes the number and shows `negate(x)`.
- A smaller word keeps the low bits (`1FFFF` → `FFFF` in a WORD); a larger one extends a set
  highest bit (`FF` in a BYTE → `FFFF` in a WORD).
- A bit flipped right after an operator flips the left operand: `5 + bit 1 =` → `5 + 7 =`.
- In the error state, digits, `CE`/`C`, `⌫`, `=`, the bits and the radix list work (each first
  clears the error); `=` only clears it.
- The one clear key shows `C` while nothing is typed and the display is 0, `CE` otherwise.
- Keys: `F5`–`F8` pick the radix; `F2`, `F3`, `F4` and `F12` pick DWORD, WORD, BYTE and QWORD
  (the shortcuts of Windows' four word size buttons, which each switch to the next size); `.`
  is NAND and `\` is NOR, as in Windows; `<` and `>` follow the Bit shift menu.
- Memory is truncated to the word, shared with the Standard and Scientific modes, and shown in
  the current radix. Switching between the calculator modes resets the mode switched to
  (Programmer back to DEC and the full keypad when it was left); the word size and shift mode
  stay.

### Graphing mode

These come from the Windows Calculator source and calc.exe, and are pinned by the tests in
`graphing.rs`, `graphing/*.rs`, `app/graphing_view.rs`, `app/graph_canvas.rs` and
`app/graphing_keypad.rs`:

- A bare expression is y = expression; `x + y` without a relation is `InvalidEquationSyntax`,
  and `4 + 83 = 9` (no x or y) `EqualWithoutGraphVariable`. `2 < y` is y > 2.
- New boxes take the colors `EquationBrush1`, `4`, `8`, `11`, `2`, `5`, `9`, `12`, `3`, `6`, `10`,
  `13`, `7`, `14` (`colorAssignmentMapping`), the first one free; at most 14 boxes.
- New variables are 1 on a slider from −5 to 5 in steps of 0.1. A value beyond the slider moves
  its end; a minimum at or above the maximum puts the maximum 10 above it, and the other way
  around; a step of 0 or less is refused. Values are shown with 6 significant digits ("G6").
- An empty graph shows −10 to 10 on both axes. The automatic view of x² in a 2140 × 1308 graph
  is x from −27.2204 to 27.2204 and y from −3.025 to 30.25, as in calc.exe.
- The graph options' window boxes keep what is typed (`GraphingSettingsViewModel`): a minimum
  at or above its maximum marks both boxes as errors, and the view waits for the pair to be in
  order (X-Min 20, then X-Max 30, gives x from 20 to 30).
- The grid labels at most 7 intervals of 1, 2 or 5 times a power of ten, with lines at a fifth
  of them (a fourth for 2s): 10 and 2 for x from −27 to 27, 5 and 1 for y from −3 to 30.
- The wheel zooms by 15 % a notch around the pointer; the buttons by 1.0625 from the center.
- The tracer rounds x to 10^(⌊log₁₀ x range⌋ − 3) and shows y with 15 decimals:
  `(3.82, 14.592400000000000)`. A value that rounds to 0 has no sign, as .NET Native writes it:
  `(0, 0.000000000000000)` just left of the origin on y = x.
- The keypad's keys type `GetButtonOutput`'s text: `sin()` with the cursor inside, `root(x,n)`
  with the n selected, `log(b, x)` with the b selected, `^2`, `10^`, `1/`.
- The analysis of the manual tests' functions, word for word: domain `{x ∈ ℝ}`, `{x ≠ 0}`,
  `{x > 0}`, `{x ∈ [−5, 5]}`, `{x ≠ πn₁ + π/2, ∀n₁ ∈ ℤ}`; range `{y ∈ ℝ∖{0}}`, `{y ∈ [0, ∞)}`,
  `{y ∈ (−∞, −2√3 − 6] ∪ [2√3 − 6, ∞)}`; families like `{(2πn₁ + π/2, 1), n₁ ∈ ℤ}`; asymptotes
  `x = 0`, `y = 0`, `y = −3x − 3`; parity, period (`2π`) and monotonicity intervals. A line isn't
  its own asymptote; a hole ((x² − 1)/(x − 1) at 1) isn't one either. `y = mx + b` and
  inequalities aren't analyzed; x = g(y) and implicit relations get `KGFVariableIsNotX`.

### Date calculation

These come from the Windows Calculator source and are pinned by the tests in `dates.rs` and
`app/date_view.rs`:

- The difference ignores the order of the dates. It counts whole years, then whole months from
  where the years ended, then weeks (7 days), and the rest is days.
- Month and year steps clamp to the end of the month: January 31 + 1 month = February 28, and
  February 29 + 1 year = February 28. Later units continue from the clamped date.
- Output:
  - Equal dates give `Same dates`.
  - Under 7 days, only the days are shown (`5 days`) and there is no second line.
  - Otherwise the non-zero units are joined with `, ` (`1 year, 2 months, 1 week, 3 days`),
    and a second line shows the total (`10 days`). That total has no thousands separators.
  - A unit is singular only for exactly 1 (`1 day`, `2 days`).
- Add goes years → months → days. Subtract goes the other way: days → months → years.
  For example, March 31 − 1 month 1 day = February 28.
- Supported dates are 1601-01-01 to 9999-12-31. A result outside that range shows
  `Date out of Bound`. The offsets go from 0 to 999.
- Formats: pickers show `October 15, 2026`, and the add/subtract result shows
  `Thursday, October 15, 2026` (in en-US; see "Regional formats").
- The From date goes along when switching pages (`DateCalcOption_Changed`): picking
  January 15, 2020 in "Difference between dates" makes it the From date of "Add or subtract
  days" too, and back. The To date stays on its page.

### Unit converters

These come from the Windows Calculator source and are pinned by the tests in `converter.rs`:

- Results get at least 7 significant digits, or as many as were typed (up to 15), and trailing
  zeros are dropped: 5 km → `3.106856` mi, 1 m → `3.28084` ft. Results below 0.000001 get 15
  decimals. Above 15 integer digits or below 1e-14 they use C++'s scientific notation, shown with
  the exponent's leading zero removed: `8.000000e+24`, `1.500000e-5`.
- Units with the same factor (milliliters and cubic centimeters) echo the input as typed,
  without trailing zeros.
- Up to 15 characters can be typed, counting the minus sign but not the decimal point.
- `±` exists only for Temperature, Power and Angle. Switching to another category drops a minus sign.
- Activating the other field keeps both values; the next key starts a new number there.
  A new source unit does the same, a new target unit just converts again.
- Paste accepts one plain number (no expression, no e-notation, at most 16 characters). A minus
  sign separated from the number is dropped (`- 3` pastes `3`).
- "About equal to" lists the other units closest to 1 first (by the size of log10), rounded to
  2 decimals below 100, 1 below 1,000 and none above, without results that round to 0 (except in
  Temperature, Power and Angle). Then comes the everyday comparison closest to 1:
  100 cm → `1 m`, `1.09 yd`, `3.28 ft`, … `5.36 hands`.

### Regional formats

Windows takes these from the user's regional format, not the display language. They are pinned
by the tests in `locale.rs`, `region.rs` and `dates.rs`:

- Numbers use the region's decimal separator and grouping, applied like the engine's
  `GroupDigits`: grouping `3;0` gives `1,234,567`, India's `3;2;0` gives `12,34,56,789`, and
  `3` groups only the last three digits. The expression line has the decimal separator but no
  grouping, and `Ctrl+C` copies without group separators.
- The date pickers show the region's long date without the day of the week (`15. Oktober 2026`)
  and the words and separators that go with it (Danish `dddd 'den' d. MMMM yyyy` gives
  `d. MMMM yyyy`, as in CLDR's long date), the add/subtract result the long date, and the
  calendar the region's month and weekday names, starting on its first day of the week.
- The date difference joins its units with the region's list separator and a space (`; ` in
  Germany), with the unit words in the interface language.
- On Android they come from Java's locale classes through JNI (`android::region_source`),
  read once per region, and `region/java.rs` turns them into `Formats` without JNI, so the
  host tests pin them with the values Android gives: `1.234.567,89`, `15. Oktober 2026`,
  `Donnerstag, 15. Oktober 2026` (de-DE); for ru-RU `1 234 567,89`, the day, the month's
  genitive name and the year with the abbreviation for "year", the calendar's title with the
  month's nominative name (the standalone month, `LLLL`) and a header of two-letter weekday
  names from Monday; India's `3;2;0` grouping off `12,34,56,78,90,123.5`; the era of th-TH's Gregorian
  pattern left out. A failed JNI call or odd data gives en-US, never a panic. The list
  separator is `;` with a decimal comma, else `,`, as in the web version.

## Conventions

- **English everywhere:** code, comments, docs, UI strings, commit messages and PR text.
- Keep `engine.rs`, `dates.rs`, `converter.rs` and `graphing.rs` (with `graphing/`) free of
  egui and other UI code. Any new calculator, date, converter or graphing behavior needs a test.
- Comments explain *why*, especially Windows quirks. Public items get `///` doc comments.
- Code is formatted with `rustfmt` (default settings). The keypad table `KEYS` is marked
  `#[rustfmt::skip]` so it keeps its 4-column grid.
- Prefer small, focused changes. Don't reformat or restructure code you aren't changing.
- **Privacy:** `PRIVACY.md` (published at <https://rapalon.com/calccraft/privacy>) describes
  everything the app stores, reads and lets leave the device, from the audit in
  `docs/privacy-audit.md`; `docs/play-data-safety.md` holds Google Play's Data safety answers.
  A change that stores something new, adds a permission, a network request, an SDK, a link or
  a link parameter updates all three, and the policy's effective date, in the same pull request.
- **Website pages:** CalcCraft's product page (`website/calccraft.md` and its images), the
  privacy policy (`PRIVACY.md`) and the Terms of Service (`TERMS.md`, with its "Last updated"
  date) are edited here, never directly in rapalon/website: `website/sync.mjs` copies them there
  and overwrites any edit made there (`website/README.md`). Write the legal texts for phones:
  short sections, lists rather than tables, links named in words rather than long bare URLs.
  A change to what CalcCraft Pro or the paid app offers, or to how it's sold, checks `TERMS.md`.

## Pitfalls

- **Only permissive licenses.** Every library must be under a license on `deny.toml`'s allow-list
  (MIT, Apache-2.0, BSD, ISC, Zlib …); CI's `licenses` job fails on anything else, GPL, LGPL
  and MPL included. Before adding a dependency, check its license (and its own dependencies')
  with `cargo deny check licenses`. Don't widen the allow-list for one library: an exception
  for it (as for egui's fonts) needs a reason in `deny.toml`.
- **`num-bigint` must stay on 0.4.x.** `num-rational` 0.4 depends on it, and `BigRational`'s
  integer type has to match ours. `cargo add num-bigint` without a version picks 0.5 and breaks the build.
- **egui 0.36 API:** the app implements `App::ui(&mut self, ui, frame)` (not `update`).
- **Focus:** egui presses a focused widget on `Space` *and* `Enter`. `handle_key` consumes
  `Enter` when it means `=`, so the focused key isn't pressed on top of it. Don't call
  `ui.interact` with `Sense::click()` (which is focusable) for controls: go through
  `access::control`. egui's arrow-key search remembers focusable widgets as long as their ids are
  used, which is why covered controls take another id. egui never scrolls to the focused widget
  on its own, and `gained_focus` is false after an arrow key, so `access::control` scrolls when
  the focused control isn't fully visible.
- **Focus and AccessKit:** a `request_focus` for a widget that isn't drawn in that frame (a new
  Graphing box) makes egui name a focus without a node, and AccessKit's consumer panics on that
  tree: it ended the Android app. `access::settle_focus` (end of `App::ui`) moves such a focus to
  the next frame; keep it last. A text that isn't a control (`access::text`) sets its own
  bounds: egui gives them only to interactive widgets, and Android drops nodes without them.
- egui's text layout has no bidirectional text and no shaping. `strings::UNSUPPORTED` lists the
  languages it can't draw (Arabic, Hebrew, the Indic scripts, …); CJK, Thai and Amharic need the
  Windows font from `strings::script_font`, which `setup_fonts` loads.
- The locale files are generated: edit `tools/import-strings.ps1`, not `locales/` (except
  `locales/calccraft.txt`). A test checks that every language has every key and that every unit's
  key names it in English.
- Language and region switch while the app runs. `tr` and `region::formats` hand out
  `&'static` data, so every language and region loaded stays loaded (`LOADED`); don't cache a
  string or a format across frames, look it up when drawing.
- The font families `semibold`, `light`, `symbols` and `icons` must always be registered in
  `setup_fonts`, even when the font files are missing: egui panics on unknown font families.
- Don't bundle the Windows Calculator's `CalculatorIcons.ttf` (it has the converter icons and
  the `±` glyph) or any Segoe font: their licenses allow them only inside Windows. The icons
  are Fluent System Icons (MIT) in the embedded icon font (`category_icon` has the converters'),
  and Inter (SIL OFL) stands in for Segoe UI. Keep `assets/fonts/README.md` and the license
  files there up to date when a font changes.
- The title bar tint (`TitleBar`) calls `DwmSetWindowAttribute` and only does something under
  `#[cfg(windows)]`. It keeps the window handle as an `isize`, so the app struct doesn't hold a
  raw pointer.
- eframe's `persistence` feature saves state in `%APPDATA%\CalcCraft\data\app.ron` (the folder
  is named after the app name passed to `run_native`). `persist_window` is off, so the window
  always opens at its default size. egui's memory is saved there too, scroll positions
  included: a list that should open at a given place has to set its offset when it opens.
- The minimum window size is Windows' 320 × 500 (`MIN_SIZE`); keep-on-top mode lowers it while
  it's on, so its 320 × 394 window fits.
- Release builds use `windows_subsystem = "windows"`, so `println!` output goes nowhere.
  Debug with `cargo run`.
- **The web version** (`target_arch = "wasm32"`, `index.html`, `docs/web-version-plan.md`) runs
  the same code in the browser:
  - `std::time::SystemTime::now()` and `Instant::now()` panic there; use `web_time`.
  - `std::fs` and `std::env::var` find nothing, threads can't be spawned, and `arboard` doesn't
    build. Code that needs the browser goes into `src/app/web.rs`.
  - The page can't read the system's fonts. The web version (and the Android app) puts the
    embedded Noto Sans Math (`assets/fonts/`; every build has it for the keys' italic `𝑥`) in
    the text fonts for the symbols Inter lacks (`ℝ ℤ ∖ ∈ ⌊ ⌋ ∛`), before egui's Hack, which
    comes last in every family on all systems.
  - Browsers keep some keys: eframe never stops F5 or Ctrl+R (the calculator's GRAD and MR), so
    `web::guard_shortcuts` does, with the other browser shortcuts the calculator uses.
  - The canvas has a CSS minimum of 320 × 500 (`MIN_SIZE`): the layout code never sees a smaller
    window, the page scrolls instead. On large screens with a mouse (`index.html`'s media
    query) the canvas is a centered, resizable window, sized by `index.html`'s own script, not by
    the app: eframe just follows the canvas's size.
  - The service worker (`assets/web/sw.js`) fetches the page from the network first and keeps
    everything else, deleting the build files a new page no longer names. That's safe only for
    files whose names change with their contents, as Trunk names the app's: a new file the
    page loads should get a hashed name too (a Trunk asset), not a fixed one.
- `build.rs` includes `src/icon.rs` with `#[path]`, so `icon.rs` must not use egui or any other
  dependency. Its images (`assets/icon/`) are generated: change the icon in
  `tools/app-icon.py` (or its reference PNG) and run `uv run tools/app-icon.py`, which rewrites
  them with the Android, Play and web icons. The build writes a multi-size .ico file to `OUT_DIR` and embeds it with
  `winresource`, which needs `rc.exe` from the Windows SDK. After rebuilding, Explorer may show
  the old icon from its icon cache for a while.
- egui 0.36 has no `raw_scroll_delta`. The calendar reads `Event::MouseWheel` events itself
  (`wheel_delta`) and flips one page per 40 points of wheel movement.
- `chrono` is used without default features (`clock`, `std` only). Use `checked_*` methods for
  date math and keep results inside `MIN_DATE..=MAX_DATE`.
- Inside a `ScrollArea`, place a widget at a rect with `ui.new_child(UiBuilder::new().max_rect(…))`,
  not `scope_builder` or `put`: those move the parent's layout cursor to the widget, so the rows
  after it overlap (the Graphing mode's `text_ui`).
- A `TextEdit` inside a list keeps the arrow keys only for itself; a hand-made control that
  uses them (the variable slider, the graph while tracing) sets `set_focus_lock_filter`, or
  egui moves the focus on the next arrow. The calendar, whose arrow keys move the focus from cell
  to cell, cancels egui's move in that frame instead (`move_focus(FocusDirection::None)`): a
  cell that has just got the focus can't set a lock filter yet.
- `LayoutJob::append` merges a section into the previous one when the format is the same, so
  sections don't map to characters; look a character's format up by byte range.
- The graphing math is `f64` and the calculator engine's `BigRational`: don't mix them. `tan(π/2)`
  is 1.6e16 in doubles, not infinite, so the analysis treats values above 1e14 as poles
  (`Fun::defined`). Likewise sin(2x)/cos(x) is 2 at π/2 (1.2e-16 / 6.1e-17), so the zeros of
  denominators (where they cross or reach 0) are never in the domain, whatever f evaluates to
  there (`boundary_functions`); next to such a hole, where 0/0 cancels, f′, f″ and the closest
  values are rounding noise.
- `arboard` reads the clipboard for the equation boxes' Paste menu item (keyboard pastes arrive
  as egui events); egui writes it, images included, through its own `arboard`.
- `f32::clamp(min, max)` panics when `min > max`. Placing a popup with
  `clamp(left + 4, right - width - 4)` crashes the app once the popup is wider than the window
  (the graph options in a 320 px window did). Use `.min(right - width - 4).max(left + 4)`, and
  make wide popups narrower in narrow windows.

## Verifying UI changes

Unit tests don't cover the UI. After visual or interaction changes:

1. Run the app (`cargo run`) and click through what you changed: hover, press, disabled
   states (enter `1/0=` for the error state), the history and memory panels and their
   right-click menus, a window wider than 560 px (docked panel), the menu,
   keep-on-top mode, and resizing. Go through it with the keyboard too (`Tab`, arrows, `Space`,
   `Enter`, `Esc`), and check a screen reader's view (Narrator, or a UI Automation tree
   inspector). Check both themes (Settings → App theme), and a long-worded language and another
   regional format: `CALCCRAFT_LANGUAGE=de-DE` and `CALCCRAFT_REGION=de-DE`. In the date mode,
   check the page list, both date pickers with the month, year and decade views, the 0–999
   lists, `Esc`, and both a narrow (< 480 px) and a wide window. In the converters, check
   both unit drop-downs (a short list like Pressure and a scrolling one like Data), switching
   the active field, `±` in Temperature, paste, and a landscape and a ≥ 640 px window. In the
   Programmer mode, check every radix with a negative value (the BIN row wraps), all four word
   sizes, the Bitwise and Bit shift menus, the bit toggling keypad, the radix list's Copy menu,
   the error state (`1 Lsh 64 =`), the memory flyout and the docked memory list, and switching
   to Standard and back. In the Scientific mode, check `2nd` (and its hover), both menus with
   their toggles, DEG/RAD/GRAD and F-E, `exp`, the error state (`0 ln`), the history and memory
   flyouts and the docked panel with 32-digit results, a ≥ 527 × 523 px keypad (the `Medium`
   sizes), and the letter keys (`S`, `Shift+S`, `Ctrl+S`). In the Graphing mode, check a window
   under and over 800 px (the switch in the header), typing equations and the keypad (with `2nd`
   and its three menus) into the box that had the focus, Enter, an error (`x)` and its icon's
   tooltip), the hover buttons and the context menu, hiding a line, the line options flyout,
   inequalities and an implicit curve, `y = mx + b` with its sliders and options, the function
   analysis of a few manual-test functions, dragging, the wheel, the zoom buttons held down,
   `Ctrl` + plus, minus and 0, hover tracing and active tracing with the arrow keys and `Esc`,
   the graph options (ranges, units, line thickness, graph theme in both app themes), Copy, and
   the share sheet (Paint and Outlook among its targets). Open every popup in the narrowest
   window (320 px) too. For editing (Standard, Scientific, Programmer): click into the number
   being typed (grouped, `de-DE`, HEX and BIN), a number of the expression line before and after
   `=`, the display after `)`, Edit in the history menu, and the caret keys.
2. Compare it side by side with the real Windows Calculator.
3. If the look changed, update `docs/screenshot.png` (dark), `docs/screenshot-light.png`,
   `docs/scientific.png`, `docs/date-calculation.png`, `docs/programmer.png` or
   `docs/unit-converter.png`.

## Git and pull requests

- Work on a branch and open a pull request against the default branch.
- Write commit subjects in the imperative mood (`Add memory list flyout`), and keep each
  commit to one topic.
- In the PR description, say what changed, why, and how you verified it (tests and manual
  checks). Include before/after screenshots for UI changes.
- Pushing a `v*` tag publishes a GitHub release for Windows and Linux (x64), macOS (Universal 2
  for Apple silicon and Intel) and Android (arm64) (`.github/workflows/release.yml`, see "Releasing" in the README). The tag must match `version` in `Cargo.toml`. Don't create or push tags unless
  a maintainer asks. Run by hand (`workflow_dispatch`), the release workflow builds only the
  signed Google Play test files of a branch as a run artifact, without a tag or a release
  (`android/README.md`, "Version codes, and the release"); starting it is a maintainer's call too.
- **Before a Google Play release**, run the device scenarios (`tests/device/README.md`) on a
  phone with the build to be released: on pc_010 through adb (`pwsh -File tests/device/run.ps1
  -Serial <serial> -Package <application id> -Out target/device-results.md`), or through Marionet
  with the README's agent prompt. Attach the result table to the release's pull request or
  task; a failed step blocks the release until it's understood.
