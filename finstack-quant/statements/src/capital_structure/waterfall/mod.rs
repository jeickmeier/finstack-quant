//! Cash Flow Waterfall & Sweep Mechanics
//!
//! This module implements dynamic cash flow allocation according to priority of payments,
//! excess cash flow sweeps, and PIK toggles based on model results.
//!
//! # Excess cash flow
//!
//! The sweep is authored term by term (`EcfSweepSpec`): a cash measure, the
//! deductions subtracted from it and the credits added back. Nothing is
//! deducted unless it is listed.
//!
//! ```text
//! ECF     = cash_measure - sum(deductions) + sum(credits)
//! Request = sweep_percentage x ECF     when ECF is positive, otherwise zero
//! ```
//!
//! Every term is a money amount in the pool currency. An `Input` term is a
//! signed formula over the model's nodes: a deduction of `2_000_000` lowers
//! ECF by that amount, and a negative deduction (a working-capital release
//! of 500,000 authored as `-500_000`) raises it. The `PaidCashInterest`, `PaidFees` and
//! `PaidScheduledPrincipal` terms are the cash this period's waterfall
//! actually paid at priorities ranking ahead of the sweep, including arrears;
//! PIK coupons and unpaid claims are not cash and are never deducted.
//!
//! The request is paid from remaining available cash across the elected
//! facilities (`FacilitySelection`), each up to its prepayment capacity. The
//! requested, paid and unpaid amounts of every prepayment rung are recorded as
//! a `PrepaymentOutcome`.
//!
//! ## Example
//!
//! ```text
//! EBITDA (cash measure):  10,000,000
//! Taxes (deduction):       2,000,000
//! CapEx (deduction):       1,500,000
//! Working capital build:     500,000
//! ECF:                     6,000,000
//! Request at 50%:          3,000,000
//! ```

mod cash_distribution;
mod excess_cash_flow;
mod payment_in_kind;
mod payment_stack;

use crate::capital_structure::cashflows::{CashflowBreakdown, add_optional_money};
use crate::capital_structure::state::CapitalStructureState;
use crate::capital_structure::waterfall_spec::{PaymentPriority, WaterfallSpec};
use crate::error::Result;
use crate::evaluator::{CapitalStructureWarning, EvalWarning, EvaluationContext};
use finstack_quant_core::money::Money;
use indexmap::IndexMap;

use cash_distribution::{
    PrincipalComponent, StagedInstrumentFlow, allocate_by_class, apply_cash_cap_to_category,
    apply_prepayment,
};
use excess_cash_flow::{PaidAhead, calculate_ecf_sweep};
use payment_in_kind::evaluate_pik_toggle;
use payment_stack::waterfall_currency;

/// Result of executing the waterfall for a single period.
#[derive(Debug, Clone)]
pub(crate) struct WaterfallPeriodResult {
    /// Per-instrument cashflow breakdowns after sweep, PIK, and
    /// priority-of-payments allocation have been applied.
    pub(crate) flows: IndexMap<String, CashflowBreakdown>,
    /// Structured warnings raised during allocation (e.g. cash shortfalls).
    pub(crate) warnings: Vec<EvalWarning>,
    /// Residual cash distributed to equity: the cash left after the payment
    /// stack when `Equity` is its terminal priority, zero otherwise.
    /// `None` only when no waterfall ran because there was neither a debt
    /// instrument nor a pool currency to denominate it in.
    pub(crate) equity_distribution: Option<Money>,
    /// Residual cash kept by the company: the cash left after the payment
    /// stack when `Equity` is not a priority, zero otherwise. With the
    /// allocations in `flows`,
    /// `fees + cash interest + principal + equity + retained == available cash`
    /// exactly. `None` as for `equity_distribution`.
    pub(crate) retained_cash: Option<Money>,
    /// What each configured prepayment requested, paid and left unpaid, in
    /// priority order.
    pub(crate) prepayments: Vec<crate::capital_structure::PrepaymentOutcome>,
    /// Cash the waterfall allocated from: the `available_cash_node` value
    /// floored at zero, in the waterfall's currency. With the allocations in
    /// `flows` it satisfies
    /// `fees + cash interest + principal + equity + retained == available cash`.
    /// `None` as for `equity_distribution`.
    pub(crate) available_cash: Option<Money>,
    /// Net principal change the waterfall posted, by instrument, in exact
    /// money: what the owners' schedules are rebuilt from.
    pub(crate) postings: IndexMap<String, Money>,
    /// The PIK toggle's request this period: `Some(true)` asks for PIK,
    /// `Some(false)` for the base coupon terms, `None` without a toggle.
    pub(crate) pik_request: Option<bool>,
}

/// The typed value of a waterfall input: a node of the model, or an inline
/// formula evaluated in the period's context.
///
/// Warnings raised while evaluating an inline expression are appended to
/// `warnings`. The expression runs against a scratch copy of the context, so
/// without this they would be dropped with the copy: an expression that guards
/// its own bad arithmetic (`coalesce(a / 0, 0)`) would otherwise hide it.
fn eval_input(
    context: &EvaluationContext,
    expr: &str,
    warnings: &mut Vec<EvalWarning>,
) -> Result<crate::evaluator::value::Value> {
    if let Ok(value) = context.get_operand(expr) {
        return Ok(value);
    }
    let compiled = crate::dsl::parse_and_compile(expr)?;
    let mut scratch = context.clone();
    // The scratch inherits the context's existing warnings; only those added
    // by this evaluation are new. Warnings are attributed to the expression
    // text: an inline waterfall expression has no node id of its own.
    let inherited = scratch.warnings.len();
    let result = crate::evaluator::formula::evaluate_value(&compiled, &mut scratch, Some(expr));
    warnings.extend(scratch.warnings.drain(inherited..));
    result
}

/// A waterfall input without a usable value: every input feeds a cash
/// allocation, where a silent zero would short every creditor.
fn input_error(context: &EvaluationContext, expr: &str, why: &str) -> crate::error::Error {
    crate::error::Error::capital_structure(format!(
        "waterfall input '{expr}' {why} in period {}",
        context.period_id
    ))
}

/// A waterfall input as an exact amount in the waterfall `currency`.
///
/// Money keeps its exact amount and must be in `currency`; a unitless number
/// is an amount of `currency`. A value in another currency, a Boolean, or an
/// input with no value is an error naming the expression.
fn eval_money(
    context: &EvaluationContext,
    expr: &str,
    currency: finstack_quant_core::currency::Currency,
    warnings: &mut Vec<EvalWarning>,
) -> Result<Money> {
    use crate::evaluator::value::Value;
    match eval_input(context, expr, warnings)? {
        Value::Money(money) if money.currency() == currency => Ok(money),
        Value::Money(money) => Err(input_error(
            context,
            expr,
            &format!(
                "is in {}, not the waterfall currency {currency}",
                money.currency()
            ),
        )),
        // `Money::new` rejects a non-finite or unrepresentable amount.
        Value::Scalar(amount) | Value::Number(amount) => {
            Money::new(amount, currency).map_err(|error| {
                input_error(
                    context,
                    expr,
                    &format!("produced {amount}, not a representable {currency} amount ({error})"),
                )
            })
        }
        Value::Bool(_) => Err(input_error(context, expr, "is a condition, not an amount")),
        Value::Unavailable(reason) => Err(input_error(
            context,
            expr,
            &format!("has no value ({})", reason.as_str()),
        )),
    }
}

/// A waterfall input as a number in its own units, for a threshold test.
fn eval_number(
    context: &EvaluationContext,
    expr: &str,
    warnings: &mut Vec<EvalWarning>,
) -> Result<f64> {
    use crate::evaluator::value::Value;
    match eval_input(context, expr, warnings)? {
        Value::Money(money) => Ok(money.amount()),
        Value::Scalar(value) | Value::Number(value) if value.is_finite() => Ok(value),
        Value::Scalar(value) | Value::Number(value) => Err(input_error(
            context,
            expr,
            &format!("evaluated to the non-finite value {value}"),
        )),
        Value::Bool(_) => Err(input_error(context, expr, "is a condition, not a number")),
        Value::Unavailable(reason) => Err(input_error(
            context,
            expr,
            &format!("has no value ({})", reason.as_str()),
        )),
    }
}

/// Payment-class rank for `instrument_id`. Empty `payment_classes` is rank `0`.
fn class_rank_for(spec: &WaterfallSpec, instrument_id: &str) -> Result<u32> {
    if spec.payment_classes.is_empty() {
        return Ok(0);
    }
    // `WaterfallSpec::validate` rejects an instrument listed in two classes.
    spec.payment_classes
        .iter()
        .find(|class| class.instrument_ids.iter().any(|id| id == instrument_id))
        .map(|class| class.rank)
        .ok_or_else(|| {
            crate::error::Error::build(format!(
                "WaterfallSpec: instrument '{instrument_id}' is not in any payment class."
            ))
        })
}

/// Apply a mandatory or voluntary prepayment at its priority rung and return
/// what it requested and paid.
///
/// `component` selects the principal component of the breakdown that records
/// the cash paid at this rung. A negative request is an error.
fn apply_named_prepay(
    context: &EvaluationContext,
    spec: &crate::capital_structure::PrepaymentSpec,
    staged: &mut [StagedInstrumentFlow],
    remaining_cash: &mut Money,
    warnings: &mut Vec<EvalWarning>,
    component: PrincipalComponent,
) -> Result<(Money, Money)> {
    let requested = eval_money(context, &spec.amount, remaining_cash.currency(), warnings)?;
    let paid = apply_prepayment(
        staged,
        remaining_cash,
        requested,
        &spec.facilities,
        component,
    )?;
    Ok((requested, paid))
}

