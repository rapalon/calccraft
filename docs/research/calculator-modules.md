# Research: pluggable calculator modules (chemistry, biology, …) as Rust crates

Status: research only, written 2026-11-18 for Mission Control task 58afde9d from `origin/main`
at 0517cd1 (CalcCraft 1.4.2). Nothing in the product changed. The code references below
(`src/app.rs:547` and the like) are to that commit.

## Краткое резюме (по-русски)

Добавлять новые калькуляторы (химический, биологический и другие) как отдельные Rust-крейты
можно, и в архитектуре CalcCraft для этого почти всё уже есть: каждый режим — это своё
состояние плюс функции отрисовки и клавиатуры, которые `app.rs` вызывает по значению `Mode`.
Мешает только то, что список режимов зашит в перечисление `Mode` и примерно в двенадцать мест
кода (меню, сохранение, клавиатура, строки, тесты); рефакторинг сводит их к одному реестру
модулей и трейту `CalcModule` в небольшом крейте `calccraft-module-api`. Модули надо
линковать статически через Cargo-features, а не подгружать как плагины: у Rust нет стабильного
ABI, в браузере (wasm) и на Android динамическая загрузка кода либо невозможна, либо запрещена
правилами Google Play, а размер модуля (десятки–сотни килобайт) на фоне 23 МБ exe и 11 МБ wasm
несущественен. Платная версия Pro получает модули через feature `pro`, которую включает
`play-paid`; бесплатная сборка их не содержит вовсе, как сегодня не содержит ссылку на донат.
Рекомендую начать с химического модуля (молярная масса по формуле, стехиометрия и
уравнивание реакций, разведение C1V1=C2V2, pH, газовые законы), а вторым сделать
биологический/лабораторный (разведения, плотность клеток, ПЦР, молярность ДНК и белков,
Харди–Вайнберг): у них общая «лабораторная» основа, данные общедоступны (атомные массы IUPAC —
факты, не защищённые авторским правом), и это две разные аудитории студентов. Рефакторинг —
примерно 5–8 агенто-дней в 4 задачах, первый модуль — ещё 8–12 агенто-дней в 3–4 задачах.

**Verdict in one line:** feasible on every platform with a static, feature-gated module
registry; recommended first modules for the Pro edition: Chemistry, then Biology/lab.

---

## 1. How modes work today

### 1.1 The `Mode` enum and its lists

- `Mode` (`src/app.rs:547`) is a closed enum: `Standard`, `Scientific`, `Graphing`,
  `Programmer`, `DateCalculation`, `Converter(Category)`. `Category` (`src/converter.rs:35`) is
  the converter's own closed enum of 12 categories with `Category::ALL`.
- `Mode::CALCULATORS` (`src/app.rs:559`) is the menu's first group in Windows' order; the
  second group is `Category::ALL.map(Mode::Converter)`. Both lists are rebuilt in three places:
  `draw_navigation` (`src/app.rs:2679`), `Mode::from_serialization_id` (`src/app.rs:632`) and
  `layout_tests::all_modes` (`src/app/layout_tests.rs:62`), plus the test
  `opens_in_the_mode_it_was_closed_in` (`src/app.rs:5433`).
- `Mode::title()` (`src/app.rs:567`) looks up Windows' resource keys (`StandardModeText`,
  `GraphingCalculatorModeText`, …) with `strings::tr`; a converter's title is `category_name`.
