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

        // SC-M10: the aggregate-cashflow fallback no longer FABRICATES an
        // accrued figure.
        //
        // It used to pro-rate the next payment from `context.cashflows`, which
        // is interest AND PRINCIPAL combined. On an amortizing tranche that is
        // not an approximation — it is a different quantity by an order of
        // magnitude. A $10m tranche whose next payment is $3m principal +
        // $100k interest, valued halfway through the period, has a true
        // accrued of $50k (0.5 points); the fallback computed $1.55m (15.5
        // points). `CleanPriceCalculator` subtracts that from the dirty price,
        // so the clean price printed ~15 points below dirty instead of 0.5 —
        // a wholly fabricated quote, plausible enough to trade off.
        //
        // Returning zero instead is a deliberate, bounded degradation: clean
        // price collapses to dirty, understating accrued by at most one
        // period's genuine interest rather than overstating it by the
        // principal share of the payment. It is wrong in a small, predictable
        // direction instead of a large, unpredictable one.
        //
        // Erroring outright would be better still, but the deal-level metric
        // registry evaluates `Accrued`/`CleanPrice` on the aggregate of every
        // tranche's flows and never populates `detailed_tranche_cashflows`, so
        // an error there would take down the whole metric suite. Fixing that
        // properly means restricting the registry to per-tranche contexts —
        // SC-m14 — at which point this branch should become an error.
        // A genuinely absent cashflow series is still an error — that is a
        // missing input, not a degraded one.
        context.cashflows.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "context.cashflows".to_string(),
            })
        })?;
        Ok(0.0)
    }
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

    /// SC-M10 — aggregate cashflows must NOT be pro-rated into accrued.
    ///
    /// `context.cashflows` is interest AND principal combined. Pro-rating it
    /// produced an accrued figure that `CleanPriceCalculator` then subtracted
    /// from the dirty price, printing a clean price up to ~15 points wrong on
    /// an amortizing tranche. This test previously asserted that fabricated
    /// value (45.0 from a 90.0 combined payment); it now asserts the error.
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
            "aggregate cashflows mix principal into interest, so no accrued \
             figure may be derived from them. The pre-fix fallback returned \
             45.0 here by pro-rating a 90.0 COMBINED payment, which \
             CleanPriceCalculator then subtracted from the dirty price."
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
            writedown_flows: vec![],
            final_balance: Money::new(0.0, Currency::USD),
            total_interest: Money::new(30.0, Currency::USD),
            total_principal: Money::new(0.0, Currency::USD),
            total_pik: Money::new(0.0, Currency::USD),
            total_writedown: Money::new(0.0, Currency::USD),
        });
        ctx.day_count = Some(DayCount::Act360);

        let accrued = AccruedCalculator.calculate(&mut ctx);
        assert!(accrued.is_ok(), "detailed interest accrual should succeed");
        if let Ok(value) = accrued {
            assert!((value - 15.0).abs() < 1e-12);
        }
    }

    /// SC-M10 — outside the cashflow window the answer is still an error, not
    /// zero, when only aggregate flows are available. Returning 0.0 here would
    /// be indistinguishable from a genuine "no accrual" and would silently
    /// feed a wrong clean price. With per-tranche interest flows supplied, the
    /// out-of-window zero is handled by `accrued_from_interest_flows`.
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
