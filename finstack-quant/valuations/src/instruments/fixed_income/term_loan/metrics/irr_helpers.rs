//! Shared IRR calculation helpers for term loan yield metrics.
//!
//! This module provides unified IRR solving for YTM, YTC, YTW, and YT2Y/3Y/4Y metrics.
//!
//! All helpers use **kind-aware cashflow filtering** from the full schedule to ensure
//! correct treatment at exercise boundaries:
//! - Coupons/fees (all variants: `Fee`, `CommitmentFee`, `UsageFee`, `FacilityFee`):
//!   included up to AND including the exercise date
//! - Amortization/Notional (including negative funding legs): included only
//!   BEFORE the exercise date (the pre-exercise outstanding used for the
//!   redemption leg captures exercise-date amortization; draws on or after
//!   exercise are cancelled by the prepayment)
//! - PIK: always excluded (capitalization, not cash)

use std::sync::Arc;

use crate::cashflow::builder::schedule::CashFlowSchedule;
use crate::instruments::TermLoan;
use crate::metrics::MetricContext;
use finstack_quant_core::cashflow::{xirr_with_daycount, CFKind};
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;

/// Return the term loan's full internal cashflow schedule — with historical
/// fixings applied to seasoned floating periods — populating the shared
/// `MetricContext` cache on first access.
///
/// Multiple yield/spread metrics on the same loan call this; the cache avoids
/// rebuilding the (potentially large) DDTL/PIK schedule per metric. Using the
/// pricer's `pricing_schedule` (rather than the raw generated schedule) keeps
/// every IRR flow set and settlement accrued consistent with the base PV: the
/// current-period coupon of a seasoned floater reflects its actual fixing, not
/// today's forward projection. The downcast is performed internally so callers
/// don't need to thread an immutable `&TermLoan` borrow through the
/// `&mut MetricContext` access.
pub(super) fn cached_full_schedule(
    context: &mut MetricContext,
) -> finstack_quant_core::Result<Arc<CashFlowSchedule>> {
    if context.internal_schedule.is_none() {
        // Clone the instrument Arc so we can drop the context borrow before
        // building the schedule, then re-borrow context to write the cache.
        let inst = Arc::clone(&context.instrument);
        let loan = inst.as_any().downcast_ref::<TermLoan>().ok_or_else(|| {
            finstack_quant_core::InputError::NotFound {
                id: format!(
                    "instrument downcast: expected TermLoan, got {} (id={})",
                    context.instrument.key(),
                    context.instrument.id(),
                ),
            }
        })?;
        let schedule =
            crate::instruments::fixed_income::term_loan::pricing::TermLoanDiscountingPricer::pricing_schedule(
                loan,
                &context.curves,
                context.as_of,
            )?;
        context.internal_schedule = Some(Arc::new(schedule));
    }
    let arc = context
        .internal_schedule
        .as_ref()
        .ok_or(finstack_quant_core::InputError::Invalid)?;
    Ok(Arc::clone(arc))
}

/// Dirty purchase price implied by a clean price quote (% of outstanding).
///
/// Loan market convention (LSTA/LMA): a quoted price applies to the
/// **funded outstanding balance at settlement**, not the original commitment,
/// and the buyer additionally pays accrued cash interest:
///
/// `dirty = px/100 × outstanding(settlement) + accrued(settlement)`
///
/// For amortized or partially-drawn loans, quoting against `notional_limit`
/// would overstate the purchase price by the repaid/undrawn fraction.
pub(crate) fn quoted_dirty_from_clean_px(
    loan: &TermLoan,
    schedule: &CashFlowSchedule,
    as_of: Date,
    px: f64,
) -> finstack_quant_core::Result<Money> {
    let settlement = loan.settlement_date(as_of)?;
    let out_path = schedule.outstanding_by_date()?;
    let outstanding = out_path
        .iter()
        .rev()
        .find(|(date, _)| *date <= settlement)
        .map_or(schedule.get_notional().initial, |(_, amount)| *amount);
    // Principal already removed from the interest base but still payable
    // after settlement is part of the balance purchased with the quote.
    let pending_principal: f64 = schedule
        .get_flows()
        .iter()
        .filter(|flow| flow.get_balance_date() <= settlement && flow.date > settlement)
        .map(|flow| match flow.principal_delta {
            Some(delta) => (-delta.amount()).max(0.0),
            None if matches!(
                flow.kind,
                CFKind::Amortization | CFKind::PrePayment | CFKind::Notional
            ) =>
            {
                flow.amount.amount().max(0.0)
            }
            _ => 0.0,
        })
        .sum();
    let accrued = crate::cashflow::accrual::accrued_interest_amount(
        schedule,
        settlement,
        &loan.accrual_config(),
    )?;
    Money::new(
        px / 100.0 * (outstanding.amount() + pending_principal) + accrued,
        loan.currency,
    )
}

