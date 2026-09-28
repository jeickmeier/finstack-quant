//! Real estate cap-rate sensitivity (finite difference).
//!
//! This is *not* a curve DV01 metric. It is a bump-and-reprice sensitivity to
//! an appraisal input: the cap rate. The appraisal discount-rate sensitivity is
//! `real_estate::discount_rate01` (see
//! [`crate::metrics::RfComponentDv01Calculator`]).
//!
//! **Units**: currency per 1bp cap-rate move,
//! `(PV(cap + h) − PV(cap − h)) / (2h) × 1bp` with `h = rate_bump_bp` from the
//! resolved sensitivities config (1bp by default).

use crate::instruments::equity::real_estate::{
    LeveredRealEstateEquity, RealEstateAsset, RealEstateValuationMethod,
};
use crate::instruments::Instrument;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Error as CoreError;

/// `real_estate::cap_rate01`: PV change per 1bp cap-rate move.
///
/// DirectCap bumps `cap_rate`; DCF bumps `terminal_cap_rate`.
pub(super) struct CapRate01Calculator;

impl CapRate01Calculator {
    /// Reject configurations whose value does not depend on a cap rate.
    fn ensure_applicable(asset: &RealEstateAsset) -> finstack_quant_core::Result<()> {
        match asset.valuation_method {
            RealEstateValuationMethod::DirectCap if asset.cap_rate.is_none() => {
                Err(CoreError::Validation(
                    "real_estate::cap_rate01: not applicable for DirectCap without cap_rate".into(),
                ))
            }
            RealEstateValuationMethod::Dcf
                if asset.sale_price.is_some() || asset.terminal_cap_rate.is_none() =>
            {
                Err(CoreError::Validation(
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
    fn bumped(asset: &RealEstateAsset, bump: f64) -> finstack_quant_core::Result<RealEstateAsset> {
        let mut bumped = asset.clone();
        let (field, rate) = match bumped.valuation_method {
            RealEstateValuationMethod::DirectCap => ("cap_rate", &mut bumped.cap_rate),
            RealEstateValuationMethod::Dcf => ("terminal_cap_rate", &mut bumped.terminal_cap_rate),
        };
        let Some(value) = rate.as_mut() else {
            return Err(CoreError::Validation(format!(
                "real_estate::cap_rate01: missing {field}"
            )));
        };
        *value += bump;
        if *value <= 0.0 {
            return Err(CoreError::Validation(format!(
                "real_estate::cap_rate01: bumped {field} must be positive"
            )));
        }
        Ok(bumped)
    }
}

impl MetricCalculator for CapRate01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bump_bp = sens_config::from_context_or_default(
            context.get_config(),
            context.get_metric_pricing_overrides(),
        )?
        .rate_bump_bp;
        let bump = bump_bp / 10_000.0;
        let market = context.curves.as_ref();
        let as_of = context.as_of;

        let (pv_up, pv_down) = if let Some(asset) = context
            .instrument
            .as_any()
            .downcast_ref::<RealEstateAsset>()
        {
            Self::ensure_applicable(asset)?;
            (
                Self::bumped(asset, bump)?.value(market, as_of)?.amount(),
                Self::bumped(asset, -bump)?.value(market, as_of)?.amount(),
            )
        } else if let Some(levered) = context
            .instrument
            .as_any()
            .downcast_ref::<LeveredRealEstateEquity>()
        {
            Self::ensure_applicable(&levered.asset)?;
            let mut up = levered.clone();
            up.asset = Self::bumped(&levered.asset, bump)?;
            let mut down = levered.clone();
            down.asset = Self::bumped(&levered.asset, -bump)?;
            (
                up.value(market, as_of)?.amount(),
                down.value(market, as_of)?.amount(),
            )
        } else {
            return Err(CoreError::Validation(
                "real_estate::cap_rate01: instrument type mismatch".into(),
            ));
        };

        Ok((pv_up - pv_down) / (2.0 * bump_bp))
    }
}
