# An MCP server for CalcCraft: pros, cons, use cases and an implementation plan

Written: 2026-10-18 · Starting point: `main` at `1567714` (v1.3.0) · Status: phases 1–3 (the
core crate, the expression front-end and the server with six tools) are built and tested: see
`crates/core`, `crates/mcp` and `crates/mcp/README.md`. A review found problems, fixed as
listed in `docs/mcp-fixes.md`; where this plan and the code differ, `crates/mcp/README.md`
describes what the server does. Phase 4 (packaging) and later are still to do.

## Summary

| Question | Answer |
|---|---|
| What is it? | A small separate program, `calccraft-mcp`, that speaks the [Model Context Protocol](https://modelcontextprotocol.io) and lets AI agents (Claude, VS Code Copilot, Cursor, Goose …) use CalcCraft's engines as tools: exact arithmetic and scientific functions, the Programmer mode, unit conversion, date calculation and function analysis. |
| Why would an agent want it? | Language models guess digits and get long arithmetic wrong; many hosts give them no code execution, and floating-point code has its own errors. CalcCraft computes exactly (big rationals, 32 digits) with the Windows Calculator's rules, offline and without running code. |
| Is it worth it? | **Yes as a small, focused MVP**; it is not a must-have. Its value is correctness, breadth (six calculators in one server) and the function analysis. The main counterweights are agents that already run Python, the many generic calculator servers, and the upkeep of a fast-moving protocol. |
| How much work? | About **3 weeks** for an MVP with six tools and packaging, **5–7 weeks** with graph images and an interactive calculator in the chat (an MCP App). |
| Recommendation | Build the MVP (phases 1–4 below), ship it with each release as an MCP Bundle and in the MCP Registry, and decide on images and the MCP App from how much it's used. A two-day spike can test the idea first (section 8). |

## 1. Why agents need a calculator

- **Models aren't calculators.** They predict digits as text: long multiplications, divisions
  and many-digit numbers come out subtly wrong, and they are confidently wrong.
- **Not every host runs code.** Chat apps, enterprise deployments and IDE assistants often
  have code execution turned off or sandboxed away. An MCP tool works wherever MCP does.
- **Code isn't exact either.** `0.1 + 0.2 - 0.3` is `5.55e-17` in Python's floats; getting exact
  results needs `fractions` or `decimal`, which agents rarely reach for. CalcCraft gives `0`.
- **Determinism and an audit trail.** Every tool call shows its input and its result in the
  conversation, which is easier to check than a model's mental arithmetic or a script.

## 2. Example use cases

Each example shows the tool call an agent would make (section 4.2 defines the tools). The
outputs are illustrative.

1. **Everyday and financial arithmetic, exactly.** "Split €1,234.56 between 7 people."
   `calculate {"expression": "1234.56 / 7"}` → `176.36571428571428571428571428571`, exactly
   `30864/175`. The agent can round as the user wants, knowing the true value.
2. **Checking its own work.** An agent writing a report verifies the totals it computed in its
   reasoning, before it states them.
3. **Scientific values.** `calculate {"expression": "sin(30)", "angle": "degrees"}` → exactly
   `0.5`; `calculate {"expression": "0.5!"}` → `0.88622692545275801364908374167057` (√π/2);
   `tan(90)` → the error `Invalid input`, as in Windows, instead of `1.6e16`.
4. **Programmer questions.** "What is −1 as a 16-bit hex number?", "FF AND 3C?", "rotate
   0x80000000 left". `programmer {"expression": "FF AND 3C", "radix": "hex", "word_size": "byte"}`
   → `3C` and its value in DEC, OCT and BIN, with the word size's two's complement.
5. **Unit conversion with Windows' factors.** `convert_units {"category": "length",
   "value": "5", "from": "km", "to": "mi"}` → `3.106856`, plus "about equal to" values in
   other units.
6. **Date arithmetic with Windows' rules.** `date_difference {"from": "2026-01-01",
   "to": "2026-10-18"}` → `9 months, 2 weeks, 3 days` (290 days). `date_add` handles month
   ends the way Windows does (January 31 + 1 month = February 28).
7. **Homework help and teaching.** `analyze_function {"equation": "y = 1/x"}` → domain
   `{x ≠ 0}`, range `{y ∈ ℝ∖{0}}`, asymptotes `x = 0` and `y = 0`, odd, with exact values where
   they exist. A tutor agent can explain them, and later draw the graph (phase 5).
8. **Answering "what would Windows Calculator show?"** For support, documentation and
   training material: `calculate {"expression": "2 + 3 * 4", "mode": "standard"}` → `20`, since
   the Standard mode calculates as you type, while the Scientific mode gives `14`.
9. **Voice assistants and accessibility.** Spoken questions go to a deterministic tool, and
   the answer comes back as text a screen reader or a voice reads out.
10. **The calculator itself in the chat** (phase 6). An MCP App shows CalcCraft's Graphing
    mode in the conversation, with the agent's equations loaded and sliders the user can drag.

## 3. Pros and cons

| Pros | Cons |
|---|---|
| **Correct results.** Exact big rationals, 32 significant digits, the Windows Calculator's rules and error messages; the same input always gives the same output. | **Agents with code execution can compute too**, and Python with `fractions` can be exact. The server competes with "just write a script". |
| **Breadth in one server.** Arithmetic, scientific functions, programmer integers, 12 unit categories, dates and function analysis, where most calculator servers do only arithmetic. | **Generic calculator servers already exist** (Python, Node, Docker; some exact, some with units), and Wolfram-style services do symbolic math CalcCraft can't. |
| **Safe.** It runs no code and touches no files or network: every tool is read-only and idempotent, which hosts can show with tool annotations. | **Windows semantics can surprise.** The Standard mode ignores operator precedence (`2 + 3 × 4 = 20`) and `%` depends on the operator before it. The server must default to the Scientific mode and say so in its descriptions. |
| **Small and fast.** A native program of a few MB without the GUI, starting in milliseconds, working offline. | **The engine reads keys, not formulas.** Its paste rules accept numbers and `+ − × ÷ ^ % ( )` only, so functions (`sin`, `√`, `log` …) need a new expression front-end: new code and tests. |
| **Mostly reuse.** The engines are UI-free and tested (295 tests and Windows oracle tests); the server is largely glue. | **Not a computer algebra system.** No solving, derivatives, integrals, matrices or statistics; the function analysis is numeric (±1000, 6 digits where no exact form is found). |
| **Visibility for CalcCraft.** A listing in the MCP Registry and the marketplaces built on it brings developers to the project. | **Upkeep.** MCP has had three revisions in about 13 months (2025-06-18, 2025-11-25, 2026-07-28) and rmcp is at 3.x; packaging for four systems and signing add work with every release. |
| **MCP Apps.** The web version could become an interactive calculator inside Claude, VS Code and other hosts. | **Dependencies.** tokio, serde and schemars must stay out of the GUI app, which takes a crate split first. |

## 4. Design

### 4.1 Shape of the server

- **A separate program**, `calccraft-mcp`, built from the same repository. It links only the
  UI-free engines (`engine`, `dates`, `converter`, `locale`, `graphing`), not eframe.
- **stdio transport** for local use, the way Claude Desktop, Claude Code, VS Code and Cursor
  start local servers. Streamable HTTP for a hosted server is optional (phase 7).
- **Stateless tools.** MCP 2026-07-28 has no protocol sessions, and each tool call carries its
  whole input. The calculator's memory and history stay in the app.
- **SDK:** the official Rust SDK, [`rmcp`](https://github.com/modelcontextprotocol/rust-sdk)
  3.x (`server`, `macros`, `transport-io`). rmcp 3.5 answers a client that asks for
  2026-07-28 with 2025-11-25, the newest revision it implements. Tools are methods with `#[tool]`, and their parameters derive
  `schemars::JsonSchema`, so the input schemas come from the Rust types.
