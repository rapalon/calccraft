# MCP server: fixes after the review

The review of `calccraft-mcp` (branch `mc/a76fc9e9`, `docs/mcp-review.md`, of
`feature/mcp-server` at `1fc5e34`) found two blockers, four important problems and some minor
ones. This lists each finding, the fix and the tests that cover it. The finding numbers are
this task's: I4 here is the review's I5 (unit names), and I5 here is the review's I4 (the
branch behind `main`).

The before and after results come from `node crates/mcp/scripts/repro.mjs <server>`, which
runs the review's reproduction cases over stdio as an MCP client does. "Before" is the merge
of `feature/mcp-server` into `main` (`d873854`), as a release build unless it says debug.

## Blockers

### B1. `programmer`: numbers of 39 or more digits overflowed `u128`

Before: `99999999999999999999999999999999999999999999` (44 nines) gave `999999999999999999` in a
release build and panicked in a debug build (`attempt to add with overflow`, reported as `The
calculation failed`); 2^128 gave `3402823669209384634`.

Fix: `type_number` no longer computes the value in a machine integer. It drops leading zeros,
refuses a number with more digits than the word can have before looking at its value, and
builds the value with `checked_mul` and `checked_add` in `u128`, which can't overflow after
that check; then it compares the value with the word's range. A negative decimal number is
typed with its sign, as Windows' ± key does while typing, so the range is the word's signed
range: −128 to 127 in a BYTE (this also fixes M1). The error names the number, the word size
and its range in the radix and in hex and dec.

Results of operations: the tool does no arithmetic of its own on them. The engine computes
with exact big rationals, which can't overflow, and truncates each displayed result to the
word, as Windows does (`TruncateNumForIntMath`). So a result that doesn't fit wraps around in
two's complement (`FFFFFFFFFFFFFFFF + 1` is 0, `9223372036854775807 + 1` is
−9223372036854775808, `4294967296 * 4294967296` is 0 in a QWORD): that is the Programmer
mode's defined behavior and stays, documented in the tool's description and the README. A
shift by the word size or more is `Result not defined`. The Programmer mode has no power
operator (`^` is XOR), and the Scientific mode's powers go through the engine's own limits
(`Overflow`, `Invalid input`).

Tests: `tools::tests::programmer_rejects_numbers_beyond_u128` (the review's repro, 2^128,
and 40, 100 and 512 digits in every radix), `programmer_word_boundaries` (the largest and
smallest value of each word size in DEC, one above and one below, and the largest unsigned
value in HEX, OCT and BIN and one above), `programmer_results_wrap_like_the_engine`
(wrapping results compared with the engine fed key by key). The stdio test
`errors_are_tool_results_the_agent_can_read` runs the repro through the protocol, and CI runs
it in debug builds, where the old code panicked.

### B2. `convert_units`: `GB`, `MB`, `KB`, `TB` were read as bits

Before: `1 GB → MiB` gave `119.2093` (from Gigabits) instead of `953.6743`; `1 KB → B` gave
Kilobits to Bits.

Fix: units are looked up in order of certainty, and the first step that finds exactly one
unit wins: the unit's own abbreviation, name or key with its case (`GB` is Gigabytes, `Gb`
Gigabits, as in the converter's table), the aliases (below), then the same without case, but
only if that names one unit. A name that is still ambiguous is an error that lists the
candidates. The unit table itself is the GUI converter's (`calccraft_core::converter`), not a
copy: bytes are 8 bits, `KB` `MB` `GB` `TB` … are decimal bytes (1000), `KiB` `MiB` `GiB`
`TiB` … binary bytes (1024), and the bit units are separate (`Kb`, `Ki` for kibibits, …).

Tests: `tools::tests::data_units_match_the_gui_converter` (every unit of the Data category
by abbreviation and by name, converted to every other one and compared with the GUI
converter driven directly), `every_category_matches_the_gui_converter` (the same for every
unit of all 12 categories, by abbreviation, name and key, to the category's first and last
unit), `byte_and_bit_abbreviations` (`GB`/`Gb`, `MB`/`Mb`, `KB`/`Kb`, `TB`/`Tb` and the
values `953.6743`, `1000`, `125`).

## Important

### I1. `programmer`: more than 25 nested parentheses gave a wrong result

Before: 40 levels of `(…(1+1)*2+1)*2…` gave `100663324` instead of `3298534883326`, without
an error; 28 levels `100663300`.

Fix: the engine keeps at most 25 open parentheses and 25 pending operations, as Windows does
(`MAXPRECDEPTH`), and ignores a key beyond them; the GUI's keys then do nothing. The tool now
checks before each `(` and each operator whether the engine has room for it
(`Calculator::open_parentheses`, and the new read-only `Calculator::pending_operations`), and
returns `The expression nests more than 25 levels of parentheses and pending operations, the
Programmer mode's limit: …` instead of a wrong result. The limit stays: it is the engine's,
and the GUI has it too. `calculate` has its own evaluator with 100 levels.

