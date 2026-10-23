//! Metrics module for revolving credit facilities.
//!
//! Provides both standard metrics (PV, DV01, Theta, BucketedDV01, CS01) and
//! facility-specific metrics (utilization rate, available capacity, weighted average
//! cost, and the draw option cost of stochastic facilities).

pub(crate) mod cs01;
pub(crate) mod exposure;
pub(crate) mod oid_eir;
pub(crate) mod quotes;
pub(crate) mod weighted_average_cost;

pub(crate) use weighted_average_cost::ApproxWeightedAverageCostCalculator;

use crate::revolving_credit::types::DrawRepaySpec;
use crate::revolving_credit::RevolvingCredit;
use crate::revolving_credit::RevolvingCreditPricer;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Drawn balance on `date`: the deterministic replay of the events after the
/// anchor `as_of` up to `date`, or the anchor balance `drawn` of a stochastic
/// facility. On `date == as_of` no event is replayed (events are future-only).
fn drawn_at(
    facility: &RevolvingCredit,
    as_of: Date,
    date: Date,
) -> finstack_quant_core::Result<Money> {
    if facility.is_deterministic() {
        super::cashflow_engine::calculate_drawn_balance_at_date(facility, as_of, date)
    } else {
        Ok(facility.drawn)
    }
}

/// Undrawn availability on `as_of` given the `drawn` balance: the commitment
/// in force less drawn and letters of credit outstanding, floored at zero.
fn undrawn_at(facility: &RevolvingCredit, as_of: Date, drawn: f64) -> f64 {
    (facility.commitment_at(as_of).amount() - drawn - facility.lc_outstanding_at(as_of).amount())
        .max(0.0)
}

/// Register all revolving credit metrics with the registry.
///
/// Registers both standard metrics (PV, DV01, Theta, BucketedDV01, CS01) and
/// facility-specific metrics (utilization rate, available capacity, weighted average cost).
pub(crate) fn register_revolving_credit_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::RevolvingCredit,
        metrics: [
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            // CS01: with a replayable credit curve, rebootstrap it after
            // bumping its par spreads; with an analyst-built (knot) curve,
            // bump the hazard rates directly. With no credit curve, survival
            // is 1.0, so fall back to the market-standard z-spread bump. See
            // `metrics::sensitivities::cs01_z_spread`.
            (Cs01, finstack_quant_valuations_core::metrics::ZSpreadParallelCs01::<
                crate::revolving_credit::RevolvingCredit,
            >::hazard_when_credit_curve()),
            (BucketedCs01, finstack_quant_valuations_core::metrics::ZSpreadBucketedCs01::<
                crate::revolving_credit::RevolvingCredit,
            >::hazard_when_credit_curve()),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
            // Quote metrics on the settlement schedule (LSTA convention: a
            // quote applies to the drawn balance plus accrued).
            (DiscountMargin, quotes::DiscountMarginCalculator),
            (Ytm, quotes::YtmCalculator),
        ],
        custom: [
            ("utilization_rate", UtilizationRateCalculator),
            ("available_capacity", AvailableCapacityCalculator),
            ("weighted_average_cost", ApproxWeightedAverageCostCalculator),
            ("draw_option_cost", DrawOptionCostCalculator),
            ("all_in_rate", quotes::AllInRateCalculator),
            ("accrued_interest", quotes::AccruedInterestCalculator),
            ("price_from_dm", quotes::PriceFromDmCalculator),
            ("exposure_at_default", exposure::ExposureAtDefaultCalculator),
            ("expected_loss", exposure::ExpectedLossCalculator),
            ("oid_eir_amortization", oid_eir::OidEirAmortizationCalculator),
        ]
    }

    // Register facility-specific metrics with custom IDs

    Ok(())
}

/// Calculator for available facility capacity (commitment - drawn).
///
/// Returns the amount as a float (in the instrument's currency units).
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct AvailableCapacityCalculator;

impl MetricCalculator for AvailableCapacityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let facility: &RevolvingCredit = context.instrument_as()?;
        let drawn = drawn_at(facility, context.as_of, context.as_of)?;
        let available = facility
            .commitment_at(context.as_of)
            .checked_sub(drawn)?
            .checked_sub(facility.lc_outstanding_at(context.as_of))?;
        Ok(available.amount())
    }
}

/// Calculator for the Monte Carlo mean draw option cost: the value to the
/// lender of the facility's simulated draws having been made at the
/// contractual margin instead of each path's fair spread, anchored to the
/// margin at the valuation date so only spread changes count (negative when
/// spreads widen). Deterministic facilities have no simulated draws and
/// report `0.0`.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct DrawOptionCostCalculator;

impl MetricCalculator for DrawOptionCostCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let facility: &RevolvingCredit = context.instrument_as()?;
        match &facility.draw_repay_spec {
            DrawRepaySpec::Deterministic(_) => Ok(0.0),
            DrawRepaySpec::Stochastic(_) => {
                let result = RevolvingCreditPricer::price_with_paths(
                    facility,
                    &context.curves,
                    context.as_of,
                )?;
                Ok(result.draw_option_cost.mean.amount())
            }
        }
    }
}

/// Calculator for facility utilization rate (drawn / committed).
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct UtilizationRateCalculator;

impl MetricCalculator for UtilizationRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let facility: &RevolvingCredit = context.instrument_as()?;
        let drawn = drawn_at(facility, context.as_of, context.as_of)?.amount();
        let commitment = facility.commitment_at(context.as_of).amount();
        let utilization_rate = if commitment > 0.0 {
            drawn / commitment
        } else {
            0.0
        };
        Ok(utilization_rate)
    }
}
