//! CDS-specific DV01 calculator.
//!
//! CDS rate risk is a cross-curve sensitivity when the credit curve is stored
//! with the market par spreads used to build it: after a rate-curve bump, the
//! hazard curve must be re-bootstrapped from unchanged CDS spreads. This matches
//! Bloomberg-style IR DV01 for CDS screens.

use super::{deal_quote_override, market_doc_clause};
use crate::instruments::credit_derivatives::cds::CreditDefaultSwap;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::sensitivities::cs01::sensitivity_central_diff;
use crate::metrics::{MetricCalculator, MetricContext};
use crate::recalibration::{
    DiscountCurveRecalibrationRequest, HazardRecalibrationAction, HazardRecalibrationRequest,
    QuoteBump,
};
use finstack_quant_core::market_data::bumps::BumpSpec;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;

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
            .get_discount(cds.premium_leg.discount_curve_id.as_str())?;
        if let Some(calibration) = base_discount.rate_calibration() {
            let bumped_discount = context.rebuild_discount_curve(
                DiscountCurveRecalibrationRequest {
                    curve: std::sync::Arc::clone(&base_discount),
                    recipe: calibration.clone(),
                    market: std::sync::Arc::clone(&context.curves),
                    bump: QuoteBump::ParallelBp(bump_bp),
                },
                "dv01",
            )?;
            bumped_market = bumped_market.insert(bumped_discount.as_ref().clone());
        } else {
            bumped_market.apply_curve_bump_in_place(
                &cds.premium_leg.discount_curve_id,
                BumpSpec::parallel_bp(bump_bp),
            )?;
        }

        if rebootstrap_hazard {
            let base_hazard = context
                .curves
                .get_hazard(cds.protection_leg.credit_curve_id.as_str())?;
            let recalibrated = context.rebuild_hazard_curve(
                HazardRecalibrationRequest {
                    hazard: base_hazard,
                    source_market: std::sync::Arc::clone(&context.curves),
                    target_market: std::sync::Arc::new(bumped_market.clone()),
                    discount_curve_id: cds.premium_leg.discount_curve_id.clone(),
                    doc_clause: Some(market_doc_clause(cds)),
                    cds_valuation_convention: Some(cds.valuation_convention),
                    deal_quote_override: deal_quote_override(cds),
                    action: HazardRecalibrationAction::DependencyMarketReplay,
                },
                "dv01",
            )?;
            bumped_market = bumped_market.insert(recalibrated.as_ref().clone());
        }

        context.reprice_raw(&bumped_market, context.as_of)
    }
}

impl MetricCalculator for CdsDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: CreditDefaultSwap = context.instrument_as::<CreditDefaultSwap>()?.clone();
        let defaults = sens_config::from_context_or_default(
            context.get_config(),
            context.get_metric_overrides(),
        )?;
        let bump_bp = defaults.rate_bump_bp;
        if bump_bp.abs() <= 1e-10 {
            return Ok(0.0);
        }

        let hazard = context
            .curves
            .get_hazard(cds.protection_leg.credit_curve_id.as_str())?;
        let rebootstrap_hazard = hazard.hazard_calibration().is_some();

        let pv_up = Self::price_at_rate_bump(&cds, context, bump_bp, rebootstrap_hazard)?;
        let pv_down = Self::price_at_rate_bump(&cds, context, -bump_bp, rebootstrap_hazard)?;

        Ok(sensitivity_central_diff(pv_up, pv_down, bump_bp))
    }
}
