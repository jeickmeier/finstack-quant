//! Shared held-position P&L for exact regulatory quote shocks.

use crate::evaluation::{EvaluationProfile, PortfolioEvaluationPlan, PositionInvalidation};
use crate::types::{EntityId, PositionId};
use crate::valuation::PortfolioValuation;
use crate::{Error, Portfolio, Result};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use serde::{Deserialize, Serialize};

/// One signed position-level repricing difference for an exact regulatory quote vertex.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct RegulatoryQuoteReprice {
    /// Position whose risk is represented, after applying its signed quantity.
    pub position_id: PositionId,
    /// Owner of the position within the trading book.
    pub entity_id: EntityId,
    /// Signed scaling from the instrument price to the held position.
    pub position_scale: f64,
    /// Held reporting FX rate in units of reporting currency per native unit.
    /// Absent when the currencies match.
    pub fx_rate: Option<f64>,
    /// Whether the held FX rate was triangulated through the market pivot.
    /// Absent when the currencies match.
    pub fx_triangulated: Option<bool>,
    /// Baseline value on the held valuation basis in the instrument's currency.
    pub center_native: Money,
    /// Value on the same basis after an upward one-basis-point quote replay.
    pub shocked_native: Money,
    /// Signed upward reprice difference in instrument currency per basis point.
    pub pnl_native_per_bp: Money,
    /// Signed upward reprice difference in portfolio currency per one percentage point.
    pub pnl_base_per_pp: f64,
}

pub(super) fn reprice_rows(
    portfolio: &Portfolio,
    shocked_market: &MarketContext,
    pricing: &FinstackConfig,
    center: &PortfolioValuation,
    affected: &[(usize, &PositionId)],
) -> Result<Vec<RegulatoryQuoteReprice>> {
    let mut plan = PortfolioEvaluationPlan::new(pricing);
    let market_state = plan.register_market(shocked_market, center.as_of);
    let portfolio_state = plan.register_portfolio(portfolio);
    let reprice = plan.register_selective_evaluation(
        market_state,
        portfolio_state,
        EvaluationProfile::strict_metrics(&[]),
        center,
        PositionInvalidation::new(affected.iter().map(|(index, _)| *index).collect(), false),
    )?;
    let shocked = plan.execute().take_valuation(reprice)?;

    affected
        .iter()
        .map(|&(_, id)| {
            let old = center
                .position_values
                .get(id)
                .ok_or_else(|| Error::invalid_input(format!("Missing center position '{id}'")))?;
            let new = shocked
                .position_values
                .get(id)
                .ok_or_else(|| Error::invalid_input(format!("Missing shocked position '{id}'")))?;
            if old.fx_rate != new.fx_rate || old.fx_triangulated != new.fx_triangulated {
                return Err(Error::invalid_input(format!(
                    "Regulatory quote replay changed the held FX basis of position '{id}'"
                )));
            }
            let difference = new.value_native.checked_sub(old.value_native)?;
            let held_fx = match old.fx_rate {
                Some(rate) if rate.is_finite() && rate > 0.0 => rate,
                None if old.value_native.currency() == portfolio.base_currency => 1.0,
                _ => {
                    return Err(Error::invalid_input(format!(
                        "Missing valid held FX for position '{id}'"
                    )));
                }
            };
            let per_pp = difference.amount() * held_fx * 100.0;
            if !per_pp.is_finite() {
                return Err(Error::invalid_input(format!(
                    "Non-finite regulatory delta for position '{id}'"
                )));
            }
            Ok(RegulatoryQuoteReprice {
                position_id: id.clone(),
                entity_id: old.entity_id.clone(),
                position_scale: old.metric_scale,
                fx_rate: old.fx_rate,
                fx_triangulated: old.fx_triangulated,
                center_native: old.value_native,
                shocked_native: new.value_native,
                pnl_native_per_bp: difference,
                pnl_base_per_pp: per_pp,
            })
        })
        .collect()
}