- **Every tool:**
  - is annotated `readOnlyHint: true`, `destructiveHint: false`, `idempotentHint: true` and
    `openWorldHint: false`;
  - returns a short text for the model and `structuredContent` that matches its
    `outputSchema`;
  - reports a calculation error as a tool result with `isError: true`, using Windows' own
    messages (`Cannot divide by zero`, `Invalid input`, `Overflow`, `Result is undefined`) and
    a hint, so the agent can correct itself.
- **Locale-neutral.** Numbers use `.` as the decimal point and dates ISO 8601 in and out,
  whatever the user's region. Unit, category and function names are English.
- **Limits:** input of at most 512 characters (the paste limit), unknown parameters refused,
  a time budget of 10 s per call that really stops the calculation (a cancellation flag the
  evaluators check), at most 4 calculations at a time, and each call on a blocking thread
  whose panic becomes an error, so no input can hang or crash the server. Logs go to stderr
  only: stdout belongs to the protocol. `crates/mcp/README.md` lists every limit.

### 4.2 Tools

| Tool | Input | Output | Built on |
|---|---|---|---|
| `calculate` | `expression`; `angle`: `degrees` (default, as in Windows), `radians` or `gradians`; `mode`: `scientific` (default) or `standard` | `result` (32 significant digits), `fraction` (`p/q` when the result is a short exact fraction; no expression line yet) | New exact evaluator (section 4.3); `standard` uses the engine's paste path |
| `programmer` | `expression` in the radix; `radix`: `hex`, `dec`, `oct` or `bin`; `word_size`: `qword`, `dword`, `word` or `byte` | `hex`, `dec`, `oct` and `bin` values, `expression_line` | `Calculator::programmer()`, its paste rules, and `AND`/`OR`/`XOR`/`NOT`/`NAND`/`NOR`, shifts and rotations mapped to the engine's commands |
| `convert_units` | `category` (one of the 12), `value`, `from`, `to` (English names or abbreviations) | `value` (Windows' rounding), `about_equal_to` (the "about equal to" list) | `Converter`: `set_category`, `set_unit1`/`set_unit2`, `paste`, `value2`, `suggestions` |
| `date_difference` | `from`, `to` (ISO dates, 1601–9999) | `years`, `months`, `weeks`, `days`, `total_days`, `description` | `dates::difference`, `days_between`, `describe_difference` |
| `date_add` | `date`; `years`, `months`, `days` (0–999); `subtract` | `date` (ISO) and `long_date` | `dates::add`, `dates::subtract` |
| `analyze_function` | `equation` (as typed in the Graphing mode); `angle` | domain, range, x and y intercepts, minima, maxima, inflection points, asymptotes, parity, period, monotonicity (exact strings where possible), or "too complex" per feature | `Equation::parse`, `analysis::analyze` |
| `plot` (phase 5) | `equations` (1–14), optional ranges, width and height | A PNG image (`image/png` content) and the view used | `plot` sampling plus a new renderer |

**Resources and prompts.** Two optional resources help agents, mainly for discovery:
- `calccraft://units`: the categories and their unit names and abbreviations;
- `calccraft://syntax`: the expression grammar and the function list.

An "explain this function" prompt can pair `analyze_function` with a teaching template.

An example definition, as `tools/list` would show it:

```json
{
  "name": "calculate",
  "title": "Calculate exactly",
  "description": "Evaluates an arithmetic expression exactly, with the Windows Calculator's Scientific mode rules: operator precedence, 32 significant digits, and its error messages. Supports + - * / ^ % (mod), parentheses, sqrt, cbrt, root(x, n), log, ln, log(b, x), sin, cos, tan, sec, csc, cot and their inverses and hyperbolic forms, abs, floor, ceil, n!, pi and e. Angles are in degrees unless 'angle' says otherwise.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "expression": { "type": "string", "maxLength": 512, "description": "For example: 1234.56 / 7, or sin(30) + 2^10" },
      "angle": { "type": "string", "enum": ["degrees", "radians", "gradians"], "default": "degrees" },
      "mode": { "type": "string", "enum": ["scientific", "standard"], "default": "scientific", "description": "standard calculates as you type, without precedence, as the Windows Standard mode does" }
    },
    "required": ["expression"],
    "additionalProperties": false
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "result": { "type": "string" },
      "fraction": { "type": "string" }
    },
    "required": ["result"]
  },
  "annotations": { "readOnlyHint": true, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false }
}
```

### 4.3 The expression front-end

The engine is a port of Windows' key-by-key engine. That is what makes it exact and faithful,
but it takes commands, not formulas, and its paste rules (like Windows') accept only numbers
and `+ − × ÷ ^ % ( )`. Two pieces already exist:

- the Graphing mode's parser (`graphing::parse`) reads full formulas, with functions, implicit
  multiplication, `|x|`, `x!`, `√`, `π`, `log(b, x)` and `root(x, n)`, into an expression tree
  (`graphing::expr::Expr`);
- the Scientific mode's functions (`engine::math`: `power`, `root`, `ln`, `log_base`, `sin` …
  `cosh`, `factorial`) are public and work on exact `Number`s, returning Windows' errors.

The plan is a new `engine::evaluate(expr, angle) -> Result<Number, CalcError>` that walks the
tree with those functions, then formats the result with the engine's `format_with` (32 digits;
today private to `engine.rs`, so the new module needs it made visible). Formulas with `x` or
`y` are refused with a hint to use `analyze_function`. A test compares it with the Scientific
engine fed key by key over many expressions, so both always agree.

