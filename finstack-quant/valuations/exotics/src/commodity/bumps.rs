//! Commodity price-curve bumping helpers.

/// Market with the price curve `curve_id` shifted in parallel by `pct` of each
/// forward (`0.01` = +1%), the bump the commodity forward-delta and gamma
/// calculators reprice under.
///
/// # Arguments
///
/// * `market` - Market holding the price curve; returned bumped, the input is unchanged.
/// * `curve_id` - Identifier of the `PriceCurve` to shift.
/// * `pct` - Relative shift as a decimal fraction of each forward; negative shifts down.
///
/// # Errors
///
/// Returns an error when `curve_id` is not in `market` or the bump is rejected.
pub(crate) fn bump_price_curve_pct(
    market: &finstack_quant_core::market_data::context::MarketContext,
    curve_id: &str,
    pct: f64,
) -> finstack_quant_core::Result<finstack_quant_core::market_data::context::MarketContext> {
    use finstack_quant_core::market_data::bumps::{
        BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump,
    };
    market.bump([MarketBump::PriceCurve {
        id: finstack_quant_core::types::CurveId::new(curve_id),
        spec: BumpSpec {
            bump_type: BumpType::Parallel,
            mode: BumpMode::Multiplicative,
            units: BumpUnits::Percent,
            value: pct * 100.0,
        },
    }])
}
