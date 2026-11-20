# Device tests

Flows of the Android app, described by the **accessible names** of its controls (what TalkBack
reads, `content-desc` or `text` in a uiautomator dump), never by coordinates. The same scenario
files run three ways:

| Runner | Where | How |
| --- | --- | --- |
| Host test | every `cargo test`, CI | `device_scenarios_pass_on_the_host` in `src/app/a11y_tests.rs` runs them on the Android screen sizes through the same accessibility tree the phone gets |
| adb + uiautomator | a machine with adb and the phone on USB (pc_010) | `pwsh -File tests/device/run.ps1 -Serial <serial>` |
| AI agent | anywhere with the Marionet MCP tools | the prompt below |

Before a Google Play release, run them on a phone with one of the device runners and attach the
result table to the release (AGENTS.md, "Git and pull requests").

## The scenarios

`scenarios/*.json`, one flow each; the file name is the scenario's `name`.

| Scenario | What it checks |
| --- | --- |
| `basic-calculations` | 7 + 8, 9 × 6, 100 − 37, √144, 25², 81 ÷ 9, 3.5 × 4, 999 + 1, 50 ÷ 8, 12 × 12 in the Standard mode |
| `scientific-precedence` | 12 + 7 × 3 = 33 in the Scientific mode, and its expression line |
| `token-edit` | the expression line's parts as nodes: `Number 7` tapped at its centre (the caret goes after the 7), ⌫ and 9 give 39, `Operator +` changed into − gives −15 |
| `graphing-three-equations` | x^2, sin(x) and 2x+1 typed on the Graphing keypad (its Trigonometry menu too) and submitted; the graph's node says `displaying 3 equations` |
| `length-converter` | a typed value in the Length converter |
| `mode-switching` | the navigation menu opens every kind of page, and the Standard mode still calculates after them |
| `support-popup` | the free app only: the support popup opened from the menu's `Please Support!` has no `I already supported`, and nothing is named `Supported by you` (docs/support-prompt.md, section 8). `Get CalcCraft Pro` is never tapped (that opens Google Play); its pause is pinned by the host tests. A free build without a paid app ID has no prompt, and the steps only check that none of it shows |
| `themes` | Settings → Color theme (docs/color-themes.md). The free app: Classic checked, the Pro themes named `Gold, Pro theme, locked, opens CalcCraft Pro on Google Play` and so on, never tapped (that opens Google Play), AMOLED Black and High Contrast among them. CalcCraft Pro: Night, then High Contrast picked, 7 × 6 = 42 still on the display, Gold picked back |

Each step is an object with one key:

| Step | Does |
| --- | --- |
| `{ "launch": true }` | stops the app and starts it again |
| `{ "open": "Scientific Calculator" }` | taps `Open Navigation`, then the menu's item. An item the list doesn't show: swipe the list up, from the center of its lowest item shown to the center of its highest (items are named `… Calculator` and `… Converter`), and look again, up to 4 times |
| `{ "tap": "Seven" }` | taps the center of the node named exactly so |
| `{ "taps": ["One", "Plus", "Two"] }` | taps them in order |
| `{ "tap_if_present": "Switch to graph mode" }` | taps it if it's there (the Graphing mode's views differ by width) |
| `{ "expect": "Number 7" }` | a node is named exactly so |
| `{ "expect_prefix": "Convert from 12.5 " }` | a node's name starts so |
| `{ "expect_contains": "displaying 3 equations" }` | a node's name contains it |
| `{ "expect_absent": "Trigonometry" }` | no node is named so |
| `{ "display": "1000" }` | the display's node is `Display is 1000`, compared **loosely** |
| `{ "expression": "12 + 7 × 3 =" }` | the expression line's node starts with `Expression is 12 + 7 × 3 =`, loosely |
| `{ "scroll_to": "Night" }` | until a node is named so: a finger drawn up the middle of the screen, from 65 % of its height to 35 %, up to 6 times (Settings on a low screen) |
| `{ "expect_checked": "Classic" }` | a node is named so and is checked (a radio button: uiautomator's `checked="true"`) |
| `{ "edition": "free" }` | the steps up to the next `edition` are only for that edition: `free` (the free Play app and its test builds) or `pro` (CalcCraft Pro, an application ID ending in `.pro`); `any` ends the part. The other edition's steps are left out, not failed |

A node's name is its `content-desc`, else its `text`. **Loosely** means without `,` `.` `'` `’`,
spaces and non-breaking spaces on both sides, so that the scenarios' en-US numbers match the
phone's regional format (`1,000`, `1’000`, `6,25`). After a tap, wait a moment (about 0.4 s)
before reading the screen; an expectation may look again a few times before it fails.

What the app exposes (android/README.md, "Accessibility"): every key by its Windows name
(`Seven`, `Multiply by`, `Square root`, `Equals`), the display (`Display is 33`, `Editing 7`
while a number is edited) and the expression line (`Expression is 12 + 7 × 3 =`, then `…, Editing
7` or `…, Selected operator +`), the expression line's numbers and operators as their own nodes
(`Number 12`, `Operator +`), the keypads and open popup menus as groups (`Full keypad`,
`Trigonometry`), the navigation items (`Standard Calculator`, `Length Converter`), the
Graphing equation box (`Enter an expression`) and the graph (`Graph viewing window, x-axis
bounded by … displaying 3 equations`, its text the equations, one a line).

### Before you start

- The phone is unlocked, its screen stays on (Developer options → Stay awake while charging),
  and CalcCraft's language is English (Settings → Language, or the phone's): the names are
  English.
- The build under test is installed: the accessibility test build is
  `com.rapalon.calccraft.a11ytest` (`pwsh -File android/build-apk.ps1 -ApplicationId
  com.rapalon.calccraft.a11ytest -Channel play-free -Debuggable`, then `adb install -r
  target/android/<the APK>`), a release `com.rapalon.calccraft`, and CalcCraft Pro
  `com.rapalon.calccraft.pro` (run the scenarios for each app: `themes` differs between them).
- Android 8.0 or later: the accessibility tree needs it.

## Runner: adb and uiautomator

PowerShell 7.2 or later (`pwsh`) and adb on the `PATH` (or `-Adb <path>`).

```powershell
adb devices                                    # the serial
pwsh -File tests/device/run.ps1 -Serial <serial> -Package com.rapalon.calccraft.a11ytest `
    -Out target/device-results.md
pwsh -File tests/device/run.ps1 -Serial <serial> -Scenario token-edit   # one of them
pwsh -File tests/device/run.ps1 -SelfTest      # no phone: the script against a recorded dump
```

It prints a line per step while it runs, then a table of every step (`PASS`, `FAIL` with the
reason and the names on screen, `SKIP` after a failure) and one of the scenarios, and exits with
1 if any step failed. `-Out` writes both tables as Markdown, with the phone's model and the
app's version, for the release. It reads the screen with `uiautomator dump`, which fails while
another UiAutomation client (an instrumentation test) is connected; Marionet's accessibility
service doesn't get in the way.

## Runner: an AI agent through Marionet

Give the agent this prompt, with the device's Marionet name and the scenario files (paste their
JSON or let it read them from the repository):

