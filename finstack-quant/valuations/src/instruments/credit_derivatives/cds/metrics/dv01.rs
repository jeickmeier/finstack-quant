//! CDS-specific DV01 calculator.
//!
//! CDS rate risk is a cross-curve sensitivity when the credit curve is stored
//! with the market par spreads used to build it: after a rate-curve bump, the
//! hazard curve must be re-bootstrapped from unchanged CDS spreads. This matches
//! Bloomberg-style IR DV01 for CDS screens.

use super::{hazard_with_deal_quote, market_doc_clause};
use crate::calibration::bumps::hazard::bump_hazard_spreads_with_doc_clause_and_valuation_convention;
use crate::calibration::bumps::BumpRequest;
use crate::instruments::credit_derivatives::cds::CreditDefaultSwap;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::sensitivities::cs01::sensitivity_central_diff;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::market_data::bumps::BumpSpec;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;
use std::sync::Arc;

/// CDS DV01 calculator with par-spread hazard re-bootstrap when possible.
pub(crate) struct CdsDv01Calculator;

impl CdsDv01Calculator {
    fn price_at_rate_bump(
        cds: &CreditDefaultSwap,
        context: &MetricContext,
        bump_bp: f64,
        rebootstrap_hazard: bool,
    ) -> Result<f64> {
        let mut bumped_market: MarketContext = context.curves.as_ref().clone();
        let base_discount = context
            .curves
            .get_discount(cds.premium.discount_curve_id.as_str())?;
        if let Some(calibration) = base_discount.rate_calibration() {
            let bumped_discount = context.bump_discount_rate_quotes_cached(
                base_discount.as_ref(),
                calibration,
                &BumpRequest::Parallel(bump_bp),
            )?;
            bumped_market = bumped_market.insert(bumped_discount.as_ref().clone());
        } else {
            bumped_market.apply_curve_bump_in_place(
                &cds.premium.discount_curve_id,
                BumpSpec::parallel_bp(bump_bp),
            )?;
        }

        if rebootstrap_hazard {
            let base_hazard = context
                .curves
                .get_hazard(cds.protection.credit_curve_id.as_str())?;
            let recalibrated = bump_hazard_spreads_with_doc_clause_and_valuation_convention(
                base_hazard.as_ref(),
                &bumped_market,
                &BumpRequest::Parallel(0.0),
                Some(&cds.premium.discount_curve_id),
                Some(market_doc_clause(cds)),
                Some(cds.valuation_convention),
            )?;
            bumped_market = bumped_market.insert(recalibrated);
        }

        context.reprice_raw(&bumped_market, context.as_of)
    }
}

impl MetricCalculator for CdsDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: CreditDefaultSwap = context.instrument_as::<CreditDefaultSwap>()?.clone();
        let defaults =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?;
        let bump_bp = defaults.rate_bump_bp;
        if bump_bp.abs() <= 1e-10 {
            return Ok(0.0);
        }

        let original_curves = Arc::clone(&context.curves);
        let result = (|| {
            let hazard = original_curves.get_hazard(cds.protection.credit_curve_id.as_str())?;
            if let Some(quote_hazard) = hazard_with_deal_quote(&cds, hazard.as_ref())? {
                context.curves = Arc::new(original_curves.as_ref().clone().insert(quote_hazard));
            }

            let hazard = context
                .curves
                .get_hazard(cds.protection.credit_curve_id.as_str())?;
            let rebootstrap_hazard = hazard.par_spread_points().next().is_some();

            let pv_up = Self::price_at_rate_bump(&cds, context, bump_bp, rebootstrap_hazard)?;
            let pv_down = Self::price_at_rate_bump(&cds, context, -bump_bp, rebootstrap_hazard)?;

            Ok(sensitivity_central_diff(pv_up, pv_down, bump_bp))
        })();
        context.curves = original_curves;
        result
    }
}
