# Research: editing the entry and the expression with a cursor

Status: research only, no product code changed. Written 2026-11-18 for task 39036547.

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