> You are testing the CalcCraft Android app on the device `<device>` through the Marionet MCP
> tools. Run every scenario in `tests/device/scenarios/*.json` of the CalcCraft repository, in
> file name order, step by step, exactly as `tests/device/README.md` defines the steps. Find
> controls only by name: a node's name is its description, else its text, and it must match
> exactly (a fuzzy `find` result whose name differs is no match). Never tap coordinates you read
> off a screenshot, and don't take screenshots unless a step fails and you need to say why.
>
> - `launch`: `launch_app` with the package `<package>` and `clear_task=true`, then
>   `wait_for_screen(until="text", text="Open Navigation")`.
> - `tap` / `taps` / `open`: `get_screen` (text format), take the element named exactly so,
>   `tap(element, sid)`; use `find(text)` only when it isn't on screen (it scrolls). For `open`, tap
>   `Open Navigation` first, then the item.
> - `expect`, `expect_prefix`, `expect_contains`, `expect_absent`, `display`, `expression`: read
>   `get_screen` and compare as the README says (`display` and `expression` loosely, without
>   `,` `.` `'` `’` and spaces). If it doesn't match, wait for the screen to be stable
>   (`wait_for_screen(until="stable")`) and look once more before you call it a failure.
> - `scroll_to`: `find(text)` (it scrolls) until the element named exactly so is on screen.
>   `expect_checked`: the element named so is checked (`get_screen` marks it).
> - `edition`: `<package>` ending in `.pro` is the `pro` edition, any other the `free` one.
>   Leave out the steps after an `edition` step of the other edition, up to the next `edition`
>   step (`any` ends it); they get no row.
>
> After a failed step, skip the rest of that scenario and go on with the next one. Report a
> Markdown table with one row per step: scenario, step number, the step as written, PASS, FAIL
> or SKIP, and for a failure what the screen showed instead (the names nearby). End with one row
> per scenario (PASS only if all its steps passed), the device, the app's version
> (`get_device_info`), and the date.

`get_screen` lists elements as `e12 Button "Seven"`; the display and the expression line are
`Text` elements (`"Display is 33"`), the graph a `View` with the graph's name. The keys say
`clickable="false"` in a uiautomator dump although they take clicks (`ACTION_CLICK`,
android/README.md); Marionet taps them by their bounds.

## Adding a scenario

Write `scenarios/<name>.json` with the steps above, then run `cargo test --lib
device_scenarios`: the host test runs it on every Android screen size and names the step that
failed, with the names that were on screen. The names come from Windows' strings
(`locales/en-US.txt`, the `AutomationName` keys) and `locales/calccraft.txt` (`CalcCraft.*`);
a new control needs a name there (`access::control`). `run.ps1 -SelfTest` checks that every step
is one it knows.
