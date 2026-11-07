# calccraft-mcp

An [MCP](https://modelcontextprotocol.io) server that lets AI agents (Claude, VS Code
Copilot, Cursor, Goose …) use CalcCraft's calculators instead of doing math in their heads.
It runs CalcCraft's own engines (`crates/core`): exact big rationals, the Windows Calculator's
rules and error messages, offline, and without running any code. The plan and the reasoning
behind it are in [`docs/mcp-server-plan.md`](../../docs/mcp-server-plan.md), and the fixes
after its review in [`docs/mcp-fixes.md`](../../docs/mcp-fixes.md).

It works locally and only over stdin and stdout: it opens no port, reads no files and no
environment variables, starts no programs, and collects and sends no data. Nothing but the
protocol goes to stdout; its few log lines go to stderr.

## Tools

Every tool is read-only and idempotent (its annotations say so). It returns structured JSON
that matches its output schema, with the same JSON as text for older clients. A calculation
that fails is a tool result marked `isError`, with a message that says what to change.

| Tool | Example call | Result |
| --- | --- | --- |
| `calculate` | `{"expression": "1234.56 / 7"}` | `{"result": "176.36571428571428571428571428571", "fraction": "30864/175"}` |
| | `{"expression": "sin(pi/6)", "angle": "radians"}` | `{"result": "0.5"}` |
| | `{"expression": "2 + 3 * 4", "mode": "standard"}` | `{"result": "20"}`: Windows' Standard mode calculates as you type |
| `programmer` | `{"expression": "FF AND 3C", "radix": "hex", "word_size": "byte"}` | `{"hex": "3C", "dec": "60", "oct": "74", "bin": "00111100", …}` |
| `convert_units` | `{"category": "length", "value": "5", "from": "km", "to": "mi"}` | `{"value": "3.106856", "from": "Kilometers", "to": "Miles", "about_equal_to": […]}` |
| `date_difference` | `{"from": "2026-01-01", "to": "2026-10-18"}` | `{"months": 9, "weeks": 2, "days": 3, "total_days": 290, "description": "9 months, 2 weeks, 3 days", …}` |
| `date_add` | `{"date": "2026-01-31", "months": 1}` | `{"date": "2026-02-28", "long_date": "Saturday, February 28, 2026"}` |
| `analyze_function` | `{"equation": "x^2 - 4x + 3"}` | `{"domain": "{x ∈ ℝ}", "range": "{y ∈ [−1, ∞)}", "minima": ["(2, −1)"], "x_intercepts": "1, 3", …}` |

`calculate` takes ordinary math:
- numbers (`1.5`, `2e-3`), `+ - * / ^`, and `%` or `mod` (the remainder, with the divisor's
  sign, as the Scientific mode's `%` key);
- parentheses, `|x|`, `x!`, `√x`, `pi` and `e`;
- functions with parentheses: `sin`, `cos`, `tan`, `sec`, `csc`, `cot`, their inverses
  (`asin` or `arcsin`) and hyperbolic forms (`sinh`, `asinh`), `sqrt`, `cbrt`, `root(x, n)`,
  `log` (base 10), `log(b, x)`, `ln`, `exp`, `abs`, `floor`, `ceil` and `fact`.

Angles are in degrees unless `angle` says otherwise, `-2^2` is −4, and `2pi` and `3(4 + 1)`
multiply. Numbers use `.` as the decimal point, without digit groups, and dates are
`YYYY-MM-DD`.

`%` is **not** percent: as the Scientific mode's `%` key, it is the remainder with the
divisor's sign (`200 % 15` is 5, `-10 % 3` is 2), and `x % 0` is x, as in Windows.

`programmer` works in two's complement of the word size, as Windows' Programmer mode does: a
result that doesn't fit wraps around (`FFFFFFFFFFFFFFFF + 1` is 0 in a QWORD,
`9223372036854775807 + 1` is −9223372036854775808), and a shift by the word size or more is
`Result not defined`. It has no power operator: `^` is XOR. Each number you type has to fit
the word, though (see below).

