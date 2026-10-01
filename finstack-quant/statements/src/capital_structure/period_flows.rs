//! Per-period cashflow extraction for a single instrument.
//!
//! The schedule-classification logic and its safety clamps live here, next to
//! their tests; the evaluator's capital-structure runtime drives this function
//! per instrument and period and handles currency / FX bookkeeping. The logic here:
//!
//! 1. Pulls the full classified raw schedule, retaining history and PIK events.
//! 2. Computes a stateful `scale` factor that relates the model's
//!    period-opening balance to the schedule's notional opening — clamped to
//!    [0.0, 1.10] to prevent silent cashflow amplification (see
//!    residual rebuild; a leftover scale is only a diagnostic).
//! 3. Classifies each in-period flow by `CFKind` into the
//!    [`CashflowBreakdown`] buckets (cash interest, PIK interest, principal,
//!    fees) — credit events and unknown variants emit warnings instead of
//!    being silently aggregated.
//! 4. Snapshots the closing balance and accrued interest at `period.end - 1`.

use crate::capital_structure::cashflows::CashflowBreakdown;
use crate::error::Result;
use crate::evaluator::{CapitalStructureWarning, EvalWarning};
use finstack_quant_cashflows::builder::{CashFlowSchedule, CashflowRepresentation};
use finstack_quant_cashflows::primitives::CFKind;
use finstack_quant_cashflows::{accrued_interest_amount, AccrualConfig};
use finstack_quant_core::cashflow::CashFlow;
use finstack_quant_core::dates::{Date, Period};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::pricer::InstrumentType;

/// True when `kind` is a cash-interest flow (the legs a swap nets).
#[inline]
pub(crate) fn is_cash_interest_kind(kind: CFKind) -> bool {
    matches!(kind, CFKind::Fixed | CFKind::Stub | CFKind::FloatReset)
}

/// Principal established by a funding flow, independently of cash proceeds.
/// Explicit principal deltas own balance movements, including withheld OID.
/// Raw funding rows without a delta use their cash amount's magnitude.
///
/// # Arguments
///
/// * `flow` - A raw classified cashflow; only notional and revolving funding
///   rows establish new principal. Cash amounts and their dates are preserved.
pub(crate) fn funding_principal(flow: &CashFlow) -> Option<Money> {
    if !matches!(flow.kind, CFKind::Notional | CFKind::RevolvingDraw) {
        return None;
    }
    if let Some(delta) = flow.principal_delta {
        return (delta.amount() > 0.0).then_some(delta);
    }
    if matches!(flow.kind, CFKind::RevolvingDraw) || flow.amount.amount() <= 0.0 {
        return Some(if flow.amount.amount() < 0.0 {
            flow.amount.checked_neg()
        } else {
            flow.amount
        });
    }
    None
}

/// Whether the declared financial-statement instrument carries borrowing principal.
///
/// # Arguments
///
/// * `kind` - Canonical instrument category whose cashflow view and borrowing
///   semantics must belong to the financial-statement instrument union.
pub(crate) fn is_debt_kind(kind: InstrumentType) -> Result<bool> {
    match kind {
        InstrumentType::Bond
        | InstrumentType::Convertible
        | InstrumentType::TermLoan
        | InstrumentType::RevolvingCredit => Ok(true),
        InstrumentType::Irs | InstrumentType::CapFloor | InstrumentType::Swaption => Ok(false),
        _ => Err(crate::error::Error::invalid_input(format!(
            "instrument type '{kind:?}' is not supported in a financial statement capital structure"
        ))),
    }
}

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

/// Preserve the cash and economic dates of this period's principal claims.
pub(crate) fn period_principal_claims(
    schedule: &CashFlowSchedule,
    period: &Period,
) -> Vec<crate::capital_structure::PrincipalClaim> {
    schedule
        .get_flows()
        .iter()
        .filter(|flow| flow.date >= period.start && flow.date < period.end)
        .filter(|flow| {
            matches!(
                flow.kind,
                CFKind::Amortization | CFKind::PrePayment | CFKind::RevolvingRepayment
            ) || (flow.kind == CFKind::Notional && flow.amount.amount() > 0.0)
        })
        .filter(|flow| flow.amount.amount() != 0.0)
        .map(|flow| crate::capital_structure::PrincipalClaim {
            payment_date: flow.date,
            balance_date: flow.get_balance_date(),
            amount: if flow.amount.amount() < 0.0 {
                flow.amount.checked_neg()
            } else {
                flow.amount
            },
        })
        .collect()
}

