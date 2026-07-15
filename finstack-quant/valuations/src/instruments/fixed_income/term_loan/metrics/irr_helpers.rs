//! Shared IRR calculation helpers for term loan yield metrics.
//!
//! This module provides unified IRR solving for YTM, YTC, YTW, and YT2Y/3Y/4Y metrics.
//!
//! All helpers use **kind-aware cashflow filtering** from the full schedule to ensure
//! correct treatment at exercise boundaries:
//! - Coupons/fees (all variants: `Fee`, `CommitmentFee`, `UsageFee`, `FacilityFee`):
//!   included up to AND including the exercise date
//! - Amortization/Notional: included only BEFORE the exercise date (implicitly
//!   captured in the pre-exercise outstanding used for the redemption leg)
//! - PIK and negative Notional (funding): always excluded

use std::sync::Arc;

use crate::cashflow::builder::schedule::CashFlowSchedule;
use crate::instruments::TermLoan;
use crate::metrics::MetricContext;
use finstack_quant_core::cashflow::{xirr_with_daycount, CFKind};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

/// Return the term loan's full internal cashflow schedule, populating the
/// shared `MetricContext` cache on first access.
///
/// Multiple yield/spread metrics on the same loan call this; the cache avoids
/// rebuilding the (potentially large) DDTL/PIK schedule per metric. The
/// downcast is performed internally so callers don't need to thread an
/// immutable `&TermLoan` borrow through the `&mut MetricContext` access.
pub(super) fn cached_full_schedule(
    context: &mut MetricContext,
) -> finstack_quant_core::Result<Arc<CashFlowSchedule>> {
    if context.internal_schedule.is_none() {
        // Clone the instrument Arc so we can drop the context borrow before
        // calling generate_cashflows, then re-borrow context to write the cache.
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
        let schedule = crate::instruments::fixed_income::term_loan::cashflows::generate_cashflows(
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

/// Discount factor from `as_of` to the loan's settlement date on its discount
/// curve. Used to forward-value an `as_of` model PV to a settlement-dated price.
/// Returns 1.0 when settlement coincides with `as_of`.
pub(super) fn settlement_discount_factor(
    loan: &TermLoan,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    let settle = loan.settlement_date(as_of)?;
    if settle <= as_of {
        return Ok(1.0);
    }
    curves
        .get_discount(loan.discount_curve_id.as_str())?
        .df_between_dates(as_of, settle)
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
    let outstanding = outstanding_before(&out_path, settlement, loan.currency);
    let accrued = crate::cashflow::accrual::accrued_interest_amount(
        schedule,
        settlement,
        &loan.accrual_config(),
    )?;
    Ok(Money::new(
        px / 100.0 * outstanding.amount() + accrued,
        loan.currency,
    ))
}

/// Resolve the target purchase price for quote-derived term-loan yield metrics.
///
/// Uses the quoted clean price when present (already a settlement-date price),
/// converted to a dirty settlement amount via [`quoted_dirty_from_clean_px`];
/// otherwise forward-values the `as_of` model PV (`context.base_value`) to the
/// settlement date via `settle_df = DF(as_of → settlement)`. Anchoring the model
/// PV at `as_of` while dating the IRR price leg at settlement would otherwise
/// leave a spurious ~settlement-lag of carry in every quoted yield (the bond
/// yield path forward-values for exactly this reason).
pub(super) fn target_price_from_quote_or_model(
    loan: &TermLoan,
    schedule: &CashFlowSchedule,
    as_of: Date,
    base_value: Money,
    settle_df: f64,
) -> finstack_quant_core::Result<Money> {
    if let Some(px) = loan
        .instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price
    {
        quoted_dirty_from_clean_px(loan, schedule, as_of, px)
    } else {
        Ok(Money::new(
            base_value.amount() / settle_df,
            base_value.currency(),
        ))
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
/// - **Amortization** and positive **Notional** (redemptions): included only
///   BEFORE `exercise_date`.  At the exercise date, amortization is implicitly
///   captured in the pre-exercise outstanding used for the redemption parameter.
/// - **PIK** and negative **Notional** (funding legs): always excluded.
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
            | CFKind::FacilityFee => {
                flows.push((cf.date, cf.amount.amount()));
            }
            // Amortization: include only BEFORE exercise date.
            // At exercise date, amort is implicitly captured in the pre-amort
            // outstanding used for the redemption calculation.
            CFKind::Amortization if cf.date < exercise_date => {
                flows.push((cf.date, cf.amount.amount()));
            }
            // Positive Notional (redemptions): include only BEFORE exercise date.
            // At exercise date, the explicit redemption parameter replaces any
            // scheduled Notional to avoid double-counting.
            CFKind::Notional if cf.date < exercise_date && cf.amount.amount() > 0.0 => {
                flows.push((cf.date, cf.amount.amount()));
            }
            // Exclude: PIK, negative Notional (funding), exercise-date
            // Amortization/Notional
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
    let mut last = Money::new(0.0, currency);
    for (d, amt) in out_path {
        if *d < target {
            last = *amt;
        } else {
            break;
        }
    }
    last
}
