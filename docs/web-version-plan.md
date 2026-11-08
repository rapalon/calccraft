# CalcCraft on the web: plan

Branch: `feature/web-version` · Written: 2026-10-17 · Starting point: `main` at `83f3d5d` (v1.1.0)

## Status (2026-10-17)

The first version is built on this branch and runs in the browser. It was tested locally in
Chromium (`trunk build --release`, served from `dist/`): all modes, the docked and phone layouts,
settings kept across reloads, the German language and region through `?lang=de-DE&region=de-DE`,
the date picker, the function analysis, and F5 in the Programmer mode (HEX, no reload).

| Phase | State |
|---|---|
| 1. Compile for wasm32 | Done: dependencies per target, `web_time` for `rand`'s seed, chrono's `wasmbind`, the clipboard behind `cfg`. Clippy is clean for wasm32 and native. |
| 2. Entry point and page | Done: `main.rs`, `index.html`, `Trunk.toml`, icons from `src/icon.rs` (`assets/web/`), loading message, CSS minimum of 320 × 500. |
| 3. Browser features | Mostly done (`src/app/web.rs`): language and formats from the browser and `Intl`, `?lang=` and `?region=`, Paste through the async clipboard, Share downloads `graph.png`, keep-on-top hidden, browser shortcuts blocked. The mathematical symbols come from an embedded Noto Sans Math (OFL). **Open:** CJK, Thai and Amharic are left out; touch and screen readers aren't tested on real phones yet. |
| 4. Size and speed | Measured: 11.2 MB `.wasm`, **3.9 MB gzipped** (Noto Sans Math adds 0.4 MB), within the target. No web-only profile yet. |
| 5. CI | Done: the `web` job in `ci.yml` (clippy for wasm32 and `trunk build --release`). |
| 6. Pages | Done (`pages.yml`): deployed after every successful release, from `main`. |
| 7.–9. | Docs updated (README, AGENTS.md); release asset, wasm tests and Playwright not started. |
| 10. Installable and offline | Done: app manifest, maskable icons, a service worker (page from the network first, the app's hashed files kept), safe areas for notches. Tested locally: installable manifest, offline start, old build files cleared after an update. |

## Goal

CalcCraft runs in the browser, compiled to WebAssembly from the same Rust code as the desktop
app. GitHub Actions builds it and deploys it to GitHub Pages automatically, at
**https://rapalon.github.io/calccraft/**.

Done means:

1. All modes work in current Chrome, Edge, Firefox and Safari, on desktop and on phones:
   Standard, Scientific, Graphing, Programmer, Date calculation and the converters.
2. Settings (theme, mode, language, region, memory, converter units) survive a reload.
3. A pushed release (or a manual run) deploys the site without anyone touching it.
4. Every pull request checks that the web build still compiles, so it can't silently break.
5. The desktop builds and the existing CI and release workflows keep working unchanged.

Not part of the first version:

- Installing it as an app (PWA, offline use). That's an optional last phase.
- A web-only feature set. The web version is the same app.
- Currency, which the desktop app doesn't have either.

## 1. What the code needs today

eframe (0.36.2) already runs in the browser: it draws egui on a `<canvas>` and keeps
`persistence` in `localStorage`. Most of the app is plain Rust with no platform code: the
engine, graphing, dates and converters. These are the places that will break or behave
differently in the browser:

| # | Where | Problem in the browser | Fix |
|---|---|---|---|
| 1 | `Cargo.toml`: `arboard` | No support for `wasm32-unknown-unknown`, so the build fails. | Move it to a `cfg(not(target_arch = "wasm32"))` dependency table (step 1.2). |
| 2 | `src/app/graphing_view.rs` `clipboard_text()` (Graphing's Paste menu item) | Uses `arboard`. | Use the browser's async Clipboard API on the web (step 3.4). |
| 3 | `src/engine/math.rs` `random()` (the `rand` key) | `std::time::SystemTime::now()` **panics** on `wasm32-unknown-unknown`. | Take the seed from `web_time::SystemTime`, or from `js_sys::Math::random()` (step 1.4). |
| 4 | `src/app.rs` and `src/app/date_view.rs`: `chrono::Local::now()` (the date mode's today) | Panics without chrono's `wasmbind` feature. | Turn on `wasmbind` for the wasm target, which reads JavaScript's `Date` (step 1.3). |
| 5 | `src/app.rs` `setup_fonts`: `std::fs::read` from `%WINDIR%\Fonts` | Returns an error (no panic), so the app falls back to Inter. Segoe UI Symbol (`⌊ ⌋ ⌈ ⌉ ∛`, the converters' comparison pictures) and the CJK, Thai and Amharic fonts are missing. | Embed a small symbols font, and load or hide the script fonts (step 3.3). |
| 6 | `src/main.rs` | Only the native `eframe::run_native` entry point. | Add a wasm `main` that starts `eframe::WebRunner` on a canvas (step 2.1). |
| 7 | `src/app/strings.rs` `preferred_languages()` and `src/app/region.rs` | The system language and formats are read only on Windows, so the web version would always be English and en-US. | Read `navigator.languages` and `Intl` (step 3.2). |
| 8 | `src/app/converter_view.rs` `user_region()` | Windows only, so the default units would always be US. | Use the region of `navigator.language` (step 3.2). |
| 9 | `src/app/share.rs` | Returns `false` off Windows, so Graphing's Share button always shows an error. | Web Share API or a PNG download (step 3.5). |
| 10 | Keep-on-top and window size commands (`ViewportCommand`) | Do nothing in a browser tab. | Hide the keep-on-top button on the web (step 3.6). |
| 11 | Keyboard shortcuts | The browser owns some of them (see step 3.7): Ctrl/Cmd+R and F5 reload the page, Ctrl+L, Ctrl+E, Ctrl+H, F3, F6, F7 and F12 open browser features, and Alt+1–5 switch tabs on some systems. eframe never blocks F5 and Ctrl/Cmd+R. | Block what can be blocked while the canvas has focus, and document the rest (step 3.7). |
| 12 | `MIN_SIZE` (320 × 500) | A browser tab or phone can be smaller. Layout code assumes the minimum, and `f32::clamp` panics when min > max (see AGENTS.md pitfalls). | Give the canvas a CSS minimum of 320 × 500 and scroll the page when the window is smaller (step 2.3). |
| 13 | `CALCCRAFT_LANGUAGE` and `CALCCRAFT_REGION` (environment variables) | A browser has no environment variables. | Read `?lang=de-DE&region=de-DE` from the URL instead (step 3.2). |
| 14 | Tests that use `std::thread` (`src/graphing/parse.rs`) | Not run in the browser. | Nothing to do: the tests keep running natively. |

Already fine: `build.rs` only embeds the icon when the target OS is Windows, `std::env::var`
just returns an error, and `TitleBar` and `share::show` already have
`#[cfg(not(windows))]` stubs. The `windows` and `windows-sys` crates are Windows-only
dependencies, so they don't take part.

## 2. Decisions to make first

| Decision | Options | Recommendation |
|---|---|---|
| Build tool | **Trunk** (eframe's official template: bundles the wasm, runs `wasm-bindgen` and `wasm-opt`, adds hashed file names), or `wasm-pack` plus a hand-written loader | **Trunk.** It's the standard for eframe, one command, and it downloads the matching `wasm-bindgen` and `wasm-opt` itself. |
| Renderer | eframe 0.36's default is **wgpu** (WebGPU with a WebGL fallback); **glow** is WebGL 2 | **glow on the web.** Every current browser supports it, the download is smaller, and the app's custom painting needs nothing more. Desktop keeps its current renderer. |
| When to deploy | (a) on every `v*` tag, together with the desktop release; (b) on every push to `main` | **(a), plus a manual "Run workflow" button.** The site then always matches the latest release. Pull requests and `main` still build the web version, without deploying it. |
| Address | `rapalon.github.io/calccraft/` or a custom domain | Start with the GitHub address; a custom domain is only a `CNAME` away later. |
| Non-Latin languages (zh, ja, ko, th, am …) | Load their fonts from the site when that language is picked; or leave those languages out of the web build | **Load on demand.** Host font subsets next to the app and fetch them the first time they're needed. As a first step, they can be left out of the web language list. |
| Native code layout | One crate with `cfg(target_arch = "wasm32")` blocks, or a separate `web` crate | **One crate**, the way eframe's template does it. Platform code is already split with `cfg(windows)`. |

## 3. Steps

Each phase can be one pull request into `feature/web-version`. Merge that branch into `main`
when phases 1–6 are done and the site works.

### Phase 1: make the crate compile for the browser

1.1. Install the target and check it:

```bash
rustup target add wasm32-unknown-unknown
```

```bash
cargo check --target wasm32-unknown-unknown
```

The compile errors are the to-do list for this phase.

1.2. Split the dependencies in `Cargo.toml` by target. A dependency listed both in
`[dependencies]` and a target table gets the union of both feature lists, so eframe must move
out of `[dependencies]` into two target tables:

```toml
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
arboard = { version = "3.6", default-features = false }
eframe = { version = "0.36.2", features = ["persistence"] }

[target.'cfg(target_arch = "wasm32")'.dependencies]
eframe = { version = "0.36.2", default-features = false, features = [
    "default_fonts", "glow", "persistence", "web_screen_reader",
] }
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
js-sys = "0.3"
web-sys = { version = "0.3", features = [
    "Window", "Document", "Element", "HtmlCanvasElement", "Navigator", "Location",
    "Clipboard", "Blob", "BlobPropertyBag", "Url", "HtmlAnchorElement", "ShareData",
] }
log = "0.4"
```

Keep the `wasm-bindgen` version the one eframe itself uses; Trunk downloads the matching CLI.
Depending on the `web-sys` version, the Clipboard and Web Share bindings may still need the
`--cfg=web_sys_unstable_apis` flag (in `.cargo/config.toml` for the wasm target); check this
when adding them.

1.3. chrono: add the `wasmbind` feature for the wasm target only, so `Local::now()` reads
JavaScript's `Date` and the local time zone:

```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
chrono = { version = "0.4.45", default-features = false, features = ["clock", "std", "wasmbind"] }
```

1.4. `engine::math::random()`: seed SplitMix64 from `web_time::SystemTime` (the `web-time`
crate, which egui already depends on, works natively and in the browser), or from
`js_sys::Math::random()` under `cfg(target_arch = "wasm32")`. Add a test that `random()` still
gives values in [0, 1).

1.5. Put the clipboard read behind `cfg(not(target_arch = "wasm32"))`. Give the web a stub
for now; step 3.4 makes it work.

1.6. Lint the wasm build the way CI will, and fix the dead-code warnings that native-only
code now raises. Follow the existing `cfg_attr(…, allow(dead_code))` convention in AGENTS.md.

```bash
cargo clippy --target wasm32-unknown-unknown -- -D warnings
```

1.7. Check that nothing changed on the desktop:

```bash
cargo fmt --check
```

```bash
cargo clippy --all-targets -- -D warnings
```

```bash
cargo test
```

**Done when:** `cargo build --release --target wasm32-unknown-unknown` succeeds, and the
native checks are green on Windows, Linux and macOS.

### Phase 2: the web entry point and page

2.1. `src/main.rs`: keep today's `main` under `#[cfg(not(target_arch = "wasm32"))]` and add
a web one:

```rust
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    eframe::WebLogger::init(log::LevelFilter::Warn).ok();
    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window().expect("a window").document().expect("a document");
        let canvas = document
            .get_element_by_id("calccraft")
            .expect("the canvas in index.html")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("a canvas");
        let result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(app::CalculatorApp::new(cc)))),
            )
            .await;
        // Replace the loading text with the app, or with the error.
        if let Some(loading) = document.get_element_by_id("loading") {
            match result {
                Ok(()) => loading.remove(),
                Err(error) => loading.set_inner_html(&format!("CalcCraft couldn't start: {error:?}")),
            }
        }
    });
}
```

`WebOptions` is also where step 3.7's `should_prevent_default` hook goes.

2.2. Add `Trunk.toml` (the output folder, `dist`, and the default public URL) and add `dist/`
to `.gitignore`.

2.3. Add `index.html` in the repository root. It holds:

- `<link data-trunk rel="rust" data-wasm-opt="z" data-bin="calculator" />`.
- A full-page `<canvas id="calccraft">`, with a CSS `min-width: 320px; min-height: 500px`
  (Windows' minimum) and page scrolling below that, so layout code never sees a smaller window.
- A `<div id="loading">` with a "Loading CalcCraft…" message for the first download.
- `<title>CalcCraft</title>`, a description, `theme-color` for light and dark, and the
  viewport meta tag `width=device-width, initial-scale=1, user-scalable=no`, so a phone doesn't
  zoom the canvas.
- The favicon and app icons.

2.4. Icons. `src/icon.rs` draws the icon with std only. Add a small tool (or an ignored test)
that writes `assets/web/icon-32.png`, `icon-192.png` and `icon-512.png` from it, and commit
the PNGs. Trunk copies them with `<link data-trunk rel="copy-file" …>`.

2.5. Run it locally:

```bash
cargo install trunk --locked
```

```bash
trunk serve --open
```

**Done when:** every mode opens and works in a desktop browser at `http://127.0.0.1:8080`.

### Phase 3: platform features in the browser

3.1. **Settings that persist.** eframe stores `persistence` data in `localStorage` under the
page's origin. Check theme, mode, language, region, memory, history and converter units across
reloads. Every project page of `rapalon.github.io` shares that origin and its
`localStorage`, so make sure the keys can't collide with another egui app there; eframe's keys
aren't prefixed per app.

3.2. **Language and region.** Under `cfg(target_arch = "wasm32")`:

- `strings::preferred_languages()` reads `navigator.languages`, which is already a list of BCP-47
  tags like `de-DE`.
- `region.rs` builds `Formats` from `Intl`: decimal and group separators from
  `Intl.NumberFormat().formatToParts(1234567.5)`; date patterns and month and day names from
  `Intl.DateTimeFormat`; the first day of the week from `Intl.Locale.prototype.weekInfo` where
  it exists, otherwise from a small table.
- `converter_view::user_region()` uses the region part of `navigator.language`.
- The `?lang=` and `?region=` URL parameters stand in for `CALCCRAFT_LANGUAGE` and
  `CALCCRAFT_REGION`.

The Linux and macOS ports need the same function split, so do it once, as a per-platform
`system_locale` module.

3.3. **Fonts.**

- Embed a small, freely licensed symbols font (a subset of Noto Sans Math or Noto Sans Symbols 2)
  for the glyphs that come from Segoe UI Symbol on Windows: `⌊ ⌋ ⌈ ⌉ ∛`, `±` and the
  converters' comparison pictures. Register it as the `symbols` family, which must always be
  registered (AGENTS.md pitfall).
- For the languages in `strings::script_font`, either fetch a Noto subset from the site the
  first time the language is picked (`fetch`, then `ctx.add_font`), or leave those languages
  out of the web build's language list.
- Add the new fonts' licences to `assets/fonts/README.md`.

3.4. **Clipboard.**

- Keyboard copy and paste already work: egui gets the browser's `copy` and `paste` events.
- Graphing's **Paste** menu item uses `navigator.clipboard.readText()`. It's asynchronous: run
  it with `wasm_bindgen_futures`, put the text in a shared cell, and insert it on the next frame
  (`ctx.request_repaint()`).
- Browsers ask for permission, and Firefox only allows the read from a user gesture. Where it's
  refused, keep the menu item disabled.
- Check the context menu's **Copy** for the graph image. eframe uses the async Clipboard API
  for images (`set_clipboard_image`).

3.5. **Share.** On the web, `share::show` calls `navigator.share({ files: [graph.png], text })`
where `navigator.canShare` allows it (Safari, Chrome on Android and Windows). Otherwise it
downloads `graph.png` through a `Blob` URL and an `<a download>`. Reuse `share::html` for the
text.

3.6. **Window features.**

- Hide the keep-on-top button on the web.
- Skip `ViewportCommand`s, which do nothing in a tab.
- The docked history and memory panel (from 560 px wide) and the narrow layouts already follow
  the canvas size.

3.7. **Keyboard shortcuts.** Go through every shortcut in AGENTS.md and FEATURES.md:

- Use `WebOptions::should_prevent_default`, plus a `keydown` listener on the canvas where eframe's
  own key handling doesn't, to stop the browser's default action while the canvas has focus:
  Ctrl+R/F5 (reload), Ctrl+L, Ctrl+E, Ctrl+H, Ctrl+P, F3, F4, F6, F7 and F12.
- Some keys can't be blocked: Ctrl+W, Ctrl+T, Ctrl+N and Ctrl+Tab, Cmd+Q on macOS, and
  sometimes Alt+1–5 on Linux. Document those, and keep the on-screen alternatives (every
  shortcut has a button).

3.8. **Touch and phones.**

- Check the touch targets.
- History and memory items have hover buttons and right-click menus. Check that egui's long
  press opens the context menus.
- Check that the equation boxes bring up the on-screen keyboard (egui's text agent), and that
  pinch-zooming the graph works or is turned off.

3.9. **Screen readers.**

- egui on the web has no AccessKit tree yet. With `web_screen_reader` it only speaks focused
  controls through speech synthesis, which is less than Narrator gets on the desktop.
- Document this, and don't turn on speech by default. Test it with VoiceOver and NVDA.

3.10. **Shown only on the desktop.** The "About" and Settings text that names Windows-only
behaviour (fonts, title bar) gets a web variant where needed.

**Done when:** the AGENTS.md "Verifying UI changes" checklist passes in Chrome and Firefox on
desktop, with an Android phone and an iPhone for the touch parts.

### Phase 4: size and speed

4.1. Measure first:

```bash
trunk build --release
```

Then check the size of `dist/*.wasm`, raw and gzipped. GitHub Pages compresses with gzip.
Targets: under about 5 MB gzipped, and under 3 s to first frame on a normal connection.

4.2. Shrink it if needed:

- Trunk's `data-wasm-opt="z"`.
- `lto = true`, `codegen-units = 1` and `opt-level = "s"` for the web build only. Put them in a
  separate profile (`[profile.web]`, inheriting `release`), selected with the `data-cargo-profile`
  attribute on Trunk's rust link, so the desktop builds don't change.
- Lazy-loading the 60 locale files, if they turn out to be a big part (they're `include_str!`
  today).

4.3. **Main thread.** The browser runs everything on one thread, like the desktop UI thread
today. Check the slowest cases, which can stall the tab:

- the Graphing analysis of busy functions (the `sin²x+cos²x` case PR #66 fixes);
- `tan x` zoomed far out;
- large factorials and powers in the Scientific mode.

If any takes longer than about a second, give it a time budget or move it to a Web Worker
(a later step; egui doesn't make that easy).

4.4. **Stack.** Rust's wasm stack is 1 MiB, the size the parser's limits (64 nesting levels,
an expression tree at most `MAX_DEPTH` = 100 deep) were tuned for on the Windows UI thread. Check that the deepest allowed
equations still work in the browser.

4.5. **Floating-point details.** WebAssembly's `sin`, `exp`, `ln` and `pow` come from Rust's
`libm`, not the operating system, so the last digit can differ from the desktop. The engine's
`BigRational` maths isn't affected; the Graphing analysis can be. Compare a set of analysis
results natively and in the browser (step 8.2).

### Phase 5: CI checks the web build on every push and pull request

Add a job to `.github/workflows/ci.yml`. It runs next to the three native jobs and deploys
nothing:

```yaml
  web:
    name: web build (wasm32)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7

      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
          components: clippy

      - uses: Swatinem/rust-cache@v2
        with:
          key: wasm32

      - name: Clippy (wasm32)
        run: cargo clippy --target wasm32-unknown-unknown -- -D warnings

      # Prebuilt Trunk; it fetches the wasm-bindgen and wasm-opt versions it needs.
      - uses: taiki-e/install-action@v2
        with:
          tool: trunk

      - name: Build the web version
        run: trunk build --release
```

Then add the line "and the web build (`cargo clippy --target wasm32-unknown-unknown`)" to
AGENTS.md's definition of done.

### Phase 6: deploy to GitHub Pages

6.1. **Repository settings** (done once, by a maintainer, in the GitHub web UI):

- Settings → Pages → Build and deployment → Source: **GitHub Actions**.
- ~~Settings → Environments → `github-pages` → Deployment branches and tags: add the tag
  pattern `v*`.~~ Not needed: by default only the default branch may deploy, which rejected
  v1.2.0's deploy from its tag, so `pages.yml` now runs after the Release workflow
  (`workflow_run`, whose runs are on `main`) and checks out the released commit.

6.2. **New workflow `.github/workflows/pages.yml`:**

```yaml
name: Web

# Builds the web version and deploys it to GitHub Pages when a release is tagged, or on demand.
on:
  push:
    tags:
      - "v*"
  workflow_dispatch:

permissions:
  contents: read
  pages: write # deploy to Pages
  id-token: write # prove the deployment comes from this workflow

# One deployment at a time; a newer one waits instead of cancelling a running one.
concurrency:
  group: pages
  cancel-in-progress: false

env:
  CARGO_TERM_COLOR: always

jobs:
  build:
    name: Build the web version
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7

      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown

      - uses: Swatinem/rust-cache@v2
        with:
          key: wasm32

      - uses: taiki-e/install-action@v2
        with:
          tool: trunk

      # The site lives at https://rapalon.github.io/calccraft/, so every URL starts there.
      - name: Build
        run: trunk build --release --public-url "/${GITHUB_REPOSITORY#*/}/"

      - uses: actions/upload-pages-artifact@v4
        with:
          path: dist

  deploy:
    name: Deploy to GitHub Pages
    needs: build
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - id: deployment
        uses: actions/deploy-pages@v4
```

Notes:

- Use the current major versions of `upload-pages-artifact` and `deploy-pages` when this
  lands; the other workflows are already on `checkout@v7`.
- A tag triggers this and `release.yml` in parallel. If the site should only go live after the
  desktop release is published, merge it into `release.yml` as a job with `needs: publish`
  instead.
- Trunk hashes the file names (`calculator-<hash>.wasm`), so browsers never mix an old wasm with
  a new loader. `index.html` itself is cached briefly by GitHub Pages (about 10 minutes).
- Leave out `.nojekyll`: Actions deployments don't run Jekyll.

6.3. **First deploy.** Run the workflow by hand from the Actions tab ("Run workflow" on the
branch), open the URL, and do the phase 3 checks on the real site. HTTPS matters: the Clipboard
API and Web Share work only in secure contexts.

### Phase 7: releases

- Add a `web` asset to `release.yml` (optional): zip `dist/` as
  `calculator-vX.Y.Z-web.zip`, so the web version can also be hosted elsewhere.
- Put the web link in the release notes (the `gh release create` step) and on the repository's
  About panel (website field).

### Phase 8: testing

8.1. **Unit tests stay native.** The engine, graphing, dates and converters have no platform
code, and `cargo test` keeps covering them on three operating systems.

8.2. **Optional: tests in the browser engine.** Run a subset of `graphing` tests on wasm with
`wasm-bindgen-test`, to catch the `libm` differences from step 4.5:

```bash
cargo test --target wasm32-unknown-unknown
```

This needs `wasm-bindgen-test-runner` installed. Mark the thread-based tests
`#[cfg(not(target_arch = "wasm32"))]`.

8.3. **Smoke test in CI.** A small Playwright test against `trunk serve`:

- the page loads, with no `panicked at` in the console;
- the canvas is drawn;
- typing `2+3=` shows `5`, read through a copied result or a screenshot comparison.

8.4. **Manual browser matrix before the first public deploy:**

| Platform | Browsers |
|---|---|
| Windows | Chrome, Edge, Firefox |
| macOS | Safari, Chrome |
| Linux | Firefox, Chrome |
| Phones | Android Chrome, iOS Safari |

Each one gets the AGENTS.md checklist, both themes, a German language and region
(`?lang=de-DE&region=de-DE`), and a reload to check persistence.

### Phase 9: documentation and launch

- **README:**
  - a "Web version" section with the link and the supported browsers;
  - how to run it locally (`trunk serve`);
  - the shortcuts the browser keeps;
  - the screen reader limitation.
- **AGENTS.md:**
  - the web build in the definition of done;
  - the rules for browser-safe code: no `std::time::SystemTime::now()` or `Instant::now()`
    (use `web_time`); no reliance on `std::fs`, threads or environment variables;
    platform code under `cfg(target_arch = "wasm32")`;
  - `index.html`, `Trunk.toml` and `pages.yml` in the architecture section.
- **FEATURES.md:** a row in the revision history.
- Merge `feature/web-version` into `main`, push a tag (`v1.2.0`), and check that both the
  release and the Pages deployment ran.

### Phase 10 (optional): installable and offline

- A web app manifest (name, icons, `display: standalone`, theme colours).
- A service worker that caches the wasm, JavaScript and fonts, so the site installs as an app
  and works offline. Trunk can copy one in; give its cache name the release version, so a new
  release replaces the old files.

## 4. Pull requests

| PR | Contents | Phases |
|---|---|---|
| 1 | Compile for wasm32: dependency split, `random()`, chrono, clipboard stub, wasm Clippy in CI | 1, 5 |
| 2 | Web entry point, `index.html`, `Trunk.toml`, icons, minimum size, loading screen | 2 |
| 3 | Browser language, region and default units, URL parameters | 3.2 |
| 4 | Fonts (symbols, script fonts on demand) | 3.3 |
| 5 | Clipboard paste, share, keep-on-top hidden, shortcuts, touch | 3.4–3.8 |
| 6 | Pages workflow, repository settings, first deploy | 4, 6 |
| 7 | Docs, release link, merge to `main` | 7, 9 |

PRs 3 and 4 can share their `system_locale` and font-loading code with the planned Linux and
macOS work, which has the same gaps.

## 5. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| The browser keeps some shortcuts | MR (Ctrl+R) and GRAD (F5) reload the page. | Block them while the canvas has focus (3.7); every action also has a button. |
| Download size | Slow first load on phones. | `wasm-opt`, a release profile for the web, lazy locales and fonts (phase 4). |
| No real screen reader support in egui on the web | Narrator users are better served by the desktop app. | Document it; keep `web_screen_reader`'s speech output. |
| `libm` differences in the last digit | Some Graphing analysis values may differ from the desktop. | Wasm test run (8.2); the engine's exact maths isn't affected. |
| Long computations on the main thread | A frozen tab instead of a frozen window. | Time budgets (4.3). |
| Shared `localStorage` on `rapalon.github.io` | Settings could mix with another egui app on the same account. | Check the keys (3.1), or use a custom domain. |
| The Pages environment rejects tag deploys | The first release deploy fails. | Happened with v1.2.0; `pages.yml` now deploys from `main` after the release (6.1). |
| A contributor PR breaks the web build | Silent until the next release. | The CI `web` job (phase 5) runs on every PR. |
