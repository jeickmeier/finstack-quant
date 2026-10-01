//! Equity price shock adapter.

use crate::adapters::traits::ScenarioEffect;
use crate::engine::ExecutionContext;
use crate::error::Result;
use crate::warning::Warning;
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
            effects.push(ScenarioEffect::PriceBump {
                id: CurveId::from(id.as_str()),
                pct,
            });
        } else {
            effects.push(ScenarioEffect::Warning(Warning::EquityNotFound {
                id: id.clone(),
            }));
        }
    }
    Ok(effects)
}