`convert_units` takes the unit names and abbreviations of the app's converters, and some
common forms of them: `gallon` (US), `cup` (US), `hp` (US horsepower), `kB`, `KiB`, `Kibit`,
`m2`, `km/h`, `kph`, `metre`, `litre` and the like. Abbreviations are case-sensitive where
case matters: `GB` is gigabytes and `Gb` gigabits, `MB` megabytes and `Mb` megabits. A name the
converters don't have is an error that lists the closest valid names, then all of them. Only
the converters' own units exist: there's no parsec, torr or hectopascal.

`analyze_function` is numeric, as the Graphing mode's analysis is (x from −1000 to 1000, 6
significant digits where it finds no exact form): treat its values as a strong hint, not a
proof.

## Limits and errors

Every tool checks its input and answers within a time budget. Whatever is wrong, the agent
gets a tool result marked `isError` with a message that says what to change; the server
never crashes or hangs on an input.

| What | Limit | Error |
| --- | --- | --- |
| Parameters | only those of the tool's schema (`additionalProperties: false`) | `unknown field 'word', expected one of 'expression', 'radix', 'word_size'` |
| `calculate` formula | 512 characters, 100 nesting levels | `The formula is longer than 512 characters …`, `The formula nests too deeply …` |
| Numbers | the engine's: `Overflow` beyond 10^±9999, `3250!` and more, `Invalid input` for powers too large to hold exactly (`9^9^9`) | Windows' messages |
| `calculate` in `standard` mode | 512 characters, 100 operands, 16 digits per number (Windows' paste rules) | `Invalid input: … at most 16 digits …` |
| `programmer` expression | 512 characters; at most 25 open parentheses and pending operations, the engine's (and Windows') own limit | `The expression nests more than 25 levels …` |
| `programmer` numbers | each must fit the word size: QWORD −9223372036854775808 to 9223372036854775807 in DEC, up to `FFFFFFFFFFFFFFFF` in HEX (any digit count, leading zeros allowed) | `… doesn't fit into a Qword …` |
| `convert_units` value | a plain number of at most 15 digits, negative only for temperature, power and angle | `Invalid input: …`, `… can't be negative …` |
| `date_add` | 0 to 999 years, months and days; dates from 1601-01-01 to 9999-12-31 | `Date out of Bound` |
| `analyze_function` equation | 2048 characters, the Graphing mode's nesting limits | the Graphing mode's messages |
| Time | 10 seconds per call, at most 4 calculations at a time (more wait their turn) | `The calculation took longer than 10 seconds …` |

When a calculation runs out of time, the server stops it: the evaluators check a cancellation
flag between operations and at every evaluation of the analyzed function, so the thread and
its memory are free again within milliseconds, and the next calls are answered as usual. A
panic (a bug) becomes `The calculation failed …` and is logged to stderr.

## Build

```bash
cargo build --release -p calccraft-mcp
```

The server is `target/release/calccraft-mcp` (`calccraft-mcp.exe` on Windows). It speaks MCP
over stdin and stdout, so an MCP client starts it. `calccraft-mcp --version` and `--help` work
from a terminal.

## Connect it

Use the full path to the built server in each of these.

- **Claude Code:**

  ```bash
  claude mcp add calccraft -- /path/to/calccraft-mcp
  ```

- **Claude Desktop:** add it to `claude_desktop_config.json` (Settings → Developer → Edit
  Config), then restart Claude:

  ```json
  { "mcpServers": { "calccraft": { "command": "C:\\path\\to\\calccraft-mcp.exe" } } }
  ```

- **VS Code** (`.vscode/mcp.json`):

  ```json
  { "servers": { "calccraft": { "type": "stdio", "command": "/path/to/calccraft-mcp" } } }
  ```

- **Cursor** (`~/.cursor/mcp.json`): the same `mcpServers` entry as Claude Desktop.
- **MCP Inspector**, to try the tools by hand:

  ```bash
  npx @modelcontextprotocol/inspector /path/to/calccraft-mcp
  ```

## Tests

```bash
cargo test -p calccraft-mcp
```

The unit tests in `src/tools.rs` check each tool's results and errors. `tests/stdio.rs`
starts the built server and speaks MCP to it over stdio with rmcp's own client: the handshake,
the tool list with its schemas and annotations, a call to every tool, and error results.

## Status

This is the MVP of the plan's phases 1–3. It isn't packaged yet: an MCP Bundle for Claude
Desktop, builds in the releases, and a listing in the MCP Registry are phase 4.
