//! The MCP server: the tools of `tools.rs` with their schemas (from the parameter and result
//! types), annotations and descriptions. Every tool is read-only and idempotent, and a
//! calculation error is a tool result marked `isError`, with a message the agent can act on.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use calccraft_core::cancel;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use tokio::sync::Semaphore;
use tokio::task::JoinError;

use crate::tools::{self, *};

/// How long one calculation may take: factorials, huge powers and the function analysis
/// can be slow, and no input should hang the server. The slowest inputs found take about
/// half a second.
const BUDGET: Duration = Duration::from_secs(10);

/// How many calculations run at once; more calls wait for their turn, so that a client
/// can't keep every core busy.
const WORKERS: usize = 4;

static PERMITS: Semaphore = Semaphore::const_new(WORKERS);

const INSTRUCTIONS: &str = "CalcCraft's calculators, modeled on the Windows Calculator: \
exact arithmetic with 32 significant digits (calculate), programmer integers in hex, dec, oct \
and bin with word sizes (programmer), unit conversion with Windows' factors (convert_units), \
date differences and date arithmetic (date_difference, date_add), and the Graphing mode's \
function analysis (analyze_function). Use them instead of calculating in your head. Numbers \
use '.' as the decimal point and no digit groups; dates are YYYY-MM-DD. Errors say what to \
change.";

/// Runs a tool's work on a blocking thread within `BUDGET`.
async fn run<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    run_within(BUDGET, work).await
}

/// Runs a tool's work on a blocking thread within `budget`, so that a slow calculation
/// neither stalls the server nor hangs it, and a panic becomes an error. When the time is
/// up, the calculation's cancellation flag (`calccraft_core::cancel`) stops it, and this
/// waits until its thread is free before it answers and lets the next calculation run.
async fn run_within<T: Send + 'static>(
    budget: Duration,
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let _permit = PERMITS.acquire().await.expect("the semaphore stays open");
    let flag = Arc::new(AtomicBool::new(false));
    let mut thread = tokio::task::spawn_blocking({
        let flag = flag.clone();
        move || cancel::with_flag(flag, work)
    });
    let failed = |error: JoinError| {
        let message = error
            .try_into_panic()
            .ok()
            .and_then(|panic| {
                panic
                    .downcast_ref::<&str>()
                    .map(|text| text.to_string())
                    .or_else(|| panic.downcast_ref::<String>().cloned())
            })
            .unwrap_or_else(|| "no message".into());
        eprintln!("calccraft-mcp: a calculation panicked: {message}");
        Err("The calculation failed: an internal error, logged on the server's stderr".into())
    };
    match tokio::time::timeout(budget, &mut thread).await {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => failed(error),
        Err(_) => {
            flag.store(true, Ordering::Relaxed);
            let start = Instant::now();
            let _ = thread.await;
            eprintln!(
                "calccraft-mcp: a calculation ran out of its {} ms and was stopped after {} ms \
                 more",
                budget.as_millis(),
                start.elapsed().as_millis()
            );
            Err(format!(
                "The calculation took longer than {} and was stopped: try smaller numbers or a \
                 shorter expression",
                if budget >= Duration::from_secs(1) {
                    format!("{} seconds", budget.as_secs())
                } else {
                    format!("{} ms", budget.as_millis())
                }
            ))
        }
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
            + - * / ^, % or mod (the remainder, NOT percent: 200 % 15 is 5, and x % 0 is x, \
            as in Windows), parentheses, |x|, x!, √x, pi, e, and sin cos tan \
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
            << >>) and one-bit rotations ROL(x) ROR(x), with the Programmer mode's precedence; \
            ^ is XOR, there's no power. Each number has to fit the word (-128 to 127 in dec \
            for a byte, up to FF in hex). Results wrap around in two's complement as in \
            Windows (FFFFFFFFFFFFFFFF + 1 is 0); a shift by the word size or more is 'Result \
            not defined'. At most 25 nested parentheses and pending operations. Returns the \
            result in all four radixes.",
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

#[cfg(test)]
mod tests {
    use super::*;

    /// About half a second in a release build, much more in a debug build: the slowest
    /// formula found, functions of tiny arguments.
    fn slow() -> CalculateParams {
        CalculateParams {
            expression: vec!["asinh(1e-9999)"; 30].join("+"),
            angle: AngleUnit::Degrees,
            mode: CalcMode::Scientific,
        }
    }

    /// The review's I2: the timeout dropped the answer, but the thread calculated on.
    #[tokio::test]
    async fn a_timeout_stops_the_calculation() {
        let start = Instant::now();
        let full = tools::calculate(&slow()).unwrap();
        let uncancelled = start.elapsed();
        assert_eq!(full.result, "3.e-9998");

        let start = Instant::now();
        let error = run_within(Duration::from_millis(20), || tools::calculate(&slow()))
            .await
            .unwrap_err();
        // The answer comes once the thread is free again, long before the calculation
        // would have ended.
        let stopped = start.elapsed();
        assert_eq!(
            error,
            "The calculation took longer than 20 ms and was stopped: try smaller numbers or a \
             shorter expression"
        );
        assert!(stopped < uncancelled / 2, "{stopped:?} of {uncancelled:?}");
    }

    #[tokio::test]
    async fn the_server_answers_after_a_timeout() {
        for _ in 0..WORKERS + 2 {
            let error = run_within(Duration::from_millis(5), || tools::calculate(&slow())).await;
            assert!(error.unwrap_err().contains("was stopped"));
        }
        let params = CalculateParams {
            expression: "1+1".into(),
            angle: AngleUnit::Degrees,
            mode: CalcMode::Scientific,
        };
        assert_eq!(
            run(move || tools::calculate(&params)).await.unwrap().result,
            "2"
        );
    }

    #[tokio::test]
    async fn a_panic_is_an_error() {
        let error = run(|| -> Result<(), String> { panic!("a bug") })
            .await
            .unwrap_err();
        assert!(error.starts_with("The calculation failed"), "{error}");
    }
}
