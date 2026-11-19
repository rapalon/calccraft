# Colour themes (1.4.4, 1.4.5)

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
| AMOLED Black | `AmoledBlack` | yes | always dark | (1.4.5) Pure `#000000` behind everything: the window, the display, every key, card, flyout, menu, popup, text box and the graph, so an OLED screen leaves those pixels off. Near-white text (`#F2F2F2`), keys told apart by thin dim outlines (`#262626`) instead of grey fills, and one light accent, a soft blue (`#8AB4F8`), for the `=`, links, the checked radio and the sliders. The graph is black too, with the dark graph's 14 equation colours. |
| High Contrast | `HighContrast` | yes | always dark | (1.4.5) For low vision, after Windows' high-contrast themes: white on black, every text at least **7:1** (WCAG AAA), no low-contrast greys (disabled text is `#A6A6A6`, 8.4:1, as Windows' Night sky theme writes it). Keys have **2 px white borders** and semibold labels (`key_border`, `bold_keys`). The `=` (and every accent key, links, the checked radio) is yellow `#FFFF00` with black text; a pressed key turns yellow with a black label; the keyboard focus ring is cyan `#00FFFF`, so focus, hover and pressed never look alike. Its own graph palette: black, white axes, and 14 bright equation colours, all at least 4.5:1 on black and apart from each other. |

Why High Contrast is one fixed dark palette rather than a light and a dark variant: white on black with a yellow accent is what Windows' and Android's high-contrast modes default to and what most low-vision users pick; one palette is simpler to keep at 7:1 everywhere (the tests check it), and the system's own Light/Dark preference would otherwise make two looks to learn. Konst's brief allowed either.

- Palettes are data: one `Palette` constant a theme and variant (`palette::GOLD_DARK`, `NIGHT`,
  …), most built with struct update from Classic's, and one `ThemeSpec` a theme
  (`palette::THEMES`: its key, label, whether it is Pro, its dark and light palettes, and the
  egui theme it fixes). A new theme is a new `Palette`, a `ThemeSpec` line and a label string.
- Per palette, a unit test checks a contrast of at least 4.5:1 (WCAG's relative luminance) for
  the labels: text on the background, keys, cards, flyouts and popups; secondary text on the
  background and cards; the `=` label on the `=` key (on every stop of a metal gradient); and
  that Night has no colour with blue above red or a near-white. Each graph palette's 14
  equation colours must be at least a set distance apart (`distinguishable`).
- AMOLED Black's test: every surface (background, keys, cards, flyouts, popups, controls, the
  calendar, the graph panels and the graph) is exactly `#000000`. High Contrast's: 7:1 for
  the text, secondary, tertiary and disabled text and the bit labels on every surface, the
  label on every key state (pressed included), links, and its 14 graph colours at least 4.5:1 on
  the graph's black.
- Palette fields for them (every other theme keeps the old look): `key_border` (the keys'
  outline width, 1 px; 2 in High Contrast, which also outlines the `=`) and `bold_keys` (key
  labels in semibold).

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
- (1.4.5) Every build's About section has a **Terms of Service** link under Privacy policy
  (`https://rapalon.com/calccraft/terms/`, `TERMS.md`, nothing added to the address). CalcCraft
  Pro (`play-paid`) has, where the free app has Get CalcCraft Pro, a quiet line of text, **Thank
  you for supporting CalcCraft** (`CalcCraft.ProThanks`): no link, no support or Pro text.

## 3. Strings

New `CalcCraft.*` keys in all 50 languages of `locales/calccraft.txt`: `ColorTheme`,
`ColorThemeDescription`, `ThemeClassic`, `ThemeGold`, `ThemeNight`, `ThemeRetroLcd`,
`ThemePaper`, `ProThemeLocked`, `GetPro`. The pill's "PRO" is the product's name and stays.
1.4.5 adds `ThemeAmoledBlack` ("AMOLED" stays, "Black" is translated), `ThemeHighContrast`,
`TermsOfService` and `ProThanks`.

## 4. Release (1.4.4)

- The Play builds take `CALCCRAFT_PAID_ID` from the repository variable, falling back to
  `com.rapalon.calccraft.pro` in `release.yml`: 1.4.3's run had no variable, so its free app hid
  the support prompt. A hand-run now builds the play-paid bundle and its universal APK too.
- The Pro app has its own launcher icon (`android/icon-source/pro/`); the free app's icon stays.