Tests: `tools::tests::programmer_nesting_limit` (24 levels give `50331646`, the deepest
accepted nesting matches the formula 3·2ⁿ − 2, 25 and more nested levels and 40 levels are
errors, and the review's 28-level repro is an error).

### I2. The 10 s timeout didn't stop the calculation

Before: `tokio::time::timeout` dropped only the `JoinHandle`; the `spawn_blocking` thread ran
on, and nothing limited how many ran at once. No input that is slow today was found (the
slowest, 30 × `asinh(1e-9999)`, takes about 0.5 s), so the risk was the guarantee, not a
known input.

Fix:
- `calccraft_core::cancel`: a cancellation flag per thread. The MCP server installs one for
  each calculation; the GUI never does, so its checks find none and change nothing. The
  formula evaluator (`engine::expression`) checks it before every operand, and the function
  analysis at every evaluation of f, f′ and f″ (`Fun::f`, `d1`, `d2`, which then give NaN, so
  the rest of the analysis ends at once). The programmer and converter tools are bounded by
  their input limits.
- `server::run_within`: on timeout it sets the flag, waits for the thread to end (within
  milliseconds), logs to stderr and returns `The calculation took longer than 10 seconds and
  was stopped`. A semaphore allows at most 4 calculations at a time; a call waits for its
  turn before its budget starts. A panic is logged to stderr with its message.
- Hard limits stay as they were: 512 characters, 100 nesting levels, the engine's
  `Overflow` beyond 10^±9999 and `3250!`, powers held exactly up to about 100000 bits, the
  analysis' 2048 characters, depth 100 and 1000-node derivatives.

Tests: `server::tests::a_timeout_stops_the_calculation` (a calculation of about 0.5 s with a
budget of 20 ms returns the timeout error, and its thread is done well before the
calculation would have been), `server::tests::the_server_answers_after_a_timeout`,
`cancel::tests` in core (a cancelled formula and analysis end at once; without a flag
nothing changes), and the stdio test `pathological_inputs_answer_quickly` (huge powers,
factorials and the longest formulas answer with an error or a result within the budget, and
the next call is answered).

### I3. Unknown parameters were ignored

Before: `programmer {"expression": "-1", "word": "byte"}` silently used a QWORD
(`FFFFFFFFFFFFFFFF`); `"angles": "radians"` and `"extra": true` were ignored.

Fix: every parameter type has `#[serde(deny_unknown_fields)]`, and its schema says
`additionalProperties: false`. A call with an unknown field is an `isError` result that names
it and lists the valid ones: `unknown field 'word', expected one of 'expression', 'radix',
'word_size'`. The schemas also declare the limits the code checks (`maxLength`, `maximum`).

Tests: `tools::tests::unknown_fields_are_refused` (every tool), and the stdio test
`unknown_parameters_are_errors_that_name_them` (the review's three cases, and that every
input schema has `additionalProperties: false`).

### I4. Common unit names weren't recognized

Before: `gallon`, `cup` and `hp` were `Unknown unit`.

Fix: an alias table, only for units the GUI converters have (each alias is checked by a test
to name an existing unit): `gallon` and `gal` (US), `cup`, `pint`, `quart`, `tsp`, `tbsp`,
`fl oz` (US; the UK ones by their own names), `hp` and `horsepower` (US), `kph`, `kmh`,
`knot`, `kB`, `Kibit`, `Mibit` …, `m2`, `ft3` …, `metre`, `litre` and other British spellings,
`degree`, `celsius`, `tonne` and more. A name that matches nothing is an error that first
lists up to 5 close matches (by edit distance and by containment), then every unit of the
category. Units that the converters don't have (parsec, torr, hectopascal …) stay errors.

Tests: `tools::tests::unit_aliases` (each alias converts as its unit does),
`aliases_name_existing_units`, `unknown_units_list_close_matches`.

### I5. The branch was behind `main`, with a conflict in `Cargo.toml`

Fix: `main` (`4ea709b`, v1.3.2) merged with `origin/feature/mcp-server` (`1fc5e34`) in
`d873854`, without rewriting the branch. The conflict: `main` had raised `[package] version`
to 1.3.2, while the branch had moved the version into `[workspace.package]` (1.3.0), which
the app, the core and the server inherit. Kept the branch's workspace (`members =
["crates/core", "crates/mcp"]`, `version.workspace = true`) with main's version, 1.3.2, so all
three are 1.3.2 and `calccraft-mcp --version` says 1.3.2. `Cargo.lock` and everything else
merged without conflicts (main's six commits touch `src/app*`, `android/`, `README.md` and
`LICENSE`, not the modules the branch moved).

## Minor

| # | Finding | Status |
| --- | --- | --- |
| M1 | `-128` refused in a BYTE | Fixed with B1: negative decimal numbers are typed with their sign. Test: `programmer_word_boundaries`. |
| M2 | Standard mode: a misleading message for a 17-digit number; `2 + 3 * 4 = =` gave 80 | Fixed: the message names Windows' paste limits (16 digits per number, 100 operands, numbers and + - * / only), and every trailing `=` is removed. Test: `standard_mode_errors`. |
| M3 | No JSON-RPC answer to a line that isn't JSON | Not fixed: rmcp's transport drops such a line, and the server goes on; changing it means replacing rmcp's stdio transport. |
| M4 | The plan and the code disagreed | Fixed in `docs/mcp-server-plan.md` (budget, `word_size`, `fraction`, protocol versions, the missing branch) and in the README (the privacy statement). The plan's snapshot test of `tools/list` and fuzz target are not done (see below). |
| M5 | `%` is the remainder, `x mod 0` is x | Documented in the `calculate` tool's description and the README. |
| M6 | Questionable analysis results inherited from core (`ln(x)/sqrt(x)`, `x^x`) | Not changed: they're the GUI's analysis too, and core stays as it is. The tool's description and the README say that the analysis is numeric. To report as engine issues. |
| M7 | Tests only for happy paths | Fixed: tests for every finding above, through the tools and through the protocol. |
| M8 | `CATEGORIES[category as usize]`; `include_str!` of the locales; the programmer tokenizer in the MCP crate | The index is replaced by a lookup by category. The `include_str!` (no crates.io release is planned) and moving the tokenizer to core are not done. |

Not done, besides the above: a snapshot test of `tools/list`, a fuzz target for the
parsers (`cargo-fuzz` needs a nightly toolchain), and packaging (phase 4 of the plan).
