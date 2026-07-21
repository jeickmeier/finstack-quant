//! Accrued interest calculator for structured credit instruments.

use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

/// Calculates accrued interest for structured credit instruments.
///
/// Accrued interest is the pro-rata interest that has accrued since the last
/// payment date. For structured credit, this is calculated per tranche based on:
/// - Days elapsed since last payment
/// - Days in the current period
/// - Current coupon rate (which may be floating)
///
/// # Formula
///
/// Accrued = (Days Elapsed / Days in Period) × Coupon Rate × Notional
///
pub struct AccruedCalculator;

impl MetricCalculator for AccruedCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // Prefer interest-only flows from detailed tranche cashflows when available.
        // This avoids the systematic overstatement that occurs when principal payments
        // are mixed into the accrual calculation (common for amortizing tranches).
        if let Some(details) = context.detailed_tranche_cashflows.as_ref() {
            if !details.interest_flows.is_empty() {
                return self.accrued_from_interest_flows(&details.interest_flows, context);
            }
        }

        // SC-m14: at DEAL level there is no single tranche, so
        // `detailed_tranche_cashflows` is absent — but the deal context does
        // carry TAGGED flows, and `CFKind` distinguishes coupon kinds from
        // `Notional`. Filtering to the interest kinds recovers a correct
        // aggregate accrued without needing per-tranche flows.
        //
        // This removes the SC-M10 degradation (accrued silently returning 0,
        // which made CleanPrice collapse to DirtyPrice at deal level). The
        // earlier fix could only degrade because it looked at
        // `context.cashflows`, which is the cash-only view with principal and
        // interest already summed; the tagged view keeps them separable.
        if let Some(tagged) = context.tagged_cashflows.as_ref() {
            let interest_flows: Vec<(Date, Money)> = tagged
                .iter()
                .filter(|flow| is_interest_kind(flow.kind))
                .map(|flow| (flow.date, flow.amount))
                .collect();
            if !interest_flows.is_empty() {
                return self.accrued_from_interest_flows(&interest_flows, context);
            }
        }

        // No separable interest anywhere: a genuinely absent cashflow series is
        // a missing input; otherwise degrade to clean ≈ dirty rather than
        // pro-rating a principal-contaminated payment (SC-M10).
        context.cashflows.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "context.cashflows".to_string(),
            })
        })?;
        Ok(0.0)
    }
}

/// Whether a classified cashflow is an INTEREST payment.
///
/// SC-m14: `Notional` is principal and the fee kinds are transaction expenses;
/// only the coupon kinds accrue. `PIK` is deliberately excluded — capitalized
/// interest is added to notional rather than settled in cash, so it is not
/// part of accrued for a cash buyer.
fn is_interest_kind(kind: finstack_quant_core::cashflow::CFKind) -> bool {
    use finstack_quant_core::cashflow::CFKind;
    matches!(
        kind,
        CFKind::Fixed | CFKind::FloatReset | CFKind::InflationCoupon
    )
}

impl AccruedCalculator {
    fn accrued_from_interest_flows(
        &self,
        interest_flows: &[(Date, Money)],
        context: &MetricContext,
    ) -> Result<f64> {
        if interest_flows.is_empty() {
            return Ok(0.0);
        }

        if let (Some((first_date, _)), Some((last_date, _))) =
            (interest_flows.first(), interest_flows.last())
        {
            if context.as_of < *first_date || context.as_of >= *last_date {
                return Ok(0.0);
            }
        }

        let (last_payment, next_payment) = find_surrounding_dates(interest_flows, context.as_of)?;

        let day_count = context.day_count.unwrap_or(DayCount::Act360);
        let accrual_fraction =
            day_count.year_fraction(last_payment, context.as_of, DayCountContext::default())?;
        let period_fraction =
            day_count.year_fraction(last_payment, next_payment, DayCountContext::default())?;

        if period_fraction == 0.0 {
            return Ok(0.0);
        }

        let period_interest = interest_flows
            .iter()
            .find(|(d, _)| *d == next_payment)
            .map(|(_, m)| m.amount())
            .unwrap_or(0.0);

        Ok(period_interest * (accrual_fraction / period_fraction))
    }
}

