//! Pricing and metric helpers for interest-rate instruments.
//!
use crate::instruments::common_impl::numeric::decimal_to_f64;
use crate::instruments::rates::basis_swap::types::BasisSwap;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::{Error, Result};

/// Calculator for the **absolute** par spread on the primary leg that sets NPV to zero.
///
/// # Output Interpretation
///
/// The par spread is the spread that would make the basis swap have zero net present value,
/// calculated by solving for the spread that equates the present values of both legs.
///
/// **Important**: This calculator returns the **absolute** par spread (what the spread should be
/// for zero NPV), not the **incremental** spread needed from the current position.
///
/// # Example
///
/// If a swap has:
/// - Current primary leg spread: 5bp
/// - Computed par spread: 8bp
///
/// The swap is **out of the money** by 3bp × annuity × notional. To close the position
/// at fair value, an additional 3bp would be required (use [`IncrementalParSpreadCalculator`]
/// for this calculation).
///
/// # Formula
///
/// ```text
/// par_spread = (PV_reference - PV_primary_no_spread) / (notional × annuity)
/// ```
///
/// Where `PV_primary_no_spread` is the primary leg PV with zero spread.
///
/// # Output Units
///
/// Returns the par spread in **basis points** (e.g., 5.0 for 5bp).
///
/// See unit tests and `examples/` for usage.
pub(crate) struct ParSpreadCalculator;

/// Calculator for the **incremental** par spread from the current spread position.
///
/// # Output Interpretation
///
/// Returns the additional spread (in basis points) needed on top of the current spread
/// to bring the basis swap NPV to zero. This is useful for:
/// - Understanding how far out of the money a position is
/// - Calculating unwind costs
/// - Hedge ratio adjustments
///
/// # Formula
///
/// ```text
/// incremental_par_spread = absolute_par_spread - current_spread
/// ```
///
/// # Example
///
/// If a swap has:
/// - Current primary leg spread: 5bp
/// - Computed absolute par spread: 8bp
/// - **Incremental par spread: 3bp**
///
/// This means the position would require 3bp additional spread to be at par.
///
/// # Sign Convention
///
/// - **Positive**: Current spread is below par (primary leg receiver is losing)
/// - **Negative**: Current spread is above par (primary leg receiver is gaining)
/// - **Zero**: Swap is at par
///
/// # Output Units
///
/// Returns the incremental spread in **basis points** (e.g., 3.0 for 3bp).
pub(crate) struct IncrementalParSpreadCalculator;

/// Minimum threshold for annuity to prevent division-by-zero.
///
/// Chosen to be well below any reasonable annuity value (even a 1-day swap would have
/// annuity ~0.003) while safely above f64 precision limits (~1e-15). This corresponds
/// to roughly $1 of discounted accrual on a $1T notional, which is well below any
/// realistic trading scenario but catches true zeros.
const MIN_ANNUITY_THRESHOLD: f64 = 1e-12;

/// Minimum threshold for notional to prevent division-by-zero.
///
/// Par spread is computed as PV_diff / (notional × annuity). With zero or near-zero
/// notional, this would produce NaN or Inf. The threshold is set at $0.01 to catch
/// effectively zero notional while allowing micro-trades for testing purposes.
const MIN_NOTIONAL_THRESHOLD: f64 = 0.01;

impl MetricCalculator for ParSpreadCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::AnnuityPrimary]
    }

    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // Retrieve the pre-computed annuity dependency; fail if missing to avoid silent errors
        let annuity = context
            .computed
            .get(&MetricId::AnnuityPrimary)
            .copied()
            .ok_or_else(|| {
                Error::Validation(
                    "BasisParSpread requires AnnuityPrimary dependency to be computed first"
                        .to_string(),
                )
            })?;

        // Guard against zero/near-zero annuity which would cause divide-by-zero
        if annuity.abs() < MIN_ANNUITY_THRESHOLD {
            return Err(Error::Validation(format!(
                "Cannot compute par spread: annuity ({:.2e}) is below minimum threshold ({:.2e})",
                annuity, MIN_ANNUITY_THRESHOLD
            )));
        }

        let instrument = std::sync::Arc::clone(&context.instrument);
        let swap = instrument
            .as_any()
            .downcast_ref::<BasisSwap>()
            .ok_or(Error::Input(finstack_quant_core::InputError::Invalid))?;

        // Guard against zero/near-zero notional which would cause divide-by-zero
        let notional = swap.notional.amount();
        if notional.abs() < MIN_NOTIONAL_THRESHOLD {
            return Err(Error::Validation(format!(
                "Cannot compute par spread: notional ({:.2}) is below minimum threshold ({:.2}). \
                 Zero-notional swaps have undefined par spread.",
                notional, MIN_NOTIONAL_THRESHOLD
            )));
        }

        let curves = std::sync::Arc::clone(&context.curves);
        let as_of = context.as_of;

        // PV of reference leg
        let pv_ref = swap
            .pv_float_leg(&swap.reference_leg, curves.as_ref(), as_of)?
            .amount();

        // PV of primary at zero spread - need to create a modified leg
        let primary_leg_no_spread = crate::instruments::rates::basis_swap::BasisSwapLeg {
            forward_curve_id: swap.primary_leg.forward_curve_id.to_owned(),
            discount_curve_id: swap.primary_leg.discount_curve_id.to_owned(),
            start: swap.primary_leg.start,
            end: swap.primary_leg.end,
            frequency: swap.primary_leg.frequency,
            day_count: swap.primary_leg.day_count,
            bdc: swap.primary_leg.bdc,
            calendar_id: swap.primary_leg.calendar_id.clone(),
            stub: swap.primary_leg.stub,
            payment_lag_days: swap.primary_leg.payment_lag_days,
            reset_lag_days: swap.primary_leg.reset_lag_days,
            spread_bp: rust_decimal::Decimal::ZERO,
        };
        let pv_primary_no_spread = swap
            .pv_float_leg(&primary_leg_no_spread, curves.as_ref(), as_of)?
            .amount();

        // Solve for s (decimal). Convert to bp.
        // Formula: par_spread = (PV_reference - PV_primary_no_spread) / (notional × annuity)
        let s_decimal = (pv_ref - pv_primary_no_spread) / (notional * annuity);
        Ok(s_decimal * 1e4)
    }
}

impl MetricCalculator for IncrementalParSpreadCalculator {
    fn dependencies(&self) -> &[MetricId] {
        // Depends on the absolute par spread being computed first
        &[MetricId::BasisParSpread]
    }

    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // Retrieve the pre-computed absolute par spread
        let absolute_par_spread_bp = context
            .computed
            .get(&MetricId::BasisParSpread)
            .copied()
            .ok_or_else(|| {
                Error::Validation(
                    "IncrementalParSpread requires BasisParSpread dependency to be computed first"
                        .to_string(),
                )
            })?;

        let instrument = std::sync::Arc::clone(&context.instrument);
        let swap = instrument
            .as_any()
            .downcast_ref::<BasisSwap>()
            .ok_or(Error::Input(finstack_quant_core::InputError::Invalid))?;

        // Current spread in bp (convert Decimal to f64 at arithmetic boundary)
        let current_spread_bp = decimal_to_f64(
            swap.primary_leg.spread_bp,
            "BasisSwap primary_leg spread_bp",
        )?;

        // Incremental = absolute par spread - current spread
        Ok(absolute_par_spread_bp - current_spread_bp)
    }
}
