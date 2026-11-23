//! Real estate cap-rate sensitivity (finite difference).
//!
//! This is *not* a curve DV01 metric. It is a bump-and-reprice sensitivity to
//! an appraisal input: the cap rate. The appraisal discount-rate sensitivity is
//! `real_estate::discount_rate01` (see
//! [`finstack_quant_valuations_equity::rf_component_dv01::RfComponentDv01Calculator`]).
//!
//! **Units**: currency per 1bp cap-rate move,
//! `(PV(cap + h) − PV(cap − h)) / (2h) × 1bp` with `h = rate_bump_bp` from the
//! resolved sensitivities config (1bp by default).

use crate::real_estate::{RealEstateAsset, RealEstateValuationMethod};
use finstack_quant_core::Error;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::sensitivities::bump_config;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Reject configurations whose value does not depend on a cap rate.
///
/// # Arguments
///
/// * `asset` - Real estate asset to check for cap-rate sensitivity applicability.
pub fn ensure_cap_rate01_applicable(asset: &RealEstateAsset) -> finstack_quant_core::Result<()> {
    match asset.valuation_method {
        RealEstateValuationMethod::DirectCap if asset.cap_rate.is_none() => Err(Error::Validation(
            "real_estate::cap_rate01: not applicable for DirectCap without cap_rate".into(),
        )),
        RealEstateValuationMethod::Dcf
            if asset.sale_price.is_some() || asset.terminal_cap_rate.is_none() =>
        {
            Err(Error::Validation(
                "real_estate::cap_rate01: not applicable for DCF with explicit sale_price \
                 or missing terminal_cap_rate"
                    .into(),
            ))
        }
        _ => Ok(()),
    }
}

/// Clone `asset` with its cap rate (DirectCap) or terminal cap rate (DCF)
/// shifted by `bump` (absolute, decimal).
///
/// # Arguments
///
/// * `asset` - Real estate asset to clone and bump.
/// * `bump` - Absolute decimal shift applied to `cap_rate` or `terminal_cap_rate`.
pub fn bumped_cap_rate(
    asset: &RealEstateAsset,
    bump: f64,
) -> finstack_quant_core::Result<RealEstateAsset> {
    let mut bumped = asset.clone();
    let (field, rate) = match bumped.valuation_method {
        RealEstateValuationMethod::DirectCap => ("cap_rate", &mut bumped.cap_rate),
        RealEstateValuationMethod::Dcf => ("terminal_cap_rate", &mut bumped.terminal_cap_rate),
    };
    let Some(value) = rate.as_mut() else {
        return Err(Error::Validation(format!(
            "real_estate::cap_rate01: missing {field}"
        )));
    };
    *value += bump;
    if *value <= 0.0 {
        return Err(Error::Validation(format!(
            "real_estate::cap_rate01: bumped {field} must be positive"
        )));
    }
    Ok(bumped)
}

/// `real_estate::cap_rate01`: PV change per 1bp cap-rate move.
///
/// DirectCap bumps `cap_rate`; DCF bumps `terminal_cap_rate`.
pub(super) struct CapRate01Calculator;

impl MetricCalculator for CapRate01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bump_bp = bump_config::resolve(context)?.rate_bump_bp;
        let bump = bump_bp / 10_000.0;
        let market = context.curves.as_ref();
        let as_of = context.as_of;

        let asset: &RealEstateAsset = context.instrument_as()?;
        ensure_cap_rate01_applicable(asset)?;
        let pv_up = bumped_cap_rate(asset, bump)?.value(market, as_of)?.amount();
        let pv_down = bumped_cap_rate(asset, -bump)?
            .value(market, as_of)?
            .amount();

        Ok((pv_up - pv_down) / (2.0 * bump_bp))
    }
}
