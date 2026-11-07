//! CalcCraft's engines, without a user interface: the calculator of the Standard, Scientific
//! and Programmer modes (`engine`), date calculation (`dates`), the unit converters
//! (`converter`), regional formats (`locale`) and the Graphing mode's math (`graphing`).
//! The app (`calculator`) and the MCP server (`calccraft-mcp`) build on it.

pub mod cancel;
pub mod converter;
pub mod dates;
pub mod engine;
pub mod graphing;
pub mod locale;
#[cfg(test)]
mod monkey;
