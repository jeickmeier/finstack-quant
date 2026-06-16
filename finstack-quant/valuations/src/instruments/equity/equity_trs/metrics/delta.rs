//! Equity delta calculator for equity TRS.

use crate::instruments::common_impl::parameters::trs_common::TrsSide;
use crate::instruments::equity::equity_trs::EquityTotalReturnSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::{Error, Result};

/// Calculates delta to the underlying equity index level.
///
/// Delta measures the sensitivity of the TRS value to changes in the underlying equity level.
/// For equity TRS, delta ≈ notional × contract_size / spot.
pub(crate) struct EquityDeltaCalculator;

impl MetricCalculator for EquityDeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let trs: &EquityTotalReturnSwap = context.instrument_as()?;

        let spot = match context.curves.get_price(&trs.underlying.spot_id)? {
            MarketScalar::Unitless(v) => *v,
            MarketScalar::Price(p) => p.amount(),
        };

        if spot.abs() < 1e-10 {
            return Err(Error::Validation(
                "Spot price too small for delta calculation".into(),
            ));
        }

        let delta = trs.notional.amount() * trs.underlying.contract_size / spot;

        Ok(match trs.side {
            TrsSide::ReceiveTotalReturn => delta,
            TrsSide::PayTotalReturn => -delta,
        })
    }
}