/// Helper to find the payment dates surrounding as_of date.
fn find_surrounding_dates(flows: &[(Date, Money)], as_of: Date) -> Result<(Date, Date)> {
    // Find last payment before or on as_of
    let last = flows
        .iter()
        .filter(|(d, _)| *d <= as_of)
        .map(|(d, _)| *d)
        .max();

    // Find next payment after as_of
    let next = flows
        .iter()
        .filter(|(d, _)| *d > as_of)
        .map(|(d, _)| *d)
        .min();

    match (last, next) {
        (Some(l), Some(n)) => Ok((l, n)),
        _ => Err(finstack_quant_core::Error::from(
            finstack_quant_core::InputError::NotFound {
                id: "accrual_period".to_string(),
            },
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::structured_credit::{StructuredCredit, TrancheCashflows};
    use crate::instruments::Instrument;
    use crate::metrics::MetricContext;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use std::sync::Arc;
    use time::macros::date;

    fn context(as_of: Date) -> MetricContext {
        MetricContext::new(
            Arc::new(StructuredCredit::example()) as Arc<dyn Instrument>,
            Arc::new(MarketContext::new()),
            as_of,
            Money::new(0.0, Currency::USD),
            MetricContext::default_config(),
        )
    }

    #[test]
    fn accrued_returns_zero_without_cashflow_inputs() {
        let mut ctx = context(date!(2025 - 02 - 15));
        let calc = AccruedCalculator;

        let result = calc.calculate(&mut ctx);
        assert!(result.is_err(), "missing cashflows should error");
    }

    /// SC-m14 — at DEAL level, accrued must be recovered from the TAGGED
    /// flows rather than degrading to zero.
    ///
    /// The deal context has no `detailed_tranche_cashflows` (there is no single
    /// tranche), but it does carry classified flows, and `CFKind` separates the
    /// coupon kinds from `Notional`. Filtering to interest recovers a correct
    /// aggregate accrued — which in turn stops `CleanPrice` collapsing onto
    /// `DirtyPrice` at deal level.
    #[test]
    fn accrued_uses_tagged_interest_flows_at_deal_level() {
        use finstack_quant_core::cashflow::{CFKind, CashFlow};

        let mut ctx = context(date!(2025 - 02 - 15));
        // No per-tranche detail, exactly like the deal-level registry path.
        ctx.detailed_tranche_cashflows = None;
        ctx.cashflows = Some(vec![
            (date!(2025 - 01 - 01), Money::new(0.0, Currency::USD)),
            // A COMBINED payment: 3,000 principal + 90 interest.
            (date!(2025 - 04 - 01), Money::new(3_090.0, Currency::USD)),
        ]);
        ctx.tagged_cashflows = Some(vec![
            CashFlow::new(
                date!(2025 - 01 - 01),
                None,
                Money::new(0.0, Currency::USD),
                CFKind::Fixed,
                0.25,
                None,
            ),
            CashFlow::new(
                date!(2025 - 04 - 01),
                None,
                Money::new(90.0, Currency::USD),
                CFKind::Fixed,
                0.25,
                None,
            ),
            CashFlow::new(
                date!(2025 - 04 - 01),
                None,
                Money::new(3_000.0, Currency::USD),
                CFKind::Notional,
                0.25,
                None,
            ),
        ]);
        ctx.day_count = Some(DayCount::Act360);

        let accrued = AccruedCalculator
            .calculate(&mut ctx)
            .expect("tagged flows must yield an accrued figure");

        // Halfway through the period on the 90 of INTEREST — the 3,000 of
        // principal must not contribute.
        assert!(
            (accrued - 45.0).abs() < 1e-9,
            "accrued must pro-rate the interest leg only: expected ~45.0 from \
             the 90.0 coupon, got {accrued:.4}. A value near 1545 means \
             principal is being included; 0.0 means the tagged flows are being \
             ignored and accrued has degraded (SC-m14)."
        );
    }

    /// Aggregate cashflows are not pro-rated into accrued.
    #[test]
    fn accrued_from_aggregated_cashflows_does_not_fabricate() {
        let mut ctx = context(date!(2025 - 02 - 15));
        ctx.cashflows = Some(vec![
            (date!(2025 - 01 - 01), Money::new(0.0, Currency::USD)),
            (date!(2025 - 04 - 01), Money::new(90.0, Currency::USD)),
            (date!(2025 - 07 - 01), Money::new(90.0, Currency::USD)),
        ]);
        ctx.day_count = Some(DayCount::Act360);

        let accrued = AccruedCalculator
            .calculate(&mut ctx)
            .expect("aggregate-only accrual degrades to zero rather than erroring");
        assert_eq!(
            accrued, 0.0,
            "aggregate cashflows mix principal into interest; accrued must be 0.0"
        );
    }

    #[test]
    fn detailed_interest_flows_take_priority_over_aggregated_flows() {
        let mut ctx = context(date!(2025 - 02 - 15));
        ctx.cashflows = Some(vec![
            (date!(2025 - 01 - 01), Money::new(0.0, Currency::USD)),
            (date!(2025 - 04 - 01), Money::new(120.0, Currency::USD)),
        ]);
        ctx.detailed_tranche_cashflows = Some(TrancheCashflows {
            tranche_id: "A".to_string(),
            cashflows: vec![],
            detailed_flows: vec![],
            interest_flows: vec![
                (date!(2025 - 01 - 01), Money::new(0.0, Currency::USD)),
                (date!(2025 - 04 - 01), Money::new(30.0, Currency::USD)),
            ],
            principal_flows: vec![],
            pik_flows: vec![],
            deferred_flows: Vec::new(),
            writedown_flows: vec![],
            final_balance: Money::new(0.0, Currency::USD),
            total_interest: Money::new(30.0, Currency::USD),
            total_principal: Money::new(0.0, Currency::USD),
            total_pik: Money::new(0.0, Currency::USD),
            total_deferred: Money::new(0.0, Currency::USD),
            total_writedown: Money::new(0.0, Currency::USD),
        });
        ctx.day_count = Some(DayCount::Act360);

        let accrued = AccruedCalculator.calculate(&mut ctx);
        assert!(accrued.is_ok(), "detailed interest accrual should succeed");
        if let Ok(value) = accrued {
            assert!((value - 15.0).abs() < 1e-12);
        }
    }

    /// Aggregate-only accrued remains zero outside the cashflow window.
    #[test]
    fn accrued_outside_window_without_interest_flows_is_zero() {
        let mut ctx = context(date!(2025 - 08 - 01));
        ctx.cashflows = Some(vec![
            (date!(2025 - 01 - 01), Money::new(0.0, Currency::USD)),
            (date!(2025 - 04 - 01), Money::new(90.0, Currency::USD)),
            (date!(2025 - 07 - 01), Money::new(90.0, Currency::USD)),
        ]);

        assert_eq!(
            AccruedCalculator.calculate(&mut ctx),
            Ok(0.0),
            "outside the window, and without per-tranche interest flows, \
             accrued is zero"
        );
    }
}