### 4.4 Packaging and distribution

| Channel | What | For whom |
|---|---|---|
| **GitHub releases** | `calccraft-mcp` for Windows x64, Linux x64, macOS arm64 and x64, built by `release.yml` next to the app | Everyone; the other channels point here |
| **MCP Bundle (`.mcpb`)** | A zip with the binaries and a `manifest.json`, packed with `mcpb pack` in CI | Claude Desktop: double-click to install, no terminal |
| **MCP Registry** | `server.json` named `io.github.gavrilov/calccraft`, with the `mcpb` package (URL and SHA-256) and optionally `cargo`; published by `mcp-publisher` with GitHub's OIDC from CI | Marketplaces and clients that browse the registry |
| **crates.io** (optional) | `cargo install calccraft-mcp`; its README must show `mcp-name: io.github.gavrilov/calccraft` as visible text, since crates.io strips HTML comments | Rust developers |

How to connect, for the README:

- **Claude Code:**

  ```bash
  claude mcp add calccraft -- calccraft-mcp
  ```

- **VS Code:** a `.vscode/mcp.json` entry.
- **Cursor and other clients:** the usual `"command": "calccraft-mcp"` JSON entry.

## 5. Implementation plan

| Phase | Work | Done when | Estimate |
|---|---|---|---|
| **0. Decisions** | The name (`calccraft-mcp`), the MVP's tools, versioning with the app, the registry namespace | Agreed in an issue | 0.5 day |
| **1. A core crate** | A Cargo workspace: the UI-free modules (`engine`, `dates`, `converter`, `locale`, `graphing`, `icon`) move into `crates/core` (`calccraft-core`). The app depends on it, and its tests move with it. CI and `AGENTS.md` are updated. | `cargo test`, clippy (native, wasm32, Android) and the release build pass unchanged; the app looks and works the same | 2–3 days |
| **2. Expression front-end** | `engine::evaluate` (section 4.3), the parser in a "no variables" mode, English names for errors and units in core (from `locales/en-US.txt`, with `include_str!`), and output formatting | Unit tests for every function and error, the key-by-key comparison test, a fuzz target that never panics | 4–6 days |
| **3. The MCP server** | `crates/mcp` (`calccraft-mcp`) with rmcp 3.x and tokio (current-thread): the six tools, schemas from `schemars` types, annotations, `isError` results, the time budget, `--version`, logs to stderr | Unit tests per tool; an integration test that starts the binary over stdio with rmcp's client and runs initialize → `tools/list` (snapshot) → `tools/call`; checked by hand in the MCP Inspector, Claude Desktop and Claude Code | 3–5 days |
| **4. Packaging** | `release.yml` builds the server for four desktop targets and packs an `.mcpb`; `server.json` and `mcp-publisher` in CI; README section and client configurations | A tagged release installs in Claude Desktop with a double-click, and the registry lists it | 3–4 days |
| **5. Graph images** (optional) | The `plot` tool: plot's polylines drawn with `tiny-skia`, axes and labels with the embedded Inter (via `ab_glyph`), the same colors as the app's graph | Images match the app's graph for the manual-test functions; PNGs stay under ~200 KB | 3–5 days |
| **6. MCP App** (exploratory) | The web build as a `ui://calccraft/graph` resource for a `show_graph` tool: a small postMessage bridge in `index.html` and `web.rs` loads the agent's equations into the Graphing mode | Works in Claude and VS Code; check first that the hosts' sandbox and content security policy allow WebAssembly, and how large a UI resource may be (the wasm is 3.9 MB compressed) | 1–2 weeks |
| **7. Hosted endpoint** (optional) | Streamable HTTP with axum on a small server (Fly.io, Cloud Run), rate limited, no sign-in since the tools are read-only | Reachable at an HTTPS URL; costs and abuse watched | 2–3 days, plus hosting |

