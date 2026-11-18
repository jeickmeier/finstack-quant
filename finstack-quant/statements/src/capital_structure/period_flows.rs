//! Per-period cashflow extraction for a single instrument.
//!
//! The schedule-classification logic and its safety clamps live here, next to
//! their tests; the evaluator's capital-structure runtime drives this function
//! per instrument and period and handles currency / FX bookkeeping. The logic here:
//!
//! 1. Pulls the full classified raw schedule, retaining history and PIK events.
//! 2. Requires the period-opening balance of the debt ledger to agree with
//!    the principal the dated schedule carries into the period; a
//!    disagreement is an error and no amount is rescaled.
//! 3. Classifies each in-period flow by `CFKind` into the
//!    [`CashflowBreakdown`] buckets (cash interest, PIK interest, principal,
//!    fees) — credit events and unknown variants emit warnings instead of
//!    being silently aggregated.
//! 4. Snapshots the closing balance and accrued interest at `period.end - 1`.

use crate::capital_structure::cashflows::CashflowBreakdown;
use crate::error::Result;
use crate::evaluator::EvalWarning;
use finstack_quant_cashflows::accrual::accrued_interest_amount;
use finstack_quant_cashflows::builder::{CashFlowSchedule, CashflowRepresentation};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::cashflow::CashFlow;
use finstack_quant_core::dates::{Date, Period};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::pricer::InstrumentType;

/// Whose cash the signed amounts of a schedule are.
///
/// A schedule supplied to Statements states this; it is never inferred from
/// the amounts. Principal effects are signed changes of the outstanding
/// balance in either perspective and are not converted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulePerspective {
    /// Positive is cash received by the holder of the instrument: the form
    /// every canonical instrument schedule has. For a borrowing the company
    /// is the issuer, so its cash is the negative; a hedge is held by the
    /// company, so its cash is the amount itself.
    Holder,
    /// Positive is cash received by the reporting company.
    Company,
}

impl SchedulePerspective {
    /// The company's signed cash for a schedule amount: receipts positive,
    /// payments negative. This is the one conversion at the boundary.
    fn company_cash(self, amount: f64, is_debt: bool) -> f64 {
        match self {
            Self::Holder if is_debt => -amount,
            Self::Holder | Self::Company => amount,
        }
    }
}

/// A custom or rebuilt schedule with its stated perspective.
#[derive(Debug, Clone, Copy)]
pub struct SuppliedSchedule<'a> {
    /// The schedule to read in place of the instrument's own.
    pub schedule: &'a CashFlowSchedule,
    /// Whose cash its signed amounts are.
    pub perspective: SchedulePerspective,
}

impl<'a> SuppliedSchedule<'a> {
    /// A schedule in the holder perspective, the form canonical instrument
    /// schedules and Statements' own residual rebuilds have.
    ///
    /// # Arguments
    ///
    /// * `schedule` - Schedule whose positive amounts are cash received by
    ///   the instrument's holder.
    #[must_use]
    pub fn holder(schedule: &'a CashFlowSchedule) -> Self {
        Self {
            schedule,
            perspective: SchedulePerspective::Holder,
        }
    }
}

/// Reject an empty or reversed reporting period before any schedule is read.
///
/// # Arguments
///
/// * `period` - Reporting period; cash belongs to it when its settlement
///   date is in the half-open interval `[start, end)`, which must not be
///   empty.
///
/// # Errors
///
/// Returns a capital-structure error when `period.end <= period.start`.
pub(crate) fn validate_period(period: &Period) -> Result<()> {
    if period.end <= period.start {
        return Err(crate::error::Error::capital_structure(format!(
            "reporting period {} is empty or reversed: [{}, {})",
            period.id, period.start, period.end
        )));
    }
    Ok(())
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
    if !matches!(flow.get_kind(), CFKind::Notional | CFKind::RevolvingDraw) {
        return None;
    }
    // New principal is the explicit positive delta, never the cash
    // (record CORE-CONVENTIONS-015 #4, #5).
    flow.get_principal_delta()
        .filter(|delta| delta.amount() > 0.0)
}

