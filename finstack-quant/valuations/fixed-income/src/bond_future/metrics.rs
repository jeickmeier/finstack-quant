//! Bond-future pricing and interest-rate metrics.
//!
//! The registered metrics are:
//! - `FuturesPrice`: the lifecycle mark — `terms.quoted_price` when supplied,
//!   otherwise the carry-adjusted forward clean CTD price per 100 face divided
//!   by the resolved basket conversion factor (`BondFuture::fair_price`); the
//!   official `terms.settlement_price` after the last trading date.
//! - `ConversionFactor`: the resolved CTD's unitless factor from the
//!   deliverable basket.
//! - `Dv01`: NPV sensitivity to the configured parallel combined rate-curve
//!   bump.
//! - `BucketedDv01`: NPV sensitivity under configured triangular key-rate
//!   bumps.
//! - `Theta`: held-market change in futures NPV over the requested date roll,
//!   with zero interim contractual cash while the entire horizon precedes
//!   `delivery_start`. The CTD's coupons are not cash receipts of the future.
//!   A positive horizon reaching the delivery window is rejected because the
//!   cash-and-bond exchange requires the typed delivery result. Settled
//!   contracts have zero theta and a zero realized horizon.
//!
//! Conversion-factor scaling follows the valuation formulas:
//! `Model_Price = Forward_Clean_CTD_Percent / Conversion_Factor` and
//! `NPV = Position_Sign × Contracts × Multiplier × (Mark - Entry_Price)`, where
//! the mark is the model price while the contract is live.

use crate::bond_future::BondFuture;
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::{Error, Result};
use finstack_quant_valuations_core::metrics::MetricRegistry;
use finstack_quant_valuations_core::metrics::sensitivities::theta::{
    calculate_theta_date, theta_period,
};
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_valuations_core::pricer::InstrumentType;

/// Register bond-future prices, rate risk, and pre-delivery theta.
///
/// # Arguments
///
/// - `registry`: Metric registry to update for the `BondFuture` instrument type.
pub(crate) fn register_bond_future_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::BondFuture,
        metrics: [
            (FuturesPrice, FuturesPriceCalculator),
            (ConversionFactor, ConversionFactorCalculator),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
            (Theta, ThetaCalculator),
        ]
    };
    Ok(())
}

// Pricing diagnostics for bond futures.

/// Lifecycle-aware current or final futures price for bond futures.
pub(crate) struct FuturesPriceCalculator;

impl MetricCalculator for FuturesPriceCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let future: &BondFuture = context.instrument_as()?;
        future.mark_price(&context.curves, context.as_of)
    }
}

/// CTD conversion factor calculator for bond futures.
pub(crate) struct ConversionFactorCalculator;

impl MetricCalculator for ConversionFactorCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let future: &BondFuture = context.instrument_as()?;
        future.ctd_conversion_factor()
    }
}

/// Held-market theta before physical delivery, without inventing a cash schedule.
pub(crate) struct ThetaCalculator;

impl MetricCalculator for ThetaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let future: &BondFuture = context.instrument_as()?;
        let rolled_date = calculate_theta_date(
            context.as_of,
            theta_period(context),
            Some(future.terms.settlement_date),
        )?;
        let (theta, horizon_days) = if rolled_date <= context.as_of {
            (0.0, 0)
        } else {
            if rolled_date >= future.delivery_start {
                return Err(Error::Validation(format!(
                    "BondFuture theta horizon reaches physical delivery on {}; use the typed delivery result",
                    future.delivery_start
                )));
            }

            let base_pv = context.reprice_money(&context.curves, context.as_of)?;
            let horizon_days = (rolled_date - context.as_of).whole_days();
            let held_market = if future
                .terms
                .get_observed_mark(future.id.as_str(), rolled_date)?
                .is_some()
            {
                // Observed marks do not consume CTD projection inputs.
                context.curves.as_ref().clone()
            } else {
                let ctd = future.ctd_bond.as_ref().ok_or_else(|| {
                    Error::Validation("BondFuture theta requires an embedded CTD bond".into())
                })?;
                // The underlying schedule supplies crossed index observations only.
                // Its coupons belong to the bond holder, not the future holder.
                let schedule = ctd.cashflow_schedule(&context.curves, context.as_of)?;
                finstack_quant_cashflows::fixings::materialize_fixings(
                    &context.curves,
                    [&schedule],
                    context.as_of,
                    rolled_date,
                )?
            };
            let rolled_market = held_market.roll_forward(horizon_days)?;
            let rolled_pv = context.reprice_money(&rolled_market, rolled_date)?;
            (rolled_pv.checked_sub(base_pv)?.amount(), horizon_days)
        };

        context.computed.insert(MetricId::ThetaCarry, 0.0);
        context.computed.insert(MetricId::ThetaRollDown, theta);
        context
            .computed
            .insert(MetricId::ThetaPeriodDays, horizon_days as f64);
        Ok(theta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_valuations_core::metrics::{MetricId, MetricRegistry};

    #[test]
    fn test_metrics_registration() {
        let mut registry = MetricRegistry::new();
        register_bond_future_metrics(&mut registry).expect("bond future metric registration");

        // Verify metrics are registered for BondFuture
        let metrics = registry.metrics_for_instrument(InstrumentType::BondFuture);

        // Verify DV01 is registered
        assert!(
            metrics.contains(&MetricId::Dv01),
            "DV01 metric should be registered for BondFuture"
        );

        // Verify Bucketed DV01 is registered
        assert!(
            metrics.contains(&MetricId::BucketedDv01),
            "BucketedDv01 metric should be registered for BondFuture"
        );
        assert!(
            metrics.contains(&MetricId::FuturesPrice),
            "FuturesPrice metric should be registered for BondFuture"
        );
        assert!(
            metrics.contains(&MetricId::ConversionFactor),
            "ConversionFactor metric should be registered for BondFuture"
        );
        assert!(
            metrics.contains(&MetricId::Theta),
            "pre-delivery Theta metric should be registered for BondFuture"
        );
    }
}