**Total:** phases 0–4 (the MVP) take about 2.5–3.5 weeks; with phases 5 and 6, about 5–7
weeks.

## 6. Testing and quality

- **Exactness:** reuse the engine's 295 tests and the Windows oracle tests (`ms_oracle.rs`,
  `monkey.rs`). Compare `evaluate` against the key-by-key engine, so the server and the app
  never disagree.
- **Protocol:**
  - a `tools/list` snapshot test, so names, schemas and their order stay stable (hosts
    cache them);
  - one test per tool, covering success, a calculation error and invalid input;
  - an end-to-end stdio test in CI on Windows, Linux and macOS.
- **Robustness:** a `cargo-fuzz` target for the expression parser, and per-call time budgets
  tested with pathological inputs (`3249!`, `9^9^9`, a 512-character expression).
- **Manual:** the MCP Inspector (`npx @modelcontextprotocol/inspector calccraft-mcp`),
  Claude Desktop with the `.mcpb`, and Claude Code, before each release.

## 7. Risks and open questions

| Risk | Mitigation |
|---|---|
| Little demand: agents use code instead | Ship a small MVP first; watch release downloads and registry installs before phases 5–7 |
| Trademark and endorsement | Name and describe it as CalcCraft ("modeled on the Windows Calculator"), never "Windows Calculator MCP", as in the README |
| Grammar ambiguities: `%` (percent in Windows, modulo here), implicit multiplication, the default angle unit | Document them in the tool description and `calccraft://syntax`; return the parsed expression line so the agent can check what was computed |
| Protocol churn | Track rmcp releases; snapshot tests catch schema changes; target the newest revision and keep one older |
| Unsigned binaries: macOS Gatekeeper and Windows SmartScreen may block a server that Claude Desktop starts | Signing and notarization (Apple Developer Program, US$99 a year), or document the workaround as for the app |
| Slow or huge results | Time budget, output size limits, the engine's own limits (`Overflow` beyond 10^±9999) |
| MCP Apps' limits on WebAssembly or bundle size | Prototype in phase 6 before committing to it |

