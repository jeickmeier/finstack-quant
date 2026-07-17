//! Market inputs implementation used by the convertible subsystem.
//!
use super::ConvertibleBond;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::InputError;
use finstack_quant_core::{Error, Result};

/// Build volatility scalar/surface candidate IDs in pricing precedence order.
pub(super) fn volatility_candidate_ids(bond: &ConvertibleBond) -> Result<Vec<String>> {
    let underlying_id = bond.underlying_equity_id.as_ref().ok_or_else(|| {
        finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
            id: "underlying_equity_id".to_string(),
        })
    })?;

    let mut candidate_ids = Vec::with_capacity(3);
    if let Some(id) = bond.attributes.get_meta("vol_surface_id") {
        candidate_ids.push(id.to_string());
    }
    candidate_ids.push(format!("{underlying_id}-VOL"));
    if let Some(stripped) = underlying_id.strip_suffix("-SPOT") {
        candidate_ids.push(format!("{stripped}-VOL"));
    }
    Ok(candidate_ids)
}

/// Build dividend-yield candidate IDs in the same order used by pricing and metrics.
pub(super) fn dividend_yield_candidate_ids(bond: &ConvertibleBond) -> Result<Vec<String>> {
    let underlying_id = bond.underlying_equity_id.as_ref().ok_or_else(|| {
        finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
            id: "underlying_equity_id".to_string(),
        })
    })?;

    let mut candidate_ids = Vec::with_capacity(3);
    if let Some(id) = bond.attributes.get_meta("div_yield_id") {
        candidate_ids.push(id.to_string());
    }
    candidate_ids.push(format!("{underlying_id}-DIVYIELD"));
    if let Some(stripped) = underlying_id.strip_suffix("-SPOT") {
        candidate_ids.push(format!("{stripped}-DIVYIELD"));
    }
    Ok(candidate_ids)
}

/// Resolve the first unitless dividend yield in the candidate list.
pub(super) fn resolve_dividend_yield(ctx: &MarketContext, bond: &ConvertibleBond) -> Result<f64> {
    let candidate_ids = dividend_yield_candidate_ids(bond)?;
    Ok(resolve_unitless_scalar(ctx, &candidate_ids)?.unwrap_or(0.0))
}

/// Resolve the first available market scalar ID in the dividend candidate list.
pub(super) fn resolve_dividend_yield_market_value_id(
    ctx: &MarketContext,
    bond: &ConvertibleBond,
) -> Result<Option<String>> {
    let candidate_ids = dividend_yield_candidate_ids(bond)?;
    Ok(candidate_ids
        .into_iter()
        .find(|id| ctx.get_price(id.as_str()).is_ok()))
}

fn resolve_unitless_scalar(ctx: &MarketContext, candidate_ids: &[String]) -> Result<Option<f64>> {
    for id in candidate_ids {
        match ctx.get_price(id) {
            Ok(finstack_quant_core::market_data::scalars::MarketScalar::Unitless(value)) => {
                return Ok(Some(*value));
            }
            Ok(_) => {}
            Err(err) => {
                if matches!(err, Error::Input(InputError::NotFound { .. })) {
                    continue;
                }
                return Err(err);
            }
        }
    }
    Ok(None)
}