/// Execute waterfall logic for a single period.
///
/// This function:
/// 1. Records whether the PIK liquidity metric requests PIK (the election
///    itself is decided when the period's postings are settled)
/// 2. Allocates available cash according to the priority stack
/// 3. Sizes ECF at its sweep rung using debt service already paid
///
/// # Arguments
///
/// * `context` - Evaluation context providing statement values used by sweep,
///   PIK, and payment-priority rules.
/// * `waterfall_spec` - Validated priority-of-payments and allocation policy.
/// * `state` - Mutable capital-structure state updated with period balances
///   and carried claims.
/// * `contractual_flows` - Per-instrument contractual cashflow breakdowns to
///   allocate under the waterfall.
///
/// # Returns
///
/// Returns a [`WaterfallPeriodResult`] with per-instrument cashflow breakdowns
/// after sweep and priority-of-payments allocation have been applied, the
/// shortfall warnings, the equity residual, the principal it posted and the
/// PIK toggle's request. `state` is updated in place with closing balances
/// and carried claims.
///
/// # Limitations
///
/// Allocation within a payment category is pro-rata inside each payment
/// class, walking class rank. Empty `payment_classes` is one implicit class.
/// Prepayment penalties, call premiums, and OID are not modeled (prepayments
/// apply at par). See [`WaterfallSpec`] for details.
///
/// # Errors
///
/// Returns an error if required statement nodes are missing, if the waterfall
/// references inconsistent currencies, if sweep / PIK calculations fail, or if
/// computed allocations or balances cannot be represented as monetary values.
/// On an error `state` may be partly updated: the caller runs the period on
/// a staged copy and discards it.
///
/// # References
///
/// - Fixed-income capital structure context: `docs/REFERENCES.md#tuckman-serrat-fixed-income`
pub(crate) fn execute_waterfall(
    context: &EvaluationContext,
    waterfall_spec: &WaterfallSpec,
    state: &mut CapitalStructureState,
    contractual_flows: &IndexMap<String, CashflowBreakdown>,
    pool_currency: Option<finstack_quant_core::currency::Currency>,
) -> Result<WaterfallPeriodResult> {
    // The specification was validated with the model; it is not re-validated
    // (and its sizing formulas re-parsed) every period.
    // Equity receives the residual only when it is the terminal priority;
    // otherwise the company retains it.
    let terminal_equity =
        waterfall_spec.priority_of_payments.last() == Some(&PaymentPriority::Equity);
    let residual = |remaining: Money| {
        let zero = Money::from((0_i64, remaining.currency()));
        if terminal_equity {
            (remaining, zero)
        } else {
            (zero, remaining)
        }
    };

    // The waterfall is evaluated without debt claims too: the pool has no
    // claim to pay and is all residual. Only a structure with neither an
    // instrument nor a pool currency has nothing to denominate it in.
    if contractual_flows.is_empty() {
        let Some(currency) = pool_currency else {
            return Ok(WaterfallPeriodResult {
                flows: IndexMap::new(),
                warnings: Vec::new(),
                equity_distribution: None,
                retained_cash: None,
                prepayments: Vec::new(),
                available_cash: None,
                postings: IndexMap::new(),
                pik_request: None,
            });
        };
        let mut warnings = Vec::new();
        let available_cash = available_cash(context, waterfall_spec, currency, &mut warnings)?;
        let (equity, retained) = residual(available_cash);
        return Ok(WaterfallPeriodResult {
            flows: IndexMap::new(),
            warnings,
            equity_distribution: Some(equity),
            retained_cash: Some(retained),
            prepayments: Vec::new(),
            available_cash: Some(available_cash),
            postings: IndexMap::new(),
            pik_request: None,
        });
    }

    // Validate that configured prepayment / PIK targets actually name a debt
    // instrument. A typo (e.g. a trailing space) would otherwise silently
    // disable the entire mechanism — no sweep, no PIK toggle — with no
    // diagnostic, materially shifting IRRs.
    for (_, name, facilities) in waterfall_spec.prepayments() {
        if let crate::capital_structure::FacilitySelection::Named(ids) = facilities {
            for id in ids {
                if !contractual_flows.contains_key(id) {
                    return Err(crate::error::Error::build(format!(
                        "WaterfallSpec: `{name}.facilities` entry '{id}' does not match any \
                         debt instrument; the prepayment would be silently dropped. Check for \
                         typos or trailing whitespace."
                    )));
                }
            }
        }
    }
    if let Some(pik) = &waterfall_spec.pik_toggle {
        for election in &pik.elections {
            if !contractual_flows.contains_key(&election.instrument_id) {
                return Err(crate::error::Error::build(format!(
                    "WaterfallSpec: `pik_toggle.elections` entry '{}' does not match any debt \
                     instrument; that loan would never elect PIK. Check for typos or trailing \
                     whitespace.",
                    election.instrument_id
                )));
            }
        }
    }

    let _span = tracing::info_span!(
        "statements.capital_structure.waterfall",
        period = context.period_id.to_string(),
        instruments = contractual_flows.len(),
        has_sweep = waterfall_spec.ecf_sweep.is_some(),
        has_pik_toggle = waterfall_spec.pik_toggle.is_some()
    )
    .entered();

    let mut result = IndexMap::new();
    let mut warnings: Vec<EvalWarning> = Vec::new();
    let cash_currency = waterfall_currency(contractual_flows)?;

    // --- Step 1: PIK toggle ---
    //
    // The toggle only records its request. It never moves this period's
    // interest: an election takes effect at the start of a loan's next coupon
    // period, through the loan's own coupon terms, when the schedules are
    // rebuilt.
    let pik_request = match &waterfall_spec.pik_toggle {
        Some(pik_spec) => Some(evaluate_pik_toggle(context, pik_spec, &mut warnings)?),
        None => None,
    };

    // --- Step 2: Available cash ---
    let available_cash = available_cash(context, waterfall_spec, cash_currency, &mut warnings)?;

    // --- Step 3: Build staged per-instrument state ---
    //
    // Claims are collected before allocation. Principal capacity is consumed
    // only when its payment priority executes, never reserved for a later rung.
    let mut staged: Vec<StagedInstrumentFlow> = Vec::with_capacity(contractual_flows.len());
    for (instrument_id, breakdown) in contractual_flows {
        let currency = breakdown.interest_expense_cash.currency();
        let principal = state.stage_principal(instrument_id, breakdown)?;

        let mut staged_breakdown = breakdown.clone();
        // Carry forward any unpaid interest/fee shortfall from the prior
        // period as a claim in this period's interest category. It is removed
        // from state here and re-recorded below if it again goes unpaid.
        if let Some(shortfall) = state
            .interest_shortfall
            .shift_remove(instrument_id.as_str())
        {
            // A currency mismatch here means a creditor's carried claim would be
            // silently discarded. Instrument currency must not drift, so treat a
            // mismatch as a hard error rather than dropping the claim.
            if shortfall.amount() > 0.0 {
                if shortfall.currency() != currency {
                    return Err(crate::error::Error::CurrencyMismatch(
                        currency,
                        shortfall.currency(),
                    ));
                }
                staged_breakdown.interest_expense_cash = staged_breakdown
                    .interest_expense_cash
                    .checked_add(shortfall)?;
            }
        }
        if let Some(shortfall) = state.fee_shortfall.shift_remove(instrument_id.as_str())
            && shortfall.amount() > 0.0
        {
            if shortfall.currency() != currency {
                return Err(crate::error::Error::CurrencyMismatch(
                    currency,
                    shortfall.currency(),
                ));
            }
            staged_breakdown.fees = staged_breakdown.fees.checked_add(shortfall)?;
        }
        let scheduled_principal = principal.get_scheduled_capacity()?;
        let zero = Money::from((0_i64, currency));
        staged_breakdown.principal_payment = zero;
        // Principal is re-allocated rung by rung below; each rung records its
        // share so the components always sum to `principal_payment`.
        staged_breakdown.scheduled_principal = Some(zero);
        staged_breakdown.mandatory_prepayment = Some(zero);
        staged_breakdown.sweep_prepayment = Some(zero);
        staged_breakdown.voluntary_prepayment = Some(zero);
        // The opening balance the principal allocation above was staged from.
        let opening = state.get_opening_balance(instrument_id, currency);
        staged_breakdown.opening_balance = Some(if opening.currency() == currency {
            opening
        } else {
            zero
        });
        let class_rank = class_rank_for(waterfall_spec, instrument_id)?;
        staged.push(StagedInstrumentFlow {
            instrument_id: instrument_id.clone(),
            breakdown: staged_breakdown,
            principal,
            class_rank,
            scheduled_principal,
        });
    }

    // --- Step 5: Available cash caps ---
    //
    // Each principal rung sees the balance and cash left by earlier rungs.
    // Separate maps keep caller-supplied instrument ids from colliding with
    // synthetic category prefixes such as `fees::` or `principal::`.
    let mut interest_shortfalls: IndexMap<String, Money> = IndexMap::new();
    let mut fee_shortfalls: IndexMap<String, Money> = IndexMap::new();
    let equity_distribution: Money;
    let retained_cash: Money;
    let mut prepayments = Vec::new();
    let mut postings = IndexMap::new();
    {
        let mut remaining_cash = available_cash;
        // Snapshot planned interest and fees so every unpaid amount, however
        // small, is carried forward instead of evaporating when cash runs out.
        let planned_interest: Vec<Money> = staged
            .iter()
            .map(|s| s.breakdown.interest_expense_cash)
            .collect();
        let planned_fees: Vec<Money> = staged.iter().map(|s| s.breakdown.fees).collect();
        let mut paid = PaidAhead::zero(cash_currency);
        let mut record = |kind: PaymentPriority, requested: Money, paid: Money| -> Result<()> {
            prepayments.push(crate::capital_structure::PrepaymentOutcome {
                kind,
                requested,
                paid,
                unpaid: requested.checked_sub(paid)?,
            });
            Ok(())
        };
        for priority in &waterfall_spec.priority_of_payments {
            match priority {
                PaymentPriority::Fees => {
                    apply_cash_cap_to_category(&mut staged, &mut remaining_cash, "fee", |s| {
                        &mut s.breakdown.fees
                    })?;
                    for s in &staged {
                        paid.fees = paid.fees.checked_add(s.breakdown.fees)?;
                    }
                }
                PaymentPriority::Interest => {
                    apply_cash_cap_to_category(
                        &mut staged,
                        &mut remaining_cash,
                        "cash interest",
                        |s| &mut s.breakdown.interest_expense_cash,
                    )?;
                    for s in &staged {
                        paid.cash_interest = paid
                            .cash_interest
                            .checked_add(s.breakdown.interest_expense_cash)?;
                    }
                }
                PaymentPriority::Amortization => {
                    // Earlier prepayments extinguish the same principal as
                    // this scheduled claim. Only a remaining, unpaid claim
                    // may become arrears.
                    for s in &mut staged {
                        s.scheduled_principal = s.principal.get_scheduled_capacity()?;
                    }
                    let allocations = allocate_by_class(&mut staged, &mut remaining_cash, |s| {
                        Ok(s.scheduled_principal)
                    })?;
                    for (s, allocated) in staged.iter_mut().zip(allocations) {
                        s.scheduled_principal = allocated;
                        s.principal.pay_scheduled(s.scheduled_principal)?;
                        s.breakdown.principal_payment = s
                            .breakdown
                            .principal_payment
                            .checked_add(s.scheduled_principal)?;
                        add_optional_money(
                            &mut s.breakdown.scheduled_principal,
                            Some(s.scheduled_principal),
                        )?;
                    }
                    for s in &staged {
                        paid.scheduled_principal = paid
                            .scheduled_principal
                            .checked_add(s.scheduled_principal)?;
                    }
                }
                PaymentPriority::MandatoryPrepayment => {
                    if let Some(spec) = &waterfall_spec.mandatory_prepayment {
                        let (requested, paid) = apply_named_prepay(
                            context,
                            spec,
                            &mut staged,
                            &mut remaining_cash,
                            &mut warnings,
                            |b| &mut b.mandatory_prepayment,
                        )?;
                        record(*priority, requested, paid)?;
                    }
                }
                PaymentPriority::Sweep => {
                    if let Some(spec) = &waterfall_spec.ecf_sweep {
                        let requested = calculate_ecf_sweep(
                            context,
                            spec,
                            &paid,
                            cash_currency,
                            &mut warnings,
                        )?;
                        let swept = apply_prepayment(
                            &mut staged,
                            &mut remaining_cash,
                            requested,
                            &spec.facilities,
                            |b| &mut b.sweep_prepayment,
                        )?;
                        record(*priority, requested, swept)?;
                    }
                }
                PaymentPriority::VoluntaryPrepayment => {
                    if let Some(spec) = &waterfall_spec.voluntary_prepayment {
                        let (requested, paid) = apply_named_prepay(
                            context,
                            spec,
                            &mut staged,
                            &mut remaining_cash,
                            &mut warnings,
                            |b| &mut b.voluntary_prepayment,
                        )?;
                        record(*priority, requested, paid)?;
                    }
                }
                PaymentPriority::Equity => {}
            }
        }

        // Per-instrument shortfall: planned − allocated, exactly. Unpaid debt
        // service does not evaporate, whatever its size — it is carried as a
        // claim in the next period. Interest and fees are tracked in
        // *separate* buckets so a fee arrears re-enters the fee category (not
        // demoted into interest, which ranks one rung lower).
        for (idx, s) in staged.iter().enumerate() {
            let unpaid_interest =
                planned_interest[idx].checked_sub(s.breakdown.interest_expense_cash)?;
            if unpaid_interest.amount_decimal() > rust_decimal::Decimal::ZERO {
                interest_shortfalls.insert(s.instrument_id.clone(), unpaid_interest);
                warnings.push(EvalWarning::CapitalStructure {
                    period: context.period_id,
                    warning: CapitalStructureWarning::InterestShortfall {
                        instrument_id: s.instrument_id.clone(),
                        amount: unpaid_interest.amount(),
                    },
                });
            }

            let unpaid_fees = planned_fees[idx].checked_sub(s.breakdown.fees)?;
            if unpaid_fees.amount_decimal() > rust_decimal::Decimal::ZERO {
                fee_shortfalls.insert(s.instrument_id.clone(), unpaid_fees);
                warnings.push(EvalWarning::CapitalStructure {
                    period: context.period_id,
                    warning: CapitalStructureWarning::FeeShortfall {
                        instrument_id: s.instrument_id.clone(),
                        amount: unpaid_fees.amount(),
                    },
                });
            }

            let unpaid_principal = s.principal.get_scheduled_capacity()?;
            if unpaid_principal.amount_decimal() > rust_decimal::Decimal::ZERO {
                warnings.push(EvalWarning::CapitalStructure {
                    period: context.period_id,
                    warning: CapitalStructureWarning::PrincipalShortfall {
                        instrument_id: s.instrument_id.clone(),
                        amount: unpaid_principal.amount(),
                    },
                });
            }
        }

        // Whatever cash survives the priority stack is the residual. The
        // allocatable cash reconciles exactly to what was paid plus it.
        let mut uses = remaining_cash;
        for s in &staged {
            uses = uses
                .checked_add(s.breakdown.fees)?
                .checked_add(s.breakdown.interest_expense_cash)?
                .checked_add(s.breakdown.principal_payment)?;
        }
        if uses != available_cash {
            return Err(crate::error::Error::capital_structure(format!(
                "Waterfall cash does not reconcile in {}: {} {cash_currency} allocated and \
                 retained against {} {cash_currency} available",
                context.period_id,
                uses.amount(),
                available_cash.amount()
            )));
        }
        (equity_distribution, retained_cash) = residual(remaining_cash);
    }

    // --- Step 6: Period close ---
    //
    // For each instrument:
    // (a) principal_payment is the sum actually allocated by the payment
    // priorities, each bounded by the principal remaining at that rung.
    // (b) Dated principal accounting keeps economic balance changes separate
    // from settlement cash, including unpaid and early-settled installments.
    // (c) PIK is dated principal in the loan's own schedule; an elected
    // coupon capitalizes there, never in this close.
    // (d) accrued_interest retains interest earned but not yet due. Paying
    // principal or toggling a coupon does not extinguish that separate claim.
    // Any cash shortfall in the allocation is interest already due: it is reported
    // as interest_arrears, not accrued, and carried into the next period.
    for s in staged {
        let StagedInstrumentFlow {
            instrument_id,
            mut breakdown,
            principal,
            class_rank: _,
            scheduled_principal: _,
        } = s;
        let currency = breakdown.interest_expense_cash.currency();

        // Each cash payment was capped by its dated principal account before
        // consuming cash. Settle that account without deducting cash whose
        // principal movement already occurred in a prior reporting period.
        let principal_close = principal.finish()?;
        let post_pay_amount = principal_close.economic.amount();
        let mut principal_arrears = Money::from((0_i64, currency));
        for claim in &principal_close.arrears {
            principal_arrears = principal_arrears.checked_add(claim.amount)?;
        }
        breakdown.principal_arrears = Some(principal_arrears);
        breakdown.principal_advance_reserve = Some(principal_close.reserve);
        let (principal_shortfalls, principal_advances) =
            (principal_close.arrears, principal_close.advances);
        // A materially negative balance means principal exceeded the payable
        // balance — an upstream accounting bug the dust floor must not mask.
        // The per-rung caps make this unreachable; keep it as a loud
        // invariant guard rather than letting the floor absorb it.
        let dust = crate::capital_structure::period_flows::half_minor_unit(currency);
        if post_pay_amount < -dust {
            return Err(crate::error::Error::capital_structure(format!(
                "Post-payment balance for instrument '{instrument_id}' is negative \
                 ({post_pay_amount}): principal payment exceeded the payable balance."
            )));
        }
        // Dust floor: collapse a residual below half a minor unit on full
        // paydown; contractual schedule amounts are not rounded to the unit.
        let post_sweep_balance = if post_pay_amount.abs() < dust {
            Money::from((0_i64, currency))
        } else {
            Money::new(post_pay_amount, currency)?
        };
        // Contractual and elected PIK are already in payable principal: the
        // schedule capitalizes them on their coupon dates.
        let closing_balance = post_sweep_balance;
        state.set_closing_balance(instrument_id.to_string(), closing_balance);
        breakdown.debt_balance = closing_balance;
        // The principal this waterfall changed outside the contractual
        // schedule: the prepayments it paid.
        let mut posted = Money::from((0_i64, currency));
        for prepaid in [
            breakdown.mandatory_prepayment,
            breakdown.voluntary_prepayment,
            breakdown.sweep_prepayment,
        ]
        .into_iter()
        .flatten()
        {
            posted = posted.checked_sub(prepaid)?;
        }
        if posted != Money::from((0_i64, currency)) {
            postings.insert(instrument_id.clone(), posted);
        }

        // Unpaid interest from the available-cash cap is overdue: it is
        // reported as arrears and carried as a claim in the next period's
        // interest category.
        breakdown.interest_arrears = Some(Money::from((0_i64, currency)));
        if let Some(shortfall) = interest_shortfalls.get(instrument_id.as_str()) {
            breakdown.interest_arrears = Some(*shortfall);
            state
                .interest_shortfall
                .insert(instrument_id.clone(), *shortfall);
        }
        if !principal_shortfalls.is_empty() {
            state
                .principal_shortfall
                .insert(instrument_id.clone(), principal_shortfalls);
        }
        if !principal_advances.is_empty() {
            state
                .principal_advance_payments
                .insert(instrument_id.clone(), principal_advances);
        }
        // Unpaid fees are carried in their own bucket so they re-enter the fee
        // category next period rather than being demoted into interest.
        if let Some(shortfall) = fee_shortfalls.get(instrument_id.as_str()) {
            state
                .fee_shortfall
                .insert(instrument_id.clone(), *shortfall);
        }

        breakdown.validate_currency_invariant().map_err(|e| {
            crate::error::Error::capital_structure(format!(
                "Currency invariant violated after waterfall mutation for {instrument_id}: {e}"
            ))
        })?;

        result.insert(instrument_id.to_string(), breakdown);
    }

    Ok(WaterfallPeriodResult {
        flows: result,
        warnings,
        equity_distribution: Some(equity_distribution),
        retained_cash: Some(retained_cash),
        prepayments,
        available_cash: Some(available_cash),
        postings,
        pik_request,
    })
}