/// Resolve the target purchase price for quote-derived term-loan yield metrics.
///
/// Uses the quoted clean price when present, converted to a dirty settlement
/// amount via [`quoted_dirty_from_clean_px`]. Otherwise the instrument PV on
/// `as_of` (`base_value`, from either pricing engine) is carried to the
/// settlement date the yield is measured from.
pub(super) fn target_price_from_quote_or_model(
    loan: &TermLoan,
    schedule: &CashFlowSchedule,
    market: &finstack_quant_core::market_data::context::MarketContext,
    as_of: Date,
    base_value: Money,
) -> finstack_quant_core::Result<Money> {
    if let Some(px) = loan
        .instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price
    {
        quoted_dirty_from_clean_px(loan, schedule, as_of, px)
    } else {
        crate::instruments::fixed_income::term_loan::pricing::TermLoanDiscountingPricer::value_at_settlement(
            loan, market, as_of, base_value,
        )
    }
}

/// Solve IRR to an exercise date using kind-aware cashflow filtering.
///
/// This is the core IRR solver used by YTC and YTW metrics.
///
/// # Flow selection
///
/// Uses the full cashflow schedule for precise kind-based filtering:
/// - **Coupons and fees** (`Fixed`, `FloatReset`, `Stub`, `Fee`, `CommitmentFee`,
///   `UsageFee`, `FacilityFee`): included up to AND including `exercise_date` --
///   the holder receives accrued interest and fee payments on the exercise date.
/// - **Amortization** and **Notional** (redemptions and funding legs): included
///   only BEFORE `exercise_date`. Negative Notional flows are the holder's
///   future DDTL funding outflows; they must be in the flow set because the
///   redemption parameter is the pre-exercise outstanding, which includes the
///   balances those draws create. At the exercise date itself, amortization
///   and scheduled redemptions are implicitly captured in the pre-exercise
///   outstanding used for the redemption parameter, and any draw on or after
///   exercise is cancelled by the prepayment.
/// - **PIK**: always excluded (capitalization, not cash).
///
/// # Arguments
///
/// * `loan` - The term loan instrument
/// * `schedule` - Pre-computed full cashflow schedule (avoids regeneration)
/// * `as_of` - Valuation date
/// * `target_price` - Purchase price (dirty price, typically base PV)
/// * `exercise_date` - Exercise/call/maturity date
/// * `redemption` - Redemption amount at exercise date
///
/// # Returns
///
/// IRR (as decimal) that equates the initial price to the present value of
/// holder-view flows plus redemption.
pub(super) fn solve_irr_to_exercise(
    loan: &TermLoan,
    schedule: &CashFlowSchedule,
    as_of: Date,
    target_price: Money,
    exercise_date: Date,
    redemption: Money,
) -> finstack_quant_core::Result<f64> {
    // Compute settlement date using loan calendar/business-day conventions.
    let settlement_date = loan.settlement_date(as_of)?;

    let mut flows: Vec<(Date, f64)> = Vec::with_capacity(schedule.get_flows().len() + 2);

    // Initial price leg at settlement date (negative = cash outflow for purchase)
    flows.push((settlement_date, -target_price.amount()));

    // Kind-aware flow selection from the full schedule.
    // At the exercise date: include coupon/fee flows (holder receives accrued
    // interest) but exclude Amortization and Notional (the explicit redemption
    // parameter replaces them, using the pre-exercise outstanding).
    for cf in schedule.get_flows() {
        if cf.date <= settlement_date || cf.date > exercise_date {
            continue;
        }
        match cf.kind {
            // Coupons, interest, and all fee variants: include up to AND including
            // exercise date.  We match all fee kinds (`Fee`, `CommitmentFee`,
            // `UsageFee`, `FacilityFee`) to be forward-compatible if the cashflow
            // builder ever emits these specific fee variants directly.
            CFKind::Fixed
            | CFKind::FloatReset
            | CFKind::Stub
            | CFKind::Fee
            | CFKind::CommitmentFee
            | CFKind::UsageFee
            | CFKind::FacilityFee
            | CFKind::LcFee
            | CFKind::FrontingFee => {
                flows.push((cf.date, cf.amount.amount()));
            }
            // Principal flows strictly BEFORE the exercise date:
            // - Amortization: repayments received by the holder.
            // - Notional: positive scheduled redemptions AND negative funding
            //   legs (future DDTL draws the holder must fund). Excluding the
            //   funding legs while crediting the redemption of the balances
            //   they create would overstate the yield.
            // At the exercise date, the explicit redemption parameter replaces
            // scheduled Amortization/Notional to avoid double-counting, and
            // draws on or after exercise are cancelled by the prepayment.
            CFKind::Amortization | CFKind::Notional if cf.date < exercise_date => {
                flows.push((cf.date, cf.amount.amount()));
            }
            // Exclude: PIK, exercise-date Amortization/Notional
            _ => {}
        }
    }

    // Add explicit redemption at exercise date
    flows.push((exercise_date, redemption.amount()));

    xirr_with_daycount(flows.as_slice(), loan.day_count, None)
}

