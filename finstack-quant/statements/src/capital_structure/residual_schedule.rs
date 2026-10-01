//! Reproject future coupons after a dated discretionary balance change.
//! Contractual flows and earned accrual are preserved when reporting periods change.

use super::PrincipalClaim;
use crate::error::{Error, Result};
use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_cashflows::primitives::CFKind;
use finstack_quant_core::cashflow::{CashFlow, CashFlowAccrual};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::money::Money;

fn is_rebuildable_interest(kind: CFKind) -> bool {
    matches!(
        kind,
        CFKind::Fixed | CFKind::Stub | CFKind::FloatReset | CFKind::Pik
    )
}

/// Apply a closing-balance change at the period boundary, preserving prior accrual.
///
/// # Arguments
///
/// * `schedule` - Contractual schedule, including previously realized flows.
///   Coupon amounts retain their individual cash or PIK share; principal
///   movements follow their economic dates independently of settlement dates.
/// * `new_outstanding` - Nonnegative closing principal in the schedule currency.
/// * `from_date` - Inclusive balance snapshot. Changed principal earns interest
///   from the following day; coupons already earned remain payable.
/// * `preserved_cash_claims` - Principal already paid or carried as cash arrears
///   whose economic date remains after `from_date`. Reserve these amounts
///   before sizing future repayments so their reductions retain their dates.
///
/// # Errors
///
/// Returns an error for currency mismatches, missing accrual anchors, invalid
/// day counts or rates, and amounts outside Money's representation range.
pub(crate) fn rebuild_residual_interest(
    schedule: &CashFlowSchedule,
    new_outstanding: Money,
    from_date: Date,
    preserved_cash_claims: &[PrincipalClaim],
) -> Result<CashFlowSchedule> {
    let currency = schedule.get_notional().initial.currency();
    if new_outstanding.currency() != currency {
        return Err(Error::currency_mismatch(
            currency,
            new_outstanding.currency(),
        ));
    }
    if new_outstanding.amount() < 0.0 {
        return Err(Error::capital_structure(
            "Closing outstanding cannot be negative",
        ));
    }
    let original_path = schedule.outstanding_by_date()?;
    let initial = schedule.get_notional().initial.amount();
    let old_closing = original_path
        .iter()
        .rev()
        .find(|(date, _)| *date <= from_date)
        .map_or_else(
            || {
                if schedule
                    .get_meta()
                    .issue_date
                    .is_some_and(|issue| from_date < issue)
                {
                    0.0
                } else {
                    initial
                }
            },
            |(_, balance)| balance.amount(),
        );
    let delta = new_outstanding.amount() - old_closing;
    if delta.abs() <= 1e-9 {
        return Ok(schedule.clone());
    }
    let effective = from_date
        .next_day()
        .ok_or_else(|| Error::capital_structure("Balance change date out of range"))?;
    // A zero-cash row records the balance override without booking cash twice.
    // Keep every original row below: earned coupons and unsettled cash can
    // belong to an already-effective principal movement.
    let mut flows = Vec::with_capacity(schedule.get_flows().len() + 1);
    flows.push(
        CashFlow::new(
            from_date,
            None,
            Money::from((0_i64, currency)),
            CFKind::PrePayment,
            0.0,
            None,
        )
        .with_principal_delta(Money::new(delta, currency)?),
    );
    let mut changes = std::collections::BTreeMap::from([(effective, delta)]);
    let mut new_balance = new_outstanding.amount();
    let mut reserved = std::collections::BTreeMap::<(Date, Date), f64>::new();
    let mut reserved_total = 0.0;
    for claim in preserved_cash_claims {
        if claim.amount.currency() != currency {
            return Err(Error::currency_mismatch(currency, claim.amount.currency()));
        }
        if claim.amount.amount() < 0.0 {
            return Err(Error::capital_structure(
                "Preserved principal claims cannot be negative",
            ));
        }
        if claim.balance_date > from_date {
            let amount = claim.amount.amount();
            *reserved
                .entry((claim.payment_date, claim.balance_date))
                .or_default() += amount;
            reserved_total += amount;
        }
    }
    if reserved_total > new_balance + 1e-6 {
        return Err(Error::capital_structure(
            "Preserved principal claims exceed closing economic principal",
        ));
    }
    let mut economic_order: Vec<_> = schedule.get_flows().iter().collect();
    economic_order.sort_by_key(|flow| {
        let date = if is_rebuildable_interest(flow.kind) && flow.kind != CFKind::Pik {
            flow.accrual
                .as_ref()
                .map_or(flow.date, |accrual| accrual.end)
        } else {
            flow.get_balance_date()
        };
        // Capitalize the final PIK coupon before sizing redemption, even when
        // their payment dates differ. Other repayments retain schedule order.
        let rank = if flow.kind == CFKind::Notional && principal_change(flow) < 0.0 {
            2
        } else if is_rebuildable_interest(flow.kind) {
            1
        } else {
            0
        };
        (date, rank)
    });
    for original in economic_order {
        let mut replacement = vec![original.clone()];
        let principal_is_future = original.get_balance_date() > from_date;
        let cash_is_future = original.date > from_date;
        if is_rebuildable_interest(original.kind)
            && if original.kind == CFKind::Pik {
                principal_is_future
            } else {
                cash_is_future
            }
        {
            let accrual = original
                .accrual
                .clone()
                .or_else(|| {
                    let start = schedule
                        .get_flows()
                        .iter()
                        .filter(|flow| flow.date < original.date && flow.kind == original.kind)
                        .map(|flow| flow.date)
                        .max()
                        .or(schedule.get_meta().issue_date)?;
                    Some(CashFlowAccrual {
                        coupon_period: None,
                        end_is_termination_date: false,
                        start,
                        end: original.date,
                        day_count: schedule.get_day_count(),
                        projected_index_rate: None,
                        calendar_id: None,
                    })
                })
                .ok_or_else(|| {
                    Error::capital_structure("Cannot rebuild interest without an accrual start")
                })?;
            // A paid-down balance cannot cancel interest already earned,
            // including PIK capitalizing after its accrual period ends.
            if accrual.end <= effective {
                if principal_is_future {
                    new_balance += principal_change(original);
                }
                flows.extend(replacement);
                continue;
            }
            let context = DayCountContext {
                calendar: accrual
                    .calendar_id
                    .as_deref()
                    .map(finstack_quant_core::dates::calendar_by_id_strict)
                    .transpose()?,
                coupon_period: accrual.coupon_period.or(Some((accrual.start, accrual.end))),
                end_is_termination_date: accrual.end_is_termination_date,
                ..Default::default()
            };
            let total = accrual
                .day_count
                .year_fraction(accrual.start, accrual.end, context)?;
            // Canonical coupons are emitted in constant-principal accrual
            // segments. `rate` on a split cash/PIK row is the full coupon
            // rate, so its amount and dated principal own the component's
            // sensitivity. Using the full rate independently for both rows
            // would apply every balance change twice.
            let principal = original_path
                .iter()
                .rev()
                .find(|(date, _)| *date <= accrual.start)
                .map_or(initial, |(_, balance)| balance.amount());
            let rate = if principal == 0.0 {
                // A previous rebuild can zero a whole component without
                // extinguishing its coupon terms. Rebuilt rows retain the
                // signed component rate for a subsequent balance increase.
                original.rate.unwrap_or(f64::NAN)
            } else {
                original.amount.amount() / principal / original.accrual_factor
            };
            if !rate.is_finite()
                || original.rate.is_some_and(|rate| !rate.is_finite())
                || total <= 0.0
                || !total.is_finite()
            {
                return Err(Error::capital_structure(
                    "Cannot infer a finite coupon-component rate or accrual interval",
                ));
            }
            let mut boundaries = vec![accrual.start];
            boundaries.extend(
                changes
                    .keys()
                    .filter(|date| **date > accrual.start && **date < accrual.end)
                    .copied(),
            );
            boundaries.push(accrual.end);
            replacement.clear();
            for bounds in boundaries.windows(2) {
                let fraction = accrual.day_count.year_fraction(
                    bounds[0],
                    bounds[1],
                    DayCountContext {
                        end_is_termination_date: accrual.end_is_termination_date
                            && bounds[1] == accrual.end,
                        ..context
                    },
                )? / total;
                let balance_delta: f64 = changes.range(..=bounds[0]).map(|(_, delta)| delta).sum();
                let factor = original.accrual_factor * fraction;
                let amount = original.amount.amount() * fraction + balance_delta * rate * factor;
                let mut flow = original.clone();
                flow.amount = Money::new(amount, currency).map_err(|error| {
                    Error::capital_structure(format!(
                        "cannot rebuild {:?} interest on {}: {error}",
                        flow.kind, flow.date
                    ))
                })?;
                // Preserve this row's own cash/PIK share even if its amount
                // becomes zero; a later rebuild must not recover the full
                // unsplit rate from a zero principal denominator.
                flow.rate = Some(rate);
                flow.accrual_factor = factor;
                flow.accrual = Some(CashFlowAccrual {
                    end_is_termination_date: accrual.end_is_termination_date
                        && bounds[1] == accrual.end,
                    start: bounds[0],
                    end: bounds[1],
                    ..accrual.clone()
                });
                // Explicit PIK deltas follow the reprojected interest, split in
                // the same proportions as the original principal capitalization.
                if let Some(principal) = original.principal_delta {
                    flow.principal_delta = Some(Money::new(
                        if original.amount.amount() == 0.0 {
                            principal.amount() * fraction
                        } else {
                            principal.amount() * amount / original.amount.amount()
                        },
                        currency,
                    )?);
                }
                replacement.push(flow);
            }
        } else if matches!(
            original.kind,
            CFKind::Amortization
                | CFKind::PrePayment
                | CFKind::RevolvingRepayment
                | CFKind::Notional
        ) && original.amount.amount() > 0.0
            && principal_is_future
        {
            // Paid advances and cash arrears retain their economic reductions.
            // Earlier future installments cannot consume that reserved face.
            let reservation = reserved
                .get_mut(&(original.date, original.get_balance_date()))
                .map_or(0.0, |remaining| {
                    let amount = remaining.min(original.amount.amount());
                    *remaining -= amount;
                    amount
                });
            reserved_total -= reservation;
            let available = (new_balance - reserved_total).max(0.0);
            let repayment = if original.kind == CFKind::Notional && cash_is_future {
                available
            } else {
                original.amount.amount().min(available)
            };
            if cash_is_future {
                replacement[0].amount = Money::new(repayment, currency)?;
            }
            if original.principal_delta.is_some() || !cash_is_future {
                replacement[0].principal_delta = Some(Money::new(-repayment, currency)?);
            }
        }
        let original_delta = principal_change(original);
        let revised_delta: f64 = replacement.iter().map(principal_change).sum();
        if principal_is_future {
            new_balance += revised_delta;
        }
        let change = revised_delta - original_delta;
        if change != 0.0 {
            *changes.entry(original.get_balance_date()).or_default() += change;
        }
        flows.extend(replacement);
    }
    if reserved_total > 1e-6 {
        return Err(Error::capital_structure(
            "Preserved principal claims must match future economic repayments",
        ));
    }
    Ok(CashFlowSchedule::from_parts(
        flows,
        schedule.get_notional().clone(),
        schedule.get_day_count(),
        schedule.get_meta().clone(),
    ))
}

