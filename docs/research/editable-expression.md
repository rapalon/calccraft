# Research: editing the entry and the expression with a cursor

Status: written 2026-11-18 for task 39036547 as research; A, B and C were then built on one
branch for task 246b6e2c (`src/engine/caret.rs`, `src/engine/tape.rs`, `src/app/caret.rs`), with
the choices in "Decisions" at the end.

## The request

This came from CalcCraft 1.4.1 on Android, in the Scientific mode. The expression line showed
`2 + (5 × 7) + (100 - 92) + (6 × 54)` and the display showed `324`:

1. Put the cursor in `324` after the 3, delete the 3, type 5, and get `524`.
2. Put the cursor after `92` in the expression line, replace it with `85`, and see the whole
   expression recalculated.

**Verdict:** both are feasible on every platform: Windows, macOS, Linux, Android and the web.
The whole UI is painted by hand, so the same Rust code serves every platform. Request 1, for a
number that is still being typed, is a small and safe change. Request 2 needs a new engine
layer: a replayable "tape" of the operands and commands. Once that layer exists, request 1 for
a computed value (Konst's `324`), editing history items and live recalculation all come cheaply.
Neither change breaks a pinned Windows behavior, because the caret only appears when the user
taps or clicks the display. Both are deliberate extensions beyond Windows, though. Windows
Calculator has no caret in any mode, and its issue tracker has open requests for one: #520
"Editing expressions in scientific calculator", #163 "History editing" and #2234 "Edit the
result by pressing backspace". AGENTS.md calls the real Windows Calculator the specification,
so Konst has to approve the extension (see "Open questions").

## How input works today

- **The entry** is `Input` (`src/engine.rs`, Windows' `CalcInput`). It holds a sign, a `digits`
  string (`0-9`, or `A-F` in the Programmer mode) with at most one `.`, and an optional
  `Exponent`. `try_add_digit` only **appends**, and `backspace` only **pops**. The limits are
  checked on append: 16 or 32 digits, the integer mode's `digit_fits` and `sign_toggles`, and
  4 exponent digits. `record` (`m_bRecord`) is true only while a number is being typed. After
  an operator, `)`, a function, `=`, MR, π or a history recall, the display shows
  `current: Number`, an exact `BigRational`, formatted by `format_with` / `IntegerMode::format`.
  No input string stands behind that value.
- **In Konst's example, `324` is not an entry.** It is the value of the group `(6 × 54)` that
  `)` just closed. Its digits were never typed, so editing them means "replace the last operand
  with a typed number". That is the same operation as request 2.
- **The evaluation state** follows `ProcessCommandWorker`. It is made of `current`, `last`,
  `hold` (the right operand that a repeated `=` reuses), `op` / `prev_op`, `change_op`,
  `no_prev_equ`, `last_key` / `this_key`, `precedence_stack` and `paren_stack` (Scientific and
  Programmer only), `angle`, `notation`, and the integer mode's radix and word size. Standard
  executes immediately: `2 + 3 × 4 =` gives 20. `%`, `±` on a finished number, MR and history
  recall all set `current` directly.
- **The expression line is a rendering, not reparseable text.** `HistoryLine` is a list of
  `Token { text, value }` built by `CHistoryCollector`'s rules: operators, `(`, `)`, functions
  wrapped around operands (`sqr(`, `sin₀(`, `negate(`), precedence inversions inserting
  brackets, and `%` replacing an operand with its computed value. `shown` is a snapshot that is
  refreshed only at Windows' steps, so `1 ÷ 0 =` keeps `1 ÷`. The UI draws
  `numbers().localize_expression(calc.expression())`, elided on the left, with no positions
  (`draw_display_sized` in `src/app.rs`). `paste` reads only digits, `+ - * / ^ % ( ) e`, so it
  cannot read `sin₀(30)`, `negate(5)` or `fact(3)` back. The Graphing parser is `f64` with other
  semantics, so neither can be reused to re-evaluate the line exactly.
- **History** (`HistoryEntry { expression, result, text }`) stores only strings and the result.
  `recall_history` shows the result "as after `=`". Nothing in an entry can be replayed.
  History is not persisted, as in Windows.
- **The Graphing mode** already has a real text editor: egui `TextEdit` with the `math_job`
  layouter, the app's keypad typing at the caret through `insert_text`, and, on Android,
  `SoftKeyboard` keeping the system IME closed. That is a useful precedent for the Android caret
  UX. Its parser cannot evaluate calculator expressions, though.

## Option A: a caret inside the number being typed

The engine change is small and contained in `Input`:

- Add `caret: usize`, a logical index into `digits` (and into the exponent part, as a second
  segment). It defaults to the end, so every existing key keeps today's behavior.
- Add digit insertion at the caret, ⌫ before the caret, Delete after it, and moving the caret
  (left, right, start, end). A digit is validated on the *candidate* string: the digit limit,
  a single `.`, the leading-zero rule ("0" is never stored first), and in the Programmer mode a
  new `IntegerMode::fits(candidate, negative)` generalizing `digit_fits`. A candidate that does
  not fit is refused, as a 17th digit is refused today.
- **Group separators and padding never count.** The display is the region's grouping
  (`3;2;0` in India, `.` in de-DE), the Programmer mode's spaces in groups of 4 and 3, and BIN's
  leading zeros (`pad_binary`). The UI maps between characters and digits by counting digit
  characters in the localized galley. A tap snaps to the nearest digit boundary.
- Exact fractions and large numbers are not an issue: the entry is a typed decimal string. A
  computed value with 32 digits or `1.e+16` is not an entry, and that case belongs to Option B.

The UI (`draw_display_sized`, the keyboard handlers, `access`) changes like this:

- A tap or click on the display while `record` is true places the caret (galley hit-testing),
  and the caret is drawn as a 1–2 px bar in `p.text`. The shrink-to-fit font is a challenge:
  32 digits at 10 pt are about 6 px each. The tap target is the digit boundary, and a drag moves
  the caret.
- The caret turns on an "editing" state. In that state Left, Right, Home, End, Delete and
  Ctrl+Left/Right (by digit group) work on the caret instead of moving egui's focus. Esc only
  leaves the editing state, because Esc otherwise means C. Any operator, `=` or function
  finishes the number as today and drops the caret.
- Android and mobile web keep the app's own keypad, with no IME. The MVP has no selection
  handles and no magnifier, since egui draws neither. A long press keeps its current meaning.
- Accessibility: `egui::text_selection::accesskit_text::update_accesskit_for_text_widget` is
  public in egui 0.36.2. It publishes a galley as `TextRun` nodes with a `TextSelection`, so
  Narrator and TalkBack can read the caret position as in a text box. The display becomes
  `Role::TextInput` while editing, and the live "Display is …" announcement stays.

**Effort:** about 14–18 ideal agent hours in 3 tasks: engine and tests, UI on all platforms,
and accessibility with a device check. Pinned behavior cannot change, because a caret at the
end is exactly today's input.

## Option B: an editable expression line with recalculation

Re-parsing the text would mean writing a second evaluator that has to imitate
`ProcessCommandWorker`'s quirks: Standard's immediate execution, `%`, implicit `)` at `=`,
`2(3)` and the precedence inversions. It would drift from the engine, so this option is
rejected. **Replaying keys is exact by construction:**

- **The tape.** The `Calculator` keeps, since the last `C` (or the start of the expression), a
  list of `Item::Operand(OperandSource)` and `Item::Command(Command)`. `OperandSource` is the
  final typed text (after ⌫, ± and exp), or a recalled literal: MR, π, e, rand's drawn value, a
  history recall, or a bit-toggled value with its radix. Settings commands that change
  evaluation are on the tape too (angle, F-E, radix, word size). Memory commands are not, and
  the memory stays as it is. Each operand `Token` gets `source: Option<usize>`, the item that
  typed it.
- **An edit** replaces one `Operand` item and replays the whole tape into a fresh engine of
  the same mode (`with_precedence()`, `programmer()` plus its radix and word size, and the
  memory snapshot). The engine state that results is exactly the state that typing the edited
  sequence fresh would give: precedence, `hold` for a repeated `=`, `%`, the shown-line quirks
  and errors included. A property test pins this: for random key sequences and random operand
  edits, `replay(edited)` equals a fresh `run(edited keys)` on `display()`, `expression()`,
  `error()` and the next `=`. The MS oracle golden vectors (`ms_oracle.rs`) still have to pass
  unchanged.
- **What is editable.** Typed operands are editable (the `92`, the `54` inside `(6 × 54)`, the
  `30` inside `sin₀(30)`). A computed operand can be replaced by a typed number: a group, a
  function, `%`'s value, MR. This also covers request 1 for Konst's `324`: tapping the result
  gives the caret in a typed copy `324`. Committing it replaces the item range of the last
  operand, the group, with `Operand("524")`. Operators and functions are not editable in the
  MVP. Changing `+` to `−` inside the line could come later on the same tape.
- **When it recalculates.** The engine recalculates live on every edit key. A replay of a few
  dozen items is milliseconds, even with 400-bit trigonometry, so the display shows the new
  result, or an error, while the caret stays in the operand. `=` or a tap outside commits the
  edit, and Esc cancels it. After `=` (the line ends with ` =`), the edit produces a new result
  and adds a **new** history entry, leaving the old one alone, as iOS and Google do. This is a
  reversible choice.
- **Modes.** Standard and Scientific work the same way, each with its own precedence, since the
  replay uses the same engine. Programmer works too: operand items keep the radix they were
  typed in, the edit is typed in the current radix, and the line's radix rewriting is
  unchanged. Graphing already edits its equations as text and needs nothing.
- **UI.** The expression line is laid out per token, with byte ranges, so a tap maps to a
  token. The tapped operand becomes an inline editor with Option A's caret code. The line
  scrolls horizontally instead of eliding when the caret is inside it. Narrow windows and
  Android get the same flow, with a larger 14→16 pt line while editing so taps can hit it.
  Accessibility works as in A, with operands reachable by Tab while editing.

**Effort:** about 30–40 ideal agent hours in 5–7 tasks: tape and replay with property tests,
token positions, the inline editor, live preview and commit or cancel, the Programmer mode,
accessibility, and device checks. The risk is medium, and it sits mostly in the engine. The
tape has to see every command (paste goes through `paste_key`, so it is covered). Replay must
not create history entries or vibrations. `shown` snapshots must keep their tokens' sources.

## Option C: edit from history

With the tape stored in `HistoryEntry`, a history item gets an "Edit" action: in its context
menu, and on a tap of its expression. The action replays the item's tape up to `=` into the
engine and opens Option B's editor on it. It is the cheapest form of "change 92 to 85 later"
**once B's engine exists**: about 4–6 extra hours. Without the tape, C could only reload the
result, which is what `recall_history` already does. A text editor on a reparsed expression
(the Graphing `TextEdit`) is rejected for the reasons given under B.

## Other calculators (what users expect)

| App | Caret in the entry/expression | Notes |
| --- | --- | --- |
| Windows 11 Calculator | No | Keys only: ⌫ at the end, history recalls only the result. Issues #520, #163 and #2234 ask for editing. |
| Samsung Calculator | Yes | The formula is a text field: tap to place the cursor, then insert or delete with the keypad. |
| Google Calculator | Partly | The formula is text-based. Reports of tap-to-edit are second-hand and vary by version. Not verified on a device. |
| iOS 18 Calculator | Limited | Shows the full expression. Guides say you can tap the formula after `=` to re-edit it. History can be recalled. |
| HiPER, TI-Nspire | Yes | Expression editors with a cursor and arrows. |

Users of expression-style phone calculators (Samsung, HiPER) expect to tap the formula and edit
it. Windows users expect nothing more than ⌫. A tap-only caret serves both groups.

## Decision table

| | A: caret in the typed number | B: edit operands in the line (tape) | C: edit from history |
| --- | --- | --- | --- |
| Covers request 1 | Typed numbers only | Yes, Konst's `324` included | Via B |
| Covers request 2 | No | Yes, with live recalculation | Yes, for finished calculations |
| Exact Windows semantics | Unchanged | Equal to fresh typing by construction (property test) | Same as B |
| Engine change | Small (`Input`) | New tape, replay and token sources | Tape in `HistoryEntry` |
| Effort (ideal agent hours) | 14–18 (3 tasks) | 30–40 (5–7 tasks) | +4–6 (1 task) after B |
| Risk | Low | Medium (engine invariants) | Low after B |
| All platforms | Yes (same painted UI) | Yes | Yes |

## Recommendation and plan

1. **1.4.2: Option A** for Standard, Scientific and Programmer, on every platform: caret by tap,
   click and drag, arrows, Home/End, Delete, insertion at the caret, with grouping, BIN padding
   and hex digits handled, and with AccessKit text runs. It fixes the most common case, a typo in
   the number being typed, at low risk.
2. **1.5: Option B**, then **C**. The engine tape and the property tests come first, as an
   invisible change of their own. After that: operand editing in the expression line, with live
   recalculation and commit on `=`; replacing a computed operand, Konst's `324` case; and
   "Edit" on history items. Later: selection, Ctrl+C of a selection, magnifier/handles on
   Android, and editing operators.
3. Every phase needs: engine unit tests in `engine.rs` (caret edits, limits per radix and word
   size), the replay property test, the MS golden vectors unchanged, `layout_tests` at the
   Android sizes, and manual checks on Windows (Narrator), Android (TalkBack, Pixel and
   Samsung) and the web (desktop and mobile Chrome).

## Open questions for Konst

- **Departing from the Windows spec.** Is a tap-only caret acceptable as CalcCraft's own
  extension? The recommendation is yes, always on, because nothing changes until the user taps.
  A settings switch is the alternative.
- **After an edit of a finished calculation,** should the edit add a new history entry
  (recommended) or replace the old one?

## Decisions

Both open questions were answered on 2026-11-18 (task 246b6e2c, Konst through the relay),
and A, B and C are built together on one branch. The rest of this section is the choices made
while building them, each the simplest one and the closest to typing the number fresh.

- **Always on, no setting.** Nothing changes until the user taps or clicks the display or the
  expression line, so every pinned Windows behavior stays.
- **A finished calculation that is edited and committed adds a new history entry.** The old
  entry stays as it was. An edit that changes nothing adds nothing.
- **Android: a caret only.** No selection, handles or magnifier, and never the system keyboard:
  the app's keypad types at the caret. Desktop: click, Left/Right, Home/End, Ctrl+Left/Right,
  Delete and Backspace. Copy and paste are unchanged (paste still replaces the entry).
- **The caret is a position in the typed characters, not in the text shown.** It counts the
  digits and the decimal point of the number, and the exponent's digits as a second segment
  (`1.5e+3` has the stops before 1, 5, after it, and before and after 3). Group separators,
  the Programmer mode's spaces and BIN's leading zeros are skipped, so the caret works the same
  with `1,234,567`, `12,34,567`, `1.234.567` and `0001 0010`.
- **Limits are checked on the candidate number.** A digit typed at the caret that would make the
  number too long (16 or 32 digits) or too large for the word is refused, as a 17th digit is
  today. Zeros typed before the first digit disappear, as they do when typed first (`05` is 5).
  Backspace at the start of the exponent removes `e+0` only when the exponent has no digits.
- **Ctrl+Left and Ctrl+Right jump to the start or end of the number,** and from there to the
  previous or next number that can be edited (the line's operands, then the entry). The keys
  move the caret only while it is shown; otherwise the arrows keep moving egui's focus. Left
  while a number is typed and nothing has the keyboard focus shows the caret before the last
  character.
- **Esc first hides the caret** (and cancels an edit of the expression line); only the next
  Esc is `C`.
- **The tape starts at `C`** (and when the mode starts): editing reaches back to the last `C`.
  The state after `C` equals a fresh engine with the same angle unit, F-E and Programmer radix
  and word size, which the property test checks. It records at most 400 commands; a longer
  calculation can't be edited until the next `C`.
- **The tape records the commands as they came,** and a typed number as the keys that typed it
  (⌫ and ± included), so an unedited replay is the original key for key. An edited number is
  typed as its digits, with ± after them (after the first digit in the Programmer mode, where a
  number like −128 in a BYTE fits only while negative), then `exp` and the exponent's digits.
  MR, a history recall and `rand` replay the value they gave, not the memory's or a new random
  one. MS, M+, M− and MC replay only their effect on the calculation (they finish the number
  being typed); the memory isn't touched again. Replays add no history entries and play no
  vibration (the keypad vibrates, the engine doesn't).
- **What can be edited.** In the expression line: every number it shows (typed or computed, see
  the next point) and every binary operator. On the display: the number being typed (A), or a
  computed value that typing would
  replace: after `)`, a function, MR, π or `rand`, the copy replaces that last operand
  (Konst's `324`: `(6 × 54)` becomes the typed `524`), and after `=` or an operator the copy is
  a new number typed there. A display value whose replacement would change how the keys before
  it are read (a group opened right after a number, `2(3)`) gets no caret.
- **Computed numbers in the line** (2026-11-18, Konst's test on the edittest APK: in
  `10 + 2 = 12 + 4 = 16 ÷ 4 = 4` the `12` and the `16` couldn't be edited). A number the line
  shows that wasn't typed (a result carried over after `=`, Standard mode's `5` in `2 + 3 +`,
  `%`'s value, MR, π, a number inside `sqr(…)`) is replaced by the same number typed as a
  literal: the tape steps that computed it are dropped and the literal is typed in their place
  (`12 + 4 =` comes from `10 + 2 = + 4 =` and becomes `12 + 4 =` typed fresh, then `15 + 4 =`).
  Which steps those are is found by trying: the smallest range of steps before the number
  whose replacement by the literal gives the same line, display, error, parentheses, settings
  and next `=`; a number for which no range does (a rounded 1/3 that later steps go on with,
  a group's value inside the line) can't be edited. The tap only offers the substitution: until
  a key changes the number, Esc, `=` or a tap elsewhere leaves the original tape.
- **No cascade.** Editing a calculation never recomputes the history entries after it: editing
  `10 + 2 =` to `10 + 5 =` adds `10 + 5 = 15`, and `12 + 4 = 16` and `16 ÷ 4 = 4` stay as they
  were (relay's decision for task f85238e3, Konst may change it). Every committed edit of a
  finished calculation adds a new entry, as before.
- **Binary operators** (+ − × ÷; in the Scientific mode also `mod`, `^` (xʸ), `yroot` (ʸ√x) and
  `log base` (logᵧx); in the Programmer mode `%`, AND, OR, XOR, NAND, NOR, Lsh and Rsh). A tap
  or click on one in the line selects it, drawn like the edited number but without a caret.
  Another binary operator key (the keypad, or `+ - * /` and the mode's other operator keys on a
  keyboard) then replaces it on the tape and the tape is replayed, so precedence comes out as
  typing the edited calculation fresh: `2 + 3 × 4 =` with `×` changed to `+` is 9, and in the
  Scientific mode `2 × 3 + 4 =` with `+` changed to `×` is 24 there and in the Standard mode
  alike, while `2 + 3 × 4` → `2 × 3 + 4` follows each mode's rule. The display shows the new
  result live and the operator stays selected, so another operator key replaces it again;
  `=` commits (a finished calculation adds a new history entry), Esc cancels, and any other
  key commits and then does what it does. An operator key that the mode doesn't have isn't on
  its keypad and does nothing. In the error state that a replaced operator causes (`÷ 0`) the
  operator keys stay enabled while it is selected, so it can be changed back.
- **Unary functions stay as they are.** `sin`, `√`, `x²`, `1/x`, `negate`, `NOT`, `RoL` and
  the others can't be selected or replaced in this version; the numbers inside them can be
  edited. Neither can `(`, `)` or `=`, nor an operator a key other than an operator put in the
  line (the `+` of `8 + 3 =` that a repeated `=` writes after `5 + 3 =`, the `×` of `2(3)`).
- **Where a tap puts the caret** (2026-11-18, found through Marionet on the Pixel: a tap at the
  centre of `Number 7` in `12 + 7 × 3 =`, then ⌫ and 9, gave `97`, not `9`). A tap on a number
  of the line goes to the nearest digit boundary, but a number of one character has only its
  start and its end, and its centre is a tie that the font and the rounding to the screen's
  pixels decide. So a tap anywhere on a one-character number puts the caret after it: a finger
  can't aim at one side of a digit, and after it is where typing goes on, as at the end of the
  number being typed. A longer number keeps the nearest boundary, so a finger can still go
  between its digits. A screen reader's or an accessibility service's click on a number's node
  (`ACTION_CLICK`, AccessKit's `Click`) puts the caret at the number's end, whatever its length:
  that is what "edit this number" means there. The parts' nodes keep `clickable=false` on
  Android (decision #e4bec910: no second vendored crate), so Marionet taps their centre, which
  the rule above covers for one-digit numbers. For a longer number an agent should use the
  click action, or tap at the right edge of the node's bounds.
- **Keys.** Ctrl+Left and Ctrl+Right stop on the line's operators and computed numbers too, in
  the order of the line; on a selected operator, Left and Right go on to the number before or
  after it. Esc cancels.
- **In the Programmer mode** a typed number in the line can be edited only in the radix it was
  typed in; after a radix change only the new numbers can. A computed number is typed in the
  current radix.
- **Live result while editing the line.** While a number of the expression line is edited, the
  display shows what `=` would give for the edited expression. Committing an edit of a finished
  calculation (one ending with `=`) keeps the new result and adds the history entry; committing
  an edit of an unfinished one returns to the calculation in progress, with the edited number.
  `=` commits (and evaluates an unfinished expression), Esc cancels, and any other key commits
  and then does what it does. A tap on the display or the line outside the number commits too.
- **Edit in the history** is a context menu item (right click, or a long press on Android), and
  it is the only new control: it reloads the calculation's tape, so the expression line shows
  that calculation with its typed numbers editable, the caret in the last one. Standard mode
  entries that an operator logged (`2 + 3 +` logs `2 + 3 =`) open as if `=` had been pressed
  there.
- **A plain click or tap on a history entry recalls it with its tape too** (2026-11-18, Konst's
  report from the edittest APK: after a recall only the result took the caret). The tape is
  replayed, then the engine is left as Windows' recall leaves it: the result after `C` with the
  entry's expression line, so `=` doesn't repeat the entry's last operation and a new number
  starts over, as before. The angle unit and F-E stay as they were at the recall. The line's
  numbers then take the caret like after a fresh calculation, the display shows the edited
  result live, and committing adds a new entry (the recalled one stays). Unlike Edit, no caret
  is shown until the user taps or clicks. The recall is an item of the tape (`Recalled`), so
  keys after it can still be edited with the recalled numbers. Entries without a tape (a
  calculation longer than 400 commands) still recall only the result.
- **Where calculations come back.** The history isn't persisted (as in Windows), so there is no
  restart case and the tapes stay in memory only. A mode switch keeps each mode's history with
  its tapes. A memory recall brings a value, not a calculation: it stays a computed value (on
  the display it can be copied and edited, as after MR).
- **Screen readers:** the display says "Editing %1" while the caret is shown, and the menu item
  has the name "Edit". The expression line stays one text node: its numbers aren't new Tab
  stops, so the Tab order is unchanged. While one of its numbers is edited or an operator is
  selected, its name adds "Editing %1" or "Selected operator %1" and it is read out when that
  changes.
