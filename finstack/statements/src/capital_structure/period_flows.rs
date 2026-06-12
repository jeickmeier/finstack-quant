//! Per-period cashflow extraction for a single instrument.
//!
//! Extracted from `integration.rs` so the schedule-classification logic and
//! its safety clamps live next to their tests, and so the multi-instrument
//! `aggregate_instrument_cashflows` driver in `integration.rs` can stay
//! focused on currency / FX bookkeeping. The logic here:
//!
//! 1. Pulls the full `CashFlowSchedule` for an instrument.
//! 2. Computes a stateful `scale` factor that relates the model's
//!    period-opening balance to the schedule's notional opening — clamped to
//!    [0.0, 1.10] to prevent silent cashflow amplification (see
//!    `SCALE_CLAMP_MAX`).
//! 3. Classifies each in-period flow by `CFKind` into the
//!    [`CashflowBreakdown`] buckets (cash interest, PIK interest, principal,
//!    fees) — credit events and unknown variants emit warnings instead of
//!    being silently aggregated.
//! 4. Snapshots the closing balance and accrued interest at `period.end - 1`.

use crate::capital_structure::cashflows::CashflowBreakdown;
use crate::error::Result;
use crate::evaluator::EvalWarning;
use finstack_cashflows::primitives::CFKind;
use finstack_cashflows::CashflowProvider;
use finstack_cashflows::{accrued_interest_amount, AccrualConfig};
use finstack_core::dates::{Date, Period};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;

/// Snapshot date used for "end of period" quantities under half-open period semantics `[start, end)`.
///
/// We use `end - 1 day` so that cashflows dated exactly on `period.end` are attributed to the
/// *next* period and do not incorrectly affect the prior period's end-of-period balance/accrual.
pub(crate) fn period_snapshot_date(period: &Period) -> Date {
    if period.end <= period.start {
        return period.start;
    }
    period.end - time::Duration::days(1)
}