fn principal_change(flow: &CashFlow) -> f64 {
    if let Some(delta) = flow.principal_delta {
        return delta.amount();
    }
    match flow.kind {
        CFKind::Pik => flow.amount.amount(),
        CFKind::Amortization
        | CFKind::PrePayment
        | CFKind::DefaultedNotional
        | CFKind::Notional
        | CFKind::RevolvingDraw
        | CFKind::RevolvingRepayment => -flow.amount.amount(),
        _ => 0.0,
    }
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
            &[],
        )
        .expect("rebuild");

        assert!(
            rebuilt.get_flows().iter().any(|cf| cf.date == q1_coupon),
            "realized coupons retain the historical accrual anchors"
        );
        assert_eq!(rebuilt.get_notional().initial.amount(), 1_000_000.0);

        let q2: f64 = rebuilt
            .get_flows()
            .iter()
            .filter(|cf| cf.date == q2_coupon)
            .map(|cf| cf.amount.amount())
            .sum();
        let remaining = (q2_coupon - from_date.next_day().unwrap()).whole_days() as f64;
        let full = (q2_coupon - q1_coupon).whole_days() as f64;
        assert!((q2 - (-20_000.0 + 10_000.0 * remaining / full)).abs() < 1e-9);
        let repeated = rebuild_residual_interest(
            &rebuilt,
            Money::from((500_000_i64, Currency::USD)),
            from_date,
            &[],
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(&rebuilt).unwrap(),
            serde_json::to_value(repeated).unwrap()
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
            &[],
        )
        .expect("rebuild with inferred rate");

        let q2: f64 = rebuilt
            .get_flows()
            .iter()
            .filter(|cf| cf.date == q2_coupon)
            .map(|cf| cf.amount.amount())
            .sum();
        let remaining = (q2_coupon - from_date.next_day().unwrap()).whole_days() as f64;
        let full = (q2_coupon - issue).whole_days() as f64;
        assert!((q2 - (-20_000.0 + 10_000.0 * remaining / full)).abs() < 1e-9);
    }

    #[test]
    fn rebuild_rejects_unrepresentable_computed_interest() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let coupon_date = Date::from_calendar_date(2025, Month::May, 15).expect("valid date");
        let from_date = Date::from_calendar_date(2025, Month::March, 31).expect("valid date");

        // A component rate is inferred from its amount and principal, so keep
        // the fixture economically consistent while exercising Decimal
        // overflow in the rebuilt amount.
        for rate in [10.0, 6e28] {
            let original = schedule(
                vec![CashFlow::new(
                    coupon_date,
                    None,
                    Money::new(-rate, Currency::USD).expect("representable coupon fixture"),
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
                &[],
            )
            .expect_err("unrepresentable rebuilt interest must return an error");
            assert!(error.to_string().contains("cannot rebuild Fixed interest"));
            assert_eq!(original.get_flows()[0].amount.amount(), -rate);
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
            &[],
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
