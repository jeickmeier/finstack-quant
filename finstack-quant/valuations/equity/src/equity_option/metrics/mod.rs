//! Equity option metrics module.
//!
//! Splits equity option metrics into focused calculators per greek and
//! registers them with the `MetricRegistry`. Calculators reuse the pricing
//! engine helpers to ensure consistency between PV and greeks.

mod dividend_risk;
mod register;
pub(crate) mod selected_model;
mod spot_greek_decay;

pub(crate) use register::register_equity_option_metrics;
use register::spot_bump_decimal;
