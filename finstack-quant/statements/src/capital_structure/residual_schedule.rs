//! Rebuild remaining loan interest after outstanding changes.
//!
//! After a sweep, PIK capitalization, scheduled amort, or draw, next-period
//! interest must be `outstanding × rate × accrual_factor`, not a scale of the
//! original coupon. This module rewrites future interest/PIK flows on a residual
//! [`CashFlowSchedule`] and leaves scheduled amort, draws, and fees in place.

use crate::error::{Error, Result};
use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_cashflows::primitives::CFKind;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;

/// Interest kinds whose remaining amounts are rebuilt from the new outstanding.
fn is_rebuildable_interest(kind: CFKind) -> bool {
    matches!(
        kind,
        CFKind::Fixed | CFKind::Stub | CFKind::FloatReset | CFKind::Pik
    )
}

/// Rewrite future interest on `schedule` to accrue on `new_outstanding`.
///
/// Flows with `date > from_date` are future. Interest kinds (`Fixed`, `Stub`,
/// `FloatReset`, `Pik`) become `sign(old) × new_outstanding × rate ×
/// accrual_factor`. When `rate` is missing it is inferred from the pre-rebuild
/// amount, prior outstanding, and accrual factor. Scheduled amort, prepay,
/// draws, notionals, and fees are left unchanged.
///
/// # Arguments
///
/// * `schedule` - Residual instrument schedule whose future interest is rebuilt.
/// * `new_outstanding` - Closing outstanding after this period's waterfall
///   (sweep, PIK capitalize, scheduled amort, or draw), in the schedule
///   currency. Future coupons accrue on this balance.
/// * `from_date` - Inclusive period-end snapshot (`period.end - 1 day`). Flows
///   dated after this date are future and are rebuilt.
///
/// # Errors
///
/// Returns a capital-structure error when `new_outstanding` is a different
/// currency from the schedule, when a missing rate cannot be inferred as a
/// finite value, when rebuilt interest is non-finite or outside the monetary
/// representation range, or when the outstanding path cannot be rebuilt.
pub(crate) fn rebuild_residual_interest(
    schedule: &CashFlowSchedule,
    new_outstanding: Money,
    from_date: Date,
) -> Result<CashFlowSchedule> {
    let currency = schedule.get_notional().initial.currency();
    if new_outstanding.currency() != currency {
        return Err(Error::currency_mismatch(
            currency,
            new_outstanding.currency(),
        ));
    }

    let outstanding_path = schedule.outstanding_by_date()?;
    let initial = {
        let n = schedule.get_notional().initial;
        if n.amount() < 0.0 {
            n.checked_neg()
        } else {
            n
        }
    };
    let abs_money = |m: Money| -> Money {
        if m.amount() < 0.0 {
            m.checked_neg()
        } else {
            m
        }
    };

    let mut flows: Vec<_> = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.date > from_date)
        .cloned()
        .collect();
    for cf in &mut flows {
        if !is_rebuildable_interest(cf.kind) {
            continue;
        }
        let rate = match cf.rate {
            Some(rate) => rate,
            None => {
                let prior = outstanding_path
                    .iter()
                    .rev()
                    .find(|(d, _)| *d < cf.date)
                    .map(|(_, m)| abs_money(*m))
                    .unwrap_or(initial);
                let inferred = cf.amount.amount().abs() / prior.amount() / cf.accrual_factor;
                if !inferred.is_finite() {
                    return Err(Error::capital_structure(format!(
                        "cannot infer a finite interest rate for a {kind:?} flow on {date}: \
                         |amount|={amount} / prior_outstanding={prior} / accrual_factor={factor}",
                        kind = cf.kind,
                        date = cf.date,
                        amount = cf.amount.amount().abs(),
                        prior = prior.amount(),
                        factor = cf.accrual_factor,
                    )));
                }
                inferred
            }
        };
        let sign = if cf.amount.amount() < 0.0 { -1.0 } else { 1.0 };
        cf.amount = Money::new(
            sign * new_outstanding.amount() * rate * cf.accrual_factor,
            cf.amount.currency(),
        )
        .map_err(|error| {
            Error::capital_structure(format!(
                "cannot rebuild {:?} interest on {}: {error}",
                cf.kind, cf.date
            ))
        })?;
        cf.rate = Some(rate);
    }

    let mut notional = schedule.get_notional().clone();
    notional.initial = new_outstanding;
    Ok(CashFlowSchedule::from_parts(
        flows,
        notional,
        schedule.get_day_count(),
        schedule.get_meta().clone(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_cashflows::builder::{CashFlowMeta, Notional};
    use finstack_quant_core::cashflow::CashFlow;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use time::Month;

    fn schedule(flows: Vec<CashFlow>, notional: f64, issue: Date) -> CashFlowSchedule {
        CashFlowSchedule::from_parts(
            flows,
            Notional::par(notional, Currency::USD).expect("valid notional fixture"),
            DayCount::Act365F,
            CashFlowMeta {
                issue_date: Some(issue),
                ..CashFlowMeta::default()
            },
        )
    }

    #[test]
    fn rebuild_rewrites_future_coupon_on_new_outstanding() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let q1_coupon = Date::from_calendar_date(2025, Month::February, 15).expect("valid date");
        let q2_coupon = Date::from_calendar_date(2025, Month::May, 15).expect("valid date");
        let from_date = Date::from_calendar_date(2025, Month::March, 31).expect("valid date");

        let original = schedule(
            vec![
                CashFlow::new(
                    q1_coupon,
                    None,
                    Money::from((-20_000_i64, Currency::USD)),
                    CFKind::Fixed,
                    0.25,
                    Some(0.08),
                ),
                CashFlow::new(
                    q2_coupon,
                    None,
                    Money::from((-20_000_i64, Currency::USD)),
                    CFKind::Fixed,
                    0.25,
                    Some(0.08),
                ),
            ],
            1_000_000.0,
            issue,
        );

        let rebuilt = rebuild_residual_interest(
            &original,
            Money::from((500_000_i64, Currency::USD)),
            from_date,
        )
        .expect("rebuild");

        assert!(
            rebuilt.get_flows().iter().all(|cf| cf.date != q1_coupon),
            "realized in-period coupon must be dropped from the residual"
        );
        assert_eq!(rebuilt.get_notional().initial.amount(), 500_000.0);

        let q2 = rebuilt
            .get_flows()
            .iter()
            .find(|cf| cf.date == q2_coupon)
            .expect("q2 coupon rebuilt");
        assert!(
            (q2.amount.amount() - (-10_000.0)).abs() < 1e-9,
            "next-period coupon must be 500k × 0.08 × 0.25, got {}",
            q2.amount.amount()
        );
    }

    #[test]
    fn rebuild_infers_rate_when_missing() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let q2_coupon = Date::from_calendar_date(2025, Month::May, 15).expect("valid date");
        let from_date = Date::from_calendar_date(2025, Month::March, 31).expect("valid date");

        let original = schedule(
            vec![
                CashFlow::new(
                    issue,
                    None,
                    Money::from((-1_000_000_i64, Currency::USD)),
                    CFKind::Notional,
                    0.0,
                    None,
                ),
                CashFlow::new(
                    q2_coupon,
                    None,
                    Money::from((-20_000_i64, Currency::USD)),
                    CFKind::Fixed,
                    0.25,
                    None,
                ),
            ],
            1_000_000.0,
            issue,
        );

        let rebuilt = rebuild_residual_interest(
            &original,
            Money::from((500_000_i64, Currency::USD)),
            from_date,
        )
        .expect("rebuild with inferred rate");

        let q2 = rebuilt
            .get_flows()
            .iter()
            .find(|cf| cf.date == q2_coupon)
            .expect("q2 coupon");
        assert!(
            (q2.amount.amount() - (-10_000.0)).abs() < 1e-9,
            "inferred 8% quarterly on 500k must be 10k, got {}",
            q2.amount.amount()
        );
    }

    #[test]
    fn rebuild_rejects_unrepresentable_computed_interest() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let coupon_date = Date::from_calendar_date(2025, Month::May, 15).expect("valid date");
        let from_date = Date::from_calendar_date(2025, Month::March, 31).expect("valid date");

        // Both a finite Decimal overflow and an f64 overflow must return an
        // error rather than panic while rebuilding an otherwise valid flow.
        for rate in [2.0, f64::MAX] {
            let original = schedule(
                vec![CashFlow::new(
                    coupon_date,
                    None,
                    Money::from((-1_i64, Currency::USD)),
                    CFKind::Fixed,
                    1.0,
                    Some(rate),
                )],
                1.0,
                issue,
            );
            let error = rebuild_residual_interest(
                &original,
                Money::new(6e28, Currency::USD).expect("valid money fixture"),
                from_date,
            )
            .expect_err("unrepresentable rebuilt interest must return an error");
            assert!(error.to_string().contains("cannot rebuild Fixed interest"));
            assert_eq!(original.get_flows()[0].amount.amount(), -1.0);
        }
    }

    #[test]
    fn rebuild_leaves_scheduled_amort_unchanged() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let q2 = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let from_date = Date::from_calendar_date(2025, Month::March, 31).expect("valid date");

        let original = schedule(
            vec![
                CashFlow::new(
                    q2,
                    None,
                    Money::from((-20_000_i64, Currency::USD)),
                    CFKind::Fixed,
                    0.25,
                    Some(0.08),
                ),
                CashFlow::new(
                    q2,
                    None,
                    Money::from((100_000_i64, Currency::USD)),
                    CFKind::Amortization,
                    0.0,
                    None,
                ),
            ],
            1_000_000.0,
            issue,
        );

        let rebuilt = rebuild_residual_interest(
            &original,
            Money::from((500_000_i64, Currency::USD)),
            from_date,
        )
        .expect("rebuild");

        let amort = rebuilt
            .get_flows()
            .iter()
            .find(|cf| cf.kind == CFKind::Amortization)
            .expect("amort kept");
        assert_eq!(amort.amount.amount(), 100_000.0);
    }
}