/// Capitalized amount of a PIK row: its explicit signed principal delta,
/// never its cash amount (record CORE-CONVENTIONS-015 #1, #4). A negative
/// delta is an authorized correction and reduces the capitalized total.
fn pik_capitalized(cf: &CashFlow) -> Result<f64> {
    cf.get_principal_delta()
        .map(|delta| delta.amount())
        .ok_or_else(|| {
            crate::error::Error::capital_structure(format!(
                "PIK row on {} has no principal_delta; its capitalized amount must be explicit",
                cf.get_date()
            ))
        })
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
    period.end - time::Duration::days(1)
}

/// Whether a holder-perspective flow returns principal to the lender:
/// amortization, prepayment, revolver repayment, or a positive notional
/// (redemption) row.
fn is_repayment_like(flow: &CashFlow) -> bool {
    flow.get_kind().is_repayment_like()
        && (flow.get_kind() != CFKind::Notional || flow.get_amount().amount() > 0.0)
}

/// Distress cash of one instrument in one period: recovery, accrual paid on
/// default and protection payments, each in the company's cash perspective
/// with its actual payment date. These amounts are outside scheduled debt
/// service and change no principal.
///
/// # Arguments
///
/// * `instrument_id` - Capital-structure identifier recorded on each row.
/// * `is_debt` - Whether the company is the issuer (a borrowing) or the
///   holder (a hedge) of the instrument.
/// * `supplied` - The schedule to read and the perspective of its amounts.
/// * `period` - Reporting period; a row belongs to it when its payment date
///   is in `[start, end)`.
///
/// # Errors
///
/// Returns a capital-structure error for an empty or reversed period.
pub(crate) fn period_distress_cash(
    instrument_id: &str,
    is_debt: bool,
    supplied: SuppliedSchedule<'_>,
    period: &Period,
) -> Result<Vec<crate::capital_structure::DistressCashflow>> {
    validate_period(period)?;
    supplied
        .schedule
        .get_flows()
        .iter()
        .filter(|flow| flow.get_date() >= period.start && flow.get_date() < period.end)
        .filter(|flow| {
            matches!(
                flow.get_kind(),
                CFKind::Recovery | CFKind::AccruedOnDefault | CFKind::ProtectionPayment
            )
        })
        .map(|flow| {
            let amount = flow.get_amount();
            Ok(crate::capital_structure::DistressCashflow {
                instrument_id: instrument_id.to_string(),
                payment_date: flow.get_date(),
                kind: flow.get_kind(),
                amount: Money::new(
                    supplied.perspective.company_cash(amount.amount(), is_debt),
                    amount.currency(),
                )?,
            })
        })
        .collect()
}

/// The face a repayment row removes: its explicit principal delta, or its
/// cash when the row carries none.
fn repayment_face(flow: &CashFlow) -> Money {
    flow.get_principal_delta()
        .unwrap_or_else(|| flow.get_amount())
        .abs()
}

/// This period's scheduled principal claims, each with its due date, its
/// economic effective date, its cash and the face that cash settles.
///
/// A claim's identity is its due date, effective date and ordinal among the
/// rows that share both, so it is stable across reporting cadences.
pub(crate) fn period_principal_claims(
    schedule: &CashFlowSchedule,
    period: &Period,
) -> Vec<crate::capital_structure::PrincipalClaim> {
    let mut ordinals = std::collections::BTreeMap::<(Date, Date), usize>::new();
    schedule
        .get_flows()
        .iter()
        .filter(|flow| is_repayment_like(flow))
        .filter(|flow| flow.get_amount().amount() != 0.0)
        // A repayment row carries its explicit balance date.
        .filter_map(|flow| Some((flow, flow.get_balance_date()?)))
        .filter_map(|(flow, balance_date)| {
            let ordinal = ordinals.entry((flow.get_date(), balance_date)).or_default();
            let id = format!("{}/{}/{}", flow.get_date(), balance_date, *ordinal);
            *ordinal += 1;
            (flow.get_date() >= period.start && flow.get_date() < period.end).then(|| {
                crate::capital_structure::PrincipalClaim {
                    id,
                    payment_date: flow.get_date(),
                    balance_date,
                    amount: flow.get_amount().abs(),
                    face: repayment_face(flow),
                    payments: Vec::new(),
                }
            })
        })
        .collect()
}

