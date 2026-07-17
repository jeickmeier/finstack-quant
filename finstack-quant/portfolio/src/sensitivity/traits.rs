//! Finite-difference and repricing utilities for portfolio sensitivities.
//!
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::{Error, Result};
use finstack_quant_factor_model::sensitivity_matrix::SensitivityMatrix;
use finstack_quant_factor_model::FactorDefinition;
use finstack_quant_valuations::instruments::Instrument;

/// Validate that every position prices in the same native currency.
///
/// The factor sensitivity engines build deltas from raw native-currency PVs
/// and the downstream decomposers column-sum them across positions. Mixing
/// currencies would silently add e.g. USD and EUR DV01s unit-for-unit,
/// violating the workspace no-implicit-cross-currency invariant. This check
/// errors loudly instead of converting; callers with multi-currency
/// portfolios must convert positions to a common base currency upstream.
///
/// # Errors
///
/// Returns [`Error::Validation`] naming the two positions whose pricing
/// currencies differ, or propagates any pricing error from `base_value`.
pub(crate) fn validate_single_currency(
    positions: &[(String, &dyn Instrument, f64)],
    market: &MarketContext,
    as_of: Date,
) -> Result<()> {
    let mut first: Option<(&str, finstack_quant_core::currency::Currency)> = None;
    for (position_id, instrument, _) in positions {
        let currency = instrument.value(market, as_of)?.currency();
        match first {
            None => first = Some((position_id.as_str(), currency)),
            Some((first_id, first_currency)) if first_currency != currency => {
                return Err(Error::Validation(format!(
                    "Factor sensitivity engine requires a single pricing currency: \
                     position '{first_id}' prices in {first_currency} but position \
                     '{position_id}' prices in {currency}; convert positions to a \
                     common base currency before computing factor sensitivities"
                )));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// Engine for computing per-position, per-factor sensitivities.
pub trait FactorSensitivityEngine: Send + Sync {
    /// Compute a sensitivity matrix for `positions` against `factors`.
    fn compute_sensitivities(
        &self,
        positions: &[(String, &dyn Instrument, f64)],
        factors: &[FactorDefinition],
        market: &MarketContext,
        as_of: Date,
    ) -> Result<SensitivityMatrix>;
}
