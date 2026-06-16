//! Equity price shock adapter.

use crate::adapters::traits::ScenarioEffect;
use crate::engine::ExecutionContext;
use crate::error::Result;
use crate::warning::Warning;
use finstack_quant_core::market_data::bumps::{
    BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump,
};
use finstack_quant_core::types::CurveId;

/// Generate effects for an equity-price percent shock.
pub(crate) fn equity_pct_effects(
    ids: &[String],
    pct: f64,
    ctx: &ExecutionContext,
) -> Result<Vec<ScenarioEffect>> {
    let mut effects = Vec::with_capacity(ids.len());
    for id in ids {
        if ctx.market.get_price(id).is_ok() {
            // Additive/Percent on MarketScalar::Price is interpreted as a
            // proportional shift: new_price = old_price * (1 + pct/100).
            // See MarketScalar::apply_bump in finstack_quant_core::market_data::bumps.
            let bump = MarketBump::Curve {
                id: CurveId::from(id.as_str()),
                spec: BumpSpec {
                    mode: BumpMode::Additive,
                    units: BumpUnits::Percent,
                    value: pct,
                    bump_type: BumpType::Parallel,
                },
            };
            effects.push(ScenarioEffect::MarketBump(bump));
        } else {
            effects.push(ScenarioEffect::Warning(Warning::EquityNotFound {
                id: id.clone(),
            }));
        }
    }
    Ok(effects)
}