/// Calculate contractual flows for a single period.
///
/// This helper extracts flows for a specific period from an instrument's full schedule,
/// returning a CashflowBreakdown for that period. Used for dynamic period-by-period evaluation.
///
/// Periods are treated with half-open semantics `[start, end)`. End-of-period
/// balances and accruals are therefore snapped at `period.end - 1 day` so
/// cashflows occurring exactly on the next period boundary are not attributed
/// to the prior period. Hedge trade notionals do not establish borrowing
/// principal: hedge cashflows use full notional scale, while debt balance,
/// new funding and debt accrued interest remain zero. Derivative accrual
/// valuation is outside this debt-service extraction contract.
///
/// # Arguments
///
/// * `instrument` - Canonical instrument whose declared kind fixes the interest
///   sign convention, even when `residual_schedule` is supplied. Bonds, convertible
///   bonds, loans and revolvers emit positive debt coupons as issuer expenses;
///   negative debt coupons are income. Rate hedges emit positive cash as holder
///   receipts and negative cash as payments. Supplies the raw schedule when no
///   residual schedule is provided.
/// * `period` - The period to extract flows for
/// * `opening_balance` - Opening balance at the start of the period
/// * `toggled_pik_capitalized` - Cumulative principal capitalized via the PIK
///   *toggle* (see [`crate::capital_structure::CapitalStructureState::cumulative_toggled_pik`]).
///   Used only when `residual_schedule` is `None` to keep the scale basis from
///   treating toggle-driven compounding as schedule drift; pass zero when no
///   toggle state exists.
/// * `market_ctx` - Market context for pricing
/// * `as_of` - Valuation date used to generate the instrument's full cashflow
///   schedule and identify historical versus projected amounts.
/// * `residual_schedule` - Rebuilt remaining schedule after a prior outstanding
///   change. When present, period flows are read from this schedule (the
///   interest engine) and a warning is raised if
///   `opening / scheduled_opening` diverges from 1 by more than `1e-6`.
///
/// # Returns
///
/// Returns a tuple of:
/// - [`CashflowBreakdown`] for the period
/// - closing balance after scheduled flows
/// - newly funded principal, using explicit principal deltas rather than net cash
///   proceeds, and implicit initial face for a schedule without a funding leg
/// - evaluation warnings for ignored or unsupported cashflow kinds
///
/// # Errors
///
/// Returns an error if the instrument schedule cannot be built, if currencies
/// are inconsistent, if implicit issuance principal is negative, or if accrued
/// interest cannot be computed, or the declared instrument kind is outside
/// the supported financial-statement instrument union, or the raw schedule
/// is a placeholder whose contingent payouts are not modeled.
///
/// # References
///
/// - Cashflow discounting and schedule context: `docs/REFERENCES.md#hull-options-futures`
/// - Fixed-income balance/risk interpretation: `docs/REFERENCES.md#tuckman-serrat-fixed-income`
pub fn calculate_period_flows(
    instrument: &dyn Instrument,
    period: &Period,
    opening_balance: Money,
    toggled_pik_capitalized: Money,
    market_ctx: &MarketContext,
    as_of: Date,
    residual_schedule: Option<&CashFlowSchedule>,
) -> Result<(CashflowBreakdown, Money, Money, Vec<EvalWarning>)> {
    let is_debt = is_debt_kind(instrument.key())?;
    let interest_expense_sign = if is_debt { 1.0 } else { -1.0 };
    let using_residual = residual_schedule.is_some();
    let full_schedule = match residual_schedule {
        Some(schedule) => schedule.clone(),
        None => instrument.raw_cashflow_schedule(market_ctx, as_of)?,
    };
    if full_schedule.get_meta().representation == CashflowRepresentation::Placeholder {
        return Err(crate::error::Error::capital_structure(format!(
            "instrument '{}' has an unsupported placeholder cashflow schedule; contingent payouts are not modeled",
            instrument.id()
        )));
    }
    let currency = full_schedule.get_notional().initial.currency();
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
    let outstanding_path = if is_debt {
        full_schedule.outstanding_by_date()?
    } else {
        Vec::new()
    };
    // Boundary convention: flows dated exactly on `period.start` belong to
    // *this* period (half-open `[start, end)`), so the scheduled opening must
    // be the balance strictly *before* `period.start` to match the
    // `end - 1 day` closing snapshot of the prior period.
    let scheduled_opening = outstanding_path
        .iter()
        .filter(|(d, _)| *d < period.start)
        .map(|(_, balance)| {
            if balance.amount() < 0.0 {
                balance.checked_neg()
            } else {
                *balance
            }
        })
        .next_back()
        .unwrap_or(full_schedule.get_notional().initial);

    // Residual rebuild is the interest engine. Scale remains only as a
    // diagnostic (and as a fallback when the caller did not pass a residual):
    // after a correct rebuild, `opening / scheduled_opening ≈ 1`.
    const SCALE_WARN_THRESHOLD: f64 = 1.05;
    const RESIDUAL_SCALE_TOLERANCE: f64 = 1e-6;
    // Funding establishes principal even from a zero opening. For schedules
    // without an issuance cash leg, recognize their initial face
    // once when issuance falls in this period. An explicit funding schedule
    // owns its economic draw timing; never infer an earlier draw from its commitment.
    // Debt face follows explicit principal deltas; cash proceeds remain intact
    // on the raw flow, so withheld OID does not reduce the amount owed.
    let mut has_funding_leg = false;
    let mut net_new_funding = 0.0;
    for flow in full_schedule.get_flows().iter().filter(|_| is_debt) {
        if let Some(principal) = funding_principal(flow) {
            has_funding_leg = true;
            if principal.currency() != currency {
                return Err(crate::error::Error::currency_mismatch(
                    currency,
                    principal.currency(),
                ));
            }
            if flow.amount.currency() != currency {
                return Err(crate::error::Error::currency_mismatch(
                    currency,
                    flow.amount.currency(),
                ));
            }
            let funding_date = flow.get_balance_date();
            if funding_date >= period.start && funding_date < period.end {
                net_new_funding += principal.amount();
            }
        }
    }
    if is_debt
        && opening_balance.amount() == 0.0
        && !has_funding_leg
        && full_schedule
            .get_meta()
            .issue_date
            .is_some_and(|issue| issue >= period.start && issue < period.end)
    {
        let initial = full_schedule.get_notional().initial.amount();
        if initial < 0.0 {
            return Err(crate::error::Error::capital_structure(
                "Implicit issuance funding requires nonnegative initial principal",
            ));
        }
        net_new_funding = initial;
    }
    let has_new_funding = net_new_funding > 0.0;
    let scale = if !is_debt {
        // A hedge's trade notional sizes its coupons, not borrowed principal.
        1.0
    } else if opening_balance.amount() == 0.0 {
        if scheduled_opening.amount() == 0.0 || has_new_funding {
            // No prior balance and none scheduled (pre-issuance start), or the
            // balance is established by a draw this period: book at full scale.
            1.0
        } else {
            // Balance scheduled but not yet drawn/issued: book nothing.
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
        //
        // The increment is capped at the opening balance because it is only a
        // *component* of that balance. Once sweeps repay the debt below the
        // cumulative toggled PIK, an uncapped `pik_part` makes the
        // `adjusted_opening` floor bind and leaves `scale = pik / scheduled`,
        // which exceeds `opening / scheduled` — interest would accrue on
        // principal the borrower no longer owes, compounding every period.
        // With the cap, an unclamped scale is exactly `opening / scheduled`.
        let pik_increment = toggled_pik_capitalized
            .amount()
            .max(0.0)
            .min(opening_balance.amount().max(0.0));
        let adjusted_opening = (opening_balance.amount() - pik_increment).max(0.0);
        let adjusted_ratio = adjusted_opening / scheduled_opening.amount();
        let pik_part = pik_increment / scheduled_opening.amount();
        if using_residual {
            let residual_scale = opening_balance.amount() / scheduled_opening.amount();
            if (residual_scale - 1.0).abs() > RESIDUAL_SCALE_TOLERANCE {
                tracing::warn!(
                    raw_scale = residual_scale,
                    period = ?period.id,
                    "Residual schedule opening diverges from the stateful opening \
                     (|scale - 1| > {RESIDUAL_SCALE_TOLERANCE}); this signals a rebuild bug."
                );
                warnings.push(EvalWarning::CapitalStructure {
                    period: period.id,
                    warning: CapitalStructureWarning::ScaleClamped {
                        raw_ratio: residual_scale,
                        clamped_ratio: 1.0,
                    },
                });
            }
            residual_scale
        } else {
            if adjusted_ratio > SCALE_WARN_THRESHOLD {
                tracing::warn!(
                    raw_scale = adjusted_ratio,
                    period = ?period.id,
                    "Scale factor between opening balance (ex toggled-PIK) and scheduled opening exceeds {SCALE_WARN_THRESHOLD}. \
                     This typically indicates an unscheduled paydown / re-draw mismatch — verify the model."
                );
                warnings.push(EvalWarning::CapitalStructure {
                    period: period.id,
                    warning: CapitalStructureWarning::ScaleClamped {
                        raw_ratio: adjusted_ratio,
                        clamped_ratio: adjusted_ratio,
                    },
                });
            }
            adjusted_ratio + pik_part
        }
    };

    // Keep the economic sign through netting. Negative rates or a zero leg can
    // make every swap coupon have the same sign, so only the declared kind can
    // distinguish holder receipts from positive issuer debt payments.
    let mut net_interest_cash = 0.0_f64;

    for cf in full_schedule.get_flows() {
        if cf.date >= period.start && cf.date < period.end {
            // Currency safety: every in-period flow must share the breakdown
            // currency. Cross-currency instruments (e.g. xccy swaps) mix
            // currencies in one schedule and require explicit FX, which this
            // classifier does not perform. Reject rather than let `Money`
            // arithmetic panic on a currency mismatch.
            if cf.amount.currency() != currency {
                return Err(crate::error::Error::currency_mismatch(
                    currency,
                    cf.amount.currency(),
                ));
            }

            let scaled_abs_value =
                Money::new(cf.amount.amount().abs() * scale, cf.amount.currency())?;

            match cf.kind {
                kind if is_cash_interest_kind(kind) => {
                    net_interest_cash += cf.amount.amount() * scale;
                }
                CFKind::Amortization | CFKind::PrePayment | CFKind::RevolvingRepayment => {
                    breakdown.principal_payment += scaled_abs_value;
                }
                CFKind::Notional if cf.amount.amount() > 0.0 => {
                    breakdown.principal_payment += scaled_abs_value;
                }
                CFKind::CommitmentFee
                | CFKind::FacilityFee
                | CFKind::LcFee
                | CFKind::FrontingFee => {
                    // Commitment, facility and letter-of-credit fees accrue on the
                    // undrawn commitment, the facility size or the LC face, NOT on the drawn
                    // balance — they must not be scaled by the drawn-balance
                    // ratio. Pass them through at the scheduled amount.
                    breakdown.fees += Money::new(cf.amount.amount().abs(), cf.amount.currency())?;
                }
                CFKind::Fee | CFKind::UsageFee => {
                    // Generic and usage (drawn-balance based) fees scale with
                    // the stateful/scheduled balance ratio like interest.
                    breakdown.fees += scaled_abs_value;
                }
                CFKind::Pik => {
                    breakdown.interest_expense_pik += scaled_abs_value;
                }
                CFKind::Notional | CFKind::RevolvingDraw => {
                    // Funding / draw events are not treated as scheduled principal payments in statements.
                }
                CFKind::DefaultedNotional | CFKind::Recovery => {
                    // Credit events are not modeled as part of standard debt service in statements.
                    warnings.push(EvalWarning::CapitalStructure {
                        period: period.id,
                        warning: CapitalStructureWarning::CashflowIgnored {
                            cashflow_kind: cf.kind,
                            cashflow_date: cf.date,
                        },
                    });
                    tracing::warn!(
                        "Ignoring credit-event CFKind={:?} for period flow calc (date={:?})",
                        cf.kind,
                        cf.date
                    );
                }
                _ => {
                    // CFKind is non-exhaustive; ignore unknown variants to avoid misclassification.
                    warnings.push(EvalWarning::CapitalStructure {
                        period: period.id,
                        warning: CapitalStructureWarning::CashflowIgnored {
                            cashflow_kind: cf.kind,
                            cashflow_date: cf.date,
                        },
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

    let net_expense = net_interest_cash * interest_expense_sign;
    let (expense, income) = (net_expense.max(0.0), (-net_expense).max(0.0));
    breakdown.interest_expense_cash = Money::new(expense, currency)?;
    breakdown.interest_income_cash = Some(Money::new(income, currency)?);

    // outstanding_path only has entries on cashflow dates, so take the latest
    // entry at or before period.end.
    let scheduled_closing_balance = outstanding_path
        .iter()
        .rev()
        .find(|(date, _)| *date <= snapshot_date)
        .map(|(_, balance)| {
            if balance.amount() < 0.0 {
                balance.checked_neg()
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
                .get_meta()
                .issue_date
                .is_some_and(|issue| issue > snapshot_date)
                && outstanding_path
                    .first()
                    .is_some_and(|(date, _)| *date > snapshot_date);
            if pre_issuance {
                return Money::from((0_i64, currency));
            }
            let initial = full_schedule.get_notional().initial;
            if initial.amount() < 0.0 {
                initial.checked_neg()
            } else {
                initial
            }
        });

    let closing_balance = if !is_debt {
        Money::from((0_i64, currency))
    } else if opening_balance.amount() == 0.0 {
        if has_new_funding {
            // The stateful balance is zero (e.g. a revolver fully swept in a
            // prior period). New draws this period establish a fresh balance
            // of draws net of in-period repayments — taking the scheduled
            // closing balance here would resurrect the swept balance.
            let in_period_repayments: f64 = full_schedule
                .get_flows()
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
            Money::new(
                (net_new_funding + breakdown.interest_expense_pik.amount() - in_period_repayments)
                    .max(0.0),
                currency,
            )?
        } else {
            Money::from((0_i64, currency))
        }
    } else {
        scheduled_closing_balance
    };
    breakdown.debt_balance = closing_balance;

    // Calculate accrued interest at period end.
    // Note: detailed accrual config (day count, compounding) comes from the schedule itself.
    // `AccrualConfig::default()` leaves `frequency: None`; for ACT/ACT ISMA
    // schedules the accrual engine then fails closed with
    // `InputError::MissingFrequencyForActActIsma` (there is no silent ISDA
    // fallback), so an ISMA schedule cannot produce a wrong accrued figure
    // here — it errors until `CashFlowMeta` carries the coupon frequency.
    // Debt coupon accrual uses a single borrowing path. A hedge's merged
    // legs are not a debt schedule and require a separate valuation accrual policy.
    let accrued_interest = if !is_debt {
        0.0
    } else {
        let accrued_scalar =
            accrued_interest_amount(&full_schedule, snapshot_date, &AccrualConfig::default())?;
        if !using_residual && opening_balance.amount() == 0.0 && !has_new_funding {
            0.0
        } else {
            accrued_scalar * scale
        }
    };
    breakdown.accrued_interest = Money::new(accrued_interest, currency)?;

    // `net_new_funding` (draws plus explicit or implicit issuance principal in
    // this period) is returned so the waterfall can recover the payable balance
    // (`opening + funding`) and the draw-aware closing balance; without it the
    // waterfall would recompute closing as `opening - principal` and silently
    // wipe in-period draws.
    Ok((
        breakdown,
        closing_balance,
        Money::new(net_new_funding, currency)?,
        warnings,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_cashflows::builder::{CashFlowMeta, CashFlowSchedule, Notional};
    use finstack_quant_cashflows::primitives::CFKind;
    use finstack_quant_core::cashflow::CashFlow;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DayCount, PeriodId};
    use finstack_quant_core::money::Money;
    use time::Month;

    #[derive(Clone)]
    struct SignedFlowInstrument {
        schedule: CashFlowSchedule,
        id: finstack_quant_core::types::InstrumentId,
        attributes: finstack_quant_core::types::Attributes,
    }

    impl SignedFlowInstrument {
        fn new(schedule: CashFlowSchedule) -> Self {
            Self {
                schedule,
                id: "SCHEDULE".into(),
                attributes: Default::default(),
            }
        }
    }

    impl Instrument for SignedFlowInstrument {
        fn id(&self) -> &str {
            self.id.as_str()
        }
        fn key(&self) -> finstack_quant_valuations::pricer::InstrumentType {
            finstack_quant_valuations::pricer::InstrumentType::Bond
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
        fn attributes(&self) -> &finstack_quant_core::types::Attributes {
            &self.attributes
        }
        fn attributes_mut(&mut self) -> &mut finstack_quant_core::types::Attributes {
            &mut self.attributes
        }
        fn clone_box(&self) -> Box<dyn finstack_quant_valuations::instruments::Instrument> {
            Box::new(self.clone())
        }

        fn base_value(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<Money> {
            Err(finstack_quant_core::Error::Validation(
                "test schedule does not price".into(),
            ))
        }

        fn market_dependencies(
            &self,
        ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
        {
            Ok(Default::default())
        }
    }

    impl finstack_quant_cashflows::CashflowScheduleSource for SignedFlowInstrument {
        fn raw_cashflow_schedule(
            &self,
            _curves: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<CashFlowSchedule> {
            Ok(self.schedule.clone())
        }
    }

    fn test_schedule(flows: Vec<CashFlow>, notional: f64, issue_date: Date) -> CashFlowSchedule {
        CashFlowSchedule::from_parts(
            flows,
            Notional::par(notional, Currency::USD).expect("valid notional fixture"),
            DayCount::Act365F,
            CashFlowMeta {
                issue_date: Some(issue_date),
                ..CashFlowMeta::default()
            },
        )
    }

    #[test]
    fn negative_debt_coupon_is_booked_as_issuer_income() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let period = Period {
            id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
            start,
            end,
            is_actual: false,
        };

        let instrument = SignedFlowInstrument::new(test_schedule(
            vec![CashFlow::new(
                Date::from_calendar_date(2025, Month::February, 15).expect("valid date"),
                None,
                Money::from((-50_000_i64, Currency::USD)),
                CFKind::Fixed,
                0.25,
                None,
            )],
            1_000_000.0,
            start,
        ));

        let market_ctx = MarketContext::new();
        let (breakdown, _, _, warnings) = calculate_period_flows(
            &instrument,
            &period,
            Money::from((1_000_000_i64, Currency::USD)),
            Money::from((0_i64, Currency::USD)),
            &market_ctx,
            start,
            None,
        )
        .expect("period flow calculation should succeed");

        assert!(warnings.is_empty());
        assert_eq!(breakdown.interest_expense_cash.amount(), 0.0);
        assert_eq!(breakdown.interest_income_cash_or_zero().amount(), 50_000.0);
    }

    /// A swap whose legs net to a *receipt* must not be booked as an expense.
    ///
    /// Two-leg instruments emit both legs into one schedule as opposite-signed
    /// flows. Netting them is correct, but taking `.abs()` of the net turned an
    /// in-the-money hedge (pay-fixed while floating is higher) into a phantom
    /// expense: P&L overstated by twice the receipt, and under a waterfall the
    /// phantom expense consumed real cash in the Interest category. The receipt
    /// belongs in `interest_income_cash`.
    #[test]
    fn swap_net_receipt_is_booked_as_income_not_expense() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let period = Period {
            id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
            start,
            end,
            is_actual: false,
        };
        let pay_date = Date::from_calendar_date(2025, Month::February, 15).expect("valid date");

        // Pay fixed 4% (outflow), receive floating 5.3% (inflow) => net receipt.
        let instrument = SignedFlowInstrument::new(test_schedule(
            vec![
                CashFlow::new(
                    pay_date,
                    None,
                    Money::from((-40_000_i64, Currency::USD)),
                    CFKind::Fixed,
                    0.25,
                    None,
                ),
                CashFlow::new(
                    pay_date,
                    None,
                    Money::from((53_000_i64, Currency::USD)),
                    CFKind::FloatReset,
                    0.25,
                    None,
                ),
            ],
            1_000_000.0,
            start,
        ));

        let swap = finstack_quant_valuations::instruments::InterestRateSwap::example()
            .expect("valid IRS fixture");
        let market_ctx = MarketContext::new();
        let (breakdown, _, _, _) = calculate_period_flows(
            &swap,
            &period,
            Money::from((1_000_000_i64, Currency::USD)),
            Money::from((0_i64, Currency::USD)),
            &market_ctx,
            start,
            Some(&instrument.schedule),
        )
        .expect("period flow calculation should succeed");

        assert_eq!(
            breakdown.interest_expense_cash.amount(),
            0.0,
            "a net receipt is not an interest expense"
        );
        assert_eq!(
            breakdown.interest_income_cash_or_zero().amount(),
            13_000.0,
            "the 13k net receipt must be booked as interest income"
        );
        assert_eq!(
            breakdown
                .net_interest_expense_cash()
                .expect("same currency")
                .amount(),
            -13_000.0,
            "net interest expense is negative when the hedge is in the money"
        );
    }

    /// A period that redraws from a zero stateful balance must still book the
    /// period's interest.
    ///
    /// When stateful opening is 0 but the schedule shows a draw this period (a
    /// revolver redrawn after a full sweep, or an instrument issued mid-period),
    /// `scale` was 0, zeroing every coupon, fee, and the accrual — while the
    /// closing-balance branch correctly recognized the draw via
    /// `has_new_funding`. The two halves disagreed about whether the debt
    /// exists.
    #[test]
    fn redraw_from_zero_balance_still_books_interest() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let period = Period {
            id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
            start,
            end,
            is_actual: false,
        };
        let draw_date = Date::from_calendar_date(2025, Month::January, 5).expect("valid date");
        let coupon_date = Date::from_calendar_date(2025, Month::February, 15).expect("valid date");

        // Revolver: draw 1M this period, then accrue a 20k coupon on it.
        let instrument = SignedFlowInstrument::new(test_schedule(
            vec![
                CashFlow::new(
                    draw_date,
                    None,
                    Money::from((1_000_000_i64, Currency::USD)),
                    CFKind::RevolvingDraw,
                    0.0,
                    None,
                ),
                CashFlow::new(
                    coupon_date,
                    None,
                    Money::from((20_000_i64, Currency::USD)),
                    CFKind::Fixed,
                    0.25,
                    None,
                ),
            ],
            1_000_000.0,
            start,
        ));

        let market_ctx = MarketContext::new();
        // Stateful opening is zero — the revolver was fully swept last period.
        let (breakdown, closing, net_new_funding, _) = calculate_period_flows(
            &instrument,
            &period,
            Money::from((0_i64, Currency::USD)),
            Money::from((0_i64, Currency::USD)),
            &market_ctx,
            start,
            None,
        )
        .expect("period flow calculation should succeed");

        // The draw is recognized (closing = 1M, funding = 1M) ...
        assert_eq!(net_new_funding.amount(), 1_000_000.0);
        assert_eq!(closing.amount(), 1_000_000.0);
        // ... so the coupon on that drawn balance must be booked, not zeroed.
        assert_eq!(
            breakdown.interest_expense_cash.amount(),
            20_000.0,
            "interest on a redrawn balance must not be zeroed"
        );
    }

    /// A previously issued instrument that has been fully repaid books
    /// nothing when there is no new draw, even if its original schedule remains.
    #[test]
    fn zero_balance_without_draw_books_nothing() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let period = Period {
            id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
            start,
            end,
            is_actual: false,
        };
        let coupon_date = Date::from_calendar_date(2025, Month::February, 15).expect("valid date");

        let instrument = SignedFlowInstrument::new(test_schedule(
            vec![CashFlow::new(
                coupon_date,
                None,
                Money::from((20_000_i64, Currency::USD)),
                CFKind::Fixed,
                0.25,
                None,
            )],
            1_000_000.0,
            Date::from_calendar_date(2024, Month::December, 1).expect("valid issue date"),
        ));

        let market_ctx = MarketContext::new();
        let (breakdown, _, _, _) = calculate_period_flows(
            &instrument,
            &period,
            Money::from((0_i64, Currency::USD)),
            Money::from((0_i64, Currency::USD)),
            &market_ctx,
            start,
            None,
        )
        .expect("period flow calculation should succeed");

        assert_eq!(
            breakdown.interest_expense_cash.amount(),
            0.0,
            "a fully repaid instrument with no draw books no interest"
        );
        assert_eq!(breakdown.accrued_interest.amount(), 0.0);
    }

    /// Once sweeps push the balance below the cumulative toggled PIK, the
    /// scale must still track the real balance.
    ///
    /// `scale = clamp((opening - pik)/sched) + pik/sched` assumes the toggled
    /// PIK increment is still embedded in the balance. With `adjusted_opening`
    /// floored at zero but `pik_part` uncapped, a large paydown made the floor
    /// bind and left `scale = pik/sched`, which exceeds `opening/sched` —
    /// interest accrued on a balance the borrower no longer owes, compounding
    /// every period.
    #[test]
    fn toggled_pik_scale_never_exceeds_the_real_opening_balance() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let period = Period {
            id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
            start,
            end,
            is_actual: false,
        };

        // Scheduled notional 1M with a 20k coupon.
        let instrument = SignedFlowInstrument::new(test_schedule(
            vec![CashFlow::new(
                Date::from_calendar_date(2025, Month::February, 15).expect("valid date"),
                None,
                Money::from((20_000_i64, Currency::USD)),
                CFKind::Fixed,
                0.25,
                None,
            )],
            1_000_000.0,
            start,
        ));

        let market_ctx = MarketContext::new();
        // Sweeps have cut the balance to 50k, while 160k of PIK was toggled
        // earlier in the instrument's life.
        let (breakdown, _, _, _) = calculate_period_flows(
            &instrument,
            &period,
            Money::from((50_000_i64, Currency::USD)),
            Money::from((160_000_i64, Currency::USD)),
            &market_ctx,
            start,
            None,
        )
        .expect("period flow calculation should succeed");

        // scale must be opening/scheduled = 50k/1M = 0.05 => 20k * 0.05 = 1k.
        // The uncapped pik_part gave 0.16 => 3.2k, a 3.2x overstatement.
        assert!(
            (breakdown.interest_expense_cash.amount() - 1_000.0).abs() < 1e-6,
            "interest must scale to the real 50k balance, got {}",
            breakdown.interest_expense_cash.amount()
        );
    }

    /// PIK compounding must still be honoured when the balance genuinely
    /// contains the capitalized increment (the case the exclusion exists for).
    #[test]
    fn toggled_pik_still_accrues_on_the_capitalized_balance() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let period = Period {
            id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
            start,
            end,
            is_actual: false,
        };

        let instrument = SignedFlowInstrument::new(test_schedule(
            vec![CashFlow::new(
                Date::from_calendar_date(2025, Month::February, 15).expect("valid date"),
                None,
                Money::from((20_000_i64, Currency::USD)),
                CFKind::Fixed,
                0.25,
                None,
            )],
            1_000_000.0,
            start,
        ));

        let market_ctx = MarketContext::new();
        // Balance grew to 1.1M because 100k of PIK capitalized: interest must
        // accrue on the full 1.1M (scale 1.1), not be frozen by the clamp.
        let (breakdown, _, _, _) = calculate_period_flows(
            &instrument,
            &period,
            Money::from((1_100_000_i64, Currency::USD)),
            Money::from((100_000_i64, Currency::USD)),
            &market_ctx,
            start,
            None,
        )
        .expect("period flow calculation should succeed");

        assert!(
            (breakdown.interest_expense_cash.amount() - 22_000.0).abs() < 1e-6,
            "PIK-grown balance must accrue on 1.1M (20k * 1.1 = 22k), got {}",
            breakdown.interest_expense_cash.amount()
        );
    }

    /// A swap whose legs net to a payment stays an expense, with no income.
    #[test]
    fn swap_net_payment_is_booked_as_expense() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let period = Period {
            id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
            start,
            end,
            is_actual: false,
        };
        let pay_date = Date::from_calendar_date(2025, Month::February, 15).expect("valid date");

        // Pay fixed 4% (outflow), receive floating 1% (inflow) => net payment.
        let instrument = SignedFlowInstrument::new(test_schedule(
            vec![
                CashFlow::new(
                    pay_date,
                    None,
                    Money::from((-40_000_i64, Currency::USD)),
                    CFKind::Fixed,
                    0.25,
                    None,
                ),
                CashFlow::new(
                    pay_date,
                    None,
                    Money::from((10_000_i64, Currency::USD)),
                    CFKind::FloatReset,
                    0.25,
                    None,
                ),
            ],
            1_000_000.0,
            start,
        ));

        let swap = finstack_quant_valuations::instruments::InterestRateSwap::example()
            .expect("valid IRS fixture");
        let market_ctx = MarketContext::new();
        let (breakdown, _, _, _) = calculate_period_flows(
            &swap,
            &period,
            Money::from((1_000_000_i64, Currency::USD)),
            Money::from((0_i64, Currency::USD)),
            &market_ctx,
            start,
            Some(&instrument.schedule),
        )
        .expect("period flow calculation should succeed");

        assert_eq!(breakdown.interest_expense_cash.amount(), 30_000.0);
        assert_eq!(breakdown.interest_income_cash_or_zero().amount(), 0.0);
    }

    #[test]
    fn calculate_period_flows_zero_opening_balance_zeroes_contractual_flows() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let period = Period {
            id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
            start,
            end,
            is_actual: false,
        };

        // The debt was issued before this period and subsequently fully
        // repaid. Its original contractual schedule must not resurrect it.
        let instrument = SignedFlowInstrument::new(test_schedule(
            vec![
                CashFlow::new(
                    Date::from_calendar_date(2025, Month::February, 15).expect("valid date"),
                    None,
                    Money::from((-50_000_i64, Currency::USD)),
                    CFKind::Fixed,
                    0.25,
                    None,
                ),
                CashFlow::new(
                    Date::from_calendar_date(2025, Month::March, 15).expect("valid date"),
                    None,
                    Money::from((-100_000_i64, Currency::USD)),
                    CFKind::Amortization,
                    0.0,
                    None,
                ),
            ],
            1_000_000.0,
            Date::from_calendar_date(2024, Month::January, 1).expect("valid issue date"),
        ));

        let market_ctx = MarketContext::new();
        let (breakdown, closing_balance, _, warnings) = calculate_period_flows(
            &instrument,
            &period,
            Money::from((0_i64, Currency::USD)),
            Money::from((0_i64, Currency::USD)),
            &market_ctx,
            start,
            None,
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
            id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
            start,
            end,
            is_actual: false,
        };

        let instrument = SignedFlowInstrument::new(test_schedule(
            vec![CashFlow::new(
                Date::from_calendar_date(2025, Month::February, 15).expect("valid date"),
                None,
                Money::from((-100_000_i64, Currency::USD)),
                CFKind::RevolvingDraw,
                0.0,
                None,
            )],
            0.0,
            start,
        ));

        let market_ctx = MarketContext::new();
        let (breakdown, closing_balance, _, warnings) = calculate_period_flows(
            &instrument,
            &period,
            Money::from((0_i64, Currency::USD)),
            Money::from((0_i64, Currency::USD)),
            &market_ctx,
            start,
            None,
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
            flows.push(CashFlow::new(
                q_start(q),
                None,
                Money::new(outstanding * 0.01, Currency::USD).expect("valid money fixture"),
                CFKind::Fixed,
                0.25,
                Some(0.04),
            ));
            flows.push(CashFlow::new(
                q_start(q),
                None,
                Money::from((100_000_i64, Currency::USD)),
                CFKind::Amortization,
                0.0,
                None,
            ));
            outstanding -= 100_000.0;
        }
        let instrument = SignedFlowInstrument::new(test_schedule(flows, 1_000_000.0, issue));

        let market_ctx = MarketContext::new();
        let mut opening = Money::from((1_000_000_i64, Currency::USD));
        // Q2..Q4 each contain one boundary-dated coupon + amortization.
        let expected_interest = [10_000.0, 9_000.0, 8_000.0];
        for (idx, q) in (2u8..=4).enumerate() {
            let end = if q == 4 {
                Date::from_calendar_date(2026, Month::January, 1).expect("valid date")
            } else {
                q_start(q + 1)
            };
            let period = Period {
                id: PeriodId::quarter(2025, q).expect("valid period fixture"),
                start: q_start(q),
                end,
                is_actual: false,
            };
            let (breakdown, closing, _, warnings) = calculate_period_flows(
                &instrument,
                &period,
                opening,
                Money::from((0_i64, Currency::USD)),
                &market_ctx,
                issue,
                None,
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
            id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
            start,
            end,
            is_actual: false,
        };

        let instrument = SignedFlowInstrument::new(test_schedule(
            vec![CashFlow::new(
                Date::from_calendar_date(2025, Month::February, 15).expect("valid date"),
                None,
                Money::from((20_000_i64, Currency::USD)),
                CFKind::Fixed,
                0.25,
                Some(0.08),
            )],
            1_000_000.0,
            start,
        ));

        let market_ctx = MarketContext::new();
        // Opening balance = scheduled 1.0M + 160k of toggle-capitalized PIK
        // (raw ratio 1.16, beyond the 1.10 clamp).
        let (breakdown, _, _, warnings) = calculate_period_flows(
            &instrument,
            &period,
            Money::from((1_160_000_i64, Currency::USD)),
            Money::from((160_000_i64, Currency::USD)),
            &market_ctx,
            start,
            None,
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

        // Without the toggled-PIK exclusion the same inputs warn (no clamp).
        let (unclamped, _, _, scale_warnings) = calculate_period_flows(
            &instrument,
            &period,
            Money::from((1_160_000_i64, Currency::USD)),
            Money::from((0_i64, Currency::USD)),
            &market_ctx,
            start,
            None,
        )
        .expect("period flow calculation should succeed");
        assert_eq!(scale_warnings.len(), 1);
        assert!((unclamped.interest_expense_cash.amount() - 23_200.0).abs() < 1e-9);
    }

    /// A revolver whose stateful balance was fully swept must not resurrect
    /// the scheduled balance on a re-draw: the new closing balance is the
    /// period's draws net of in-period repayments.
    #[test]
    fn revolver_redraw_after_full_sweep_nets_new_funding_against_repayments() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let period = Period {
            id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
            start,
            end,
            is_actual: false,
        };

        let issue = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let instrument = SignedFlowInstrument::new(test_schedule(
            vec![
                CashFlow::new(
                    Date::from_calendar_date(2025, Month::February, 15).expect("valid date"),
                    None,
                    Money::from((-100_000_i64, Currency::USD)),
                    CFKind::RevolvingDraw,
                    0.0,
                    None,
                ),
                CashFlow::new(
                    Date::from_calendar_date(2025, Month::March, 15).expect("valid date"),
                    None,
                    Money::from((30_000_i64, Currency::USD)),
                    CFKind::RevolvingRepayment,
                    0.0,
                    None,
                ),
            ],
            // Scheduled balance is 500k — the stateful balance (zero,
            // fully swept upstream) takes precedence.
            500_000.0,
            issue,
        ));

        let market_ctx = MarketContext::new();
        let (breakdown, closing, _, _) = calculate_period_flows(
            &instrument,
            &period,
            Money::from((0_i64, Currency::USD)),
            Money::from((0_i64, Currency::USD)),
            &market_ctx,
            start,
            None,
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
            id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
            start,
            end,
            is_actual: false,
        };

        let instrument = SignedFlowInstrument::new(test_schedule(
            vec![
                CashFlow::new(
                    Date::from_calendar_date(2025, Month::February, 15).expect("valid date"),
                    None,
                    Money::from((-5_000_i64, Currency::USD)),
                    CFKind::CommitmentFee,
                    0.25,
                    None,
                ),
                CashFlow::new(
                    Date::from_calendar_date(2025, Month::February, 15).expect("valid date"),
                    None,
                    Money::from((-2_000_i64, Currency::USD)),
                    CFKind::FacilityFee,
                    0.25,
                    None,
                ),
            ],
            1_000_000.0,
            start,
        ));

        let market_ctx = MarketContext::new();
        // Stateful balance at half the schedule → interest-like flows scale
        // by 0.5, but commitment/facility fees must not.
        let (breakdown, _, _, _) = calculate_period_flows(
            &instrument,
            &period,
            Money::from((500_000_i64, Currency::USD)),
            Money::from((0_i64, Currency::USD)),
            &market_ctx,
            start,
            None,
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
            id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
            start,
            end,
            is_actual: false,
        };

        let instrument = SignedFlowInstrument::new(test_schedule(
            vec![CashFlow::new(
                Date::from_calendar_date(2025, Month::February, 15).expect("valid date"),
                None,
                Money::from((50_000_i64, Currency::USD)),
                CFKind::Fixed,
                0.25,
                None,
            )],
            0.01,
            start,
        ));

        let market_ctx = MarketContext::new();
        let (breakdown, _, _, warnings) = calculate_period_flows(
            &instrument,
            &period,
            Money::from((100_000_i64, Currency::USD)),
            Money::from((0_i64, Currency::USD)),
            &market_ctx,
            start,
            None,
        )
        .expect("period flow calculation should succeed");

        // Scale is no longer clamped: the residual rebuild is the interest
        // engine. A pathological opening/scheduled ratio still books the
        // scaled coupon and surfaces a warning.
        assert!(
            (breakdown.interest_expense_cash.amount() - 50_000.0 * (100_000.0 / 0.01)).abs() < 1.0,
            "unclamped scale should apply, got {}",
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
                    EvalWarning::CapitalStructure {
                        warning: CapitalStructureWarning::ScaleClamped { .. },
                        ..
                    }
                )
            })
            .collect();
        assert_eq!(
            scale_warnings.len(),
            1,
            "expected exactly one scale_clamped warning, got: {warnings:?}"
        );
    }

    /// Coupon and amort dated on the first period start belong to that period
    /// at contractual amounts when opening is the pre-payment `< start` snapshot.
    #[test]
    fn first_period_opening_books_boundary_dated_jan1_flows() {
        let issue = Date::from_calendar_date(2024, Month::October, 1).expect("valid date");
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let period = Period {
            id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
            start,
            end,
            is_actual: false,
        };

        let instrument = SignedFlowInstrument::new(test_schedule(
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
                    start,
                    None,
                    Money::from((20_000_i64, Currency::USD)),
                    CFKind::Fixed,
                    0.25,
                    Some(0.08),
                ),
                CashFlow::new(
                    start,
                    None,
                    Money::from((100_000_i64, Currency::USD)),
                    CFKind::Amortization,
                    0.0,
                    None,
                ),
            ],
            1_000_000.0,
            issue,
        ));

        let market_ctx = MarketContext::new();
        let outstanding_path = instrument
            .schedule
            .outstanding_by_date()
            .expect("outstanding path");
        let scheduled_opening = outstanding_path
            .iter()
            .filter(|(d, _)| *d < period.start)
            .map(|(_, balance)| {
                if balance.amount() < 0.0 {
                    Money::new(-balance.amount(), balance.currency()).expect("valid money fixture")
                } else {
                    *balance
                }
            })
            .next_back()
            .unwrap_or(instrument.schedule.get_notional().initial);

        assert_eq!(
            scheduled_opening.amount(),
            1_000_000.0,
            "the < start snapshot is the pre-payment outstanding"
        );

        let (breakdown, _, _, warnings) = calculate_period_flows(
            &instrument,
            &period,
            scheduled_opening,
            Money::from((0_i64, Currency::USD)),
            &market_ctx,
            issue,
            None,
        )
        .expect("period flow calculation should succeed");

        assert!(
            warnings.is_empty(),
            "contractual Jan 1 flows at the < start opening must not scale, got {warnings:?}"
        );
        assert_eq!(breakdown.interest_expense_cash.amount(), 20_000.0);
        assert_eq!(breakdown.principal_payment.amount(), 100_000.0);
    }
}
