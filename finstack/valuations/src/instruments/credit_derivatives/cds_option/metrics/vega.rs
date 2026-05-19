//! Bloomberg-screen vega for [`CDSOption`].
//!
//! Vega(1%) is the change in option premium for a `+0.01` (one absolute
//! percentage point) shift in the lognormal spread volatility, computed
//! as a one-sided forward difference through the canonical Bloomberg
//! CDSO numerical-quadrature pricer (DOCS 2055833 §2.5).

use crate::instruments::credit_derivatives::cds_option::pricer::{
    resolve_sigma, synthetic_underlying_cds,
};
use crate::instruments::credit_derivatives::cds_option::{bloomberg_quadrature, CDSOption};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::Result;

const VEGA_VOL_BUMP: f64 = 0.01;

/// Vega calculator for credit options on CDS spreads.
pub(crate) struct VegaCalculator;

impl MetricCalculator for VegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CDSOption = context.instrument_as()?;
        vega(option, &context.curves, context.as_of)
    }
}

/// Bloomberg CDSO Vega(1%) — one-sided forward difference of the
/// canonical Bloomberg quadrature NPV on a `+0.01` lognormal-vol bump.
pub(crate) fn vega(
    option: &CDSOption,
    curves: &MarketContext,
    as_of: finstack_core::dates::Date,
) -> Result<f64> {
    option.validate_supported_configuration()?;
    let base_sigma = resolve_sigma(option, curves, as_of)?;
    let bumped_sigma = base_sigma + VEGA_VOL_BUMP;
    if bumped_sigma > super::super::types::MAX_IMPLIED_VOL {
        return Err(finstack_core::Error::Validation(format!(
            "vega bump pushes implied vol above ceiling: base={base_sigma} bumped={bumped_sigma}"
        )));
    }
    let cds = synthetic_underlying_cds(option, as_of)?;
    let pv_base = bloomberg_quadrature::npv(option, &cds, curves, base_sigma, as_of)?.amount();
    let pv_bumped = bloomberg_quadrature::npv(option, &cds, curves, bumped_sigma, as_of)?.amount();
    Ok(pv_bumped - pv_base)
}
