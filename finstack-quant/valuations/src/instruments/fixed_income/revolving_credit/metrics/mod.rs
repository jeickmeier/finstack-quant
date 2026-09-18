//! Metrics module for revolving credit facilities.
//!
//! Provides both standard metrics (PV, DV01, Theta, BucketedDV01, CS01) and
//! facility-specific metrics (utilization rate, available capacity, weighted average
//! cost, and the draw option cost of stochastic facilities).

pub(crate) mod available_capacity;
pub(crate) mod cs01;
pub(crate) mod draw_option_cost;
pub(crate) mod exposure;
pub(crate) mod oid_eir;
pub(crate) mod quotes;
pub(crate) mod utilization_rate;
pub(crate) mod weighted_average_cost;

pub(crate) use available_capacity::AvailableCapacityCalculator;
pub(crate) use draw_option_cost::DrawOptionCostCalculator;
pub(crate) use utilization_rate::UtilizationRateCalculator;
pub(crate) use weighted_average_cost::ApproxWeightedAverageCostCalculator;

use crate::instruments::RevolvingCredit;
use crate::metrics::MetricRegistry;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;

/// Drawn balance on the valuation date: `drawn_amount` is the balance at the
/// simulation anchor in both modes and deterministic events are future-only,
/// so the helper only replays events dated on `as_of` itself (none can be).
fn drawn_balance_as_of(
    facility: &RevolvingCredit,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    if facility.is_deterministic() {
        super::cashflow_engine::calculate_drawn_balance_at_date(facility, as_of, as_of)
    } else {
        Ok(facility.drawn_amount)
    }
}

/// Register all revolving credit metrics with the registry.
///
/// Registers both standard metrics (PV, DV01, Theta, BucketedDV01, CS01) and
/// facility-specific metrics (utilization rate, available capacity, weighted average cost).
pub(crate) fn register_revolving_credit_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::RevolvingCredit,
        metrics: [
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::RevolvingCredit,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            // CS01: with a replayable credit curve, rebootstrap it after
            // bumping its par spreads; with an analyst-built (knot) curve,
            // bump the hazard rates directly. With no credit curve, survival
            // is 1.0, so fall back to the market-standard z-spread bump. See
            // `metrics::sensitivities::cs01_z_spread`.
            (Cs01, crate::metrics::ZSpreadParallelCs01::<
                crate::instruments::RevolvingCredit,
            >::hazard_when_credit_curve()),
            (BucketedCs01, crate::metrics::ZSpreadBucketedCs01::<
                crate::instruments::RevolvingCredit,
            >::hazard_when_credit_curve()),
            // Theta is now registered universally in metrics::standard_registry()
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::RevolvingCredit,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
            // Quote metrics on the settlement schedule (LSTA convention: a
            // quote applies to the drawn balance plus accrued).
            (DiscountMargin, quotes::DiscountMarginCalculator),
            (Ytm, quotes::YtmCalculator),
        ]
    }

    // Register facility-specific metrics with custom IDs
    use crate::metrics::MetricId;
    use std::sync::Arc;

    registry.register_metric(
        MetricId::custom("utilization_rate"),
        Arc::new(UtilizationRateCalculator),
        &[InstrumentType::RevolvingCredit],
    )?;

    registry.register_metric(
        MetricId::custom("available_capacity"),
        Arc::new(AvailableCapacityCalculator),
        &[InstrumentType::RevolvingCredit],
    )?;

    registry.register_metric(
        MetricId::custom("weighted_average_cost"),
        Arc::new(ApproxWeightedAverageCostCalculator),
        &[InstrumentType::RevolvingCredit],
    )?;

    registry.register_metric(
        MetricId::custom("draw_option_cost"),
        Arc::new(DrawOptionCostCalculator),
        &[InstrumentType::RevolvingCredit],
    )?;

    registry.register_metric(
        MetricId::custom("all_in_rate"),
        Arc::new(quotes::AllInRateCalculator),
        &[InstrumentType::RevolvingCredit],
    )?;

    registry.register_metric(
        MetricId::custom("accrued_interest"),
        Arc::new(quotes::AccruedInterestCalculator),
        &[InstrumentType::RevolvingCredit],
    )?;

    registry.register_metric(
        MetricId::custom("price_from_dm"),
        Arc::new(quotes::PriceFromDmCalculator),
        &[InstrumentType::RevolvingCredit],
    )?;

    registry.register_metric(
        MetricId::custom("exposure_at_default"),
        Arc::new(exposure::ExposureAtDefaultCalculator),
        &[InstrumentType::RevolvingCredit],
    )?;

    registry.register_metric(
        MetricId::custom("expected_loss"),
        Arc::new(exposure::ExpectedLossCalculator),
        &[InstrumentType::RevolvingCredit],
    )?;

    registry.register_metric(
        MetricId::custom("oid_eir_amortization"),
        Arc::new(oid_eir::OidEirAmortizationCalculator),
        &[InstrumentType::RevolvingCredit],
    )?;
    Ok(())
}