/// Calculate contractual flows for a single period.
///
/// This helper extracts flows for a specific period from an instrument's full schedule,
/// returning a CashflowBreakdown for that period. Used for dynamic period-by-period evaluation.
///
/// Periods are treated with half-open semantics `[start, end)`. End-of-period
/// balances and accruals are therefore snapped at `period.end - 1 day` so
/// cashflows occurring exactly on the next period boundary are not attributed
/// to the prior period.
///
/// # Arguments
///
/// * `instrument` - The instrument to calculate flows for
/// * `period` - The period to extract flows for
/// * `opening_balance` - Opening balance at the start of the period
/// * `toggled_pik_capitalized` - Cumulative principal capitalized via the PIK
///   *toggle* (see [`crate::capital_structure::CapitalStructureState::cumulative_toggled_pik`]).
///   Excluded from the scale-clamp basis so toggle-driven PIK compounding is
///   not frozen by the clamp; pass zero when no toggle state exists.
/// * `market_ctx` - Market context for pricing
/// * `as_of` - Valuation date
///
/// # Returns
///
/// Returns a tuple of:
/// - [`CashflowBreakdown`] for the period
/// - closing balance after scheduled flows
/// - evaluation warnings for ignored or unsupported cashflow kinds
///
/// # Errors
///
/// Returns an error if the instrument schedule cannot be built, if currencies
/// are inconsistent, or if accrued interest cannot be computed.
///
/// # References
///
/// - Cashflow discounting and schedule context: `docs/REFERENCES.md#hull-options-futures`
/// - Fixed-income balance/risk interpretation: `docs/REFERENCES.md#tuckman-serrat-fixed-income`
pub fn calculate_period_flows(
    instrument: &dyn CashflowProvider,
    period: &Period,
    opening_balance: Money,
    toggled_pik_capitalized: Money,
    market_ctx: &MarketContext,
    as_of: Date,
) -> Result<(CashflowBreakdown, Money, Vec<EvalWarning>)> {
    let full_schedule = instrument.cashflow_schedule(market_ctx, as_of)?;
    let currency = full_schedule.notional.initial.currency();
    if opening_balance.amount() != 0.0 && opening_balance.currency() != currency {
        return Err(crate::error::Error::currency_mismatch(
            currency,
            opening_balance.currency(),
        ));
    }
    if toggled_pik_capitalized.amount() != 0.0 && toggled_pik_capitalized.currency() != currency {
        return Err(crate::error::Error::currency_mismatch(
            currency,
            toggled_pik_capitalized.currency(),
        ));
    }
    let mut breakdown = CashflowBreakdown::with_currency(currency);
    let mut warnings = Vec::new();
    let snapshot_date = period_snapshot_date(period);
    let outstanding_path = full_schedule.outstanding_by_date()?;
    // Boundary convention: flows dated exactly on `period.start` belong to
    // *this* period (half-open `[start, end)`), so the scheduled opening must
    // be the balance strictly *before* `period.start` to match the
    // `end - 1 day` closing snapshot of the prior period.
    let scheduled_opening = outstanding_path
        .iter()
        .filter(|(d, _)| *d < period.start)
        .map(|(_, balance)| {
            if balance.amount() < 0.0 {
                Money::new(-balance.amount(), balance.currency())
            } else {
                *balance
            }
        })
        .next_back()
        .unwrap_or(full_schedule.notional.initial);

    // Use opening balance to scale cashflows when the schedule notional differs from the
    // stateful outstanding (e.g., after applying sweeps). This is an approximation but
    // prevents obviously overstated interest after large paydowns.
    //
    // The clamp is intentionally tight (1.10) — the scale factor exists to handle
    // small drift from sweeps and rounding, NOT to silently amplify cashflows by
    // 50%+ when the schedule and stateful balance have diverged. A scale > 1.10
    // historically indicated a modeling error (e.g. wrong opening-balance source);
    // we now warn at 1.05 and refuse to amplify beyond 1.10.
    const SCALE_WARN_THRESHOLD: f64 = 1.05;
    const SCALE_CLAMP_MAX: f64 = 1.10;
    let scale = if opening_balance.amount() == 0.0 {
        if scheduled_opening.amount() == 0.0 {
            1.0
        } else {
            0.0
        }
    } else if scheduled_opening.amount() == 0.0 {
        1.0
    } else {
        // Toggle-driven PIK capitalization grows the stateful balance beyond
        // the contractual schedule by design. Exclude that increment from the
        // clamp basis (so legitimate PIK compounding is never frozen by the
        // clamp) while still accruing interest on the full stateful balance:
        // scale = clamp((opening - toggled_pik) / scheduled) + toggled_pik / scheduled.
        let pik_increment = toggled_pik_capitalized.amount().max(0.0);
        let adjusted_opening = (opening_balance.amount() - pik_increment).max(0.0);
        let adjusted_ratio = adjusted_opening / scheduled_opening.amount();
        let pik_part = pik_increment / scheduled_opening.amount();
        if adjusted_ratio > SCALE_WARN_THRESHOLD {
            let clamped = adjusted_ratio.clamp(0.0, SCALE_CLAMP_MAX);
            tracing::warn!(
                raw_scale = adjusted_ratio,
                clamped_scale = clamped,
                period = ?period.id,
                "Scale factor between opening balance (ex toggled-PIK) and scheduled opening exceeds {SCALE_WARN_THRESHOLD}; \
                 clamping to {SCALE_CLAMP_MAX} to prevent cashflow amplification. \
                 This typically indicates an unscheduled paydown / re-draw mismatch — verify the model."
            );
            warnings.push(EvalWarning::CapitalStructureCashflowIgnored {
                period: period.id,
                kind: format!("scale_clamped(raw={adjusted_ratio:.4}, clamped={clamped:.4})"),
                cashflow_date: period.start.to_string(),
            });
        }
        adjusted_ratio.clamp(0.0, SCALE_CLAMP_MAX) + pik_part
    };

    // Extract flows that fall within this period
    for cf in &full_schedule.flows {
        if cf.date >= period.start && cf.date < period.end {
            let scaled_abs_value =
                Money::new(cf.amount.amount().abs() * scale, cf.amount.currency());

            match cf.kind {
                CFKind::Fixed | CFKind::Stub | CFKind::FloatReset => {
                    breakdown.interest_expense_cash += scaled_abs_value;
                }
                CFKind::Amortization | CFKind::PrePayment | CFKind::RevolvingRepayment => {
                    breakdown.principal_payment += scaled_abs_value;
                }
                CFKind::Notional if cf.amount.amount() > 0.0 => {
                    breakdown.principal_payment += scaled_abs_value;
                }
                CFKind::CommitmentFee | CFKind::FacilityFee => {
                    // Commitment / facility fees accrue on the undrawn
                    // commitment or the total facility size, NOT on the drawn
                    // balance — they must not be scaled by the drawn-balance
                    // ratio. Pass them through at the scheduled amount.
                    breakdown.fees += Money::new(cf.amount.amount().abs(), cf.amount.currency());
                }
                CFKind::Fee | CFKind::UsageFee => {
                    // Generic and usage (drawn-balance based) fees scale with
                    // the stateful/scheduled balance ratio like interest.
                    breakdown.fees += scaled_abs_value;
                }
                CFKind::PIK => {
                    breakdown.interest_expense_pik += scaled_abs_value;
                }
                CFKind::Notional | CFKind::RevolvingDraw => {
                    // Funding / draw events are not treated as scheduled principal payments in statements.
                }
                CFKind::DefaultedNotional | CFKind::Recovery => {
                    // Credit events are not modeled as part of standard debt service in statements.
                    warnings.push(EvalWarning::CapitalStructureCashflowIgnored {
                        period: period.id,
                        kind: format!("{:?}", cf.kind),
                        cashflow_date: cf.date.to_string(),
                    });
                    tracing::warn!(
                        "Ignoring credit-event CFKind={:?} for period flow calc (date={:?})",
                        cf.kind,
                        cf.date
                    );
                }
                _ => {
                    // CFKind is non-exhaustive; ignore unknown variants to avoid misclassification.
                    warnings.push(EvalWarning::CapitalStructureCashflowIgnored {
                        period: period.id,
                        kind: format!("{:?}", cf.kind),
                        cashflow_date: cf.date.to_string(),
                    });
                    tracing::warn!(
                        "Unhandled CFKind={:?} for period flow calc (date={:?}); ignoring",
                        cf.kind,
                        cf.date
                    );
                }
            }
        }
    }

    // Get closing balance from outstanding_by_date.
    // Find the most recent outstanding balance at or before period end.
    // Note: outstanding_path only has entries on dates when cashflows occur,
    // so we need to find the latest entry <= period.end to get the correct balance.
    let scheduled_closing_balance = outstanding_path
        .iter()
        .rev()
        .find(|(date, _)| *date <= snapshot_date)
        .map(|(_, balance)| {
            if balance.amount() < 0.0 {
                Money::new(-balance.amount(), balance.currency())
            } else {
                *balance
            }
        })
        .unwrap_or_else(|| {
            // No outstanding entry at or before the snapshot. If the
            // instrument has not been issued yet (forward-dated / delayed
            // draw), the balance is zero — reporting the full notional
            // pre-issuance would overstate leverage. Otherwise (an issued
            // instrument whose first flow lands in a later period) fall back
            // to the initial notional.
            let pre_issuance = full_schedule
                .meta
                .issue_date
                .is_some_and(|issue| issue > snapshot_date)
                && outstanding_path
                    .first()
                    .is_some_and(|(date, _)| *date > snapshot_date);
            if pre_issuance {
                return Money::new(0.0, currency);
            }
            let initial = full_schedule.notional.initial;
            if initial.amount() < 0.0 {
                Money::new(-initial.amount(), initial.currency())
            } else {
                initial
            }
        });

    let has_new_funding = full_schedule.flows.iter().any(|cf| {
        (cf.date >= period.start
            && cf.date < period.end
            && matches!(cf.kind, CFKind::RevolvingDraw))
            || (cf.date >= period.start
                && cf.date < period.end
                && matches!(cf.kind, CFKind::Notional)
                && cf.amount.amount() <= 0.0)
    });
    let net_new_funding: f64 = full_schedule
        .flows
        .iter()
        .filter(|cf| cf.date >= period.start && cf.date < period.end)
        .filter_map(|cf| match cf.kind {
            CFKind::RevolvingDraw => Some(cf.amount.amount().abs()),
            CFKind::Notional if cf.amount.amount() <= 0.0 => Some(cf.amount.amount().abs()),
            _ => None,
        })
        .sum();
    let closing_balance = if opening_balance.amount() == 0.0 {
        if has_new_funding {
            // The stateful balance is zero (e.g. a revolver fully swept in a
            // prior period). New draws this period establish a fresh balance
            // of draws net of in-period repayments — taking the scheduled
            // closing balance here would resurrect the swept balance.
            let in_period_repayments: f64 = full_schedule
                .flows
                .iter()
                .filter(|cf| cf.date >= period.start && cf.date < period.end)
                .filter_map(|cf| match cf.kind {
                    CFKind::Amortization | CFKind::PrePayment | CFKind::RevolvingRepayment => {
                        Some(cf.amount.amount().abs())
                    }
                    CFKind::Notional if cf.amount.amount() > 0.0 => Some(cf.amount.amount().abs()),
                    _ => None,
                })
                .sum();
            Money::new((net_new_funding - in_period_repayments).max(0.0), currency)
        } else {
            Money::new(0.0, currency)
        }
    } else {
        scheduled_closing_balance
    };
    breakdown.debt_balance = closing_balance;

    // Calculate accrued interest at period end.
    // Note: detailed accrual config (day count, compounding) comes from the schedule itself.
    // Limitation: `AccrualConfig::default()` leaves `frequency: None`, which is
    // incorrect for ACT/ACT ISMA schedules (the accrual engine falls back to
    // ISDA semantics). `CashFlowMeta` does not carry the coupon frequency, so
    // it cannot be populated from what is visible here.
    let accrued_scalar =
        accrued_interest_amount(&full_schedule, snapshot_date, &AccrualConfig::default())?;
    let accrued_interest = if opening_balance.amount() == 0.0 && !has_new_funding {
        0.0
    } else {
        accrued_scalar * scale
    };
    breakdown.accrued_interest = Money::new(accrued_interest, currency);

    Ok((breakdown, closing_balance, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_cashflows::builder::{CashFlowMeta, CashFlowSchedule, Notional};
    use finstack_cashflows::primitives::CFKind;
    use finstack_core::cashflow::CashFlow;
    use finstack_core::currency::Currency;
    use finstack_core::dates::{DayCount, PeriodId};
    use finstack_core::money::Money;
    use time::Month;

    struct SignedFlowInstrument {
        schedule: CashFlowSchedule,
    }

    impl CashflowProvider for SignedFlowInstrument {
        fn cashflow_schedule(
            &self,
            _curves: &MarketContext,
            _as_of: Date,
        ) -> finstack_core::Result<CashFlowSchedule> {
            Ok(self.schedule.clone())
        }
    }

    #[test]
    fn calculate_period_flows_normalizes_interest_to_issuer_outflow() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let period = Period {
            id: PeriodId::quarter(2025, 1),
            start,
            end,
            is_actual: false,
        };

        let instrument = SignedFlowInstrument {
            schedule: CashFlowSchedule {
                flows: vec![CashFlow {
                    date: Date::from_calendar_date(2025, Month::February, 15).expect("valid date"),
                    reset_date: None,
                    amount: Money::new(-50_000.0, Currency::USD),
                    kind: CFKind::Fixed,
                    accrual_factor: 0.25,
                    rate: None,
                }],
                notional: Notional::par(1_000_000.0, Currency::USD),
                day_count: DayCount::Act365F,
                meta: CashFlowMeta {
                    issue_date: Some(start),
                    ..CashFlowMeta::default()
                },
            },
        };

        let market_ctx = MarketContext::new();
        let (breakdown, _, warnings) = calculate_period_flows(
            &instrument,
            &period,
            Money::new(1_000_000.0, Currency::USD),
            Money::new(0.0, Currency::USD),
            &market_ctx,
            start,
        )
        .expect("period flow calculation should succeed");

        assert!(warnings.is_empty());
        assert_eq!(breakdown.interest_expense_cash.amount(), 50_000.0);
    }

    #[test]
    fn calculate_period_flows_zero_opening_balance_zeroes_contractual_flows() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let period = Period {
            id: PeriodId::quarter(2025, 1),
            start,
            end,
            is_actual: false,
        };

        let instrument = SignedFlowInstrument {
            schedule: CashFlowSchedule {
                flows: vec![
                    CashFlow {
                        date: Date::from_calendar_date(2025, Month::February, 15)
                            .expect("valid date"),
                        reset_date: None,
                        amount: Money::new(-50_000.0, Currency::USD),
                        kind: CFKind::Fixed,
                        accrual_factor: 0.25,
                        rate: None,
                    },
                    CashFlow {
                        date: Date::from_calendar_date(2025, Month::March, 15).expect("valid date"),
                        reset_date: None,
                        amount: Money::new(-100_000.0, Currency::USD),
                        kind: CFKind::Amortization,
                        accrual_factor: 0.0,
                        rate: None,
                    },
                ],
                notional: Notional::par(1_000_000.0, Currency::USD),
                day_count: DayCount::Act365F,
                meta: CashFlowMeta {
                    issue_date: Some(start),
                    ..CashFlowMeta::default()
                },
            },
        };

        let market_ctx = MarketContext::new();
        let (breakdown, closing_balance, warnings) = calculate_period_flows(
            &instrument,
            &period,
            Money::new(0.0, Currency::USD),
            Money::new(0.0, Currency::USD),
            &market_ctx,
            start,
        )
        .expect("period flow calculation should succeed");

        assert!(warnings.is_empty());
        assert_eq!(breakdown.interest_expense_cash.amount(), 0.0);
        assert_eq!(breakdown.principal_payment.amount(), 0.0);
        assert_eq!(breakdown.accrued_interest.amount(), 0.0);
        assert_eq!(breakdown.debt_balance.amount(), 0.0);
        assert_eq!(closing_balance.amount(), 0.0);
    }

    #[test]
    fn calculate_period_flows_zero_opening_balance_preserves_new_draws() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let period = Period {
            id: PeriodId::quarter(2025, 1),
            start,
            end,
            is_actual: false,
        };

        let instrument = SignedFlowInstrument {
            schedule: CashFlowSchedule {
                flows: vec![CashFlow {
                    date: Date::from_calendar_date(2025, Month::February, 15).expect("valid date"),
                    reset_date: None,
                    amount: Money::new(100_000.0, Currency::USD),
                    kind: CFKind::RevolvingDraw,
                    accrual_factor: 0.0,
                    rate: None,
                }],
                notional: Notional::par(0.0, Currency::USD),
                day_count: DayCount::Act365F,
                meta: CashFlowMeta {
                    issue_date: Some(start),
                    ..CashFlowMeta::default()
                },
            },
        };

        let market_ctx = MarketContext::new();
        let (breakdown, closing_balance, warnings) = calculate_period_flows(
            &instrument,
            &period,
            Money::new(0.0, Currency::USD),
            Money::new(0.0, Currency::USD),
            &market_ctx,
            start,
        )
        .expect("period flow calculation should succeed");

        assert!(warnings.is_empty());
        assert_eq!(breakdown.interest_expense_cash.amount(), 0.0);
        assert_eq!(breakdown.principal_payment.amount(), 0.0);
        assert_eq!(breakdown.debt_balance.amount(), 100_000.0);
        assert_eq!(closing_balance.amount(), 100_000.0);
    }

    /// Regression test for the scale-denominator off-by-one (review B2):
    /// a quarterly amortizing loan whose payments land exactly on period
    /// boundaries must produce per-period interest equal to the raw schedule
    /// with scale = 1.0 and no `scale_clamped` warning. The old
    /// `*d <= period.start` filter took the post-payment balance as the
    /// scheduled opening, inflating every flow by opening/post-payment.
    #[test]
    fn boundary_dated_amortization_matches_raw_schedule_without_scale_warning() {
        let q_start = |q: u8| {
            let month = match q {
                1 => Month::January,
                2 => Month::April,
                3 => Month::July,
                _ => Month::October,
            };
            Date::from_calendar_date(2025, month, 1).expect("valid date")
        };
        let issue = q_start(1);

        // Quarterly amortizer: payments dated exactly on the period
        // boundaries Apr 1 / Jul 1 / Oct 1 (1% coupon on the pre-payment
        // balance + 100k amortization).
        let mut flows = Vec::new();
        let mut outstanding = 1_000_000.0;
        for q in 2..=4 {
            flows.push(CashFlow {
                date: q_start(q),
                reset_date: None,
                amount: Money::new(-outstanding * 0.01, Currency::USD),
                kind: CFKind::Fixed,
                accrual_factor: 0.25,
                rate: Some(0.04),
            });
            flows.push(CashFlow {
                date: q_start(q),
                reset_date: None,
                amount: Money::new(100_000.0, Currency::USD),
                kind: CFKind::Amortization,
                accrual_factor: 0.0,
                rate: None,
            });
            outstanding -= 100_000.0;
        }
        let instrument = SignedFlowInstrument {
            schedule: CashFlowSchedule {
                flows,
                notional: Notional::par(1_000_000.0, Currency::USD),
                day_count: DayCount::Act365F,
                meta: CashFlowMeta {
                    issue_date: Some(issue),
                    ..CashFlowMeta::default()
                },
            },
        };

        let market_ctx = MarketContext::new();
        let mut opening = Money::new(1_000_000.0, Currency::USD);
        // Q2..Q4 each contain one boundary-dated coupon + amortization.
        let expected_interest = [10_000.0, 9_000.0, 8_000.0];
        for (idx, q) in (2u8..=4).enumerate() {
            let end = if q == 4 {
                Date::from_calendar_date(2026, Month::January, 1).expect("valid date")
            } else {
                q_start(q + 1)
            };
            let period = Period {
                id: PeriodId::quarter(2025, q),
                start: q_start(q),
                end,
                is_actual: false,
            };
            let (breakdown, closing, warnings) = calculate_period_flows(
                &instrument,
                &period,
                opening,
                Money::new(0.0, Currency::USD),
                &market_ctx,
                issue,
            )
            .expect("period flow calculation should succeed");

            assert!(
                warnings.is_empty(),
                "no scale warning expected in Q{q}, got {warnings:?}"
            );
            assert!(
                (breakdown.interest_expense_cash.amount() - expected_interest[idx]).abs() < 1e-9,
                "Q{q} interest should equal the raw schedule ({}), got {}",
                expected_interest[idx],
                breakdown.interest_expense_cash.amount()
            );
            assert!(
                (breakdown.principal_payment.amount() - 100_000.0).abs() < 1e-9,
                "Q{q} principal should equal the raw schedule"
            );
            opening = closing;
        }
    }

    /// Toggle-driven PIK capitalization is excluded from the clamp basis:
    /// the stateful balance legitimately exceeds the scheduled balance by the
    /// capitalized PIK, so interest must accrue on the full balance with no
    /// warning. Without the exclusion the same inputs are clamped at 1.10.
    #[test]
    fn toggled_pik_capitalization_is_excluded_from_scale_clamp() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let period = Period {
            id: PeriodId::quarter(2025, 1),
            start,
            end,
            is_actual: false,
        };

        let instrument = SignedFlowInstrument {
            schedule: CashFlowSchedule {
                flows: vec![CashFlow {
                    date: Date::from_calendar_date(2025, Month::February, 15).expect("valid date"),
                    reset_date: None,
                    amount: Money::new(-20_000.0, Currency::USD),
                    kind: CFKind::Fixed,
                    accrual_factor: 0.25,
                    rate: Some(0.08),
                }],
                notional: Notional::par(1_000_000.0, Currency::USD),
                day_count: DayCount::Act365F,
                meta: CashFlowMeta {
                    issue_date: Some(start),
                    ..CashFlowMeta::default()
                },
            },
        };

        let market_ctx = MarketContext::new();
        // Opening balance = scheduled 1.0M + 160k of toggle-capitalized PIK
        // (raw ratio 1.16, beyond the 1.10 clamp).
        let (breakdown, _, warnings) = calculate_period_flows(
            &instrument,
            &period,
            Money::new(1_160_000.0, Currency::USD),
            Money::new(160_000.0, Currency::USD),
            &market_ctx,
            start,
        )
        .expect("period flow calculation should succeed");

        assert!(
            warnings.is_empty(),
            "PIK-grown balance must not trigger the scale warning, got {warnings:?}"
        );
        assert!(
            (breakdown.interest_expense_cash.amount() - 23_200.0).abs() < 1e-9,
            "interest should accrue on the full stateful balance (20k × 1.16), got {}",
            breakdown.interest_expense_cash.amount()
        );

        // Without the toggled-PIK exclusion the same inputs are clamped.
        let (clamped, _, clamp_warnings) = calculate_period_flows(
            &instrument,
            &period,
            Money::new(1_160_000.0, Currency::USD),
            Money::new(0.0, Currency::USD),
            &market_ctx,
            start,
        )
        .expect("period flow calculation should succeed");
        assert_eq!(clamp_warnings.len(), 1);
        assert!((clamped.interest_expense_cash.amount() - 22_000.0).abs() < 1e-9);
    }

    /// A revolver whose stateful balance was fully swept must not resurrect
    /// the scheduled balance on a re-draw: the new closing balance is the
    /// period's draws net of in-period repayments.
    #[test]
    fn revolver_redraw_after_full_sweep_nets_new_funding_against_repayments() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let period = Period {
            id: PeriodId::quarter(2025, 1),
            start,
            end,
            is_actual: false,
        };

        let issue = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let instrument = SignedFlowInstrument {
            schedule: CashFlowSchedule {
                flows: vec![
                    CashFlow {
                        date: Date::from_calendar_date(2025, Month::February, 15)
                            .expect("valid date"),
                        reset_date: None,
                        amount: Money::new(100_000.0, Currency::USD),
                        kind: CFKind::RevolvingDraw,
                        accrual_factor: 0.0,
                        rate: None,
                    },
                    CashFlow {
                        date: Date::from_calendar_date(2025, Month::March, 15).expect("valid date"),
                        reset_date: None,
                        amount: Money::new(30_000.0, Currency::USD),
                        kind: CFKind::RevolvingRepayment,
                        accrual_factor: 0.0,
                        rate: None,
                    },
                ],
                // Scheduled balance is 500k — the stateful balance (zero,
                // fully swept upstream) takes precedence.
                notional: Notional::par(500_000.0, Currency::USD),
                day_count: DayCount::Act365F,
                meta: CashFlowMeta {
                    issue_date: Some(issue),
                    ..CashFlowMeta::default()
                },
            },
        };

        let market_ctx = MarketContext::new();
        let (breakdown, closing, _) = calculate_period_flows(
            &instrument,
            &period,
            Money::new(0.0, Currency::USD),
            Money::new(0.0, Currency::USD),
            &market_ctx,
            start,
        )
        .expect("period flow calculation should succeed");

        assert_eq!(
            closing.amount(),
            70_000.0,
            "closing must be draws (100k) net of repayments (30k), not the scheduled balance"
        );
        assert_eq!(breakdown.debt_balance.amount(), 70_000.0);
    }

    /// Commitment and facility fees accrue on the undrawn commitment / total
    /// facility, so they must pass through unscaled even when the drawn
    /// balance has diverged from the schedule.
    #[test]
    fn commitment_and_facility_fees_are_not_scaled_by_drawn_balance_ratio() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let period = Period {
            id: PeriodId::quarter(2025, 1),
            start,
            end,
            is_actual: false,
        };

        let instrument = SignedFlowInstrument {
            schedule: CashFlowSchedule {
                flows: vec![
                    CashFlow {
                        date: Date::from_calendar_date(2025, Month::February, 15)
                            .expect("valid date"),
                        reset_date: None,
                        amount: Money::new(-5_000.0, Currency::USD),
                        kind: CFKind::CommitmentFee,
                        accrual_factor: 0.25,
                        rate: None,
                    },
                    CashFlow {
                        date: Date::from_calendar_date(2025, Month::February, 15)
                            .expect("valid date"),
                        reset_date: None,
                        amount: Money::new(-2_000.0, Currency::USD),
                        kind: CFKind::FacilityFee,
                        accrual_factor: 0.25,
                        rate: None,
                    },
                ],
                notional: Notional::par(1_000_000.0, Currency::USD),
                day_count: DayCount::Act365F,
                meta: CashFlowMeta {
                    issue_date: Some(start),
                    ..CashFlowMeta::default()
                },
            },
        };

        let market_ctx = MarketContext::new();
        // Stateful balance at half the schedule → interest-like flows scale
        // by 0.5, but commitment/facility fees must not.
        let (breakdown, _, _) = calculate_period_flows(
            &instrument,
            &period,
            Money::new(500_000.0, Currency::USD),
            Money::new(0.0, Currency::USD),
            &market_ctx,
            start,
        )
        .expect("period flow calculation should succeed");

        assert_eq!(
            breakdown.fees.amount(),
            7_000.0,
            "commitment/facility fees must pass through at the scheduled amount"
        );
    }

    #[test]
    fn calculate_period_flows_clamps_pathological_scale_factor() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let period = Period {
            id: PeriodId::quarter(2025, 1),
            start,
            end,
            is_actual: false,
        };

        let instrument = SignedFlowInstrument {
            schedule: CashFlowSchedule {
                flows: vec![CashFlow {
                    date: Date::from_calendar_date(2025, Month::February, 15).expect("valid date"),
                    reset_date: None,
                    amount: Money::new(-50_000.0, Currency::USD),
                    kind: CFKind::Fixed,
                    accrual_factor: 0.25,
                    rate: None,
                }],
                notional: Notional::par(0.01, Currency::USD),
                day_count: DayCount::Act365F,
                meta: CashFlowMeta {
                    issue_date: Some(start),
                    ..CashFlowMeta::default()
                },
            },
        };

        let market_ctx = MarketContext::new();
        let (breakdown, _, warnings) = calculate_period_flows(
            &instrument,
            &period,
            Money::new(100_000.0, Currency::USD),
            Money::new(0.0, Currency::USD),
            &market_ctx,
            start,
        )
        .expect("period flow calculation should succeed");

        // Scale factor is now clamped to 1.10 (was 2.0) — see SCALE_CLAMP_MAX
        // in `calculate_period_flows`. Tightened to prevent silent up-to-2×
        // cashflow amplification when the schedule and stateful balance diverge.
        assert!(
            breakdown.interest_expense_cash.amount() <= 50_000.0 * 1.10 + 1e-6,
            "scale factor should be clamped to 1.10, but interest was {}",
            breakdown.interest_expense_cash.amount()
        );

        // The clamp must also surface as a structured EvalWarning so callers
        // see the divergence in their results envelope, not only in tracing
        // output. A regression that drops the warning push would be silent
        // without this assertion.
        let scale_warnings: Vec<&EvalWarning> = warnings
            .iter()
            .filter(|w| {
                matches!(
                    w,
                    EvalWarning::CapitalStructureCashflowIgnored { kind, .. }
                        if kind.starts_with("scale_clamped(")
                )
            })
            .collect();
        assert_eq!(
            scale_warnings.len(),
            1,
            "expected exactly one scale_clamped warning, got: {warnings:?}"
        );
    }
}