## 8. Recommended next step

A two-day spike, before phase 1: put the server behind a Cargo feature instead of a crate
split, implement `calculate` (Scientific paste path only) and `convert_units`, and try it in
Claude Code and Claude Desktop with real questions. If agents call it and the answers help,
go on with phases 1–4 as planned.

## Sources

- MCP specification 2026-07-28: <https://modelcontextprotocol.io/specification/latest>, tools
  (structured content, output schemas, annotations, errors, stateless tools):
  <https://modelcontextprotocol.io/specification/2026-07-28/server/tools>
- MCP Apps: <https://modelcontextprotocol.io/extensions/apps/overview>
- Rust SDK (rmcp 3.5.1, 2026-10-05): <https://github.com/modelcontextprotocol/rust-sdk>,
  <https://crates.io/crates/rmcp>
- MCP Bundles: <https://github.com/modelcontextprotocol/mcpb>
- MCP Registry and package types (cargo, mcpb): <https://modelcontextprotocol.io/registry/about>,
  <https://modelcontextprotocol.io/registry/package-types>
- Existing calculator servers, for comparison:
  <https://hub.docker.com/r/denysflnk/calculator-mcp>,
  <https://glama.ai/mcp/servers/@Ankluna72/Math-Calculator-MCP-Server->,
  <https://glama.ai/mcp/servers/@CalcsLive/calcslive-mcp-server/blob/5b8db9f813a3e3b3317e69641307536da6d72289/README.md>
