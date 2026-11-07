# calccraft-mcp

An [MCP](https://modelcontextprotocol.io) server that lets AI agents (Claude, VS Code
Copilot, Cursor, Goose …) use CalcCraft's calculators instead of doing math in their heads.
It runs CalcCraft's own engines (`crates/core`): exact big rationals, the Windows Calculator's
rules and error messages, offline, and without running any code. The plan and the reasoning
behind it are in [`docs/mcp-server-plan.md`](../../docs/mcp-server-plan.md).

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

Limits:
- 512 characters per formula;
- 10 seconds per call;
- the engine's own limits (`Overflow` beyond 10^±9999, `3250!`).

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
