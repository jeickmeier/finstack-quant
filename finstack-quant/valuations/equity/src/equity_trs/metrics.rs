//! Risk metrics for Equity Total Return Swaps.
//!
//! This module provides equity TRS-specific risk metrics:
//! - **IndexDelta**: Lifecycle-aware spot sensitivity with reset fixings held constant
//! - **Dividend01**: Sensitivity to dividend yield changes
//! - **ParSpread**: Spread that makes NPV = 0
//! - **FinancingAnnuity**: Discounted notional-weighted financing accrual sum

use crate::dividend01::dividend01_central_diff;
use crate::equity_trs::EquityTotalReturnSwap;
use finstack_quant_core::Error;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::instruments::TrsEngine;
use finstack_quant_valuations_core::metrics::bump_scalar_price;
use finstack_quant_valuations_core::metrics::sensitivities::config as sens_config;
use finstack_quant_valuations_core::metrics::MetricRegistry;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_valuations_core::pricer::InstrumentType;

/// Registers all equity TRS metrics with the metric registry.
///
/// # Arguments
/// * `registry` — Metric registry to add equity TRS metrics to
pub(crate) fn register_equity_trs_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::EquityTotalReturnSwap,
        metrics: [
            // Equity TRS specific metrics
            (Dividend01, Dividend01Calculator),
            (IndexDelta, EquityDeltaCalculator),
            (ParSpread, ParSpreadCalculator),
            (FinancingAnnuity, FinancingAnnuityCalculator),
            // DV01 for financing leg sensitivity
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }

    Ok(())
}

/// Calculates the financing annuity for an equity TRS.
///
/// The annuity is the discounted notional-weighted accrual sum used by the
/// par-spread solve. Multiplying it by `0.0001` gives the PV of one basis point
/// of financing spread.
pub(crate) struct FinancingAnnuityCalculator;

impl MetricCalculator for FinancingAnnuityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let trs: &EquityTotalReturnSwap = context.instrument_as()?;
        trs.financing_annuity(context.curves.as_ref(), context.as_of)
    }
}

/// Calculates delta to the underlying equity index level.
///
/// Delta measures the sensitivity of the actual lifecycle-aware TRS PV to the
/// underlying equity level. Contractual reset fixings are held constant while
/// the live spot is bumped.
pub(crate) struct EquityDeltaCalculator;

impl MetricCalculator for EquityDeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let spot_bump = sens_config::resolve(context)?.spot_bump_decimal;
        let trs: &EquityTotalReturnSwap = context.instrument_as()?;

        let scalar = context.curves.get_price(&trs.underlying.spot_id)?;
        let spot = finstack_quant_valuations_core::metrics::scalar_numeric_value(scalar);
        if !spot.is_finite() || spot.abs() < 1e-10 {
            return Err(Error::Validation(
                "Spot price too small for delta calculation".into(),
            ));
        }

        // If the trade is fresh and no initial fixing was supplied, freeze the
        // contractual level before bumping the live market spot. Otherwise a
        // naive reprice would move both the driver and the contract anchor.
        let mut frozen = trs.clone();
        if frozen.initial_level.is_none() {
            frozen.initial_level = Some(spot);
        }
        let up = bump_scalar_price(context.curves.as_ref(), &trs.underlying.spot_id, spot_bump)?;
        let down = bump_scalar_price(context.curves.as_ref(), &trs.underlying.spot_id, -spot_bump)?;
        let pv_up = frozen.value(&up, context.as_of)?.amount();
        let pv_down = frozen.value(&down, context.as_of)?.amount();
        Ok((pv_up - pv_down) / (2.0 * spot * spot_bump))
    }
}

/// Dividend risk (Dividend01) calculator for equity TRS.
///
/// Dividend01 is the change in PV for a 1bp (0.0001) move in dividend yield;
/// see [`crate::dividend01`] for the finite-difference
/// contract. For equity TRS, dividend yield affects the forward price of the
/// underlying equity, which impacts the total return leg value.
///
/// The dividend-yield scalar is accepted as either `Unitless` or `Price`
/// (matching the equity-option calculator); earlier revisions rejected a
/// `Price`-typed scalar here.
pub(crate) struct Dividend01Calculator;

impl MetricCalculator for Dividend01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let trs: &EquityTotalReturnSwap = context.instrument_as()?;
        dividend01_central_diff(trs, trs.underlying.div_yield_id.as_ref(), context)
    }
}

// Par spread calculator for equity TRS.

/// Calculates the par spread for an equity TRS (spread that makes NPV = 0).
///
/// The par spread solves `NPV_receiver = 0`:
///
/// ```text
/// NPV = PV(total_return) - PV(financing_float) - s * Annuity = 0
/// s_par = (PV(total_return) - PV(financing_float)) / Annuity
/// ```
///
/// where `PV(financing_float)` is the financing leg PV excluding the spread
/// component.
pub(crate) struct ParSpreadCalculator;

impl MetricCalculator for ParSpreadCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::FinancingAnnuity]
    }

    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let annuity = context.computed_metric(&MetricId::FinancingAnnuity)?;
        let trs: &EquityTotalReturnSwap = context.instrument_as()?;
        let curves = context.curves.as_ref();
        let tr_pv = trs.pv_total_return_leg(curves, context.as_of)?;
        TrsEngine::par_spread_bp(
            tr_pv,
            &trs.financing_leg,
            &trs.schedule,
            trs.notional,
            curves,
            context.as_of,
            annuity,
        )
    }
}
