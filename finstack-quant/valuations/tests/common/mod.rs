//! Equity-option builders for the umbrella test suite.
//!
//! Market-context builders, fixtures, tolerances and assertions live in
//! `finstack_quant_valuations_test_support`; this module keeps only the
//! helpers that construct an `EquityOption`.

// Allow unused items in test utilities - they're available for future tests
#![allow(dead_code, unused_imports)]

pub mod builders;

pub use builders::*;

/// Request a native equity Greek without entering selected-model registry dispatch.
pub fn direct_equity_greek(
    option: &finstack_quant_valuations::instruments::equity::equity_option::EquityOption,
    market: &finstack_quant_core::market_data::context::MarketContext,
    as_of: finstack_quant_core::dates::Date,
    kind: finstack_quant_valuations::instruments::OptionGreekKind,
) -> finstack_quant_core::Result<f64> {
    use finstack_quant_valuations::instruments::{
        GreekBumps, OptionGreekKind, OptionGreeksProvider, OptionGreeksRequest,
    };
    let result = option.option_greeks(
        market,
        as_of,
        &OptionGreeksRequest {
            greek: kind,
            base_pv: None,
            bumps: GreekBumps::default(),
        },
    )?;
    let value = match kind {
        OptionGreekKind::Delta => result.delta,
        OptionGreekKind::Gamma => result.gamma,
        OptionGreekKind::Vega => result.vega,
        OptionGreekKind::Theta => result.theta,
        OptionGreekKind::Rho => result.rho_bp,
        _ => None,
    };
    value.ok_or_else(|| finstack_quant_core::Error::Validation("native Greek absent".into()))
}