/// The period's allocatable cash: the pre-debt-service pool, with a negative
/// pool reported as a funding shortfall and allocated as zero.
fn available_cash(
    context: &EvaluationContext,
    waterfall_spec: &WaterfallSpec,
    currency: finstack_quant_core::currency::Currency,
    warnings: &mut Vec<EvalWarning>,
) -> Result<Money> {
    let node = &waterfall_spec.available_cash_node;
    let cash = eval_money(context, node, currency, warnings)?;
    // The shortfall itself must stay visible: without this warning the period
    // simply shorts every creditor and the negative signal disappears.
    if cash.amount_decimal() < rust_decimal::Decimal::ZERO {
        warnings.push(EvalWarning::CapitalStructure {
            period: context.period_id,
            warning: CapitalStructureWarning::NegativeAvailableCashFloored {
                node_id: node.clone(),
                amount: cash.amount(),
            },
        });
        return Ok(Money::from((0_i64, currency)));
    }
    Ok(cash)
}

#[cfg(test)]
mod test_support;
#[cfg(test)]
pub(crate) use test_support::{execute_waterfall_same_period, execute_waterfall_staged};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capital_structure::{
        CapitalStructureState, CashflowBreakdown, EcfSweepSpec, PaymentClassSpec, PaymentPriority,
        PikToggleSpec, WaterfallSpec,
    };
    use finstack_quant_core::currency::Currency;
    #[cfg(test)]
    use finstack_quant_core::dates::PeriodId;
    use finstack_quant_core::money::Money;
    use indexmap::IndexMap;

    /// Cash pool used by fixtures that predate the required
    /// `available_cash_node`. Large enough that every scheduled flow is funded,
    /// so those tests keep asserting the same numbers — the difference is that
    /// the funding is now stated rather than assumed.
    const AMPLE_CASH: f64 = 1e12;

    fn build_context(period: PeriodId, values: &[(&str, f64)]) -> EvaluationContext {
        let mut values = values.to_vec();
        if !values.iter().any(|(name, _)| *name == "cash") {
            values.push(("cash", AMPLE_CASH));
        }
        let values = values.as_slice();
        let mut node_to_column = IndexMap::new();
        for (idx, (name, _)) in values.iter().enumerate() {
            node_to_column.insert(crate::types::NodeId::new(*name), idx);
        }
        let mut ctx = EvaluationContext::new(
            period,
            &finstack_quant_core::dates::PeriodPlan::try_new(
                finstack_quant_core::dates::PeriodCalendar::Gregorian,
                Vec::new(),
            )
            .expect("empty plan"),
            std::sync::Arc::new(node_to_column),
            std::sync::Arc::new(IndexMap::new()),
        );
        for (name, value) in values {
            ctx.set_value(name, *value)
                .expect("sample context should accept provided node values");
        }
        ctx
    }

    #[test]
    fn test_execute_waterfall_applies_ecf_sweep_and_updates_state() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let context = build_context(
            period,
            &[
                ("ebitda", 1_000_000.0),
                ("taxes", 200_000.0),
                ("capex", 50_000.0),
            ],
        );

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut tl_breakdown = CashflowBreakdown::with_currency(Currency::USD);
        tl_breakdown.principal_payment = Money::from((100_000_i64, Currency::USD));
        contractual_flows.insert("TL-1".to_string(), tl_breakdown);

        let mut state = CapitalStructureState::new();
        state.opening_balances.insert(
            "TL-1".to_string(),
            Money::from((10_000_000_i64, Currency::USD)),
        );

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash".into(),
            ecf_sweep: Some(EcfSweepSpec {
                cash_measure: "ebitda".into(),
                deductions: vec![
                    crate::capital_structure::EcfTerm::Input {
                        expression: "taxes".into(),
                    },
                    crate::capital_structure::EcfTerm::Input {
                        expression: "capex".into(),
                    },
                    crate::capital_structure::EcfTerm::PaidCashInterest,
                    crate::capital_structure::EcfTerm::PaidFees,
                    crate::capital_structure::EcfTerm::PaidScheduledPrincipal,
                ],
                credits: Vec::new(),
                sweep_percentage: 0.5,
                facilities: crate::capital_structure::FacilitySelection::Named(vec!["TL-1".into()]),
            }),
            pik_toggle: None,
            ..Default::default()
        };

        let results =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect("waterfall should execute");

        // Corrected ECF convention (per standard LPA / S&P LCD definitions):
        // scheduled amortization ranked ahead of the sweep is deducted from
        // ECF before applying the sweep percentage.
        // ECF = 1,000,000 - 200,000 - 50,000 - 100,000 (sched amort) = 650,000
        // Sweep = 650,000 × 0.5 = 325,000; total principal = 100,000 + 325,000.
        let tl_result = results.flows.get("TL-1").expect("instrument exists");
        assert_eq!(tl_result.principal_payment.amount(), 425_000.0);
        assert_eq!(
            state
                .closing_balances
                .get("TL-1")
                .expect("closing balance")
                .amount(),
            10_000_000.0 - 425_000.0
        );
    }

    /// A negative cash pool is floored to zero for allocation, but the
    /// operating shortfall must be surfaced, not silently erased: every
    /// creditor gets shorted and the only diagnostic would otherwise be
    /// downstream shortfall warnings that never mention the negative pool.
    #[test]
    fn negative_available_cash_is_warned_before_flooring() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let context = build_context(period, &[("cash", -500.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash".into(),
            ecf_sweep: None,
            pik_toggle: None,
            ..Default::default()
        };

        let result =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect("a negative pool floors to zero and still executes");

        assert!(
            result.warnings.iter().any(|w| matches!(
                w,
                EvalWarning::CapitalStructure {
                    warning: CapitalStructureWarning::NegativeAvailableCashFloored { .. },
                    ..
                }
            )),
            "the negative cash pool must be surfaced: {:?}",
            result.warnings
        );
    }

    /// Cash conservation under an available-cash cap: every cash outflow
    /// (fees + cash interest + cash principal) plus the equity residual must
    /// sum exactly to the available pool when claims exceed it.
    #[test]
    fn capped_waterfall_conserves_available_cash_identity() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let context = build_context(period, &[("cash", 260.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
        breakdown.principal_payment = Money::from((200_i64, Currency::USD));
        breakdown.fees = Money::from((50_i64, Currency::USD));
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash".into(),
            ecf_sweep: None,
            pik_toggle: None,
            ..Default::default()
        };

        let result =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect("waterfall should execute");

        let flows = result.flows.get("TL-1").expect("instrument exists");
        let cash_out = flows.fees.amount()
            + flows.interest_expense_cash.amount()
            + flows.principal_payment.amount();
        let equity = result
            .equity_distribution
            .map_or(0.0, |money| money.amount());

        // Claims total 350 against a 260 pool: waterfall order pays fees 50,
        // interest 100, principal 110; nothing reaches equity.
        assert!(
            (cash_out + equity - 260.0).abs() < 1e-9,
            "cash out ({cash_out}) + equity ({equity}) must equal the 260 pool"
        );
        assert_eq!(flows.fees.amount(), 50.0);
        assert_eq!(flows.interest_expense_cash.amount(), 100.0);
        assert_eq!(flows.principal_payment.amount(), 110.0);
    }

    /// A non-finite `available_cash_node` must be a hard error, not a silent
    /// zero. `NaN.max(0.0)` evaluates to 0.0, which would short every creditor
    /// against an empty cash pool while reporting only downstream shortfall
    /// warnings — no diagnostic pointing at the broken formula.
    #[test]
    fn available_cash_node_rejects_an_input_without_a_value() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        // A lag with no prior period has no value.
        let context = build_context(period, &[("cash", 0.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            available_cash_node: "lag(cash, 1)".into(),
            ecf_sweep: None,
            pik_toggle: None,
            ..Default::default()
        };

        let err =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect_err("available cash without a value must error rather than read as zero");
        let msg = err.to_string();
        assert!(
            msg.contains("has no value") && msg.contains("lag(cash, 1)"),
            "error must name the offending expression: {msg}"
        );
    }

    /// Typed inputs keep their units: money in another currency is refused,
    /// not allocated as if it were the waterfall currency.
    #[test]
    fn a_cash_pool_in_another_currency_is_refused() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let mut context = build_context(period, &[]);
        context
            .set_cell(
                "cash",
                crate::evaluator::cell::Cell::Money(Money::from((500_i64, Currency::EUR))),
            )
            .expect("cell");
        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        contractual_flows.insert(
            "TL-1".to_string(),
            CashflowBreakdown::with_currency(Currency::USD),
        );
        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));
        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash".into(),
            ..Default::default()
        };
        let msg =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect_err("a EUR pool cannot pay a USD waterfall")
                .to_string();
        assert!(msg.contains("EUR") && msg.contains("USD"), "{msg}");
    }

    /// `available_cash_node` is the pre-waterfall cash pool. Deducting
    /// `cs.*` debt service in that formula double-pays interest / principal.
    #[test]
    fn available_cash_node_rejects_cs_debt_service_formula() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let context = build_context(
            period,
            &[("ebitda", 1_000_000.0), ("cash_available", 500_000.0)],
        );

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

        let stack = vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Equity,
        ];
        let forbidden = WaterfallSpec {
            priority_of_payments: stack.clone(),
            available_cash_node: "ebitda - cs.interest_expense_cash.total".into(),
            ecf_sweep: None,
            pik_toggle: None,
            ..Default::default()
        };
        let err =
            execute_waterfall_same_period(&context, &forbidden, &mut state, &contractual_flows)
                .expect_err("deducting cs debt service from available cash must error");
        let msg = err.to_string();
        assert!(
            msg.contains("before debt service") && msg.contains("cs.interest_expense"),
            "error must name the pre-debt-service contract: {msg}"
        );

        let allowed = WaterfallSpec {
            priority_of_payments: stack,
            available_cash_node: "cash_available".into(),
            ecf_sweep: None,
            pik_toggle: None,
            ..Default::default()
        };
        execute_waterfall_same_period(&context, &allowed, &mut state, &contractual_flows)
            .expect("a standalone cash_available node must still be accepted");
    }

    /// A finite-but-astronomical cash value must error rather than panic.
    /// `Money::new` asserts a `rust_decimal`-representable amount (~7.9e28) and
    /// panics beyond it, which would abort evaluation from ordinary model data.
    #[test]
    fn available_cash_node_rejects_amount_beyond_decimal_range() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let context = build_context(period, &[("cash", 1e30)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash".into(),
            ecf_sweep: None,
            pik_toggle: None,
            ..Default::default()
        };

        let err =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect_err("out-of-range available cash must error rather than panic");
        let msg = err.to_string();
        assert!(
            msg.contains("1e30") || msg.contains("Decimal") || msg.contains("representable"),
            "error must describe the out-of-range amount, not an unrelated spec problem: {msg}"
        );
    }

    /// Warnings raised inside an inline waterfall expression must survive.
    ///
    /// The expression runs against a scratch copy of the context, and the copy
    /// (with its warnings) was dropped. A guarded expression returns a finite
    /// value, so the bad arithmetic inside it left no trace at all.
    #[test]
    fn a_guarded_division_by_zero_still_fails() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let context = build_context(period, &[("cash", 500.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            // `coalesce` recovers an unavailable value, not invalid arithmetic.
            available_cash_node: "coalesce(cash / 0, cash)".into(),
            ecf_sweep: None,
            pik_toggle: None,
            ..Default::default()
        };

        let err =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect_err("a division by zero is an error even inside coalesce");
        let msg = err.to_string();
        assert!(
            msg.contains("division by zero") && msg.contains("coalesce(cash / 0, cash)"),
            "error must name the cause and the expression: {msg}"
        );
    }

    /// An internally inconsistent input breakdown must error, not panic.
    ///
    /// `waterfall_currency` compares only `interest_expense_cash` *across*
    /// instruments, so a breakdown whose other legs carry a different currency
    /// passed the entry check and reached `Money`'s asserting `AddAssign`,
    /// aborting evaluation. `execute_waterfall` is public and
    /// `CashflowBreakdown`'s fields are `pub`, so this is reachable input.
    #[test]
    fn inconsistent_breakdown_currency_errors_rather_than_panicking() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let context = build_context(period, &[("cash", 1_000.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
        // A caller-built breakdown with one leg in the wrong currency.
        breakdown.interest_expense_pik = Money::from((50_i64, Currency::EUR));
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

        // A firing PIK toggle reaches `interest_expense_pik += moved` in Step
        // 4b, where `Money`'s asserting `AddAssign` aborts on the mismatch.
        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash".into(),
            ecf_sweep: None,
            pik_toggle: Some(PikToggleSpec {
                liquidity_metric: "cash".into(),
                threshold: 1_000_000.0, // always below => PIK on
                elections: vec![crate::capital_structure::PikElectionSpec {
                    instrument_id: "TL-1".to_string(),
                    pik_fraction: rust_decimal::Decimal::ONE,
                    min_coupon_periods: 0,
                }],
            }),
            ..Default::default()
        };

        let err =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect_err("an inconsistent breakdown must error rather than panic");
        assert!(
            err.to_string().contains("Currency mismatch"),
            "expected a currency-mismatch diagnostic naming the field: {err}"
        );
    }

    /// A non-finite PIK liquidity metric must error rather than silently
    /// evaluate `NaN < threshold` as false (i.e. "PIK not triggered").
    #[test]
    fn pik_liquidity_metric_rejects_non_finite_value() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let context = build_context(period, &[("liquidity", 0.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Amortization,
                PaymentPriority::Interest,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash".into(),
            ecf_sweep: None,
            pik_toggle: Some(PikToggleSpec {
                liquidity_metric: "liquidity / 0".into(),
                threshold: 1.0,
                elections: vec![crate::capital_structure::PikElectionSpec {
                    instrument_id: "TL-1".to_string(),
                    pik_fraction: rust_decimal::Decimal::ONE,
                    min_coupon_periods: 0,
                }],
            }),
            ..Default::default()
        };

        let err =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect_err("an invalid PIK metric must error rather than read as 'not triggered'");
        assert!(
            err.to_string().contains("division by zero"),
            "error must flag the invalid metric: {err}"
        );
    }

    #[test]
    fn test_ecf_sweep_deducts_fees_before_sweep_percentage() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let context = build_context(period, &[("ebitda", 1_000.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.fees = Money::from((100_i64, Currency::USD));
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Amortization,
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash".into(),
            ecf_sweep: Some(EcfSweepSpec {
                cash_measure: "ebitda".into(),
                deductions: vec![
                    crate::capital_structure::EcfTerm::PaidCashInterest,
                    crate::capital_structure::EcfTerm::PaidFees,
                    crate::capital_structure::EcfTerm::PaidScheduledPrincipal,
                ],
                credits: Vec::new(),
                sweep_percentage: 0.5,
                facilities: crate::capital_structure::FacilitySelection::Named(vec!["TL-1".into()]),
            }),
            pik_toggle: None,
            ..Default::default()
        };

        let result =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect("waterfall should execute");

        let tl = result.flows.get("TL-1").expect("TL-1");
        assert_eq!(
            tl.principal_payment.amount(),
            450.0,
            "fees paid ahead of sweep reduce ECF before the sweep percentage: (1000 - 100) * 50%"
        );
    }

    /// The swept fraction is rounded to the minor unit, half away from zero:
    /// 50% of 100.03 is 50.015, requested and paid as 50.02.
    #[test]
    fn test_ecf_sweep_request_is_rounded_to_the_minor_unit() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let context = build_context(period, &[("ebitda", 100.03)]);
        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        contractual_flows.insert(
            "TL-1".to_string(),
            CashflowBreakdown::with_currency(Currency::USD),
        );
        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));
        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Amortization,
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: "ebitda".into(),
            ecf_sweep: Some(EcfSweepSpec {
                cash_measure: "ebitda".into(),
                deductions: Vec::new(),
                credits: Vec::new(),
                sweep_percentage: 0.5,
                facilities: crate::capital_structure::FacilitySelection::Named(vec!["TL-1".into()]),
            }),
            pik_toggle: None,
            ..Default::default()
        };
        let result =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect("waterfall should execute");
        let paid = result.flows.get("TL-1").expect("TL-1").principal_payment;
        assert_eq!(paid.amount_decimal().to_string(), "50.02");
    }

    #[test]
    fn test_execute_waterfall_conserves_sweep_across_multiple_instruments() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let context = build_context(
            period,
            &[
                ("ebitda", 1_000_000.0),
                ("taxes", 200_000.0),
                ("capex", 50_000.0),
            ],
        );

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        contractual_flows.insert(
            "TL-1".to_string(),
            CashflowBreakdown::with_currency(Currency::USD),
        );
        contractual_flows.insert(
            "TL-2".to_string(),
            CashflowBreakdown::with_currency(Currency::USD),
        );

        let mut state = CapitalStructureState::new();
        state.opening_balances.insert(
            "TL-1".to_string(),
            Money::from((200_000_i64, Currency::USD)),
        );
        state.opening_balances.insert(
            "TL-2".to_string(),
            Money::from((300_000_i64, Currency::USD)),
        );

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash".into(),
            ecf_sweep: Some(EcfSweepSpec {
                cash_measure: "ebitda".into(),
                deductions: vec![
                    crate::capital_structure::EcfTerm::Input {
                        expression: "taxes".into(),
                    },
                    crate::capital_structure::EcfTerm::Input {
                        expression: "capex".into(),
                    },
                    crate::capital_structure::EcfTerm::PaidCashInterest,
                    crate::capital_structure::EcfTerm::PaidFees,
                    crate::capital_structure::EcfTerm::PaidScheduledPrincipal,
                ],
                credits: Vec::new(),
                sweep_percentage: 0.5,
                facilities: crate::capital_structure::FacilitySelection::AllEligible,
            }),
            pik_toggle: None,
            ..Default::default()
        };

        let results =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect("waterfall should execute");

        let total_principal = results
            .flows
            .values()
            .map(|breakdown| breakdown.principal_payment.amount())
            .sum::<f64>();
        let tl1 = results.flows.get("TL-1").expect("TL-1 result");
        let tl2 = results.flows.get("TL-2").expect("TL-2 result");

        assert_eq!(total_principal, 375_000.0);
        assert!((tl1.principal_payment.amount() - 150_000.0).abs() < 1e-9);
        assert!((tl2.principal_payment.amount() - 225_000.0).abs() < 1e-9);
    }

    #[test]
    fn test_available_cash_caps_scheduled_payments_by_priority() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let context = build_context(period, &[("cash_available", 150.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.fees = Money::from((20_i64, Currency::USD));
        breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
        breakdown.principal_payment = Money::from((200_i64, Currency::USD));
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::from((1_000_i64, Currency::USD)));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash_available".into(),
            ecf_sweep: None,
            pik_toggle: None,
            ..Default::default()
        };

        let results =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect("waterfall should execute");

        let tl = results.flows.get("TL-1").expect("TL-1");
        assert_eq!(tl.fees.amount(), 20.0);
        assert_eq!(tl.interest_expense_cash.amount(), 100.0);
        assert_eq!(tl.principal_payment.amount(), 30.0);
    }

    /// A dated schedule cannot reduce principal below zero: scheduled
    /// principal beyond the balance is rejected, not clamped.
    #[test]
    fn scheduled_principal_beyond_the_balance_is_rejected() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let context = build_context(period, &[("cash", 1_000.0)]);
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.principal_payment = Money::from((300_i64, Currency::USD));
        let contractual_flows = IndexMap::from([("TL-1".to_string(), breakdown)]);
        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::from((200_i64, Currency::USD)));
        let waterfall = WaterfallSpec {
            available_cash_node: "cash".into(),
            ..Default::default()
        };

        let error =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect_err("300 scheduled against 200 outstanding");
        assert!(error.to_string().contains("negative"), "{error}");
        assert!(state.closing_balances.is_empty());
    }

    // Regression (C5): a revolver with an in-period draw must keep the drawn
    // balance at close. Before the fix, Step 6 recomputed closing as
    // `opening - principal` (no draw term), wiping the draw to zero.
    #[test]
    fn test_revolver_in_period_draw_preserved_at_close() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let context = build_context(period, &[]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        // In-period repayment of 30k against a freshly-drawn balance.
        breakdown.principal_payment = Money::from((30_000_i64, Currency::USD));
        contractual_flows.insert("REVOLVER".to_string(), breakdown);

        // Fully-swept revolver: the ledger opens at zero and the dated
        // schedule draws 100k and repays 30k in the period, so its economic
        // close is 70k.
        contractual_flows["REVOLVER"].debt_balance = Money::from((70_000_i64, Currency::USD));
        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("REVOLVER".to_string(), Money::from((0_i64, Currency::USD)));
        let dated = finstack_quant_core::dates::Date::MIN;
        state
            .set_period_principal_flows(
                "REVOLVER",
                &finstack_quant_core::dates::Period {
                    id: period,
                    start: dated,
                    end: finstack_quant_core::dates::Date::MAX,
                    is_actual: false,
                },
                vec![crate::capital_structure::PrincipalClaim::at_par(
                    dated,
                    dated,
                    Money::from((30_000_i64, Currency::USD)),
                )],
            )
            .expect("dated repayment");

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
            ],
            available_cash_node: "cash".into(),
            ecf_sweep: None,
            pik_toggle: None,
            ..Default::default()
        };

        let results =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect("waterfall should execute");

        let rev = results.flows.get("REVOLVER").expect("REVOLVER");
        // closing = opening(0) + draws(100k) - principal(30k) = 70k.
        assert_eq!(rev.debt_balance.amount(), 70_000.0);
        assert_eq!(
            state
                .closing_balances
                .get("REVOLVER")
                .expect("closing balance")
                .amount(),
            70_000.0
        );
    }

    #[test]
    fn test_sweep_before_amortization_does_not_produce_negative_balance() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let context = build_context(period, &[("ebitda", 5_000.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.principal_payment = Money::from((300_i64, Currency::USD));
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::from((500_i64, Currency::USD)));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Sweep,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash".into(),
            ecf_sweep: Some(EcfSweepSpec {
                cash_measure: "ebitda".into(),
                deductions: vec![
                    crate::capital_structure::EcfTerm::PaidCashInterest,
                    crate::capital_structure::EcfTerm::PaidFees,
                    crate::capital_structure::EcfTerm::PaidScheduledPrincipal,
                ],
                credits: Vec::new(),
                sweep_percentage: 1.0,
                facilities: crate::capital_structure::FacilitySelection::Named(vec!["TL-1".into()]),
            }),
            pik_toggle: None,
            ..Default::default()
        };

        let results =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect("waterfall should execute");

        let tl = results.flows.get("TL-1").expect("TL-1");
        assert!(
            tl.debt_balance.amount() >= 0.0,
            "debt balance must never go negative, got {}",
            tl.debt_balance.amount()
        );
    }

    #[test]
    fn test_ecf_defaults_cash_interest_from_contractual_flows() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let context = build_context(
            period,
            &[("ebitda", 1_000.0), ("taxes", 100.0), ("capex", 50.0)],
        );

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_expense_cash = Money::from((200_i64, Currency::USD));
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash".into(),
            ecf_sweep: Some(EcfSweepSpec {
                cash_measure: "ebitda".into(),
                deductions: vec![
                    crate::capital_structure::EcfTerm::Input {
                        expression: "taxes".into(),
                    },
                    crate::capital_structure::EcfTerm::Input {
                        expression: "capex".into(),
                    },
                    crate::capital_structure::EcfTerm::PaidCashInterest,
                    crate::capital_structure::EcfTerm::PaidFees,
                    crate::capital_structure::EcfTerm::PaidScheduledPrincipal,
                ],
                credits: Vec::new(),
                sweep_percentage: 0.5,
                facilities: crate::capital_structure::FacilitySelection::Named(vec!["TL-1".into()]),
            }),
            pik_toggle: None,
            ..Default::default()
        };

        let results =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect("waterfall should execute");

        let tl = results.flows.get("TL-1").expect("TL-1");
        assert_eq!(tl.principal_payment.amount(), 325.0);
    }

    #[test]
    fn a_negative_interest_claim_is_rejected() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let context = build_context(
            period,
            &[("ebitda", 1_000.0), ("taxes", 100.0), ("capex", 50.0)],
        );

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_expense_cash = Money::from((-200_i64, Currency::USD));
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::from((10_000_i64, Currency::USD)));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash".into(),
            ecf_sweep: Some(EcfSweepSpec {
                cash_measure: "ebitda".into(),
                deductions: vec![
                    crate::capital_structure::EcfTerm::Input {
                        expression: "taxes".into(),
                    },
                    crate::capital_structure::EcfTerm::Input {
                        expression: "capex".into(),
                    },
                    crate::capital_structure::EcfTerm::PaidCashInterest,
                    crate::capital_structure::EcfTerm::PaidFees,
                    crate::capital_structure::EcfTerm::PaidScheduledPrincipal,
                ],
                credits: Vec::new(),
                sweep_percentage: 0.5,
                facilities: crate::capital_structure::FacilitySelection::Named(vec!["TL-1".into()]),
            }),
            pik_toggle: None,
            ..Default::default()
        };

        // A negative outflow claim is a sign error upstream: it is rejected,
        // not read as zero, and the state is left untouched.
        let error =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect_err("negative cash interest claim")
                .to_string();
        assert!(error.contains("TL-1"), "{error}");
        assert!(error.contains("negative cash interest claim"), "{error}");
        assert!(state.closing_balances.is_empty());
    }

    /// Unpaid interest under an available-cash cap must not evaporate: it
    /// accrues, raises a structured warning, and is carried as a claim in the
    /// next period's interest category.
    #[test]
    fn test_cash_shortfall_accrues_and_carries_forward() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::from((1_000_i64, Currency::USD)));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash_available".into(),
            ecf_sweep: None,
            pik_toggle: None,
            ..Default::default()
        };

        // Period 1: only 60 of cash against a 100 coupon.
        let ctx_short = build_context(period, &[("cash_available", 60.0)]);
        let result_short =
            execute_waterfall_same_period(&ctx_short, &waterfall, &mut state, &contractual_flows)
                .expect("waterfall should execute");

        let tl = result_short.flows.get("TL-1").expect("TL-1");
        assert_eq!(tl.interest_expense_cash.amount(), 60.0);
        // The unpaid coupon was due: it is arrears, not accrued interest.
        assert_eq!(tl.interest_arrears.map(|a| a.amount()), Some(40.0));
        assert_eq!(tl.accrued_interest.amount(), 0.0);
        assert_eq!(
            state
                .interest_shortfall
                .get("TL-1")
                .expect("shortfall recorded")
                .amount(),
            40.0
        );
        assert_eq!(
            result_short.warnings.len(),
            1,
            "shortfall must surface a structured warning, got {:?}",
            result_short.warnings
        );
        assert_eq!(
            result_short
                .equity_distribution
                .expect("equity populated")
                .amount(),
            0.0
        );

        // Period 2: 200 of cash funds the 100 coupon plus the carried 40 claim.
        state.advance_period();
        let ctx_recovered = build_context(period, &[("cash_available", 200.0)]);
        let result_recovered = execute_waterfall_same_period(
            &ctx_recovered,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect("waterfall should execute");

        let tl = result_recovered.flows.get("TL-1").expect("TL-1");
        assert_eq!(
            tl.interest_expense_cash.amount(),
            140.0,
            "carried claim is paid in the next period's interest category"
        );
        assert_eq!(tl.interest_arrears.map(|a| a.amount()), Some(0.0));
        assert!(
            state.interest_shortfall.get("TL-1").is_none(),
            "shortfall cleared once paid"
        );
        assert!(result_recovered.warnings.is_empty());
        assert_eq!(
            result_recovered
                .equity_distribution
                .expect("equity populated")
                .amount(),
            60.0
        );
    }

    #[test]
    fn test_principal_shortfall_carries_forward_as_amortization_claim() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.principal_payment = Money::from((100_i64, Currency::USD));
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::from((1_000_i64, Currency::USD)));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash_available".into(),
            ecf_sweep: None,
            pik_toggle: None,
            ..Default::default()
        };

        let ctx_short = build_context(period, &[("cash_available", 60.0)]);
        let result_short =
            execute_waterfall_same_period(&ctx_short, &waterfall, &mut state, &contractual_flows)
                .expect("waterfall should execute");

        let tl = result_short.flows.get("TL-1").expect("TL-1");
        assert_eq!(tl.principal_payment.amount(), 60.0);
        assert_eq!(
            state
                .principal_shortfall
                .get("TL-1")
                .expect("principal shortfall recorded")
                .iter()
                .map(|claim| claim.amount.amount())
                .sum::<f64>(),
            40.0
        );
        assert!(
            result_short.warnings.iter().any(|warning| matches!(
                warning,
                EvalWarning::CapitalStructure {
                    warning: CapitalStructureWarning::PrincipalShortfall { .. },
                    ..
                }
            )),
            "principal shortfall must surface a structured warning"
        );

        state.advance_period();
        let ctx_recovered = build_context(period, &[("cash_available", 200.0)]);
        let result_recovered = execute_waterfall_same_period(
            &ctx_recovered,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect("waterfall should execute");

        let tl = result_recovered.flows.get("TL-1").expect("TL-1");
        assert_eq!(
            tl.principal_payment.amount(),
            140.0,
            "carried principal claim is paid in the next period's amortization category"
        );
        assert!(
            state.principal_shortfall.get("TL-1").is_none(),
            "principal shortfall clears once paid"
        );
        assert_eq!(
            result_recovered
                .equity_distribution
                .expect("equity populated")
                .amount(),
            60.0
        );
    }

    /// Conservation: fees + cash interest + principal + equity == available cash.
    #[test]
    fn test_equity_distribution_conserves_available_cash() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let context = build_context(period, &[("cash_available", 1_000.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.fees = Money::from((20_i64, Currency::USD));
        breakdown.interest_expense_cash = Money::from((100_i64, Currency::USD));
        breakdown.principal_payment = Money::from((200_i64, Currency::USD));
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::from((5_000_i64, Currency::USD)));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash_available".into(),
            ecf_sweep: None,
            pik_toggle: None,
            ..Default::default()
        };

        let result =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect("waterfall should execute");

        let tl = result.flows.get("TL-1").expect("TL-1");
        let equity = result
            .equity_distribution
            .expect("equity populated")
            .amount();
        assert_eq!(equity, 680.0);
        let conserved = tl.fees.amount()
            + tl.interest_expense_cash.amount()
            + tl.principal_payment.amount()
            + equity;
        assert!(
            (conserved - 1_000.0).abs() < 1e-9,
            "fees + interest + principal + equity must equal available cash, got {conserved}"
        );
    }

    #[test]
    fn payment_classes_pay_senior_interest_before_junior() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let context = build_context(period, &[("cash_available", 50_000.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut first_lien = CashflowBreakdown::with_currency(Currency::USD);
        first_lien.interest_expense_cash = Money::from((50_000_i64, Currency::USD));
        contractual_flows.insert("1L".to_string(), first_lien);
        let mut second_lien = CashflowBreakdown::with_currency(Currency::USD);
        second_lien.interest_expense_cash = Money::from((50_000_i64, Currency::USD));
        contractual_flows.insert("2L".to_string(), second_lien);

        let mut state = CapitalStructureState::new();
        state.opening_balances.insert(
            "1L".to_string(),
            Money::from((1_000_000_i64, Currency::USD)),
        );
        state.opening_balances.insert(
            "2L".to_string(),
            Money::from((1_000_000_i64, Currency::USD)),
        );

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash_available".into(),
            payment_classes: vec![
                PaymentClassSpec {
                    id: "first-lien".into(),
                    rank: 0,
                    instrument_ids: vec!["1L".into()],
                },
                PaymentClassSpec {
                    id: "second-lien".into(),
                    rank: 1,
                    instrument_ids: vec!["2L".into()],
                },
            ],
            ..Default::default()
        };

        let results =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect("waterfall should execute");

        let first = results.flows.get("1L").expect("1L");
        let second = results.flows.get("2L").expect("2L");
        assert!(
            (first.interest_expense_cash.amount() - 50_000.0).abs() < 1e-9,
            "1L interest should be paid in full, got {}",
            first.interest_expense_cash.amount()
        );
        assert!(
            second.interest_expense_cash.amount().abs() < 1e-9,
            "2L interest should be unpaid, got {}",
            second.interest_expense_cash.amount()
        );
        assert!(
            results.warnings.iter().any(|w| matches!(
                w,
                EvalWarning::CapitalStructure {
                    warning: CapitalStructureWarning::InterestShortfall {
                        instrument_id,
                        ..
                    },
                    ..
                } if instrument_id == "2L"
            )),
            "2L should record an interest shortfall, not a pro-rata split"
        );
    }

    #[test]
    fn mandatory_and_sweep_rungs_apply_independently() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let context = build_context(
            period,
            &[
                ("ebitda", 300_000.0),
                ("mandatory", 200_000.0),
                ("cash_available", 1_000_000.0),
            ],
        );

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        contractual_flows.insert(
            "TL-1".to_string(),
            CashflowBreakdown::with_currency(Currency::USD),
        );

        let mut state = CapitalStructureState::new();
        state.opening_balances.insert(
            "TL-1".to_string(),
            Money::from((10_000_000_i64, Currency::USD)),
        );

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::MandatoryPrepayment,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash_available".into(),
            mandatory_prepayment: Some(crate::capital_structure::PrepaymentSpec {
                amount: "mandatory".into(),
                facilities: crate::capital_structure::FacilitySelection::AllEligible,
            }),
            ecf_sweep: Some(EcfSweepSpec {
                cash_measure: "ebitda".into(),
                deductions: vec![
                    crate::capital_structure::EcfTerm::PaidCashInterest,
                    crate::capital_structure::EcfTerm::PaidFees,
                    crate::capital_structure::EcfTerm::PaidScheduledPrincipal,
                ],
                credits: Vec::new(),
                sweep_percentage: 1.0,
                facilities: crate::capital_structure::FacilitySelection::Named(vec!["TL-1".into()]),
            }),
            pik_toggle: None,
            ..Default::default()
        };

        let results =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect("waterfall should execute");

        let tl = results.flows.get("TL-1").expect("TL-1");
        assert!(
            (tl.principal_payment.amount() - 500_000.0).abs() < 1e-6,
            "mandatory 200k + sweep 300k should both apply, got {}",
            tl.principal_payment.amount()
        );
    }

    /// Scheduled amortization, a mandatory prepayment, the ECF sweep and a
    /// voluntary prepayment all land in `principal_payment`; the breakdown
    /// keeps each source separately and they add back to the total.
    #[test]
    fn principal_components_sum_to_principal_payment() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let context = build_context(
            period,
            &[
                ("ebitda", 300_000.0),
                ("mandatory", 200_000.0),
                ("voluntary", 50_000.0),
                ("cash_available", 1_000_000.0),
            ],
        );

        let mut contractual = CashflowBreakdown::with_currency(Currency::USD);
        contractual.interest_expense_cash = Money::from((80_000_i64, Currency::USD));
        contractual.fees = Money::from((5_000_i64, Currency::USD));
        contractual.principal_payment = Money::from((100_000_i64, Currency::USD));
        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        contractual_flows.insert("TL-1".to_string(), contractual);

        let mut state = CapitalStructureState::new();
        state.opening_balances.insert(
            "TL-1".to_string(),
            Money::from((10_000_000_i64, Currency::USD)),
        );

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::MandatoryPrepayment,
                PaymentPriority::Sweep,
                PaymentPriority::VoluntaryPrepayment,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash_available".into(),
            mandatory_prepayment: Some(crate::capital_structure::PrepaymentSpec {
                amount: "mandatory".into(),
                facilities: crate::capital_structure::FacilitySelection::AllEligible,
            }),
            voluntary_prepayment: Some(crate::capital_structure::PrepaymentSpec {
                amount: "voluntary".into(),
                facilities: crate::capital_structure::FacilitySelection::AllEligible,
            }),
            ecf_sweep: Some(EcfSweepSpec {
                cash_measure: "ebitda".into(),
                deductions: vec![
                    crate::capital_structure::EcfTerm::PaidCashInterest,
                    crate::capital_structure::EcfTerm::PaidFees,
                    crate::capital_structure::EcfTerm::PaidScheduledPrincipal,
                ],
                credits: Vec::new(),
                sweep_percentage: 1.0,
                facilities: crate::capital_structure::FacilitySelection::Named(vec!["TL-1".into()]),
            }),
            pik_toggle: None,
            ..Default::default()
        };

        let results =
            execute_waterfall_same_period(&context, &waterfall, &mut state, &contractual_flows)
                .expect("waterfall should execute");

        let tl = results.flows.get("TL-1").expect("TL-1");
        let amount = |money: Option<Money>| money.expect("recorded by the waterfall").amount();
        let scheduled = amount(tl.scheduled_principal);
        let mandatory = amount(tl.mandatory_prepayment);
        let sweep = amount(tl.sweep_prepayment);
        let voluntary = amount(tl.voluntary_prepayment);

        assert_eq!(scheduled, 100_000.0);
        assert_eq!(mandatory, 200_000.0);
        // ECF = EBITDA 300k - interest 80k - fees 5k - scheduled principal 100k.
        assert_eq!(sweep, 115_000.0);
        assert_eq!(voluntary, 50_000.0);
        assert_eq!(
            scheduled + mandatory + sweep + voluntary,
            tl.principal_payment.amount()
        );
        assert_eq!(tl.principal_payment.amount(), 465_000.0);

        // Opening balance less principal paid is the closing balance here
        // (no draws, no PIK).
        assert_eq!(amount(tl.opening_balance), 10_000_000.0);
        assert_eq!(
            amount(tl.opening_balance) - tl.principal_payment.amount(),
            tl.debt_balance.amount()
        );

        // Sources and uses of cash for the period reconcile.
        let available = results.available_cash.expect("waterfall ran").amount();
        let equity = results.equity_distribution.expect("residual").amount();
        assert_eq!(available, 1_000_000.0);
        assert_eq!(
            tl.fees.amount()
                + tl.interest_expense_cash.amount()
                + tl.principal_payment.amount()
                + equity,
            available
        );
        tl.validate_currency_invariant()
            .expect("components share the breakdown currency");
    }

    /// A breakdown written before the principal components existed still
    /// deserializes, with the components unrecorded.
    #[test]
    fn breakdown_without_principal_components_deserializes() {
        let breakdown = CashflowBreakdown::with_currency(Currency::USD);
        let json = serde_json::to_value(&breakdown).expect("serialize");
        for field in [
            "opening_balance",
            "scheduled_principal",
            "mandatory_prepayment",
            "sweep_prepayment",
            "voluntary_prepayment",
        ] {
            assert!(
                json.get(field).is_none(),
                "{field} is skipped when unrecorded"
            );
        }
        let back: CashflowBreakdown = serde_json::from_value(json).expect("deserialize");
        assert!(back.scheduled_principal.is_none());
        assert!(back.opening_balance.is_none());
    }
    #[test]
    fn waterfall_errors_preserve_state_and_retry_matches_a_clean_run() {
        let period = PeriodId::quarter(2025, 1).unwrap();
        let mut flow = CashflowBreakdown::with_currency(Currency::USD);
        flow.interest_expense_cash = Money::from((100_i64, Currency::USD));
        flow.principal_payment = Money::from((50_i64, Currency::USD));
        flow.debt_balance = Money::from((950_i64, Currency::USD));
        let contractual = IndexMap::from([("TL-1".to_string(), flow)]);
        let mut initial = CapitalStructureState::new();
        initial
            .opening_balances
            .insert("TL-1".into(), Money::from((1000_i64, Currency::USD)));
        initial
            .interest_shortfall
            .insert("TL-1".into(), Money::from((20_i64, Currency::USD)));
        initial
            .fee_shortfall
            .insert("TL-1".into(), Money::from((10_i64, Currency::USD)));
        let valid_spec = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::MandatoryPrepayment,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash".into(),
            pik_toggle: Some(PikToggleSpec {
                liquidity_metric: "liquidity".into(),
                threshold: 100.0,
                elections: vec![crate::capital_structure::PikElectionSpec {
                    instrument_id: "TL-1".into(),
                    pik_fraction: rust_decimal::Decimal::ONE,
                    min_coupon_periods: 3,
                }],
            }),
            mandatory_prepayment: Some(crate::capital_structure::PrepaymentSpec {
                amount: "0".into(),
                facilities: crate::capital_structure::FacilitySelection::AllEligible,
            }),
            ..Default::default()
        };
        let valid_context = build_context(period, &[("liquidity", 50.0), ("cash", 1000.0)]);
        for late in [false, true] {
            let mut spec = valid_spec.clone();
            let context = if late {
                spec.mandatory_prepayment = Some(crate::capital_structure::PrepaymentSpec {
                    amount: "1 / 0".into(),
                    facilities: crate::capital_structure::FacilitySelection::AllEligible,
                });
                valid_context.clone()
            } else {
                build_context(period, &[("liquidity", 50.0), ("cash", 1e100)])
            };
            let mut state = initial.clone();
            let before = format!("{state:?}");
            assert!(
                execute_waterfall_same_period(&context, &spec, &mut state, &contractual).is_err()
            );
            assert_eq!(
                format!("{state:?}"),
                before,
                "failed waterfall changed runtime state"
            );
            let retry = execute_waterfall_same_period(
                &valid_context,
                &valid_spec,
                &mut state,
                &contractual,
            )
            .unwrap();
            let mut clean_state = initial.clone();
            let clean = execute_waterfall_same_period(
                &valid_context,
                &valid_spec,
                &mut clean_state,
                &contractual,
            )
            .unwrap();
            assert_eq!(format!("{state:?}"), format!("{clean_state:?}"));
            assert_eq!(format!("{retry:?}"), format!("{clean:?}"));
        }
    }

    /// The waterfall reports the principal it changed in exact money and the
    /// PIK toggle's request; the evaluator rebuilds the schedules from them.
    #[test]
    fn the_waterfall_reports_its_postings_and_pik_request() {
        use finstack_quant_core::dates::{Date, Period};
        let id = PeriodId::quarter(2025, 1).unwrap();
        let period = Period {
            id,
            start: Date::MIN,
            end: Date::MAX,
            is_actual: false,
        };
        let usd = |amount: i64| Money::from((amount, Currency::USD));
        let mut flow = CashflowBreakdown::with_currency(Currency::USD);
        flow.interest_expense_cash = usd(30);
        flow.debt_balance = usd(1000);
        let contractual = IndexMap::from([("TL-1".to_string(), flow)]);
        let mut initial = CapitalStructureState::new();
        initial.opening_balances.insert("TL-1".into(), usd(1000));
        initial
            .set_period_principal_flows("TL-1", &period, Vec::new())
            .unwrap();
        let spec = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::MandatoryPrepayment,
                PaymentPriority::Equity,
            ],
            available_cash_node: "cash".into(),
            pik_toggle: Some(PikToggleSpec {
                liquidity_metric: "liquidity".into(),
                threshold: 100.0,
                elections: vec![crate::capital_structure::PikElectionSpec {
                    instrument_id: "TL-1".into(),
                    pik_fraction: rust_decimal::Decimal::ONE,
                    min_coupon_periods: 0,
                }],
            }),
            mandatory_prepayment: Some(crate::capital_structure::PrepaymentSpec {
                amount: "200".into(),
                facilities: crate::capital_structure::FacilitySelection::AllEligible,
            }),
            ..Default::default()
        };
        let context = build_context(id, &[("liquidity", 50.0), ("cash", 1000.0)]);

        // 200 prepaid. The toggle only records its request: the 30 coupon
        // due now is paid in cash, not capitalized.
        let mut state = initial;
        let result =
            execute_waterfall(&context, &spec, &mut state, &contractual, None).expect("waterfall");
        assert_eq!(result.postings["TL-1"], usd(-200));
        assert_eq!(result.pik_request, Some(true));
        assert_eq!(state.closing_balances["TL-1"], usd(800));
    }
}
