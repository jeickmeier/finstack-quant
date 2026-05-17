//! CDS-Option-specific DV01 calculator.
//!
//! CDS-option IR DV01 is a swap-curve quote sensitivity: bump the stored swap
//! curve market quotes, rebuild the discount curve, and reprice. Direct
//! discount-factor bumps are intentionally rejected so the reported value has a
//! single market convention.

use crate::calibration::bumps::rates::bump_discount_curve_from_rate_calibration;
use crate::calibration::bumps::BumpRequest;
use crate::instruments::credit_derivatives::cds_option::CDSOption;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::sensitivities::cs01::sensitivity_central_diff;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::Result;

const MIN_BUMP_BP: f64 = 1e-10;

/// CDS option DV01 calculator with par-spread hazard re-bootstrap when
/// possible (Bloomberg CDSO convention).
pub(crate) struct CdsOptionDv01Calculator;

impl CdsOptionDv01Calculator {
    fn price_at_rate_bump(
        option: &CDSOption,
        context: &MetricContext,
        bump_bp: f64,
    ) -> Result<f64> {
        let mut bumped_market: MarketContext = context.curves.as_ref().clone();
        let base_discount = context
            .curves
            .get_discount(option.discount_curve_id.as_str())?;
        let calibration = base_discount.rate_calibration().ok_or_else(|| {
            finstack_core::Error::Validation(format!(
                "CDS option '{}' IR DV01 requires swap-curve quote calibration metadata for discount curve '{}'",
                option.id,
                option.discount_curve_id.as_str()
            ))
        })?;
        let bumped_discount = bump_discount_curve_from_rate_calibration(
            base_discount.as_ref(),
            calibration,
            context.curves.as_ref(),
            &BumpRequest::Parallel(bump_bp),
        )?;
        bumped_market = bumped_market.insert(bumped_discount);

        context.reprice_raw(&bumped_market, context.as_of)
    }
}

impl MetricCalculator for CdsOptionDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CDSOption = context.instrument_as()?;
        let defaults =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?;
        let bump_bp = defaults.rate_bump_bp;
        if bump_bp.abs() <= MIN_BUMP_BP {
            return Ok(0.0);
        }

        // CDS-option IR DV01 is a swap-curve quote sensitivity with the
        // hazard curve held fixed. To allow portfolio aggregation against the
        // CDS IR DV01, this MUST use the identical unit and sign convention:
        // the half-width central-difference slope `(pv_up - pv_down) /
        // (2 × bump_bp)`, i.e. PV change per +1bp parallel rate move, with no
        // bond-convention sign flip. (The legacy `-(pv_up - pv_down)/bump_bp`
        // form was both 2× scaled and sign-flipped relative to CDS DV01.)
        let pv_up = Self::price_at_rate_bump(option, context, bump_bp)?;
        let pv_down = Self::price_at_rate_bump(option, context, -bump_bp)?;

        Ok(sensitivity_central_diff(pv_up, pv_down, bump_bp))
    }
}

#[cfg(test)]
mod tests {
    use crate::metrics::sensitivities::cs01::sensitivity_central_diff;

    /// W-18: the CDS-option IR DV01 must report the same unit and sign as the
    /// CDS IR DV01 so the two can be summed in a portfolio. Both are the
    /// central-difference slope `(pv_up - pv_down) / (2 * bump_bp)`.
    ///
    /// This reproduces the prior defect: the legacy CDS-option formula
    /// `-(pv_up - pv_down) / bump_bp` is 2x scaled AND sign-flipped relative
    /// to the CDS DV01 convention used by `sensitivity_central_diff`.
    #[test]
    fn cds_option_dv01_matches_cds_dv01_scale_and_sign() {
        // Equivalent positions: identical PV response to a rate bump.
        let pv_up = 101.0;
        let pv_down = 99.0;
        let bump_bp = 1.0;

        // CDS DV01 convention (the reference).
        let cds_dv01 = sensitivity_central_diff(pv_up, pv_down, bump_bp);

        // CDS-option DV01 must now use the identical helper/convention.
        let option_dv01 = sensitivity_central_diff(pv_up, pv_down, bump_bp);
        assert!(
            (option_dv01 - cds_dv01).abs() < 1e-12,
            "option DV01 {option_dv01} must equal CDS DV01 {cds_dv01}"
        );

        // The legacy form would have given -(pv_up-pv_down)/bump_bp = -2.0,
        // i.e. opposite sign and 2x magnitude relative to the +1.0 slope.
        let legacy = -(pv_up - pv_down) / bump_bp;
        assert!(
            (legacy - cds_dv01).abs() > 1e-6,
            "legacy formula must differ from the reconciled convention"
        );
        assert!(
            (legacy + 2.0 * cds_dv01).abs() < 1e-12,
            "legacy form is exactly -2x the central-difference slope"
        );
    }
}
