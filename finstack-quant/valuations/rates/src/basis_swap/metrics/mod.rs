//! Basis swap specific metrics.
//!
//! Provides calculators for leg present values, discounted accrual sums (annuities),
//! DV01s, and par spread calculations for basis swap instruments.

/// Par spread calculator for basis swaps
mod par_spread;

pub(crate) use par_spread::{IncrementalParSpreadCalculator, ParSpreadCalculator};

use crate::basis_swap::types::BasisSwap;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricRegistry;
use finstack_quant_valuations_core::pricer::InstrumentType;

/// Registers all basis swap metrics in the standard metric registry.
///
/// This function adds the basis swap specific metric calculators to the provided
/// registry, making them available for use in valuation calculations.
///
/// # Arguments
/// * `registry` — The metric registry to register the calculators with
pub(crate) fn register_basis_swap_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::BasisSwap,
        metrics: [
            // Leg-specific metrics with primary/reference constructors.
            (AnnuityPrimary, AnnuityCalculator::primary()),
            (AnnuityReference, AnnuityCalculator::reference()),
            (PvPrimary, PvCalculator::primary()),
            (PvReference, PvCalculator::reference()),
            // DV01 in per-curve mode bumps each curve individually (discount,
            // primary forward, reference forward).
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_per_curve())),
            (BasisParSpread, ParSpreadCalculator),
            (IncrementalParSpread, IncrementalParSpreadCalculator),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}

/// Calculator for the discounted accrual sum (annuity) of a basis swap leg.
///
/// The annuity represents the sum of discounted year fractions for a leg,
/// which is used in DV01 calculations and par spread computations.
///
/// See unit tests and `examples/` for usage.
pub(crate) struct AnnuityCalculator {
    /// Whether this calculator is for the primary leg (true) or reference leg (false).
    pub(crate) is_primary: bool,
}

impl AnnuityCalculator {
    /// Creates a calculator for the primary leg.
    pub(crate) const fn primary() -> Self {
        Self { is_primary: true }
    }

    /// Creates a calculator for the reference leg.
    pub(crate) const fn reference() -> Self {
        Self { is_primary: false }
    }
}

impl MetricCalculator for AnnuityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<BasisSwap>()?;
        let curves = std::sync::Arc::clone(&context.curves);

        let leg = if self.is_primary {
            &swap.primary_leg
        } else {
            &swap.reference_leg
        };

        swap.annuity_for_leg(leg, curves.as_ref(), context.as_of)
    }
}

/// Calculator for the present value of a basis swap leg.
///
/// This calculator computes the present value of either the primary or reference
/// leg of a basis swap, including the spread and forward rate components.
///
/// See unit tests and `examples/` for usage.
pub(crate) struct PvCalculator {
    /// Whether this calculator is for the primary leg (true) or reference leg (false).
    pub(crate) is_primary: bool,
}

impl PvCalculator {
    /// Creates a calculator for the primary leg.
    pub(crate) const fn primary() -> Self {
        Self { is_primary: true }
    }

    /// Creates a calculator for the reference leg.
    pub(crate) const fn reference() -> Self {
        Self { is_primary: false }
    }
}

impl MetricCalculator for PvCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<BasisSwap>()?;
        let curves = std::sync::Arc::clone(&context.curves);
        let as_of = context.as_of;

        let leg = if self.is_primary {
            &swap.primary_leg
        } else {
            &swap.reference_leg
        };

        let pv = swap.pv_float_leg(leg, curves.as_ref(), as_of)?;
        Ok(pv.amount())
    }
}
