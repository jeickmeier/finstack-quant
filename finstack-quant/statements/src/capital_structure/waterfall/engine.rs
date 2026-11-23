//! Waterfall period execution engine and cash-allocation helpers.

use crate::capital_structure::cashflows::{CashflowBreakdown, add_optional_money};
use crate::capital_structure::state::CapitalStructureState;
use crate::capital_structure::waterfall_spec::{PaymentPriority, WaterfallSpec};
use crate::error::Result;
use crate::evaluator::{CapitalStructureWarning, EvalWarning, EvaluationContext};
use finstack_quant_core::money::Money;
use indexmap::IndexMap;

use super::cash_distribution::{
    PrincipalComponent, StagedInstrumentFlow, allocate_by_class, apply_cash_cap_to_category,
    apply_prepayment,
};
use super::excess_cash_flow::{PaidAhead, calculate_ecf_sweep};
use super::payment_in_kind::evaluate_pik_toggle;
use super::payment_stack::waterfall_currency;

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
pub(super) fn eval_input(
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
pub(super) fn input_error(
    context: &EvaluationContext,
    expr: &str,
    why: &str,
) -> crate::error::Error {
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
pub(super) fn eval_money(
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
pub(super) fn eval_number(
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
pub(super) fn class_rank_for(spec: &WaterfallSpec, instrument_id: &str) -> Result<u32> {
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
pub(super) fn apply_named_prepay(
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
        let zero_cash = Money::from((0_i64, cash_currency));
        let mut record =
            |kind: PaymentPriority, requested: Money, paid: Money, carried: bool| -> Result<()> {
                let unpaid = requested.checked_sub(paid)?;
                prepayments.push(crate::capital_structure::PrepaymentOutcome {
                    kind,
                    requested,
                    paid,
                    unpaid,
                    carried: if carried { unpaid } else { zero_cash },
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
                        record(*priority, requested, paid, false)?;
                    }
                }
                PaymentPriority::Sweep => {
                    if let Some(spec) = &waterfall_spec.ecf_sweep {
                        let measured = calculate_ecf_sweep(
                            context,
                            spec,
                            &paid,
                            cash_currency,
                            &mut warnings,
                        )?;
                        let Some(lag) = spec.payment_lag_days else {
                            // Paid in the period it is measured; an unpaid
                            // amount is reported, not carried.
                            let swept = apply_prepayment(
                                &mut staged,
                                &mut remaining_cash,
                                measured,
                                &spec.facilities,
                                |b| &mut b.sweep_prepayment,
                            )?;
                            record(*priority, measured, swept, false)?;
                            continue;
                        };
                        // A dated obligation: due `lag` days after this
                        // period's close, paid in the period that holds its
                        // due date, oldest first, with arrears carried.
                        let (_, period_end) = context
                            .history
                            .period_bounds(&context.period_id)
                            .ok_or_else(|| {
                                crate::error::Error::capital_structure(format!(
                                    "a sweep with `payment_lag_days` needs the dates of period {}",
                                    context.period_id
                                ))
                            })?;
                        let close = period_end - time::Duration::days(1);
                        if measured.amount_decimal() > rust_decimal::Decimal::ZERO {
                            state
                                .sweep_obligations
                                .push((close + time::Duration::days(i64::from(lag)), measured));
                        }
                        let mut due = zero_cash;
                        for (date, amount) in &state.sweep_obligations {
                            if *date <= close {
                                due = due.checked_add(*amount)?;
                            }
                        }
                        let swept = apply_prepayment(
                            &mut staged,
                            &mut remaining_cash,
                            due,
                            &spec.facilities,
                            |b| &mut b.sweep_prepayment,
                        )?;
                        let mut left = swept;
                        for (date, amount) in &mut state.sweep_obligations {
                            if *date <= close {
                                let settled = if left.amount_decimal() < amount.amount_decimal() {
                                    left
                                } else {
                                    *amount
                                };
                                *amount = amount.checked_sub(settled)?;
                                left = left.checked_sub(settled)?;
                            }
                        }
                        state.sweep_obligations.retain(|(_, amount)| {
                            amount.amount_decimal() > rust_decimal::Decimal::ZERO
                        });
                        record(*priority, due, swept, true)?;
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
                        record(*priority, requested, paid, false)?;
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
pub(super) fn available_cash(
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
