//! WASM bindings for P&L attribution across multiple methodologies.
//!
//! # Number safety
//!
//! The structured entry points (`attributePnl`, `attributePnlEnvelope`,
//! `attributePnlMany`) return objects built by `crate::utils::to_js_value`, so
//! counts and metrics (`num_repricings`, residuals, factor P&Ls) arrive as JS
//! numbers; that serializer rejects an integer above `Number.MAX_SAFE_INTEGER`
//! (2^53 − 1) instead of rounding it. Only the `*Json` wire twins carry them
//! inside JSON strings, where `JSON.parse` would silently round a larger
//! integer. Today every count in the attribution surface is bounded by a
//! handful of factors (≤ 12) and a handful of repricings (≤ ~30), well under
//! the safe-integer ceiling.

mod inspection;
mod pnl;

pub use inspection::*;
pub use pnl::*;
