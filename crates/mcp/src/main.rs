//! `calccraft-mcp`: an MCP server that lets AI agents use CalcCraft's calculators (see
//! docs/mcp-server-plan.md and crates/mcp/README.md). It speaks MCP over stdin and stdout,
//! the way local MCP clients start their servers; anything it logs goes to stderr.

mod english;
mod server;
mod tools;

use rmcp::ServiceExt;

const HELP: &str = "calccraft-mcp: CalcCraft's calculators for AI agents, as an MCP server.

It speaks the Model Context Protocol over stdin and stdout: an MCP client (Claude Desktop,
Claude Code, VS Code, Cursor …) starts it. Its tools: calculate, programmer, convert_units,
date_difference, date_add and analyze_function.

Usage: calccraft-mcp [--help | --version]";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    match std::env::args().nth(1).as_deref() {
        Some("--version" | "-V") => {
            println!("calccraft-mcp {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some("--help" | "-h") => {
            println!("{HELP}");
            return Ok(());
        }
        Some(other) => {
            eprintln!("Unknown argument {other}\n\n{HELP}");
            std::process::exit(2);
        }
        None => {}
    }
    let service = server::CalcCraft::new()
        .serve(rmcp::transport::stdio())
        .await
        .inspect_err(|error| eprintln!("calccraft-mcp couldn't start: {error}"))?;
    service.waiting().await?;
    Ok(())
}
