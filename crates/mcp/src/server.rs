//! The MCP server: the tools of `tools.rs` with their schemas (from the parameter and result
//! types), annotations and descriptions. Every tool is read-only and idempotent, and a
//! calculation error is a tool result marked `isError`, with a message the agent can act on.

use std::time::Duration;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};

use crate::tools::{self, *};

/// How long one calculation may take: factorials, huge powers and the function analysis
/// can be slow, and no input should hang the server.
const BUDGET: Duration = Duration::from_secs(10);

const INSTRUCTIONS: &str = "CalcCraft's calculators, modeled on the Windows Calculator: \
exact arithmetic with 32 significant digits (calculate), programmer integers in hex, dec, oct \
and bin with word sizes (programmer), unit conversion with Windows' factors (convert_units), \
date differences and date arithmetic (date_difference, date_add), and the Graphing mode's \
function analysis (analyze_function). Use them instead of calculating in your head. Numbers \
use '.' as the decimal point and no digit groups; dates are YYYY-MM-DD. Errors say what to \
change.";

/// Runs a tool's work on a blocking thread within `BUDGET`, so that a slow calculation
/// neither stalls the server nor hangs it, and a panic becomes an error.
async fn run<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    match tokio::time::timeout(BUDGET, tokio::task::spawn_blocking(work)).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err("The calculation failed".into()),
        Err(_) => Err(format!(
            "The calculation took longer than {} seconds",
            BUDGET.as_secs()
        )),
    }
}

#[derive(Clone)]
pub struct CalcCraft {
    tool_router: ToolRouter<Self>,
}

impl Default for CalcCraft {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl CalcCraft {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        title = "Calculate exactly",
        description = "Evaluates a formula exactly, as the Windows Calculator's Scientific mode \
            does: operator precedence, 32 significant digits, its error messages \
            ('Cannot divide by zero', 'Invalid input', 'Overflow'). Takes numbers (1.5, 2e-3), \
            + - * / ^, % or mod (remainder), parentheses, |x|, x!, √x, pi, e, and sin cos tan \
            sec csc cot with their inverses (asin …) and hyperbolic forms (sinh, asinh …), \
            sqrt, cbrt, root(x, n), log (base 10), log(b, x), ln, exp, abs, floor, ceil. \
            Functions need parentheses: sin(30). -2^2 is -4. Returns the value, and the exact \
            fraction when there is one.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn calculate(
        &self,
        Parameters(params): Parameters<CalculateParams>,
    ) -> Result<Json<CalculateResult>, String> {
        run(move || tools::calculate(&params)).await.map(Json)
    }

    #[tool(
        title = "Programmer calculator",
        description = "Integer arithmetic as the Windows Calculator's Programmer mode does it: \
            numbers in hex, dec, oct or bin, a word size of 64, 32, 16 or 8 bits in two's \
            complement, + - * / % (remainder), AND OR XOR NAND NOR NOT, shifts (LSH, RSH or \
            << >>) and one-bit rotations ROL(x) ROR(x), with the Programmer mode's precedence. \
            Returns the result in all four radixes.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn programmer(
        &self,
        Parameters(params): Parameters<ProgrammerParams>,
    ) -> Result<Json<ProgrammerResult>, String> {
        run(move || tools::programmer(&params)).await.map(Json)
    }

    #[tool(
        title = "Convert units",
        description = "Converts a value between units with the Windows Calculator's unit \
            converters, factors and rounding: volume, length, weight, temperature, energy, \
            area, speed, time, power, data, pressure and angle. Units by English name or \
            abbreviation (km, mi, °C, gallon, hp, GB); abbreviations are case-sensitive where \
            it matters: GB, MB, KB are bytes (decimal, KiB MiB GiB binary), Gb, Mb, Kb bits. \
            Gallons, cups and horsepower are US ones unless named (UK). Only the converters' \
            own units exist; an unknown name gets the close matches. Also returns the \
            converter's 'About equal to' values.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn convert_units(
        &self,
        Parameters(params): Parameters<ConvertParams>,
    ) -> Result<Json<ConvertResult>, String> {
        run(move || tools::convert_units(&params)).await.map(Json)
    }

    #[tool(
        title = "Difference between dates",
        description = "The difference between two dates (YYYY-MM-DD) in years, months, weeks \
            and days and in total days, with the Windows Calculator's rules.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn date_difference(
        &self,
        Parameters(params): Parameters<DateDifferenceParams>,
    ) -> Result<Json<DateDifferenceResult>, String> {
        run(move || tools::date_difference(&params)).await.map(Json)
    }

    #[tool(
        title = "Add or subtract from a date",
        description = "Adds years, months and days (0-999 each) to a date (YYYY-MM-DD), or \
            subtracts them, with the Windows Calculator's rules: a month added to January 31 \
            gives the end of February.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn date_add(
        &self,
        Parameters(params): Parameters<DateAddParams>,
    ) -> Result<Json<DateAddResult>, String> {
        run(move || tools::date_add(&params)).await.map(Json)
    }

    #[tool(
        title = "Analyze a function",
        description = "The key features of a function of x, as the Windows Calculator's \
            Graphing mode analyzes them: domain, range, intercepts, minima, maxima, inflection \
            points, asymptotes, parity, period and monotonicity, with exact values (fractions, \
            multiples of pi, square roots) where it finds them. Takes 'y = 1/x', 'x^2 - 4x + 3', \
            'sin(x)'. Numeric, within -1000 to 1000; not a computer algebra system.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn analyze_function(
        &self,
        Parameters(params): Parameters<AnalyzeParams>,
    ) -> Result<Json<AnalyzeResult>, String> {
        run(move || tools::analyze_function(&params))
            .await
            .map(Json)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for CalcCraft {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new("calccraft-mcp", env!("CARGO_PKG_VERSION"))
                    .with_title("CalcCraft"),
            )
            .with_instructions(INSTRUCTIONS)
    }
}