/// Solve IRR to a fixed horizon using kind-aware filtering and outstanding at horizon.
///
/// This is the core IRR solver used by YT2Y/3Y/4Y metrics.  The redemption is
/// the pre-exercise outstanding principal (the "sale" price at the horizon).
///
/// Uses the same kind-aware filtering convention as [`solve_irr_to_exercise`]
/// and the cached internal schedule from `MetricContext`.
pub(super) fn solve_irr_to_date(
    context: &mut MetricContext,
    target_price: Money,
    exercise_date: Date,
) -> finstack_quant_core::Result<f64> {
    let as_of = context.as_of;
    let schedule = cached_full_schedule(context)?;
    let out_path = schedule.outstanding_by_date()?;

    // Re-borrow loan for the IRR solver after dropping the cache write borrow.
    let loan: &TermLoan = context.instrument_as()?;
    let outstanding = outstanding_before(&out_path, exercise_date, loan.currency);

    solve_irr_to_exercise(
        loan,
        &schedule,
        as_of,
        target_price,
        exercise_date,
        outstanding,
    )
}

/// Exercisable call candidates as `(exercise_date, price_pct_of_par)` pairs.
///
/// A loan call entry is an effective-dated **standing** provision: it stays
/// exercisable until the next entry replaces it. Two families of candidates
/// result:
///
/// - **Future-dated provisions** (`call.date > as_of`): candidate at the
///   provision's own effective date, the standard yield-to-call convention.
/// - **The active standing provision** (last entry with `date <= as_of`): the
///   loan is callable *today* at that price — the normal state of a seasoned
///   leveraged loan past its soft-call window. Its candidate exercise date is
///   the first coupon date strictly after settlement (the market convention
///   for evaluating an immediately exercisable prepayment), matching the tree
///   engine's treatment of past-dated provisions as active from step 0.
///
/// `MakeWhole` provisions are skipped in both families: the borrower pays at
/// least the continuation value, so the option is non-economic.
///
/// # Arguments
///
/// * `loan` - Term loan whose `call_schedule` is resolved; entries after
///   `loan.maturity` are ignored.
/// * `schedule` - Full internal cashflow schedule used to locate the first
///   coupon date after settlement for the standing-call candidate.
/// * `as_of` - Valuation date separating standing from future provisions.
///
/// Returns an empty vector when the loan has no exercisable calls.
pub(super) fn exercisable_call_candidates(
    loan: &TermLoan,
    schedule: &CashFlowSchedule,
    as_of: Date,
) -> finstack_quant_core::Result<Vec<(Date, f64)>> {
    use crate::instruments::fixed_income::term_loan::LoanCallType;

    let Some(cs) = &loan.call_schedule else {
        return Ok(Vec::new());
    };

    let mut candidates: Vec<(Date, f64)> = cs
        .calls
        .iter()
        .filter(|c| {
            c.date > as_of
                && c.date <= loan.maturity
                && !matches!(c.call_type, LoanCallType::MakeWhole { .. })
        })
        .map(|c| (c.date, c.price_pct_of_par))
        .collect();

    // Active standing provision: only relevant when some entry is already
    // effective (`date <= as_of`). The candidate exercise date is the first
    // coupon date after settlement; the price is the provision active AT that
    // exercise date (a later entry may have replaced the one active today).
    // If the active entry is MakeWhole the loan is not economically callable.
    if cs.calls.iter().any(|c| c.date <= as_of) {
        let settlement = loan.settlement_date(as_of)?;
        let first_coupon_after_settlement = schedule
            .get_flows()
            .iter()
            .filter(|cf| {
                matches!(
                    cf.kind,
                    finstack_quant_core::cashflow::CFKind::Fixed
                        | finstack_quant_core::cashflow::CFKind::FloatReset
                        | finstack_quant_core::cashflow::CFKind::Stub
                )
            })
            .map(|cf| cf.date)
            .filter(|date| *date > settlement)
            .min();
        if let Some(exercise) = first_coupon_after_settlement {
            let active = cs
                .calls
                .iter()
                .rfind(|c| c.date <= exercise)
                .filter(|c| !matches!(c.call_type, LoanCallType::MakeWhole { .. }));
            if let Some(call) = active {
                if exercise < loan.maturity && !candidates.iter().any(|(d, _)| *d == exercise) {
                    candidates.push((exercise, call.price_pct_of_par));
                }
            }
        }
    }

    candidates.sort_by_key(|a| a.0);
    Ok(candidates)
}

/// Look up outstanding BEFORE a target date from the outstanding path.
///
/// Uses `<` comparison since `outstanding_by_date()` returns balances AFTER
/// processing all flows on each date.  This gives the balance just before
/// any events (amortization, notional, PIK) on the target date.
///
/// # Precondition
///
/// `out_path` must be sorted by date (as returned by `outstanding_by_date()`).
pub(super) fn outstanding_before(
    out_path: &[(Date, Money)],
    target: Date,
    currency: finstack_quant_core::currency::Currency,
) -> Money {
    let mut last = Money::from((0_i64, currency));
    for (d, amt) in out_path {
        if *d < target {
            last = *amt;
        } else {
            break;
        }
    }
    last
}
