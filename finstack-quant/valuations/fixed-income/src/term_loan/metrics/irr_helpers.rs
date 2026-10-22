//! Shared IRR calculation helpers for term loan yield metrics.
//!
//! This module provides unified IRR solving for YTM, YTC, YTW, and YT2Y/3Y/4Y metrics.
//!
//! All helpers select the schedule's cash-settlement flows (PIK excluded) paid
//! after settlement and on or before the exercise date; the redemption leg is
//! the claim remaining after exercise-date events.

use std::sync::Arc;

use crate::term_loan::TermLoan;
use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_core::cashflow::{xirr, CFKind};
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_valuations_core::metrics::MetricContext;

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
    crate::loan_quotes::cached_schedule(
        context,
        crate::term_loan::pricing::TermLoanDiscountingPricer::pricing_schedule,
    )
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
    let accrued = finstack_quant_cashflows::accrual::accrued_interest_amount(
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
        .quoted_clean_price_pct
    {
        quoted_dirty_from_clean_px(loan, schedule, as_of, px)
    } else {
        crate::term_loan::pricing::TermLoanDiscountingPricer::value_at_settlement(
            loan, market, as_of, schedule, base_value,
        )
    }
}

/// Solve IRR to an exercise date using kind-aware cashflow filtering.
///
/// This is the core IRR solver used by YTC and YTW metrics.
///
/// # Flow selection
///
/// Every cash-settlement flow ([`CFKind::is_cash_settlement`], the same set
/// YTM uses) paid after settlement and on or before `exercise_date` is
/// included: coupons, fees, scheduled amortization and redemptions, and
/// negative Notional DDTL funding legs. The redemption parameter is the claim
/// remaining after those exercise-date events, so nothing is counted twice.
/// PIK is excluded (capitalization, not cash). For the maturity leg, pass the
/// final *payment* date (see [`maturity_exercise_date`]) so a final payment
/// rolled past the unadjusted maturity is kept.
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

    // Scheduled cash paid on or before the exercise date. Redemption covers
    // the remaining claim after PIK capitalization and scheduled principal.
    for cf in schedule.get_flows() {
        if cf.date <= settlement_date || cf.date > exercise_date {
            continue;
        }
        // Same cash-settlement kinds as YTM; PIK is capitalized into the
        // remaining exercise claim.
        if cf.kind.is_cash_settlement() {
            flows.push((cf.date, cf.amount.amount()));
        }
    }

    // Add explicit redemption at exercise date
    flows.push((exercise_date, redemption.amount()));

    xirr(flows.as_slice(), loan.day_count, None)
}

/// Exercise date of the hold-to-maturity leg: the final scheduled payment
/// date, or the contractual maturity if that is later.
///
/// A business-day convention can roll the final coupon and redemption past the
/// unadjusted `loan.maturity`; truncating at the unadjusted date would drop
/// them from the YTW maturity leg while YTM keeps them.
///
/// # Arguments
///
/// * `loan` - Term loan whose contractual `maturity` is the floor.
/// * `schedule` - Full internal cashflow schedule with adjusted payment dates.
pub(super) fn maturity_exercise_date(loan: &TermLoan, schedule: &CashFlowSchedule) -> Date {
    schedule
        .get_flows()
        .iter()
        .map(|cf| cf.date)
        .max()
        .map_or(loan.maturity, |last| last.max(loan.maturity))
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
    let loan: &TermLoan = context.instrument_as()?;
    let (principal, cash_accrued) =
        super::super::pricing::call::ExerciseClaims::new(loan, &schedule)?.at(exercise_date)?;
    let outstanding = Money::new(principal + cash_accrued, loan.currency)?;

    solve_irr_to_exercise(
        loan,
        &schedule,
        as_of,
        target_price,
        exercise_date,
        outstanding,
    )
}

/// Exercisable call candidates as `(exercise_date, dirty_redemption)` pairs.
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
/// Make-whole provisions use the same dated reference-curve redemption as
/// deterministic tree pricing, with the clean floor and accrued cash once.
///
/// # Arguments
///
/// * `loan` - Term loan whose `call_schedule` is resolved; entries after
///   `loan.maturity` are ignored.
/// * `schedule` - Full internal cashflow schedule used to locate the first
///   coupon date after settlement for the standing-call candidate.
/// * `as_of` - Valuation date separating standing from future provisions.
/// * `market` - Reference discount curves for make-whole redemptions.
///
/// Returns an empty vector when the loan has no exercisable calls.
pub(super) fn exercisable_call_candidates(
    loan: &TermLoan,
    schedule: &CashFlowSchedule,
    as_of: Date,
    market: &finstack_quant_core::market_data::context::MarketContext,
) -> finstack_quant_core::Result<Vec<(Date, Money)>> {
    let Some(cs) = &loan.call_schedule else {
        return Ok(Vec::new());
    };

    let mut candidates = cs
        .calls
        .iter()
        .filter(|c| c.date > as_of && c.date <= loan.maturity)
        .map(|c| (c.date, c))
        .collect::<Vec<_>>();

    // Active standing provision: only relevant when some entry is already
    // effective (`date <= as_of`). The candidate exercise date is the first
    // coupon date after settlement; the price is the provision active AT that
    // exercise date (a later entry may have replaced the one active today).
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
            let active = cs.calls.iter().rfind(|c| c.date <= exercise);
            if let Some(call) = active {
                if exercise < loan.maturity && !candidates.iter().any(|(d, _)| *d == exercise) {
                    candidates.push((exercise, call));
                }
            }
        }
    }

    candidates.sort_by_key(|a| a.0);
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let claims = super::super::pricing::call::ExerciseClaims::new(loan, schedule)?;
    candidates
        .into_iter()
        .map(|(exercise, call)| {
            let (outstanding, accrued) = claims.at(exercise)?;
            let clean = super::super::pricing::call::clean_call_price(
                call,
                schedule,
                market,
                exercise,
                outstanding,
                accrued,
            )?;
            Ok((exercise, Money::new(clean + accrued, loan.currency)?))
        })
        .collect()
}
