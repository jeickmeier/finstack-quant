//! Discount Margin for floating-rate term loans.
//!
//! Solves for an additive spread (decimal) to the loan's base spread such that
//! discounted PV of all cashflows (coupons + principal) matches observed price.
//!
//! # Market-Standard Implementation
//!
//! This implementation uses **full-fidelity re-pricing**:
//! - Clones the TermLoan and adjusts the spread by DM
//! - Re-runs complete cashflow generation including:
//!   * DDTL draw timing and fees
//!   * Amortization schedules
//!   * PIK capitalization
//!   * Cash sweeps and covenants
//!   * Principal redemptions
//! - Uses the actual pricer to compute PV
//! - Solves for DM using Brent's method
//!
//! This ensures DM is consistent with the loan's true cashflow structure.

use crate::instruments::TermLoan;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::math::solver::{BrentSolver, Solver};

/// Discount margin calculator for floating rate term loans.
///
/// Returns an error if called on a fixed-rate loan (DM is only defined for
/// floating-rate instruments).
pub(crate) struct DiscountMarginCalculator;

impl DiscountMarginCalculator {
    /// Compute PV of term loan with adjusted spread (base_spread + dm_bp).
    ///
    /// Clones the loan, adds `dm_bp` to the floating spread, and re-prices using
    /// the full cashflow engine and pricer.
    ///
    /// # Errors
    ///
    /// Returns `InputError::Invalid` if called on a fixed-rate loan.
    fn pv_given_dm(
        loan: &TermLoan,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        dm_bp: f64,
    ) -> finstack_quant_core::Result<f64> {
        use crate::instruments::fixed_income::term_loan::types::RateSpec;

        // Clone loan and adjust spread
        let mut loan_with_dm = loan.clone();

        match &mut loan_with_dm.rate {
            RateSpec::Floating(spec) => {
                // Add DM (in bp) to base spread.
                // Propagate conversion error to surface solver divergence (NaN/Inf).
                use rust_decimal::Decimal;
                let dm_decimal = Decimal::try_from(dm_bp)
                    .map_err(|_| finstack_quant_core::InputError::Invalid)?;
                spec.spread_bp += dm_decimal;
            }
            RateSpec::Fixed { .. } => {
                // DM is not defined for fixed-rate loans
                return Err(finstack_quant_core::InputError::Invalid.into());
            }
        }

        // Re-price using full cashflow engine and pricer
        let pv =
            crate::instruments::fixed_income::term_loan::pricing::TermLoanDiscountingPricer::price(
                &loan_with_dm,
                curves,
                as_of,
            )?;

        Ok(pv.amount())
    }
}

impl MetricCalculator for DiscountMarginCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let loan: &TermLoan = context.instrument_as()?;

        // DM is only defined for floating-rate loans; fixed-rate instruments
        // should not request this metric.
        if let crate::instruments::fixed_income::term_loan::types::RateSpec::Fixed { .. } =
            loan.rate
        {
            return Err(finstack_quant_core::InputError::Invalid.into());
        }

        // Callable loans require a quoted price for DM: without an observed market price,
        // the DM would trivially be zero (model PV == target PV) and is not meaningful.
        if loan.call_schedule.is_some()
            && loan
                .instrument_pricing_overrides
                .market_quotes
                .quoted_clean_price
                .is_none()
        {
            return Err(finstack_quant_core::Error::Validation(
                "DiscountMargin requires quoted_clean_price for callable loans".to_string(),
            ));
        }

        // Target price: quoted clean price converted to a dirty settlement
        // amount (% of outstanding at settlement + accrued) if set, else base PV
        let quoted_px = loan
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price;
        let target = if let Some(px) = quoted_px {
            let as_of = context.as_of;
            let schedule = super::irr_helpers::cached_full_schedule(context)?;
            let loan: &TermLoan = context.instrument_as()?;
            super::irr_helpers::quoted_dirty_from_clean_px(loan, &schedule, as_of, px)?.amount()
        } else {
            context.base_value.amount()
        };
        let loan: &TermLoan = context.instrument_as()?;

        // Objective function: PV(dm) - target_price
        // Return NAN on pricing errors so the solver does not converge to a
        // wrong root based on artificial large values.
        let objective = |dm_bp: f64| -> f64 {
            match Self::pv_given_dm(loan, &context.curves, context.as_of, dm_bp) {
                Ok(pv) => pv - target,
                Err(_) => f64::NAN,
            }
        };

        // Solve for DM in basis points.
        // Tolerance of 1e-8 provides sub-basis-point accuracy (1e-8 bp = 1e-12 decimal)
        // without exceeding f64 precision limits.
        let solver = BrentSolver::new()
            .tolerance(1e-8)
            .initial_bracket_size(Some(50.0)); // Start with +/- 50bp bracket

        let dm_bp = solver.solve(objective, 0.0)?;

        // Validate DM is within reasonable bounds
        if dm_bp.abs() > 2000.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Discount margin {} bp exceeds reasonable bounds (±2000 bp)",
                dm_bp
            )));
        }

        // Return DM as decimal (bp / 10000)
        Ok(dm_bp * 1e-4)
    }
}
