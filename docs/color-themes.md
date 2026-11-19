# Colour themes (1.4.4)

CalcCraft's own setting; Windows has only Light, Dark and "Use system setting". Konst's brief of
2026-11-19: CalcCraft Pro (`com.rapalon.calccraft.pro`, the `play-paid` channel) gets selectable
colour themes, with a gold `=` key by default; the free Play app shows the Pro themes locked, and
a tap on one opens Pro's Play listing.

## 1. The setting

- Settings → Appearance: a **Colour theme** card right under App theme, an expander like it,
  with one radio item a theme. Stored as `CalcCraftColorTheme` (the key of the theme below).
- The App theme card keeps working: a theme follows Light, Dark or the system unless it fixes
  its own (Night is always dark, Paper and Retro LCD always light). The App theme choice stays
  stored and applies again when a theme that follows it is picked.

| Theme | Key | Pro | Light / Dark | What it is |
| --- | --- | --- | --- | --- |
| Classic | `Classic` | no | follows App theme | Today's look (`palette::DARK`, `LIGHT`), unchanged. |
| Gold | `Gold` | yes | follows App theme | Classic with a metallic gold `=`: a vertical multi-stop gradient, a soft specular highlight and a thin darker edge, like the gold key of the Pro icon. Static. Hover brightens, pressed darkens and drops the highlight. Also the Graphing keypad's submit key and the toggled `2nd`/`hyp` keys (every accent key). |
| Night | `Night` | yes | always dark | For the dark: near-black, every text, icon, accent, focus ring, popup, egui tooltip and text selection in dim red; no white and no blue. The graph uses its own dark-red palette (axes, grid, 14 equation colours of red to amber hues, no blue). The `=` is red with a near-black label. |
| Retro LCD | `RetroLcd` | yes | always light | A greenish-grey LCD panel behind the display with dark digits, muted grey-green keys, a dark olive `=`. Colours only (no seven-segment font is bundled, and none is added). |
| Paper | `Paper` | yes | always light | Warm cream with ink-dark text, minimal accents: the `=` is ink. |

- Palettes are data: one `Palette` constant a theme and variant (`palette::GOLD_DARK`, `NIGHT`,
  …), most built with struct update from Classic's, and one `ThemeSpec` a theme
  (`palette::THEMES`: its key, label, whether it is Pro, its dark and light palettes, and the
  egui theme it fixes). A new theme is a new `Palette`, a `ThemeSpec` line and a label string.
- Per palette, a unit test checks a contrast of at least 4.5:1 (WCAG's relative luminance) for
  the labels: text on the background, keys, cards, flyouts and popups; secondary text on the
  background and cards; the `=` label on the `=` key (on every stop of a metal gradient); and
  that Night has no colour with blue above red or a near-white. Each graph palette's 14
  equation colours must be at least a set distance apart (`distinguishable`).

## 2. Gating by distribution channel

The channel is the build's Cargo feature (docs/support-prompt.md, section 8). No in-app
purchase, no licence check, no network: the paid app is a separate Play app.

| Channel | Pro themes | Default |
| --- | --- | --- |
| Default (GitHub, F-Droid, desktops, web; open source with Donate) | all unlocked | Classic |
| `play-paid` (CalcCraft Pro) | all unlocked | **Gold** |
| `play-free` with `CALCCRAFT_PAID_ID` | shown locked: dimmed, with an oval **PRO** pill; a tap opens Pro's listing (`market://details?id=<id>`, else `https://play.google.com/store/apps/details?id=<id>`) and changes nothing | Classic |
| `play-free` without `CALCCRAFT_PAID_ID` | hidden | Classic |

- A stored theme that this build doesn't unlock (a free-app user's `Gold` from a copied
  `app.ron`) loads as the channel's default, Classic: a free user can't end up on a Pro theme.
- Screen readers: an unlocked item is a radio button named by its theme; a locked one is a
  button named "Gold, Pro theme, locked, opens CalcCraft Pro on Google Play"
  (`CalcCraft.ProThemeLocked`, `%1` the theme).
- The free Play app also gets a quiet **Get CalcCraft Pro** link in the About section, under
  Privacy policy (`play-free` with an ID only), which opens the same listing.

## 3. Strings

New `CalcCraft.*` keys in all 50 languages of `locales/calccraft.txt`: `ColorTheme`,
`ColorThemeDescription`, `ThemeClassic`, `ThemeGold`, `ThemeNight`, `ThemeRetroLcd`,
`ThemePaper`, `ProThemeLocked`, `GetPro`. The pill's "PRO" is the product's name and stays.

## 4. Release (1.4.4)

- The Play builds take `CALCCRAFT_PAID_ID` from the repository variable, falling back to
  `com.rapalon.calccraft.pro` in `release.yml`: 1.4.3's run had no variable, so its free app hid
  the support prompt. A hand-run now builds the play-paid bundle and its universal APK too.
- The Pro app has its own launcher icon (`android/icon-source/pro/`); the free app's icon stays.