- `Mode::for_alt_key` (`src/app.rs:581`): `Alt+1` … `Alt+5` for the five calculators, none
  for converters (Windows' `NavCategory`). `take_mode_shortcut` (`src/app.rs:4448`) consumes
  every `Alt` chord's text.
- `is_calculator()` (Standard, Scientific, Programmer: an engine, a keypad, the memory) and
  `has_history()` (Standard, Scientific) gate the shared machinery: `engine()`/`engine_mut()`
  (`src/app.rs:1111`), the memory hand-over in `set_mode` (`src/app.rs:1190`), the docked
  panel (`self.docked`, `src/app.rs:3858`), the landscape layout (`landscape`, `src/app.rs:1094`,
  returns `None` for every other mode), the support prompt's "calculator mode" condition
  (`draw_frame`, `src/app.rs:3923`).

### 1.2 State

`CalculatorApp` (`src/app.rs:881`) owns one state struct per mode family:

| Field | Type | Module |
| --- | --- | --- |
| `calc` | `engine::Calculator` | Standard |
| `scientific` | `scientific_view::ScientificState` (holds its `calc`) | Scientific |
| `programmer` | `programmer_view::ProgrammerState` (holds its `calc`) | Programmer |
| `date` | `date_view::DateState` | Date calculation |
| `converter` | `converter_view::ConverterState` (holds `converter::Converter`) | all 12 converters |
| `graphing` | `graphing_view::GraphingState` | Graphing |

Each `*State` has `new()`, `popup_open()` and `close_popup()`; `popup_covers` (`src/app.rs:1079`)
and `set_mode` (`src/app.rs:1210`) call them one by one. The Android back button closes "what is
open" through `popup_covers` (`raw_input_hook`, `src/app.rs:3842`).

### 1.3 Drawing and keyboard dispatch

`draw_frame` (`src/app.rs:3918`) is the dispatcher:

1. `handle_mode_keys` (every mode), then a `match self.mode` for the keyboard:
   `handle_date_keyboard`, `handle_converter_keyboard`, `handle_graphing_keyboard`, or the
   shared `handle_keyboard` (Standard, Scientific, Programmer, and the Settings page).
2. Settings page → `draw_settings` and return.
3. Graphing → `draw_header` + `draw_graphing_mode(ui, full, header.bottom())` +
   `draw_graphing_popups` + support popup + navigation.
4. Non-calculators → `draw_header` + `draw_date_mode` or `draw_converter_mode`
   `(ui, full, top)` + support popup + navigation.
5. Calculators: landscape, docked and flyout layouts with `draw_display`, `draw_memory_row`,
   `draw_keypad`, `draw_dock`, `draw_flyout`.

The pattern a new non-keypad mode follows is the date mode's and the converters' (steps 1 and
4): a method `draw_<mode>_mode(&mut self, ui, full: Rect, top: f32)` as an `impl CalculatorApp`
in a child module (`src/app/date_view.rs:263`, `src/app/converter_view.rs:431`), a
`handle_<mode>_keyboard`, and popups drawn last over a backdrop (`popup_frame`, `list_item` in
`date_view.rs:1019` and `:1086`, reused by the converters).

`draw_header` (`src/app.rs:1573`) draws the ☰ button (name `Open Navigation`), the title
`self.mode.title()` at 20 px semibold, keep-on-top (Standard only) and the history button
(`has_history()`). The header has no screen reader name beyond the ☰ button; Windows'
`HeaderAutomationName_*` keys are imported but unused.

### 1.4 The navigation menu

`draw_navigation` (`src/app.rs:2627`) draws two groups from a literal array
(`src/app.rs:2679`): heading `CalculatorModeTextCaps` / `ConverterModeTextCaps`, screen reader
name `NavCategoryHeader_AutomationNameFormat` with the plural (`CalculatorModePluralText`), and
items named `NavCategoryItem_AutomationNameFormat` ("Standard Calculator", "Length Converter").
Icons: a `match mode` (`src/app.rs:2719`) of glyphs from the embedded icon font
(`ICON_CALCULATOR`, `ICON_SCIENTIFIC`, `ICON_CODE`, `ICON_CALENDAR`, `category_icon`), except
Graphing, which paints its icon in code (`graphing_view::paint_graph_icon`), a precedent for a
module that can't add a glyph to the font. Item ids are `Id::new("nav-mode").with(mode.title())`.

### 1.5 Persistence

`save` (`src/app.rs:3807`) writes flat string keys to eframe's storage (`%APPDATA%\CalcCraft\
data\app.ron`, the browser's local storage, Android's `app.ron`):

- `Mode` (`MODE_KEY`): Windows' `SerializationId` as an integer, `Mode::serialization_id`
  (`src/app.rs:606`; 0–3, 4–15 converters, 17 Graphing; 16 is Currency, absent). `new`
  (`src/app.rs:1014`) parses it back and `set_mode`s it; an unknown id opens Standard. The test
  `modes_are_saved_with_windows_ids` (`src/app.rs:5506`) pins the table.
- `UnitConverterPreferences` (`Converter::preferences`), `IsGraphThemeMatchApp`
  (`graphing_view::THEME_KEY`), `SelectedAppTheme`, `CalcCraftLanguage`,
  `CalcCraftRegionalFormat`, the Android keys, the keep-on-top size, `SupportStatsV2`.
- `CALCCRAFT_MODE=<id>` (desktop, `src/app.rs:1022`) opens a mode for screenshots.
- `PRIVACY.md`, `docs/privacy-audit.md` and `docs/play-data-safety.md` list every stored key;
  AGENTS.md ("Privacy") requires updating all three, and the policy's date, for a new key.

### 1.6 History and memory

Both belong to `engine::Calculator` (`history()`, `memory()`, `take_memory`, `set_memory`),
shown by `draw_dock`/`draw_flyout`/`draw_list` for `Panel::History | Memory`, only in
`is_calculator()` modes, and the Programmer mode has no history. Date calculation, the
converters and Graphing have neither, as in Windows. History is not persisted.

### 1.7 Accessibility

Every control goes through `access::control` / `interact` (`src/app/access.rs:59`, `:78`),
which makes it clickable + focusable (unless "covered" by a popup, `set_covered`,
`draw_covered`), gives it an AccessKit `WidgetInfo` (`access::button`, `item`, `combo_box`) and
draws WinUI's focus ring. Non-control text is `access::text(…, live)` (`:141`), a `Label`
node, `Live::Polite` for results. Names are Windows' `AutomationName` resource strings through
`tr`. The layout tests (`src/app/layout_tests.rs:107`) read the AccessKit tree and require every
`Role::Button` to be at least 40 pt high and inside the safe area, in every mode of
`all_modes()` at five Android screen sizes.

### 1.8 Localisation

- `strings::LOCALES` (`src/app/strings.rs:10`): 61 Windows languages, each
  `include_str!("../../locales/<tag>.txt")` (2.0 MB on disk, all compiled in), generated by
  `tools/import-strings.ps1` from microsoft/calculator's `.resw` files (MIT). `UNSUPPORTED`
  (`:78`) lists 10 scripts egui can't draw.
- CalcCraft's own strings: `locales/calccraft.txt` with `[language]` sections and `CalcCraft.*`
  keys (`OWN`, `own()`, `:84`, `:132`), translated by hand; `Strings::new` (`:101`) merges a
  language's table over English.
- `tr(key) -> &'static str` (`:234`), `tr_format` (`%1`), `unit_name`, `unit_abbreviation`.
  Languages are loaded lazily and leaked (`LOADED`), so `tr` is `'static` and the language can
  switch at runtime (`set_language`, `:175`).
- Tests (`:410`–`:612`): every language has every English key; every drawable language has the
  listed `CalcCraft.*` core keys (the support strings are allowed to fall back to English).
- Numbers: `region::formats().numbers` → `locale::NumberFormat::localize` / `delocalize`
  (`src/locale.rs:47`, `:85`); dates through `region::formats()`.

### 1.9 Tests that know every mode

`src/app.rs` tests (`start_in`, `run`, `accesskit_nodes`, `src/app.rs:5206`–`:5900`) drive the
whole app with `egui::Context::run_ui` and `eframe::CreationContext::_new_kittest`;
`layout_tests` paints every mode; `monkey.rs` fuzzes the engines, the converter, dates and
graphing (not the UI).

### 1.10 Platforms

- Web (`wasm32`): no `std::fs`, `std::env`, threads or `SystemTime` (`web_time`); the system's
  fonts are unreachable; browser shortcuts are guarded in `web::guard_shortcuts`; Trunk builds
  one `.wasm` (`index.html` names the binary `calculator`), 11.2 MB / 3.9 MB gzipped per
  `docs/web-version-plan.md`.
- Android: `NativeActivity`, `cargo ndk … rustc --lib --release --crate-type cdylib`
  (`android/build-apk.ps1:162`) with `--features play-free|play-paid`; stripped
  `libcalculator.so`; the Play build fails if the library contains the donate or Stripe URL
  (`build-apk.ps1:175`); the back button goes through `popup_covers`.
- Channels (`src/support/channel.rs`, `docs/support-prompt.md` §8): Cargo features
  `support-prompt` (default), `play-free`, `play-paid` (mutually exclusive), `CALCCRAFT_PAID_ID`
  via `option_env!`. `play-paid` is the future **CalcCraft Pro** (`com.rapalon.calccraft.pro`).
  CI runs clippy/tests for both Play channels (`.github/workflows/ci.yml:51`, `:132`).
- Release binary measured today on Windows (`cargo build --release`, default profile, no LTO
  or strip settings in `Cargo.toml`):

| Build | `calculator.exe` |
| --- | --- |
| default features | 23,101,440 bytes |
| `--no-default-features` (no support prompt) | 23,023,104 bytes |

  So the whole support module, with its UI and policy, costs about 78 KB. Embedded assets:
  fonts 1.5 MB (`Inter.ttf` 877 KB, `NotoSansMath-Regular.ttf` 657 KB, icons 11 KB), locales
  2.0 MB.

### 1.11 What a new mode touches today (the checklist the refactor removes)

Adding, say, Chemistry as a sixth mode the way Graphing was added means editing:

1. `Mode` + `CALCULATORS` (or a new list), `title`, `serialization_id`/`from_serialization_id`
   (`src/app.rs:547`–`:638`).
2. `CalculatorApp` field + `new()` construction (`:881`, `:1030`).
3. `popup_covers`, `set_mode` close-popup calls (`:1079`, `:1210`).
4. `draw_frame`'s keyboard `match` and drawing branch (`:3928`, `:3963`).
5. `draw_navigation`: group list and icon `match` (`:2679`, `:2719`); icon glyph in
   `tools/icon-font.py` + `src/app/icons.rs` + font rebuild, or a painted icon.
6. A new `src/app/<mode>_view.rs` with `draw_<mode>_mode`, `handle_<mode>_keyboard`, popups.
7. Pure logic in `src/<mode>.rs` with tests (the convention for engine/dates/converter/graphing).
8. Strings: keys in `tools/import-strings.ps1` (if Windows has them, it won't) or
   `locales/calccraft.txt` sections, plus `every_language_has_calccraft_strings`.
9. Persistence: `save`/`new` keys; `PRIVACY.md`, `docs/privacy-audit.md`,
   `docs/play-data-safety.md`.
10. Tests: `modes_are_saved_with_windows_ids`, `opens_in_the_mode_it_was_closed_in`,
    `layout_tests::all_modes`.
11. AGENTS.md architecture table, README mode list, `index.html` description.
12. Optional: `Mode::for_alt_key`, `CALCCRAFT_MODE`.

Twelve places, all inside the one crate `calculator`. That is the cost the module boundary
below turns into "add one crate and one feature line".

---

## 2. The module boundary

### 2.1 Decision: static linking through Cargo features, not dynamic plugins

Recommendation: **static**. Each module is a Rust crate in the workspace, compiled in when its
feature is on, absent from the binary when it is off. Reasons dynamic plugins (a `.dll`/`.so`
per module, or downloadable modules) aren't worth it here:

- **Rust has no stable ABI.** A plugin and the host must be built by the same compiler with the
  same egui, or use a C ABI. egui's `Ui`, `Context`, `Painter`, `Response` can't cross a C ABI;
  a plugin would have to re-express the whole drawing and accessibility API in C types, or
  carry its own egui and allocator (a second copy of everything, and no shared fonts or
  AccessKit tree). `abi_stable`/`stabby` exist but add large dependencies and a second API
  surface to maintain. Nothing in CalcCraft's goals (one company shipping a few modules it
  writes itself) needs third parties to compile plugins.
- **Android.** Google Play's Device and Network Abuse policy forbids an app from downloading
  executable code (dex, JAR, `.so`) from anywhere but Google Play
  ([policy](https://support.google.com/googleplay/android-developer/answer/9888379)). A module
  could only ship inside the APK/bundle, which is what static linking does already, and
  `dlopen` of a second `.so` buys nothing but a second strip/align/16 KB-page step.
- **Web.** The `.wasm` is one module; wasm dynamic linking of Rust is experimental and Trunk
  doesn't do it. A second `.wasm` could be fetched, but it couldn't call egui in the first.
- **Size** is not a reason either: the whole support prompt (policy, stats, popup, menu button,
  logo) costs 78 KB of a 23 MB exe (§1.10). A chemistry module with a 118-element table and
  a dozen screens will be in the same range, well under 1 % of the binary or the `.wasm`.
- **Paid vs free** is solved at build time already (`play-free`/`play-paid`, §1.10); "the Pro
  app has the modules" is one more feature in the same mechanism, with the same CI check that
  the free library doesn't contain them.

Static modules keep everything the project values: one `cargo test`, clippy over every target,
`cargo deny` over one lock file, the single AccessKit tree, the runtime language switch.

### 2.2 Crates and workspace

```
Cargo.toml                       [workspace] members = ["crates/*"]; the root package stays
                                 `calculator` (Trunk's data-bin, libcalculator.so and the
                                 release artifacts are named after it)
crates/calccraft-module-api/     the trait, the host handle, the shared widget kit
crates/calccraft-module-chemistry/
crates/calccraft-module-biology/
src/app/modules.rs               the registry: which modules this build has (cfg features)
```

Root `Cargo.toml` additions:

```toml
[workspace]
members = ["crates/*"]

[features]
default = ["support-prompt"]
support-prompt = []
play-free = ["support-prompt"]
play-paid = ["support-prompt", "pro"]
# The Pro edition's modules. Each module is its own feature, so a desktop or test build can
# take one; `pro` is the set the paid app ships.
module-chemistry = ["dep:calccraft-module-chemistry"]
module-biology = ["dep:calccraft-module-biology"]
pro = ["module-chemistry", "module-biology"]

[dependencies]
calccraft-module-api = { path = "crates/calccraft-module-api" }
calccraft-module-chemistry = { path = "crates/calccraft-module-chemistry", optional = true }
calccraft-module-biology = { path = "crates/calccraft-module-biology", optional = true }
```

`calccraft-module-api` depends on `egui` (the version eframe pins; `eframe` re-exports it, so
the modules take `eframe::egui` to keep one egui) and nothing else. Module crates depend on
the api crate only; their logic has no egui (the `engine.rs` convention: a `chem.rs`-style pure
module with tests, a `view.rs` with the UI). `publish = false` on all of them: `deny.toml`
already ignores private crates' licences (`[licenses.private] ignore = true`).

CI changes: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
plus one job line with `--features pro` (desktop) and the Android clippy with `--features
play-paid` already covers the Pro library. `cargo deny` sees workspace members through the one
`Cargo.lock`.

Whether the **desktop and web** builds ship the modules is a product decision for Konst (see
§4.4). Technically it is one line: `default = ["support-prompt", "pro"]` or not. The Android
`play-free` build must not have them, and `build-apk.ps1` should check the free library for a
module marker string the same way it checks for the donate URL (`build-apk.ps1:175`).

### 2.3 The trait

The trait lives in `calccraft-module-api`. It mirrors what `date_view.rs` and
`converter_view.rs` already give `app.rs` (`new`, `popup_open`, `close_popup`, a keyboard
handler, a draw method below the header, popups drawn last), so the refactor of the existing
non-keypad modes into it is mechanical.

```rust
/// Identifies a module across builds and saved settings: ASCII, stable forever
/// (`"chemistry"`). Never reuse one for another module.
pub type ModuleId = &'static str;

/// What a build knows about a module before it is opened: enough for the menu.
pub trait ModuleFactory: Sync {
    fn id(&self) -> ModuleId;
    /// The resource key of the module's name (`"Module.chemistry.Name"`).
    fn title_key(&self) -> &'static str;
    /// The module's icon: a glyph of the icon font, or painted in code.
    fn icon(&self) -> Icon;
    /// The module's strings, in the `locales/calccraft.txt` format (`[language]` sections,
    /// `Module.<id>.*` keys); English is required, other languages fall back to it.
    fn strings(&self) -> &'static str;
    fn create(&self, host: &Host) -> Box<dyn CalcModule>;
}

pub enum Icon {
    Glyph(char),
    Painted(fn(&egui::Painter, egui::Pos2, f32, egui::Color32)),
}

/// An open module. The app calls these the way it calls the date mode and the converters.
pub trait CalcModule {
    /// Draws the module below the header. `full` is the whole window (or the left column of
    /// a phone on its side), `top` where the header ends. Popups are drawn later, in
    /// `draw_popups`, over the app's backdrop.
    fn draw(&mut self, ui: &mut egui::Ui, host: &Host, full: egui::Rect, top: f32);
    fn draw_popups(&mut self, ui: &mut egui::Ui, host: &Host);
    /// The frame's keyboard events, before drawing (`handle_date_keyboard`'s place). Esc
    /// closes an open popup; the host already took the mode shortcuts.
    fn keyboard(&mut self, ctx: &egui::Context, host: &Host);
    /// Whether a drop-down, list or dialog of the module is open (`popup_covers`): the
    /// controls under it leave the Tab order, and the Android back button closes it.
    fn popup_open(&self) -> bool;
    fn close_popup(&mut self);
    /// Called when the mode is left (the app's `set_mode`).
    fn leave(&mut self) {}
    /// Saved settings, namespaced by the host (`load` gets them back in `create`'s `host`).
    fn save(&self, storage: &mut dyn ModuleStorage) {}
    /// What `Ctrl+C` copies in this mode, if anything.
    fn copy_text(&self) -> Option<String> { None }
}
```

`Host` is a small struct of borrowed handles the app builds each frame, so the module needs
none of `CalculatorApp`'s private state:

```rust
pub struct Host<'a> {
    pub palette: &'a Palette,            // moved to the api crate (see 2.4)
    pub strings: &'a dyn Strings,        // tr / tr_format over the merged tables
    pub numbers: &'a NumberFormat,       // locale::NumberFormat (localize / delocalize)
    pub kit: &'a dyn Kit,                // the widgets: button, text box, combo, list, card…
    pub storage: &'a dyn ModuleStorage,  // read side of the saved settings at `create`
    pub now: f64,                        // ui.input(|i| i.time), for key flashes
    pub covered: bool,                   // access::is_covered
}
```

### 2.4 What moves into the api crate (the refactor)

Today everything a mode needs is `pub(super)` inside `src/app`. A module crate can only use
what the api crate exports, so these move (or get a thin exported twin):

| Today | In `calccraft-module-api` | Why |
| --- | --- | --- |
| `app/palette.rs` (`Palette`, `DARK`, `LIGHT`) | moved as is | colors come only from `Palette` (AGENTS.md); modules must not have color literals either |
| `app/access.rs` (`control`, `interact`, `text`, `button`, `item`, `combo_box`, `set_covered`, `is_covered`, `draw_covered`, `scroll_source`) | moved as is | every module control must be focusable and named; the "covered" flag lives in egui's temp data, so it already crosses crate lines |
| `CalculatorApp::button` (`Kind`, `button_flash`), `draw_combo_card`, `radio_button`, `date_view::list_item`, `popup_frame`, the converter's value field and unit picker, Graphing's `TextEdit` styling (`math_job` excluded) | a `Kit` trait implemented by the app, or free functions taking `&Palette` | one look everywhere; the key-flash timer and `access` wiring stay in one place |
| `strings::tr`, `tr_format` | a `Strings` trait object; the app's implementation merges the modules' `[language]` tables into `Strings::new` through the registry | `tr` returns `&'static str` from leaked tables; module tables are leaked the same way, so nothing changes for the callers |
| `locale::NumberFormat` | `pub` already; re-exported from `src/locale.rs` into the api crate (move the file; it has no UI) | numbers shown by a module must follow the region (AGENTS.md "Text") |
| `converter::Category`, `Unit`, factors | a `pub fn convert(category, from_key, to_key, value) -> f64` and `pub fn units(category)` in `converter.rs`, re-exported | a chemistry gas-law screen converts pressure and volume units; no second unit table |
| `engine::Number` (`BigRational`) and `format_with` | optional: `pub` helpers `exact::Rational`/`format_significant(f64, digits)` | stoichiometry balancing wants exact rationals; most modules use `f64` like the converter |

Everything else (`CalculatorApp`, the engines, the flyouts, the dock, the Settings page) stays
private in `calculator`. The api crate is small: palette, access, the kit trait, the strings
trait, the number format, and the trait above. Roughly 1,000 lines moved, 300 new.

### 2.5 The registry and the `Mode` change

```rust
// src/app/modules.rs
pub(super) static MODULES: &[&dyn ModuleFactory] = &[
    #[cfg(feature = "module-chemistry")]
    &calccraft_module_chemistry::FACTORY,
    #[cfg(feature = "module-biology")]
    &calccraft_module_biology::FACTORY,
];
```

- `Mode` gets one variant, `Mode::Module(ModuleId)` (a `&'static str` keeps `Mode: Copy + Eq`).
  `CalculatorApp` gets `modules: Vec<Box<dyn CalcModule>>` (created lazily on first open, in
  `MODULES` order) and `module_index(id)`.
- `draw_navigation` adds a third group when `MODULES` isn't empty: heading
  `CalcCraft.ModulesCaps`, plural `CalcCraft.ModulesPlural` (new strings in `calccraft.txt`,
  translated into all drawable languages like the other core keys), items from
  `MODULES.iter().map(|f| Mode::Module(f.id()))`, icon from `ModuleFactory::icon`. Windows has
  no such group, so its wording is CalcCraft's own ("Modules" / "Modules", or "Pro" if Konst
  prefers the brand; see §4.4).
- `draw_frame`: `Mode::Module(_)` takes the non-calculator branch (header + `module.draw(ui,
  &host, full, header.bottom())` + `draw_popups` + support popup + navigation); the keyboard
  `match` gets `Mode::Module(_) => module.keyboard(&ctx, &host)`. The landscape layout
  (`landscape()`) returns `None` for modules as it does for the date mode: the module gets the
  full rect and lays out for 780 × 360 itself (`layout_tests` will check it).
- `popup_covers` / `set_mode`: `self.open_module().is_some_and(|m| m.popup_open())` and
  `close_popup()` on the module being left, plus `leave()`.
- `Mode::title()` → `host.strings.tr(factory.title_key())`. `for_alt_key`: no shortcuts for
  modules (Windows gives converters none either); `Ctrl+Shift+<digit>` could be CalcCraft's own
  later.
- Persistence (§2.7) replaces `serialization_id` for modules.
- `layout_tests::all_modes` and the two mode tests iterate `MODULES` too, so a module that is
  compiled in is laid out at every Android size and round-trips through `save`/`new`.

### 2.6 UI conventions a module follows

- **Look:** Windows' own form-style modes are the template. The date mode's 16 px page margin,
  32 px controls, 12 px captions and 14 px values (`src/app/date_view.rs:23`) and the
  converter's value fields and drop-downs give a module everything it needs: captions, text
  boxes, unit pickers, radio buttons, result lines, a page drop-down for sub-tools (the date
  mode's `Popup::Pages` is exactly a "which calculation" picker). A module should **not**
  invent a keypad: on the desktop people type, and on Android the system keyboard is fine for
  a form (Graphing already handles `SoftKeyboard` for `TextEdit`s).
- **Input:** `egui::TextEdit` (the Graphing boxes' precedent, `src/app/graphing_view.rs:1448`)
  with `numbers.delocalize` on read and `numbers.localize` on write; a module never parses `.`
  or `,` itself.
- **Popups:** drawn in `draw_popups` over the app's backdrop with `popup_frame`/`list_item`
  from the kit, state in the module, `popup_open` true while open, Esc closes.
- **Accessibility:** every control through `access::control` with a name from the module's
  strings; every result `access::text(…, live: true)`; captions as `Label`s. Nothing else is
  needed for Narrator, TalkBack (once the AccessKit route lands) or the web screen reader.
- **Theme:** `host.palette` only.
- **Sizes:** must work from 320 × 500 (`MIN_SIZE`) and at 780 × 360; a long form scrolls in a
  `ScrollArea` with `access::scroll_source`.

### 2.7 Persistence

- Opening mode: keep `Mode` (Windows' integer) for the Windows modes, and add
  `CalcCraftModule=<ModuleId>` written when a module is open and deleted otherwise (or `""`).
  `new` reads `CalcCraftModule` first; if that module is compiled into this build it opens,
  else `Mode` applies (so a saved Pro state opens Standard in the free app, never crashes).
  No integer ids: `MODULES`' order changes with features.
- Module settings: `ModuleStorage` is a view over eframe's storage that prefixes keys with
  `CalcCraft.Module.<id>.` (`set_string`, `get_string`), so modules can't collide with each
  other or with Windows' keys. Modules store **preferences only** (last sub-tool, chosen units),
  never calculation history or user inputs, which keeps the privacy documents short: the
  refactor task updates `PRIVACY.md`, `docs/privacy-audit.md` and `docs/play-data-safety.md`
  once with the two new key families, and a module's own keys are listed when the module lands.
- No network, no files, no identifiers, in any module: the privacy policy promises it, and the
  web build can't anyway.

### 2.8 Strings

- A module ships `crates/calccraft-module-<id>/locales/<id>.txt` in `calccraft.txt`'s format
  (`[en-US]`, `[ru-RU]`, `[de-DE]` sections, `Module.<id>.<Key>=…`), `include_str!`ed and
  returned by `ModuleFactory::strings`. `Strings::new` (`src/app/strings.rs:101`) extends the
  language's table with every module's section and English with their English sections, so
  `tr` works unchanged and missing translations fall back to English, as the support strings do.
- Test in the api crate's helper, run by each module's tests: the English section exists and
  has every key the module's code uses (the module lists its keys in a `const KEYS: &[&str]`
  like `every_language_has_calccraft_strings` does), `%1` counts match across languages, and
  every section's keys are a subset of English's.
- Translation scope: English plus Russian and German at launch (Konst's languages and the
  long-worded test language), the rest falling back. A module's screen-reader names are
  strings too.
- Unit names reuse Windows' `UnitName_*` keys through the converter re-export: already in 61
  languages.

### 2.9 History, memory and the engines

- Modules get no entry in the Windows history list and don't touch the shared memory in v1;
  nothing in Windows corresponds, and the memory's hand-over in `set_mode` is tied to
  `is_calculator()`. A module keeps its own "recent results" list in its state if it wants one
  (a `Vec<String>` drawn with the kit's list items).
- Later, cheaply: `Host::memory_store(value)` to push a module's result into the shared memory
  (`engine_mut().set_memory`), and `Host::copy(text)` for `Ctrl+C`. Not needed for a first
  module.
- A module never drives a `Calculator` (the expression-line/tape machinery is Windows'
  semantics). It computes in `f64` with the converter's rounding, or in exact rationals through
  the re-exported `Number` where it matters (stoichiometric coefficients).

### 2.10 Tests

- Pure logic: unit tests in the module crate (molar mass of `Ca(OH)2`, pH of 0.01 M HCl, …),
  checked against published values, as the converter's are against Windows'.
- UI: the module crate can't start `CalculatorApp`, but it can run frames of its own `draw`
  with `egui::Context::run_ui` and a test `Kit`/`Strings` from the api crate's `test_support`
  feature, and read the AccessKit tree (`ctx.enable_accesskit()`, as `layout_tests` does) to
  assert names and focus order.
- Integration: the root crate's `layout_tests`, `opens_in_the_mode_it_was_closed_in`,
  `modes_are_saved_with_windows_ids` iterate `MODULES`, so `cargo test --features pro` covers
  the Pro build and plain `cargo test` the free one. CI runs both.
- Manual: the "Verifying UI changes" checklist in AGENTS.md gets a module paragraph (both
  themes, de-DE, 320 px, 780 × 360, keyboard-only, screen reader).

### 2.11 Size and platform constraints

- Binary: a module is code plus small tables. Estimate per module 100–400 KB of `.exe`
  (optimized), 50–200 KB of `.wasm`, against 23 MB / 11.2 MB today; well under what any
  profile tuning would save (`[profile.release] lto = "thin", codegen-units = 1, strip = true`
  is a separate, worthwhile task: likely several MB).
- Data tables are `const` arrays in code: 118 elements × (symbol, name key, atomic weight, a
  flag) is about 4 KB. No JSON parsing at runtime, no `serde`.
- Dependencies: none beyond the api crate for chemistry, biology, electronics, finance, health,
  cooking, physics, statistics, aviation, petroleum (all closed-form or small series:
  the statistics module implements the regularized incomplete beta/gamma itself, some 150
  lines, instead of pulling `statrs`). Time zones are the exception (`chrono-tz`, MIT OR
  Apache-2.0, with `filter-by-regex` to drop zones; still a few hundred KB of tzdata).
- Web/Android: modules use `web_time` for clocks, no `std::fs`/`env`, no threads, no text in
  scripts egui can't shape; nothing else differs. The Android back button, safe area and
  landscape all come from the host.
- Fonts: chemistry wants subscripts (`H₂O`) and `→`; Segoe UI/Inter have the subscript digits
  and arrows, Noto Sans Math is loaded everywhere for `⇌` and the like. No new font.

### 2.12 Effort

| Work | Estimate (agent-days) |
| --- | --- |
| Workspace + `calccraft-module-api` (move palette/access/locale, `Kit` and `Strings` traits, `Host`, `CalcModule`) | 2–3 |
| Registry, `Mode::Module`, navigation group, dispatch, persistence (`CalcCraftModule`, `ModuleStorage`), privacy docs | 1.5–2 |
| Strings merge + tests, layout/mode tests over `MODULES`, a tiny `module-demo` behind a test-only feature to exercise the path | 1 |
| Features `module-*`/`pro`, `play-paid` wiring, `build-apk.ps1` marker check, CI `--workspace` and `--features pro` lines, AGENTS.md | 0.5–1 |
| **Refactor total** | **5–7** |
| Chemistry module: formula parser + element table + molar mass/composition, equation balancer (rational nullspace), dilution, pH, gas laws, with tests | 3–4 |
| Chemistry UI: sub-tool picker, forms, results, popups, a11y, 320 px and landscape, en/ru/de strings, screenshots | 4–6 |
| Review, manual checks on Windows/web/Android, docs | 1–2 |
| **First module total** | **8–12** |

Agent-days are Mission Control task-days with review; calendar time depends on how many run
in parallel (the module's logic and UI tasks can overlap once the api crate exists).

---

## 3. Module ideas

Complexity: S = a few screens of closed-form math (≤ 1 week), M = a parser or a data table
and 4–6 screens (1–2 weeks), L = numerical methods, large data, or liability work (> 2
weeks). "Free/Pro" is a suggestion for the Play editions; the desktop and web builds follow
Konst's decision in §4.4.

### 3.1 Chemistry (required)

- **For:** school and university chemistry students, lab technicians, teachers.
- **Functions:** (1) molar mass from a formula with parentheses, hydrates and charges
  (`Ca(OH)2`, `CuSO4·5H2O`, `SO4^2-`) with percent composition; (2) stoichiometry: balance an
  equation (`Fe + O2 → Fe2O3`) and convert grams ↔ moles ↔ particles ↔ litres at STP,
  limiting reagent and yield; (3) dilution `C1V1 = C2V2` and molarity/molality/mass
  percent conversions; (4) pH/pOH of strong and weak acids and bases (`Ka`/`Kb`, quadratic),
  buffers (Henderson–Hasselbalch); (5) gas laws: ideal `PV = nRT` solved for any variable,
  combined and Dalton's law, with the converter's pressure and volume units.
- **Data:** 118 standard atomic weights (abridged, five significant digits) from IUPAC's
  CIAAW table ([ciaaw.org](https://ciaaw.org/abridged-atomic-weights.htm); the 2021 values with
  the 2024 Gd, Lu, Zr revisions). Measured values are facts and not copyrightable; type them
  into a `const` table with the source cited in a comment, don't copy a database file.
  Avoid Bowserinator's Periodic-Table-JSON: its data is reported as CC BY-SA 3.0 (share-alike,
  not on `deny.toml`'s list). `R` and other constants from CODATA 2018 (NIST, US government
  work). Element names: 118 new strings (`Module.chemistry.Element.Fe`), English + ru + de.
- **Complexity:** M (the formula parser and the balancer are the only real logic; the rest is
  algebra).
- **Fit:** Pro.
- **Risks:** low. Accuracy depends on the table and on stating the convention (abridged
  weights, STP as 0 °C/1 bar vs 1 atm: let the user choose, default IUPAC 1 bar and show it).
  Balancing can have no or infinite solutions: show "can't balance" rather than guess.

### 3.2 Biology and lab (required)

- **For:** molecular biology students, wet-lab researchers, lab technicians.
- **Functions:** (1) dilutions: `C1V1 = C2V2`, serial dilution tables, stock-to-working
  volumes, percent/molar/mass-per-volume conversions; (2) cell density from a hemocytometer
  count (cells/mL = count ÷ squares × dilution × 10⁴), cells to seed per well, doubling time;
  (3) PCR/qPCR helpers: master-mix volumes for n reactions with overage, primer Tm (Wallace and
  salt-adjusted rules; nearest-neighbour as a later step), primer efficiency from a standard
  curve slope (E = 10^(−1/slope) − 1), ΔΔCt relative expression; (4) nucleic acid and protein
  molarity: ng/µL ↔ nM from length (dsDNA ≈ 650 g/mol per bp, ssDNA 330, RNA 340, protein ≈
  110 Da per residue), A260 to concentration (50/40/33 µg/mL per unit), copies from mass
  (Avogadro); (5) Hardy–Weinberg: allele and genotype frequencies from counts, χ² against
  equilibrium.
- **Data:** constants only (Avogadro, average residue masses, A260 factors): textbook values,
  cite a source in comments. No tables with licences.
- **Complexity:** S–M (all closed-form; the χ² p-value needs the incomplete gamma, shared with
  the statistics module).
- **Fit:** Pro. Shares its dilution/molarity core with chemistry: build a small `lab` crate
  both depend on, or make biology a second page set of one "Lab" module.
- **Risks:** low; the results are planning aids, not clinical. State the approximations (the
  650 g/mol/bp average) on screen so a user with exact sequence data knows the error.

### 3.3 Electronics

- **For:** hobbyists, electronics and electrical engineering students, makers.
- **Functions:** Ohm's law and power (any two of V, I, R, P); resistor colour code both ways
  (4, 5 and 6 bands, tolerance and temperature coefficient, nearest E12/E24/E96 value);
  series/parallel resistors and capacitors; RC/RL time constants and RC/LC cut-off and
  resonant frequencies; voltage divider and LED series resistor; SI prefixes.
- **Data:** IEC 60062 colour code and E-series tables (standard facts). Colours are the one
  place a module needs colors outside `Palette`: define them as module constants documented
  as the band colours, not theme colors (or add a `Palette::band(...)` list).
- **Complexity:** S–M. **Fit:** free (a strong, popular feature that shows what modules
  are) or Pro. **Risks:** none beyond colour-blind users: always print the value next to
  the bands and name the colours for screen readers.

### 3.4 Finance

- **For:** anyone with a loan or savings; small-business owners.
- **Functions:** loan/mortgage payment with an amortization schedule; compound interest and
  future/present value; savings goal (monthly amount to reach a sum); APR ↔ APY; tip and
  discount/VAT; simple currency-free ROI/CAGR.
- **Data:** none (no exchange rates: that would need the network, which the privacy policy
  rules out; Currency stays absent as in CalcCraft today).
- **Complexity:** S (the schedule needs a scrolling table). **Fit:** free.
- **Risks:** rounding conventions differ by country (bank rounding of payments): state
  "estimate" and round per period, like the common online calculators.

### 3.5 Health and fitness

- **For:** general public, fitness enthusiasts.
- **Functions:** BMI with WHO categories; BMR/TDEE (Mifflin–St Jeor, Harris–Benedict) and
  macro split; body-fat estimate (US Navy method); target heart-rate zones; pace/speed/time
  for running; hydration.
- **Data:** published formulas and WHO cut-offs (facts).
- **Complexity:** S. **Fit:** free, or left out.
- **Risks:** **liability and store policy.** Anything that looks like medical advice (drug
  dosing, insulin, paediatric doses, pregnancy dating) must stay out: a wrong dose is harm, and
  Google Play's Health apps policy and Apple's rules treat such apps as health/medical with
  extra requirements. BMI/BMR calculators are low-risk if they carry a "not medical advice"
  line and never suggest a diet. Recommend: no dosing, ever, in CalcCraft.

### 3.6 Cooking

- **For:** home cooks, bakers.
- **Functions:** volume ↔ weight by ingredient (flour, sugar, butter, …) with density; cups,
  tablespoons, ml, g, oz; oven temperature ↔ with fan adjustment; recipe scaling (servings,
  pan size by area); baker's percentages.
- **Data:** ingredient densities: USDA FoodData Central (US government, public domain) or a
  small hand-typed table of 30–50 common ingredients with a cited source.
- **Complexity:** S. **Fit:** free (broad appeal). **Risks:** low; densities vary (sifted
  vs packed flour): show the assumption.

### 3.7 Physics (kinematics and mechanics)

- **For:** high-school and first-year physics students.
- **Functions:** SUVAT equations solved for any unknown with units; projectile motion (range,
  height, time); Newton's second law, friction, inclined plane; work/energy/power;
  circular motion; simple pendulum and springs.
- **Data:** constants (g, G) from CODATA. **Complexity:** S–M (the "solve for any unknown"
  form is the pattern chemistry's gas laws need too: build it once in the kit).
- **Fit:** Pro (students) or free. **Risks:** none.

### 3.8 Statistics

- **For:** students, researchers, QA engineers.
- **Functions:** descriptive statistics of a pasted list (mean, median, mode, SD, quartiles);
  normal/t/χ²/binomial/Poisson distributions (PDF, CDF, inverse); one- and two-sample t-tests
  and χ² with p-values; linear regression and correlation; confidence intervals; sample size.
- **Data:** none; implement the regularized incomplete beta and gamma functions (Numerical
  Recipes-style continued fractions, written fresh: don't copy NR's code, which isn't free) and
  the inverse normal (Acklam's algorithm, public). No `statrs` (MIT, but large and generic).
- **Complexity:** M (numerics need careful tests against published tables). **Fit:** Pro.
- **Risks:** accuracy in the tails; test against R/SciPy values to 1e-10.

### 3.9 Date and time zones

- **For:** people who schedule across time zones, travellers.
- **Functions:** convert a time between zones with DST; meeting planner (several zones in a
  row); countdown and duration between two date-times; Unix timestamp ↔ date; week numbers
  (ISO), day of year.
- **Data:** the IANA time zone database through `chrono-tz` (MIT OR Apache-2.0; tzdata itself is
  public domain), with `filter-by-regex` to limit zones if size matters; tzdata changes a few
  times a year, so each release updates the crate.
- **Complexity:** S–M (UI for picking zones: a searchable list of ~400 names). **Fit:** free.
- **Risks:** stale tzdata between releases; show the database version in the mode.

### 3.10 Aviation: an E6B in the pocket

- **For:** student and private pilots (Konst is training for a PPL), flight instructors.
- **Functions:** wind triangle (wind correction angle, ground speed, heading from course,
  TAS and wind); pressure altitude and density altitude (ISA, from altimeter setting and
  temperature); true airspeed from indicated/calibrated airspeed and altitude; fuel burn,
  endurance and range, time–speed–distance; weight and balance (moments, CG, envelope check
  per aircraft entered by the user); unit conversions (nm/sm/km, kt/mph, °C/°F, inHg/hPa,
  lb/kg, USG/L); crosswind and headwind components; descent planning (rate for a glide slope,
  top of descent); magnetic variation applied to true/magnetic/compass.
- **Data:** ISA constants (ICAO Doc 7488, published), the FAA's Pilot's Handbook of
  Aeronautical Knowledge and *Density Altitude* pamphlet (FAA-P-8740-2) are US government
  works (public domain); no proprietary charts. No performance tables of specific aircraft:
  the user types their POH numbers.
- **Complexity:** M. **Fit:** Pro (a niche with a strong willingness to pay: E6B apps are
  paid on both stores) and a personal fit for Konst, who can verify every result against his
  training.
- **Risks:** moderate. Flight planning errors matter; results must match the mechanical E6B
  and the FAA formulas to the displayed digits, carry a "for training and planning; verify
  with approved sources" line, and never claim to replace the POH. No live weather (no
  network), which keeps it out of EFB territory.

### 3.11 Oil and gas: reservoir and PVT basics

- **For:** petroleum engineering students and engineers (Konst's background).
- **Functions:** gas properties: pseudocritical properties from gas gravity (Sutton), Z-factor
  (Dranchuk–Abou-Kassem or Hall–Yarborough), gas FVF `Bg`, gas density and viscosity (Lee,
  Gonzalez, Eakin); oil correlations: bubble-point pressure and solution GOR (Standing,
  Vazquez–Beggs), oil FVF `Bo`, dead/live oil viscosity (Beggs–Robinson); formation water
  properties; volumetric OOIP/OGIP with recovery factor; material balance basics (p/Z plot
  for a dry gas: input pairs, fit, OGIP); unit conversions (psi/bar/kPa, bbl/m³, scf/m³, cp,
  md, API gravity ↔ specific gravity).
- **Data:** published correlations (formulas and coefficients from the original papers and
  textbooks: facts). Cite each correlation and its validity range on screen.
- **Complexity:** M–L (the Z-factor correlations are iterative; many correlations with ranges).
- **Fit:** Pro; small audience but almost no competition on Android, and Konst can review it.
- **Risks:** correlations have validity ranges; show them and warn outside. Engineering
  decisions aren't made on a phone calculator, so liability is low, but accuracy to the
  published coefficients must be tested against the papers' examples.

### 3.12 Fractions and ratios (a small free one)

- **For:** school pupils, carpenters, cooks.
- **Functions:** fraction arithmetic with exact results (the engine's `BigRational` already
  does this), mixed numbers, simplification, ratio and proportion, percent change,
  inch-fraction ↔ decimal ↔ mm.
- **Data:** none. **Complexity:** S (mostly UI over `Number`). **Fit:** free; a cheap
  showcase of the module path and of CalcCraft's exact arithmetic. **Risks:** none.

### Summary table

| Module | Complexity | Edition | Data licence | Main risk |
| --- | --- | --- | --- | --- |
| Chemistry | M | Pro | IUPAC/CIAAW values (facts), CODATA | STP convention; unbalanceable equations |
| Biology / lab | S–M | Pro | constants | approximations shown |
| Electronics | S–M | free or Pro | IEC/E-series facts | colour names for a11y |
| Finance | S | free | none | rounding conventions |
| Health / fitness | S | free or out | WHO/formulas | **liability**: no dosing |
| Cooking | S | free | USDA (public domain) | density assumptions |
| Physics | S–M | Pro or free | CODATA | none |
| Statistics | M | Pro | none (own numerics) | tail accuracy |
| Time zones | S–M | free | chrono-tz (MIT/Apache), tzdata (PD) | stale tzdata |
| Aviation (E6B) | M | Pro | ISA/FAA (PD) | planning errors; disclaimer |
| Oil & gas PVT | M–L | Pro | published correlations | validity ranges |
| Fractions | S | free | none | none |

---

## 4. Recommendation and plan

### 4.1 Build first: Chemistry, then Biology/lab

1. **Chemistry** is the one Konst named, the broadest paying audience among the Pro candidates
   (every chemistry student needs molar masses and dilutions weekly), its data problem is
   solved with one 118-row table of facts, and it exercises every part of the module boundary:
   a parser, a table, unit reuse from the converter, exact rationals for balancing, forms,
   a sub-tool picker, popups and 118 translatable names. If the boundary carries chemistry, it
   carries anything.
2. **Biology/lab** second: it shares the dilution/molarity core, so its logic is half done
   after chemistry, and it doubles the Pro audience (life-science labs) for a few days of work.
   Shipping both makes "CalcCraft Pro" a *science edition*, a clearer story than one module.

**Aviation** is the best third candidate: a paid niche, a personal fit and a verification
source (Konst's training). The free modules (finance, cooking, time zones, fractions) are good
later additions to the free app, which also keeps the module path exercised in the free build.

### 4.2 Why not skins as the Pro feature

Skins (alternative palettes) are cheap, but they fight the project's specification: "the
real Windows Calculator is the specification; visual style should match it" (AGENTS.md). A
module adds capability without changing the look of the modes people know, and a module is
visible in the menu, where a paid feature should be. Skins can still come later as a Pro extra.

### 4.3 Step-by-step plan (Mission Control tasks)

Refactor first, with no module in the free build; then the module.

| # | Task | Depends on | Size | Deliverable / acceptance |
| --- | --- | --- | --- | --- |
| R0 | **Spec:** `docs/modules.md` from §2 of this document: trait, host, registry, strings, persistence, channel rules, test rules; AGENTS.md architecture entries | — | S | reviewed spec, decisions of §4.4 recorded |
| R1 | **Workspace and api crate:** `[workspace]`, `crates/calccraft-module-api` with `Palette`, `access`, `locale::NumberFormat` moved; `Kit` and `Strings` traits implemented by the app; everything still compiles for Windows, Linux, macOS, wasm32, Android; CI `--workspace` | R0 | M | green CI; no behaviour change; `cargo deny` green |
| R2 | **Registry and `Mode::Module`:** `src/app/modules.rs`, the navigation's third group (strings `CalcCraft.ModulesCaps`/`ModulesPlural` in every drawable language), dispatch in `draw_frame`, `popup_covers`, `set_mode`, Android back; a `module-demo` crate (one form, one popup) behind a feature used only by tests | R1 | M | with `--features module-demo` the demo appears and works with keyboard and screen reader; without it nothing changes |
| R3 | **Persistence and strings merge:** `CalcCraftModule`, `ModuleStorage`, module `[language]` tables merged into `Strings::new`, tests over `MODULES` in `app.rs` and `layout_tests`; `PRIVACY.md`, `docs/privacy-audit.md`, `docs/play-data-safety.md` updated | R2 | S | round-trip test; demo saved and restored; a saved module absent from the build opens Standard |
| R4 | **Channels:** features `module-*`, `pro`, `play-paid = [..., "pro"]`; `build-apk.ps1` marker check that the free library has no module; CI jobs for `--features pro` on desktop and Android; `android/README.md` and `docs/support-prompt.md` §8 rows | R2 | S | CI builds free and Pro bundles; the free library lacks the marker |
| C1 | **Chemistry core:** `crates/calccraft-module-chemistry` logic: element table (CIAAW 2021, cited), formula parser (parentheses, hydrates, charges, Unicode subscripts), molar mass and composition, equation balancer on `Number`, dilution and concentration conversions, pH/pOH/buffer, ideal and combined gas laws with the converter's units; tests against textbook values | R1 | M | `cargo test -p calccraft-module-chemistry` green; documented conventions (STP) |
| C2 | **Chemistry UI:** sub-tool picker (date mode's page list pattern), five forms with results, popups, a11y names, 320 × 500 and 780 × 360 layouts, Android keyboard behaviour, en/ru/de strings; `docs/chemistry.png` | R2, R3, C1 | L | manual checklist on Windows, web and Android; layout tests; screen reader pass |
| C3 | **Pro release prep:** `pro` on in `play-paid`, Play listing text for the Pro app's "what's different", version bump, test bundle through the release workflow (`workflow_dispatch`), a Play internal-testing upload by Konst | R4, C2 | S | signed `CalcCraft-Pro-<version>-arm64.aab` artifact |
| B1 | **Biology/lab core** (shares a `lab` helper with C1) and **B2 UI**, as C1/C2 | C1, C2 | M + M | second Pro module |

R1–R4 are the refactor (5–7 agent-days); C1–C3 the first module (8–12). R2 and C1 can run in
parallel once R1 is merged; C2 waits for R2 and R3.

### 4.4 Decisions for Konst (reversible; defaults in bold)

1. **Do the desktop and web builds get the Pro modules?** Options: **(a)** Pro-only everywhere
   (the GitHub release and the web app stay the free app; a `pro` desktop build could follow as
   a paid download later), (b) free on desktop and web, Pro only on Play (consistent with the
   donate model: the desktop is already "free, please donate"). Technically one feature line
   either way. Recommend (a) at launch so "Pro" means one thing in the listing.
2. **Menu group wording:** **"Modules"** (neutral, CalcCraft's own string), or "Pro", or a
   per-domain grouping ("Science"). Recommend "Modules" now; the brand can go into the Settings
   About card ("CalcCraft Pro").
3. **Translation scope at launch:** **English, Russian, German**, the rest falling back to
   English (as the support strings do), or all 51 drawable languages by machine translation
   reviewed later. Recommend the first.
4. **Shared memory and `Ctrl+C` from a module (§2.9):** **not in v1**; add `Host::memory_store`
   when a module needs it.
5. **Health module:** **leave out**, or BMI/BMR only with a disclaimer; never dosing.
6. **Icon:** glyphs from Fluent System Icons through `tools/icon-font.py` (one line per module,
   consistent with the menu) rather than painted icons. Needs `uv` on the machine that
   rebuilds the font; otherwise the painted fallback.

### 4.5 What this does not change

- No Windows behaviour pinned in AGENTS.md changes: the six Windows modes, their ids,
  shortcuts, history and memory stay as they are; the module group appears only in builds that
  have a module.
- No new third-party dependency for the refactor or for chemistry and biology (the api crate
  depends on `eframe`'s egui; modules depend on the api crate). `cargo deny` stays green by
  construction.
- No network, no files, no telemetry in any module: the privacy policy keeps its promises.

## Sources consulted

- CalcCraft at 0517cd1: `src/app.rs`, `src/app/access.rs`, `src/app/strings.rs`,
  `src/app/date_view.rs`, `src/app/converter_view.rs`, `src/app/layout_tests.rs`,
  `src/converter.rs`, `src/engine.rs`, `src/locale.rs`, `src/lib.rs`, `src/support/channel.rs`,
  `Cargo.toml`, `deny.toml`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`,
  `android/build-apk.ps1`, `android/README.md`, `docs/support-prompt.md` §8,
  `docs/web-version-plan.md`, `locales/`, `assets/fonts/`.
- Google Play, Device and Network Abuse policy (no executable code from outside Play):
  <https://support.google.com/googleplay/android-developer/answer/9888379>.
- IUPAC CIAAW, abridged standard atomic weights (2021, with 2024 revisions):
  <https://ciaaw.org/abridged-atomic-weights.htm>.
- `chrono-tz` (MIT OR Apache-2.0, `filter-by-regex`): <https://docs.rs/crate/chrono-tz>.
- Periodic-Table-JSON data reported as CC BY-SA 3.0 (share-alike; not used):
  <https://github.com/Bowserinator/Periodic-Table-JSON>.
- FAA, *Pilot's Handbook of Aeronautical Knowledge* (FAA-H-8083-25) and *Density Altitude*
  (FAA-P-8740-2): US government works, public domain.
