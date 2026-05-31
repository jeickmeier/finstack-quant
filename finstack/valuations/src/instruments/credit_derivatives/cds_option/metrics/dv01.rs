//! CDS-Option-specific DV01 calculator.
//!
//! CDS-option IR DV01 is a swap-curve quote sensitivity: when the discount
//! curve stores its swap-curve calibration quotes, bump those quotes, rebuild
//! the curve, and reprice (Bloomberg CDSO convention). When the curve carries
//! no calibration metadata (e.g. a directly-specified discount curve), fall
//! back to a parallel bump of the discount factors — identical to the sibling
//! [`CdsDv01Calculator`](crate::instruments::credit_derivatives::cds::metrics)
//! so CDS and CDS-option IR DV01 share a single unit and sign convention and
//! can be aggregated across a portfolio regardless of how the curve was built.

use crate::calibration::bumps::rates::bump_discount_curve_from_rate_calibration;
use crate::calibration::bumps::BumpRequest;
use crate::instruments::credit_derivatives::cds_option::CDSOption;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::sensitivities::cs01::sensitivity_central_diff;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::market_data::bumps::BumpSpec;
use finstack_core::market_data::context::MarketContext;
use finstack_core::Result;

const MIN_BUMP_BP: f64 = 1e-10;

/// CDS option DV01 calculator: swap-curve quote bump when calibration metadata
/// is present, parallel discount-factor bump otherwise (Bloomberg CDSO
/// convention, hazard curve held fixed).
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
        // Prefer the market-standard swap-quote bump when the curve records its
        // calibration; otherwise bump the discount factors directly so a
        // directly-specified curve still yields a well-defined IR DV01 (matches
        // `CdsDv01Calculator`).
        if let Some(calibration) = base_discount.rate_calibration() {
            let bumped_discount = bump_discount_curve_from_rate_calibration(
                base_discount.as_ref(),
                calibration,
                context.curves.as_ref(),
                &BumpRequest::Parallel(bump_bp),
            )?;
            bumped_market = bumped_market.insert(bumped_discount);
        } else {
            bumped_market.apply_curve_bump_in_place(
                &option.discount_curve_id,
                BumpSpec::parallel_bp(bump_bp),
            )?;
        }

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

// W-18 unit-and-sign reconciliation between CDS-option IR DV01 and CDS IR
// DV01 is exercised at the integration level by
// `tests/instruments/cds_option/test_metrics_registry.rs::
// test_cds_option_dv01_bumps_swap_curve_quotes_and_matches_cds_convention`,
// which prices a CDS option and a CDS on a shared discount curve, bumps
// the swap-curve quotes, and asserts both metrics report the same sign
// and unit. A purely-numeric unit test that calls `sensitivity_central_diff`
// twice with identical inputs adds no coverage on top of that, so it was
// removed.
