//! Discount Margin for floating-rate term loans.
//!
//! Definition (Fabozzi; Bloomberg YAS convention): the discount margin is the
//! constant additive spread (decimal, e.g. `0.025` = 250 bp) over the loan's
//! **discount curve** such that the PV of the loan's projected cashflows —
//! projected at the **contractual margin**, with step-ups, PIK, DDTL draws,
//! amortization, and fees exactly as contracted — equals the observed dirty
//! market price. PV is strictly **decreasing** in DM, so a loan quoted below
//! par solves to a DM above its contractual margin, and vice versa. On a flat,
//! consistent curve where the discount curve equals the projection index
//! curve, a loan priced at par has DM equal to its contractual margin.
//!
//! # Implementation
//!
//! - Cashflows are generated **unchanged** by the full-fidelity term-loan
//!   engine (DDTL draw timing, amortization, PIK capitalization, sweeps,
//!   covenant margin step-ups) at the contractual margin.
//! - Each flow is then re-discounted with the DM added to the
//!   periodically-compounded zero rate derived from the loan's discount
//!   curve — the same mechanics as the bond Z-spread/DM path (see
//!   `z_spread_discount_factor` and the FRN `price_from_dm`). This keeps the
//!   term-loan DM methodologically identical to the FRN DM.
//! - Because this is a **curve DM** (a spread over the loan's *discount*
//!   curve), any basis between the discount curve and the projection index
//!   curve is included in the solved DM.

use crate::instruments::fixed_income::loan_quotes::{
    compounding_frequency, pv_with_discount_margin, solve_discount_margin,
};
use crate::instruments::fixed_income::term_loan::pricing::TermLoanDiscountingPricer;
use crate::instruments::TermLoan;
use crate::metrics::{MetricCalculator, MetricContext};
use rust_decimal::prelude::ToPrimitive;

/// Discount margin calculator for floating rate term loans.
///
/// Returns an error if called on a fixed-rate loan (DM is only defined for
/// floating-rate instruments).
pub(crate) struct DiscountMarginCalculator;

impl MetricCalculator for DiscountMarginCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let loan: &TermLoan = context.instrument_as()?;

        // DM is only defined for floating-rate loans; fixed-rate instruments
        // should not request this metric.
        let contractual_margin = match &loan.rate {
            crate::instruments::fixed_income::term_loan::types::RateSpec::Floating(spec) => {
                spec.spread_bp
                    .to_f64()
                    .ok_or(finstack_quant_core::InputError::Invalid)?
                    * 1e-4
            }
            crate::instruments::fixed_income::term_loan::types::RateSpec::Fixed { .. } => {
                return Err(finstack_quant_core::InputError::Invalid.into());
            }
        };

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
        // amount (% of outstanding at settlement + accrued) if set.
        let quoted_px = loan
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price;
        let as_of = context.as_of;
        let schedule = super::irr_helpers::cached_full_schedule(context)?;
        let loan: &TermLoan = context.instrument_as()?;

        // The loan's unchanged contractual flows (coupons at the contractual
        // margin including step-ups; PIK and pre-settlement flows excluded),
        // built once and repriced at each margin. Each flow is discounted
        // from settlement with the DM added to the periodically compounded
        // zero rate at the coupon frequency (z-spread mechanics), so DM = 0
        // gives the settlement-date model value.
        let (settlement_date, flows) =
            TermLoanDiscountingPricer::pricing_flows(loan, &schedule, as_of)?;
        let flows: Vec<(finstack_quant_core::dates::Date, f64)> = flows
            .iter()
            .map(|(date, amount)| (*date, amount.amount()))
            .collect();
        let disc = context
            .curves
            .get_discount(loan.discount_curve_id.as_str())?;
        let compounding = compounding_frequency(loan.frequency);
        let pv_given_dm = |dm: f64| {
            pv_with_discount_margin(&flows, settlement_date, disc.as_ref(), compounding, dm)
        };

        let target = if let Some(px) = quoted_px {
            super::irr_helpers::quoted_dirty_from_clean_px(loan, &schedule, as_of, px)?.amount()
        } else {
            // No quote: the settlement-date model value, so the DM is the
            // spread over the loan's own curve (zero by construction).
            pv_given_dm(0.0)?
        };

        // PV is strictly decreasing in dm; the contractual margin is the
        // exact solution for a par-quoted loan on a flat consistent curve.
        let dm = solve_discount_margin(pv_given_dm, target, contractual_margin)?;

        // DM as decimal (e.g. 0.025 = 250 bp), directly comparable to the
        // contractual margin.
        Ok(dm)
    }
}
