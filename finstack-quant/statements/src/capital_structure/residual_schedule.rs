//! Reproject future coupons after a dated discretionary balance change.
//! Contractual flows and earned accrual are preserved when reporting periods change.

use super::PrincipalClaim;
use crate::error::{Error, Result};
use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::cashflow::{CashFlow, CashFlowAccrual};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::money::Money;

fn is_rebuildable_interest(kind: CFKind) -> bool {
    matches!(
        kind,
        CFKind::Fixed | CFKind::Stub | CFKind::Floating | CFKind::Pik
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
        return Err(Error::CurrencyMismatch(
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
        .with_principal_delta(Money::new(delta, currency)?)
        .with_principal_date(from_date),
    );
    let mut changes = std::collections::BTreeMap::from([(effective, delta)]);
    let mut new_balance = new_outstanding.amount();
    let mut reserved = std::collections::BTreeMap::<(Date, Date), f64>::new();
    let mut reserved_total = 0.0;
    for claim in preserved_cash_claims {
        if claim.amount.currency() != currency {
            return Err(Error::CurrencyMismatch(currency, claim.amount.currency()));
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
        let date = if is_rebuildable_interest(flow.get_kind()) && flow.get_kind() != CFKind::Pik {
            flow.get_accrual()
                .as_ref()
                .map_or(flow.get_date(), |accrual| accrual.end)
        } else {
            economic_date(flow)
        };
        // Capitalize the final PIK coupon before sizing redemption, even when
        // their payment dates differ. Other repayments retain schedule order.
        let rank = if flow.get_kind() == CFKind::Notional && principal_change(flow) < 0.0 {
            2
        } else if is_rebuildable_interest(flow.get_kind()) {
            1
        } else {
            0
        };
        (date, rank)
    });
    for original in economic_order {
        let mut replacement = vec![original.clone()];
        let principal_is_future = economic_date(original) > from_date;
        let cash_is_future = original.get_date() > from_date;
        if is_rebuildable_interest(original.get_kind())
            && if original.get_kind() == CFKind::Pik {
                principal_is_future
            } else {
                cash_is_future
            }
        {
            let accrual = original
                .get_accrual()
                .clone()
                .or_else(|| {
                    let start = schedule
                        .get_flows()
                        .iter()
                        .filter(|flow| {
                            flow.get_date() < original.get_date()
                                && flow.get_kind() == original.get_kind()
                        })
                        .map(|flow| flow.get_date())
                        .max()
                        .or(schedule.get_meta().issue_date)?;
                    Some(CashFlowAccrual {
                        start,
                        end: original.get_date(),
                        day_count: schedule.get_day_count(),
                        projected_index_rate: None,
                        context: finstack_quant_core::dates::DayCountContextState {
                            calendar_id: None,
                            coupon_period: None,
                            end_is_termination_date: Some(false),
                            frequency: None,
                            bus_basis: None,
                        },
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
            let mut context = accrual.day_count_context()?;
            context.coupon_period = context.coupon_period.or(Some((accrual.start, accrual.end)));
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
            // A PIK row's interest is its explicit capitalization, not its
            // cash amount (record CORE-CONVENTIONS-015 #1, #4).
            let original_value = interest_value(original)?;
            let rate = if principal == 0.0 {
                // A previous rebuild can zero a whole component without
                // extinguishing its coupon terms. Rebuilt rows retain the
                // signed component rate for a subsequent balance increase.
                original.get_rate().unwrap_or(f64::NAN)
            } else {
                original_value / principal / original.get_accrual_factor()
            };
            if !rate.is_finite()
                || original.get_rate().is_some_and(|rate| !rate.is_finite())
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
                        end_is_termination_date: accrual
                            .context
                            .end_is_termination_date
                            .map(|termination| termination && bounds[1] == accrual.end),
                        ..context
                    },
                )? / total;
                let balance_delta: f64 = changes.range(..=bounds[0]).map(|(_, delta)| delta).sum();
                let factor = original.get_accrual_factor() * fraction;
                let value = original_value * fraction + balance_delta * rate * factor;
                let mut flow = original.clone();
                // A PIK row keeps its zero cash; its value is the delta below.
                if original.get_kind() != CFKind::Pik {
                    flow.amount = Money::new(value, currency).map_err(|error| {
                        Error::capital_structure(format!(
                            "cannot rebuild {:?} interest on {}: {error}",
                            flow.get_kind(),
                            flow.get_date()
                        ))
                    })?;
                }
                // Preserve this row's own cash/PIK share even if its amount
                // becomes zero; a later rebuild must not recover the full
                // unsplit rate from a zero principal denominator.
                flow.rate = Some(rate);
                flow.accrual_factor = factor;
                flow.accrual = Some(CashFlowAccrual {
                    start: bounds[0],
                    end: bounds[1],
                    context: finstack_quant_core::dates::DayCountContextState {
                        end_is_termination_date: accrual
                            .context
                            .end_is_termination_date
                            .map(|termination| termination && bounds[1] == accrual.end),
                        ..accrual.context.clone()
                    },
                    ..accrual.clone()
                });
                // A PIK row capitalizes exactly its reprojected interest; any
                // other explicit delta splits with the reprojected cash.
                if original.get_kind() == CFKind::Pik {
                    flow.principal_delta = Some(Money::new(value, currency)?);
                } else if let Some(principal) = original.get_principal_delta() {
                    flow.principal_delta = Some(Money::new(
                        if original.get_amount().amount() == 0.0 {
                            principal.amount() * fraction
                        } else {
                            principal.amount() * value / original.get_amount().amount()
                        },
                        currency,
                    )?);
                }
                replacement.push(flow);
            }
        } else if matches!(
            original.get_kind(),
            CFKind::Amortization
                | CFKind::PrePayment
                | CFKind::RevolvingRepayment
                | CFKind::Notional
        ) && original.get_amount().amount() > 0.0
            && principal_is_future
        {
            // Paid advances and cash arrears retain their economic reductions.
            // Earlier future installments cannot consume that reserved face.
            let reservation = reserved
                .get_mut(&(original.get_date(), economic_date(original)))
                .map_or(0.0, |remaining| {
                    let amount = remaining.min(original.get_amount().amount());
                    *remaining -= amount;
                    amount
                });
            reserved_total -= reservation;
            let available = (new_balance - reserved_total).max(0.0);
            let repayment = if original.get_kind() == CFKind::Notional && cash_is_future {
                available
            } else {
                original.get_amount().amount().min(available)
            };
            if cash_is_future {
                replacement[0].amount = Money::new(repayment, currency)?;
            }
            replacement[0].principal_delta = Some(Money::new(-repayment, currency)?);
        }
        let original_delta = principal_change(original);
        let revised_delta: f64 = replacement.iter().map(principal_change).sum();
        if principal_is_future {
            new_balance += revised_delta;
        }
        let change = revised_delta - original_delta;
        if change != 0.0 {
            *changes.entry(economic_date(original)).or_default() += change;
        }
        flows.extend(replacement);
    }
    if reserved_total > 1e-6 {
        return Err(Error::capital_structure(
            "Preserved principal claims must match future economic repayments",
        ));
    }
    Ok(CashFlowSchedule::from_flows(
        flows,
        schedule.get_notional().clone(),
        schedule.get_day_count(),
        schedule.get_meta().clone(),
    )?)
}

/// Interest a rebuildable row carries: a PIK row's explicit capitalization,
/// any other row's cash amount.
fn interest_value(flow: &CashFlow) -> Result<f64> {
    if flow.get_kind() != CFKind::Pik {
        return Ok(flow.get_amount().amount());
    }
    flow.get_principal_delta()
        .map(|delta| delta.amount())
        .ok_or_else(|| {
            Error::capital_structure(format!(
                "PIK row on {} has no principal_delta; its capitalized amount must be explicit",
                flow.get_date()
            ))
        })
}

/// The date a row takes economic effect: a principal row's explicit balance
/// date, or, for a row that moves no principal, its own payment date. A
/// principal row always states its balance date (record CORE-CONVENTIONS-015
/// #4, #6), so nothing is inferred for principal.
fn economic_date(flow: &CashFlow) -> Date {
    match flow.get_balance_date() {
        Some(balance_date) => balance_date,
        None => flow.get_date(),
    }
}

/// The explicit principal delta of a row; a non-principal row moves nothing.
/// Nothing is inferred from cash or kind (record CORE-CONVENTIONS-015 #4).
fn principal_change(flow: &CashFlow) -> f64 {
    flow.get_principal_delta()
        .map_or(0.0, |delta| delta.amount())
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
        CashFlowSchedule::from_flows(
            flows,
            Notional::par(notional, Currency::USD).expect("valid notional fixture"),
            DayCount::Act365F,
            CashFlowMeta {
                issue_date: Some(issue),
                ..CashFlowMeta::default()
            },
        )
        .expect("valid cashflow rows")
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
            rebuilt
                .get_flows()
                .iter()
                .any(|cf| cf.get_date() == q1_coupon),
            "realized coupons retain the historical accrual anchors"
        );
        assert_eq!(rebuilt.get_notional().initial.amount(), 1_000_000.0);

        let q2: f64 = rebuilt
            .get_flows()
            .iter()
            .filter(|cf| cf.get_date() == q2_coupon)
            .map(|cf| cf.get_amount().amount())
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
                {
                    let cash = Money::from((-1_000_000_i64, Currency::USD));
                    CashFlow::new(issue, None, cash, CFKind::Notional, 0.0, None)
                        .with_principal_delta(Money::zero(Currency::USD))
                        .with_principal_date(issue)
                },
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
            .filter(|cf| cf.get_date() == q2_coupon)
            .map(|cf| cf.get_amount().amount())
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
            assert_eq!(original.get_flows()[0].get_amount().amount(), -rate);
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
                {
                    let cash = Money::from((100_000_i64, Currency::USD));
                    CashFlow::new(q2, None, cash, CFKind::Amortization, 0.0, None)
                        .with_principal_delta(cash.checked_neg())
                        .with_principal_date(q2)
                },
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
            .find(|cf| cf.get_kind() == CFKind::Amortization)
            .expect("amort kept");
        assert_eq!(amort.get_amount().amount(), 100_000.0);
    }
}
