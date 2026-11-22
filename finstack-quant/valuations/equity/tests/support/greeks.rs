//! Direct native-provider Greek helper for equity option tests (also
//! `#[path]`-included by the umbrella crate's `tests/common/mod.rs`).

/// Request a native equity Greek without entering selected-model registry dispatch.
pub fn direct_equity_greek(
    option: &finstack_quant_valuations_equity::equity_option::EquityOption,
    market: &finstack_quant_core::market_data::context::MarketContext,
    as_of: finstack_quant_core::dates::Date,
    kind: finstack_quant_valuations_core::instruments::OptionGreekKind,
) -> finstack_quant_core::Result<f64> {
    use finstack_quant_valuations_core::instruments::{
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
