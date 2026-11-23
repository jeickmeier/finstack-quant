//! Taylor-expansion P&L attribution.
//!
//! Decomposes P&L into risk-factor contributions using first-order sensitivities
//! computed via bump-and-reprice:
//!
//!   ΔP&L ≈ Σ DV01ᵢ × Δrateᵢ + Σ Fwd01ₖ × Δfwdₖ + Σ CS01ⱼ × Δspreadⱼ + vega × Δvol + theta
//!
//! Optionally includes second-order (gamma/convexity) terms:
//!
//!   + ½ Σ Gammaᵢ × Δrateᵢ² + ½ CsGamma × Δspread² + ½ Volga × Δvol²
//!
//! The FX-exposure factor is the exception: rather than a sensitivity × move
//! product it is isolated by repricing with the T₀ FX matrix restored (the same
//! restore-and-reprice technique the parallel methodology uses), so cross-
//! currency FX P&L is attributed instead of falling into the residual.
//!
//! Inflation curves, base-correlation curves, market scalars (spots, dividends,
//! published inflation indices, commodity price curves), and model-parameter
//! snapshots use the same restore-and-reprice isolation. The isolated P&L of
//! each family already includes that family's higher-order effects, so they
//! do not take a separate `include_gamma` term.
//!
//! This is complementary to the waterfall (full-reval) approach: it produces a
//! factor-level explained/unexplained decomposition without sequential market
//! state construction.

pub(crate) mod engine;
mod factors;
#[cfg(test)]
pub(crate) mod tests;

pub(crate) use engine::TaylorAttributionConfig;
pub(crate) use engine::attribute_pnl_taylor;