/// Scheduled principal at a period close whose cash and face fall on
/// opposite sides of it: cash owed for a reduction already effective
/// (`pending`), and face not yet reduced whose cash was already due
/// (`advance`, a reserve when that cash was paid).
fn principal_across_close(
    schedule: &CashFlowSchedule,
    snapshot: Date,
    currency: finstack_quant_core::currency::Currency,
) -> Result<(Money, Money)> {
    let mut pending = Money::from((0_i64, currency));
    let mut advance = Money::from((0_i64, currency));
    for flow in schedule
        .get_flows()
        .iter()
        .filter(|flow| is_repayment_like(flow))
    {
        let Some(balance_date) = flow.get_balance_date() else {
            continue;
        };
        if balance_date <= snapshot && flow.get_date() > snapshot {
            pending = pending.checked_add(flow.get_amount().abs())?;
        } else if balance_date > snapshot && flow.get_date() <= snapshot {
            advance = advance.checked_add(repayment_face(flow))?;
        }
    }
    Ok((pending, advance))
}

/// Principal the dated schedule carries into a period that starts on
/// `period_start`: the balance strictly before that date.
///
/// A flow dated on `period_start` belongs to the period, so it does not move
/// the opening. Before the first dated balance, an instrument funded on
/// `period_start` or issued later opens at zero, and any other instrument
/// opens at its initial notional. [`calculate_period_flows`] requires the
/// opening balance it is given to agree with this amount.
///
/// # Arguments
///
/// * `schedule` - Dated holder-perspective schedule of one borrowing, raw
///   or rebuilt, with its notional, issue date and principal events.
/// * `period_start` - Inclusive first day of the reporting period; a flow
///   dated on it belongs to the period and does not move the opening.
///
/// # Returns
///
/// The non-negative principal outstanding strictly before `period_start`,
/// in the schedule's currency.
///
/// # Errors
///
/// Propagates the schedule's outstanding-path error.
pub fn scheduled_opening_principal(
    schedule: &CashFlowSchedule,
    period_start: Date,
) -> Result<Money> {
    let outstanding_path = schedule.outstanding_by_date()?;
    if let Some((_, balance)) = outstanding_path
        .iter()
        .rfind(|(date, _)| *date < period_start)
    {
        return Ok(balance.abs());
    }
    let currency = schedule.get_notional().initial.currency();
    // Initial funding dated on the period boundary belongs to that period:
    // the opening is zero and the funding is booked in the period.
    let funding_at_start = schedule.get_meta().issue_date == Some(period_start)
        && schedule.get_flows().iter().any(|flow| {
            flow.get_balance_date() == Some(period_start)
                && funding_principal(flow).is_some_and(|principal| principal.amount() > 0.0)
        });
    // A forward-dated instrument has no principal before issuance, including
    // a schedule that omits the issuance cash leg.
    let pre_issuance = schedule
        .get_meta()
        .issue_date
        .is_some_and(|issue| issue > period_start);
    if funding_at_start || pre_issuance {
        return Ok(Money::from((0_i64, currency)));
    }
    // Before the first event, principal is the schedule's initial notional.
    // The first path row is an after-event balance, so using it here would
    // apply boundary amortization or a future redemption too early.
    Ok(schedule.get_notional().initial.abs())
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
/// * `market_ctx` - Market context for pricing
/// * `as_of` - Valuation date used to generate the instrument's full cashflow
///   schedule and identify historical versus projected amounts.
/// * `residual_schedule` - Rebuilt remaining schedule after a prior outstanding
///   change; `None` reads the instrument's raw schedule instead. Period flows
///   are read from this schedule (the interest engine) and a warning is raised if
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
    market_ctx: &MarketContext,
    as_of: Date,
    residual_schedule: Option<SuppliedSchedule<'_>>,
) -> Result<(CashflowBreakdown, Money, Money, Vec<EvalWarning>)> {
    validate_period(period)?;
    let is_debt = is_debt_kind(instrument.key())?;
    // The instrument's own schedule is in the holder perspective by the
    // cashflow contract; a supplied schedule states its own.
    let (full_schedule, perspective) = match residual_schedule {
        Some(supplied) => (supplied.schedule.clone(), supplied.perspective),
        None => (
            instrument.raw_cashflow_schedule(market_ctx, as_of)?,
            SchedulePerspective::Holder,
        ),
    };
    // Balances, funding and repayment rows are read structurally in the
    // holder form every producer emits. A company-perspective borrowing
    // schedule would need its cash rows restated first, which no producer
    // requires.
    if is_debt && perspective == SchedulePerspective::Company {
        return Err(crate::error::Error::capital_structure(format!(
            "instrument '{}': a borrowing schedule must be supplied in the holder perspective",
            instrument.id()
        )));
    }
    let company_cash = |amount: f64| perspective.company_cash(amount, is_debt);
    if full_schedule.get_meta().representation == CashflowRepresentation::Placeholder {
        return Err(crate::error::Error::capital_structure(format!(
            "instrument '{}' has an unsupported placeholder cashflow schedule; contingent payouts are not modeled",
            instrument.id()
        )));
    }
    // The native currency is the schedule's own, for zero amounts too.
    let currency = full_schedule.get_notional().initial.currency();
    if opening_balance.currency() != currency {
        return Err(crate::error::Error::CurrencyMismatch(
            currency,
            opening_balance.currency(),
        ));
    }
    let mut breakdown = CashflowBreakdown::with_currency(currency);
    let warnings = Vec::new();
    let snapshot_date = period_snapshot_date(period);
    let outstanding_path = if is_debt {
        full_schedule.outstanding_by_date()?
    } else {
        Vec::new()
    };
    // Boundary convention: flows dated exactly on `period.start` belong to
    // *this* period (half-open `[start, end)`), so the scheduled opening is
    // the balance strictly *before* `period.start`, matching the
    // `end - 1 day` closing snapshot of the prior period.
    let scheduled_opening = scheduled_opening_principal(&full_schedule, period.start)?;

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
                return Err(crate::error::Error::CurrencyMismatch(
                    currency,
                    principal.currency(),
                ));
            }
            if flow.get_amount().currency() != currency {
                return Err(crate::error::Error::CurrencyMismatch(
                    currency,
                    flow.get_amount().currency(),
                ));
            }
            if flow.get_balance_date().is_some_and(|funding_date| {
                funding_date >= period.start && funding_date < period.end
            }) {
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
    // The dated schedule and the debt ledger must agree on the principal
    // carried into the period. Debt service is read from the schedule as it
    // stands; a disagreement is an error, never a rescaling of the period's
    // amounts. Half a minor unit is the same dust bound the ledger uses.
    if is_debt {
        const OPENING_TOLERANCE: f64 = 0.005;
        let gap = opening_balance.amount() - scheduled_opening.amount();
        if gap.abs() > OPENING_TOLERANCE {
            return Err(crate::error::Error::capital_structure(format!(
                "instrument '{}', period {}: the debt ledger opens at {} {currency} but its \
                 dated schedule carries {} {currency} into the period. The schedule and the \
                 ledger must agree on opening principal at the same boundary",
                instrument.id(),
                period.id,
                opening_balance.amount(),
                scheduled_opening.amount(),
            )));
        }
    }

    // Company cash, receipts positive and payments negative, converted once
    // from the schedule's perspective. Coupons are netted with their sign, so
    // negative rates or a zero leg cannot turn a receipt into an expense.
    let mut net_coupon_cash = 0.0_f64;
    let unclassified = |cf: &CashFlow, why: &str| {
        crate::error::Error::capital_structure(format!(
            "instrument '{}': {:?} cashflow on {} {why}",
            instrument.id(),
            cf.get_kind(),
            cf.get_date()
        ))
    };

    for cf in full_schedule.get_flows() {
        if cf.get_date() >= period.start && cf.get_date() < period.end {
            // Currency safety: every in-period flow must share the breakdown
            // currency. Cross-currency instruments (e.g. xccy swaps) mix
            // currencies in one schedule and require explicit FX, which this
            // classifier does not perform. Reject rather than let `Money`
            // arithmetic panic on a currency mismatch.
            if cf.get_amount().currency() != currency {
                return Err(crate::error::Error::CurrencyMismatch(
                    currency,
                    cf.get_amount().currency(),
                ));
            }
            let cash = company_cash(cf.get_amount().amount());
            let payable = |cash: f64| -> Result<Money> {
                if cash > 0.0 {
                    return Err(unclassified(
                        cf,
                        "is a receipt where the company pays; rebates and refunds are not reported yet",
                    ));
                }
                // The row's own amount, exactly: no round trip through a float.
                Ok(cf.get_amount().abs())
            };

            // Every kind is named: a new kind cannot be classified silently.
            match cf.get_kind() {
                CFKind::Fixed | CFKind::Floating | CFKind::Stub | CFKind::InflationCoupon => {
                    net_coupon_cash += cash;
                }
                CFKind::Amortization | CFKind::PrePayment | CFKind::RevolvingRepayment => {
                    breakdown.principal_payment =
                        breakdown.principal_payment.checked_add(payable(cash)?)?;
                }
                // A notional row is a redemption when the company pays it.
                CFKind::Notional if is_repayment_like(cf) => {
                    breakdown.principal_payment =
                        breakdown.principal_payment.checked_add(payable(cash)?)?;
                }
                CFKind::CommitmentFee
                | CFKind::FacilityFee
                | CFKind::LcFee
                | CFKind::FrontingFee
                | CFKind::Fee
                | CFKind::UsageFee => {
                    breakdown.fees = breakdown.fees.checked_add(payable(cash)?)?;
                }
                CFKind::Pik => {
                    let capitalized = Money::new(pik_capitalized(cf)?, currency)?;
                    breakdown.interest_expense_pik =
                        breakdown.interest_expense_pik.checked_add(capitalized)?;
                }
                // Rows with no debt-service amount of their own:
                // - funding (a notional row the company receives, or a draw)
                //   establishes principal through its explicit delta, read above;
                // - a write-down is a dated explicit principal change that
                //   moves the balance through the outstanding path;
                // - distress cash is reported separately with its payment
                //   date by `period_distress_cash`.
                CFKind::Notional
                | CFKind::RevolvingDraw
                | CFKind::DefaultedNotional
                | CFKind::Recovery
                | CFKind::AccruedOnDefault
                | CFKind::ProtectionPayment => {}
                CFKind::InitialMarginPost
                | CFKind::InitialMarginReturn
                | CFKind::VariationMarginReceive
                | CFKind::VariationMarginPay
                | CFKind::MarginInterest
                | CFKind::CollateralSubstitutionIn
                | CFKind::CollateralSubstitutionOut => {
                    return Err(unclassified(cf, "has no capital-structure classification"));
                }
                _ => {
                    return Err(unclassified(
                        cf,
                        "is an unknown kind with no capital-structure classification",
                    ));
                }
            }
        }
    }

    // Net company coupon cash C: expense max(-C, 0), income max(C, 0).
    let (expense, income) = ((-net_coupon_cash).max(0.0), net_coupon_cash.max(0.0));
    breakdown.interest_expense_cash = Money::new(expense, currency)?;
    breakdown.interest_income_cash = Some(Money::new(income, currency)?);

    // outstanding_path only has entries on cashflow dates, so take the latest
    // entry at or before period.end.
    let scheduled_closing_balance = outstanding_path
        .iter()
        .rev()
        .find(|(date, _)| *date <= snapshot_date)
        .map(|(_, balance)| balance.abs())
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
            initial.abs()
        });

    // The close follows the dated schedule on one clock: every funding,
    // repayment and capitalization moves the balance on its
    // balance-effective date, whether the debt was carried in, newly issued
    // or redrawn, and whenever its cash settles.
    let closing_balance = if is_debt {
        scheduled_closing_balance
    } else {
        Money::from((0_i64, currency))
    };
    breakdown.debt_balance = closing_balance;

    // Contractual cash interest earned through the period close and due
    // after it, from the instrument's own earning terms: its accrual method
    // and coupon frequency (which ACT/ACT ISMA needs), with no settlement
    // ex-coupon window and no PIK. An instrument that does not state those
    // terms cannot be accrued. A hedge's merged legs are not a debt schedule
    // and need a separate valuation accrual policy.
    let accrued_interest = if !is_debt {
        0.0
    } else {
        let terms = instrument.contractual_accrual_config().ok_or_else(|| {
            crate::error::Error::capital_structure(format!(
                "instrument '{}' states no contractual accrual terms; accrued interest at the \
                 close of {} cannot be calculated",
                instrument.id(),
                period.id
            ))
        })?;
        accrued_interest_amount(&full_schedule, snapshot_date, &terms)?.amount()
    };
    breakdown.accrued_interest = Money::new(accrued_interest, currency)?;

    // Audit detail: every contractual principal flow is scheduled principal.
    breakdown.opening_balance = Some(opening_balance);
    breakdown.mark_principal_as_scheduled();
    // Principal whose cash and face fall on opposite sides of the close. On
    // the contractual path every scheduled payment is made when due, so
    // nothing is in arrears and cash paid ahead of its reduction is reserved.
    let (pending, advance) = if is_debt {
        principal_across_close(&full_schedule, snapshot_date, currency)?
    } else {
        let zero = Money::from((0_i64, currency));
        (zero, zero)
    };
    breakdown.principal_pending = Some(pending);
    breakdown.principal_advance_reserve = Some(advance);

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
    use finstack_quant_core::cashflow::CFKind;
    use finstack_quant_core::cashflow::{CashFlow, CashFlowParams};
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

        fn contractual_accrual_config(
            &self,
        ) -> Option<finstack_quant_cashflows::accrual::AccrualConfig> {
            Some(finstack_quant_cashflows::accrual::AccrualConfig {
                frequency: Some(finstack_quant_core::dates::Tenor::quarterly()),
                ..Default::default()
            })
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
        CashFlowSchedule::from_flows(
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
            vec![
                CashFlowParams::new(
                    Date::from_calendar_date(2025, Month::February, 15).expect("valid date"),
                    None,
                    Money::from((-50_000_i64, Currency::USD)),
                    CFKind::Fixed,
                    0.25,
                    None,
                )
                .build()
                .expect("valid cashflow"),
            ],
            1_000_000.0,
            start,
        ));

        let market_ctx = MarketContext::new();
        let (breakdown, _, _, warnings) = calculate_period_flows(
            &instrument,
            &period,
            Money::from((1_000_000_i64, Currency::USD)),
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
                CashFlowParams::new(
                    pay_date,
                    None,
                    Money::from((-40_000_i64, Currency::USD)),
                    CFKind::Fixed,
                    0.25,
                    None,
                )
                .build()
                .expect("valid cashflow"),
                CashFlowParams::new(
                    pay_date,
                    None,
                    Money::from((53_000_i64, Currency::USD)),
                    CFKind::Floating,
                    0.25,
                    None,
                )
                .build()
                .expect("valid cashflow"),
            ],
            1_000_000.0,
            start,
        ));

        let swap = finstack_quant_valuations::instruments::rates::irs::InterestRateSwap::example()
            .expect("valid IRS fixture");
        let market_ctx = MarketContext::new();
        let (breakdown, _, _, _) = calculate_period_flows(
            &swap,
            &period,
            Money::from((1_000_000_i64, Currency::USD)),
            &market_ctx,
            start,
            Some(SuppliedSchedule::holder(&instrument.schedule)),
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
                CashFlowParams::new(
                    pay_date,
                    None,
                    Money::from((-40_000_i64, Currency::USD)),
                    CFKind::Fixed,
                    0.25,
                    None,
                )
                .build()
                .expect("valid cashflow"),
                CashFlowParams::new(
                    pay_date,
                    None,
                    Money::from((10_000_i64, Currency::USD)),
                    CFKind::Floating,
                    0.25,
                    None,
                )
                .build()
                .expect("valid cashflow"),
            ],
            1_000_000.0,
            start,
        ));

        let swap = finstack_quant_valuations::instruments::rates::irs::InterestRateSwap::example()
            .expect("valid IRS fixture");
        let market_ctx = MarketContext::new();
        let (breakdown, _, _, _) = calculate_period_flows(
            &swap,
            &period,
            Money::from((1_000_000_i64, Currency::USD)),
            &market_ctx,
            start,
            Some(SuppliedSchedule::holder(&instrument.schedule)),
        )
        .expect("period flow calculation should succeed");

        assert_eq!(breakdown.interest_expense_cash.amount(), 30_000.0);
        assert_eq!(breakdown.interest_income_cash_or_zero().amount(), 0.0);
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
            vec![{
                let cash = Money::from((-100_000_i64, Currency::USD));
                CashFlowParams::new(
                    Date::from_calendar_date(2025, Month::February, 15).expect("valid date"),
                    None,
                    cash,
                    CFKind::RevolvingDraw,
                    0.0,
                    None,
                )
                .with_principal_delta(cash.checked_neg())
                .with_principal_date(
                    Date::from_calendar_date(2025, Month::February, 15).expect("valid date"),
                )
                .build()
                .expect("valid cashflow")
            }],
            0.0,
            start,
        ));

        let market_ctx = MarketContext::new();
        let (breakdown, closing_balance, _, warnings) = calculate_period_flows(
            &instrument,
            &period,
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
    /// with scale = 1.0 and no `residual_scale_mismatch` warning. The old
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
            flows.push(
                CashFlowParams::new(
                    q_start(q),
                    None,
                    Money::new(outstanding * 0.01, Currency::USD).expect("valid money fixture"),
                    CFKind::Fixed,
                    0.25,
                    Some(0.04),
                )
                .build()
                .expect("valid cashflow"),
            );
            flows.push({
                let cash = Money::from((100_000_i64, Currency::USD));
                CashFlowParams::new(q_start(q), None, cash, CFKind::Amortization, 0.0, None)
                    .with_principal_delta(cash.checked_neg())
                    .with_principal_date(q_start(q))
                    .build()
                    .expect("valid cashflow")
            });
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
            let (breakdown, closing, _, warnings) =
                calculate_period_flows(&instrument, &period, opening, &market_ctx, issue, None)
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

    /// The ledger and the dated schedule must agree on the principal carried
    /// into a period: a disagreement names both amounts and nothing is
    /// rescaled.
    #[test]
    fn an_opening_balance_that_disagrees_with_the_schedule_is_an_error() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let period = Period {
            id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
            start,
            end,
            is_actual: false,
        };
        let issue = Date::from_calendar_date(2024, Month::October, 1).expect("valid date");
        let instrument = SignedFlowInstrument::new(test_schedule(
            vec![
                CashFlowParams::new(
                    Date::from_calendar_date(2025, Month::February, 15).expect("valid date"),
                    None,
                    Money::from((50_000_i64, Currency::USD)),
                    CFKind::Fixed,
                    0.25,
                    None,
                )
                .build()
                .expect("valid cashflow"),
            ],
            1_000_000.0,
            issue,
        ));
        let market_ctx = MarketContext::new();
        let flows = |opening: i64| {
            calculate_period_flows(
                &instrument,
                &period,
                Money::from((opening, Currency::USD)),
                &market_ctx,
                start,
                None,
            )
        };

        // Agreement: the coupon is read from the schedule as it stands.
        let (breakdown, closing, _, warnings) = flows(1_000_000).expect("agreeing opening");
        assert_eq!(breakdown.interest_expense_cash.amount(), 50_000.0);
        assert_eq!(closing.amount(), 1_000_000.0);
        assert!(warnings.is_empty());

        // Half the scheduled principal used to halve the coupon.
        for opening in [500_000, 0, 1_100_000] {
            let error = flows(opening).expect_err("mismatched opening").to_string();
            for needle in ["SCHEDULE", "2025Q1", "USD", &opening.to_string(), "1000000"] {
                assert!(error.contains(needle), "{needle}: {error}");
            }
        }
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
                {
                    let cash = Money::from((-1_000_000_i64, Currency::USD));
                    CashFlowParams::new(issue, None, cash, CFKind::Notional, 0.0, None)
                        .with_principal_delta(finstack_quant_core::money::Money::zero(
                            cash.currency(),
                        ))
                        .with_principal_date(issue)
                        .build()
                        .expect("valid cashflow")
                },
                CashFlowParams::new(
                    start,
                    None,
                    Money::from((20_000_i64, Currency::USD)),
                    CFKind::Fixed,
                    0.25,
                    Some(0.08),
                )
                .build()
                .expect("valid cashflow"),
                {
                    let cash = Money::from((100_000_i64, Currency::USD));
                    CashFlowParams::new(start, None, cash, CFKind::Amortization, 0.0, None)
                        .with_principal_delta(cash.checked_neg())
                        .with_principal_date(start)
                        .build()
                        .expect("valid cashflow")
                },
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
