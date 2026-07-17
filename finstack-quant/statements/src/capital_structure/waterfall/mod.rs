//! Cash Flow Waterfall & Sweep Mechanics
//!
//! This module implements dynamic cash flow allocation according to priority of payments,
//! excess cash flow sweeps, and PIK toggles based on model results.
//!
//! # Sign Conventions
//!
//! The ECF (Excess Cash Flow) sweep calculation follows standard LBO model conventions:
//!
//! ## Input Nodes
//!
//! - **EBITDA** (`ebitda_node`): Positive value representing operating cash generation.
//!   Example: $10M EBITDA → use `10_000_000.0`
//!
//! - **Taxes** (`taxes_node`): Positive value representing cash tax payments (outflow).
//!   Example: $2M taxes paid → use `2_000_000.0` (not negative)
//!
//! - **CapEx** (`capex_node`): Positive value representing capital expenditures (outflow).
//!   Example: $1.5M capex → use `1_500_000.0` (not negative)
//!
//! - **Working Capital** (`working_capital_node`): Signed value representing change in NWC.
//!   - Positive = cash consumed (increase in receivables/inventory)
//!   - Negative = cash released (increase in payables)
//!   - Example: $500K increase in NWC → use `500_000.0`
//!
//! ## ECF Calculation
//!
//! ```text
//! ECF = EBITDA - Taxes - CapEx - Working_Capital_Change - Cash_Interest
//!       - Fees                  (when Fees rank ahead of prepayment)
//!       - Scheduled_Principal   (when Amortization ranks ahead of the
//!                                prepayment priority in the waterfall)
//! Sweep = max(0, ECF × sweep_percentage)
//! ```
//!
//! The `cash_interest_node` is optional. Per S&P LCD / standard LPA definitions,
//! ECF should include a cash interest deduction. Set it to include this deduction.
//! Fees and scheduled amortization paid ahead of the sweep are likewise
//! deducted so the sweep cannot double-spend cash already consumed by payment
//! categories that rank ahead of debt prepayment.
//!
//! The sweep is floored at zero (cannot sweep negative cash flow) and then
//! applied as additional principal prepayment to the target instrument.
//!
//! ## Example
//!
//! ```text
//! EBITDA:    $10,000,000  (positive)
//! Taxes:     $ 2,000,000  (positive = outflow)
//! CapEx:     $ 1,500,000  (positive = outflow)
//! ΔWC:       $   500,000  (positive = cash used)
//! ─────────────────────────────────
//! ECF:       $ 6,000,000
//! Sweep @50%: $ 3,000,000 → applied to debt prepayment
//! ```

mod cash_distribution;
mod excess_cash_flow;
mod payment_in_kind;
mod payment_stack;
mod period_close;

use crate::capital_structure::cashflows::CashflowBreakdown;
use crate::capital_structure::state::CapitalStructureState;
use crate::capital_structure::waterfall_spec::{PaymentPriority, WaterfallSpec};
use crate::error::Result;
use crate::evaluator::{EvalWarning, EvaluationContext};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::money::Money;
use indexmap::IndexMap;
use std::collections::HashSet;

use cash_distribution::{allocate_pro_rata, apply_cash_cap_to_category, StagedInstrumentFlow};
use excess_cash_flow::calculate_ecf_sweep;
use payment_in_kind::{apply_pik_transitions, evaluate_pik_toggle, is_pik_enabled};
use payment_stack::{extra_principal_priority, priority_index, waterfall_currency};
use period_close::update_cumulative_metrics;

/// Money comparison tolerance for waterfall allocation, in currency units.
///
/// Allocation math is done in `f64`, so pro-rata splits leave sub-cent
/// rounding residue. This tolerance (1e-6 ≈ a millionth of a unit) is compared
/// against dollar magnitudes to decide whether a residual/shortfall is real;
/// `f64::EPSILON` (~2.2e-16) is far too tight for that — any float residue
/// would register as a genuine claim and be carried forward.
const MONEY_TOLERANCE: f64 = 1e-6;

/// Result of executing the waterfall for a single period.
#[derive(Debug, Clone)]
pub struct WaterfallPeriodResult {
    /// Per-instrument cashflow breakdowns after sweep, PIK, and
    /// priority-of-payments allocation have been applied.
    pub flows: IndexMap<String, CashflowBreakdown>,
    /// Structured warnings raised during allocation (e.g. cash shortfalls).
    pub warnings: Vec<EvalWarning>,
    /// Post-waterfall residual cash distributed to equity.
    ///
    /// Only `Some` when `available_cash_node` is configured: it is the cash
    /// remaining after fees, interest, and principal allocations, so
    /// `fees + cash interest + principal + equity == available cash`.
    pub equity_distribution: Option<Money>,
}

/// Evaluate a node reference or inline DSL expression against the current context.
///
/// The result is required to be finite. The evaluator deliberately produces
/// `NaN` for division by zero and `±Inf` on overflow, and stores them so
/// downstream formulas can guard them — but every waterfall input feeds a
/// cash allocation, where a non-finite value degrades silently rather than
/// loudly: `NaN.max(0.0)` is `0.0` (an empty cash pool that shorts every
/// creditor) and `NaN < threshold` is `false` (a PIK toggle that never fires).
/// Both surface only as misleading downstream shortfall warnings. Fail here
/// instead, naming the expression that produced the value.
///
/// Warnings raised while evaluating an inline expression are appended to
/// `warnings`. The expression runs against a scratch copy of the context, so
/// without this they would be dropped with the copy: an expression that guards
/// its own bad arithmetic (`coalesce(a / 0, 0)`) returns a finite value and
/// would otherwise hide the division entirely.
fn eval_value_or_formula(
    context: &EvaluationContext,
    expr: &str,
    warnings: &mut Vec<EvalWarning>,
) -> Result<f64> {
    let value = if let Ok(value) = context.get_value(expr) {
        value
    } else {
        let compiled = crate::dsl::parse_and_compile(expr)?;
        let mut scratch = context.clone();
        // The scratch inherits the context's existing warnings; only those
        // added by this evaluation are new.
        let inherited = scratch.warnings.len();
        // Attribute warnings to the expression itself. The formula evaluator
        // only records a warning when it has a node id to attach it to, so
        // passing `None` here silently suppressed them at the source; an
        // inline waterfall expression has no node id of its own, and the
        // expression text is the most useful thing to name in a diagnostic.
        let result =
            crate::evaluator::formula::evaluate_formula(&compiled, &mut scratch, Some(expr));
        warnings.extend(scratch.warnings.drain(inherited..));
        result?
    };

    if !value.is_finite() {
        return Err(crate::error::Error::capital_structure(format!(
            "waterfall input '{expr}' evaluated to the non-finite value {value} in period {}. \
             Cash allocation requires finite inputs; a non-finite value would be silently \
             read as zero cash (or as a PIK toggle that never fires). Check the expression \
             for division by zero or overflow.",
            context.period_id
        )));
    }

    Ok(value)
}

/// Convert a waterfall-derived `f64` into `Money`, naming `expr` on failure.
///
/// `Money::new` panics on amounts outside `rust_decimal`'s representable range
/// (~7.9e28), which a plain model value can reach. Library code must not panic
/// on user input (INVARIANTS.md §5), so surface it as an error instead.
fn money_from_expr(
    amount: f64,
    currency: finstack_quant_core::currency::Currency,
    expr: &str,
) -> Result<Money> {
    Money::try_new(amount, currency).map_err(|e| {
        crate::error::Error::capital_structure(format!(
            "waterfall input '{expr}' produced the amount {amount}, which is not a \
             representable {currency} value: {e}"
        ))
    })
}

/// Execute waterfall logic for a single period.
///
/// This function:
/// 1. Checks PIK toggle conditions and updates interest mode
/// 2. Calculates contractual flows (interest, amortization)
/// 3. Calculates ECF and applies sweep if configured
/// 4. Allocates available cash according to priority stack
///
/// # Arguments
///
/// * `period_id` - Current period being evaluated
/// * `context` - Evaluation context with model results
/// * `waterfall_spec` - Waterfall configuration
/// * `state` - Current capital structure state (opening balances, etc.)
/// * `contractual_flows` - Pre-calculated contractual flows by instrument
///
/// # Returns
///
/// Returns a [`WaterfallPeriodResult`] with per-instrument cashflow breakdowns
/// after sweep, PIK, and priority-of-payments allocation have been applied,
/// plus shortfall warnings and the equity residual. `state` is updated
/// in-place with opening/closing balances and cumulative tracking fields.
///
/// # Limitations
///
/// Allocation within a payment category is single-class **pro-rata** across
/// all instruments — there is no intra-category tranche seniority. Prepayment
/// penalties, call premiums, and OID are not modeled (prepayments apply at
/// par). See [`WaterfallSpec`] for details.
///
/// # Errors
///
/// Returns an error if required statement nodes are missing, if the waterfall
/// references inconsistent currencies, or if sweep / PIK calculations fail.
///
/// # References
///
/// - Fixed-income capital structure context: `docs/REFERENCES.md#tuckman-serrat-fixed-income`
pub fn execute_waterfall(
    _period_id: &PeriodId,
    context: &EvaluationContext,
    waterfall_spec: &WaterfallSpec,
    state: &mut CapitalStructureState,
    contractual_flows: &IndexMap<String, CashflowBreakdown>,
) -> Result<WaterfallPeriodResult> {
    waterfall_spec.validate()?;

    // A configured waterfall over zero instruments is a no-op, not an error: a
    // staged model (waterfall defined before instruments are added, or all
    // instruments matured / pre-issuance) should pass through rather than fail.
    if contractual_flows.is_empty() {
        return Ok(WaterfallPeriodResult {
            flows: IndexMap::new(),
            warnings: Vec::new(),
            equity_distribution: None,
        });
    }

    // Validate that configured sweep / PIK targets actually name a debt
    // instrument. A typo (e.g. a trailing space) would otherwise silently
    // disable the entire mechanism — no sweep, no PIK toggle — with no
    // diagnostic, materially shifting IRRs.
    if let Some(target) = waterfall_spec
        .ecf_sweep
        .as_ref()
        .and_then(|s| s.target_instrument_id.as_ref())
    {
        if !contractual_flows.contains_key(target) {
            return Err(crate::error::Error::build(format!(
                "WaterfallSpec: `ecf_sweep.target_instrument_id` '{target}' does not match any \
                 debt instrument; the sweep would be silently dropped. Check for typos or \
                 trailing whitespace."
            )));
        }
    }
    if let Some(targets) = waterfall_spec
        .pik_toggle
        .as_ref()
        .and_then(|p| p.target_instrument_ids.as_ref())
    {
        for target in targets {
            if !contractual_flows.contains_key(target) {
                return Err(crate::error::Error::build(format!(
                    "WaterfallSpec: `pik_toggle.target_instrument_ids` entry '{target}' does not \
                     match any debt instrument; that instrument would never PIK. Check for typos \
                     or trailing whitespace."
                )));
            }
        }
    }

    let _span = tracing::info_span!(
        "statements.capital_structure.waterfall",
        period = _period_id.to_string(),
        instruments = contractual_flows.len(),
        has_sweep = waterfall_spec.ecf_sweep.is_some(),
        has_pik_toggle = waterfall_spec.pik_toggle.is_some()
    )
    .entered();

    let mut result = IndexMap::new();
    let mut warnings: Vec<EvalWarning> = Vec::new();
    let cash_currency = waterfall_currency(contractual_flows)?;

    // --- resolve priority positions ---
    let fees_priority = priority_index(&waterfall_spec.priority_of_payments, PaymentPriority::Fees);
    let amortization_priority = priority_index(
        &waterfall_spec.priority_of_payments,
        PaymentPriority::Amortization,
    );
    let extra_principal_priority = extra_principal_priority(&waterfall_spec.priority_of_payments);
    let equity_priority = priority_index(
        &waterfall_spec.priority_of_payments,
        PaymentPriority::Equity,
    );

    // --- Step 1: PIK toggle ---
    //
    // Evaluate PIK mode BEFORE ECF so that ECF cash-interest deduction
    // correctly reflects actual cash interest paid (instruments in PIK mode
    // pay zero cash interest that period).
    let (pik_enable, pik_targets): (Option<bool>, Option<HashSet<String>>) =
        if let Some(pik_spec) = &waterfall_spec.pik_toggle {
            (
                Some(evaluate_pik_toggle(context, pik_spec, &mut warnings)?),
                pik_spec
                    .target_instrument_ids
                    .as_ref()
                    .map(|ids| ids.iter().cloned().collect()),
            )
        } else {
            (None, None)
        };

    // Apply PIK mode transitions to `state` before using it for ECF.
    let min_periods_in_pik = waterfall_spec
        .pik_toggle
        .as_ref()
        .map(|spec| spec.min_periods_in_pik)
        .unwrap_or(0);

    if let Some(enable_pik) = pik_enable {
        apply_pik_transitions(
            state,
            contractual_flows,
            enable_pik,
            pik_targets.as_ref(),
            min_periods_in_pik,
        );
    }

    // --- Step 2: ECF / sweep ---
    //
    // ECF is PIK-aware: when `cash_interest_node` is omitted, the fallback
    // deducts contractual cash interest only for instruments NOT in PIK mode
    // this period.
    let sweep_amount = if let Some(ecf_spec) = &waterfall_spec.ecf_sweep {
        // Scheduled amortization that ranks ahead of the prepayment priority
        // consumes cash before the sweep, so it is deducted from ECF (per
        // standard LPA ECF definitions) to avoid double-spending that cash.
        let deduct_scheduled_principal = amortization_priority < extra_principal_priority;
        let deduct_fees = fees_priority < extra_principal_priority;
        calculate_ecf_sweep(
            context,
            ecf_spec,
            contractual_flows,
            state,
            deduct_scheduled_principal,
            deduct_fees,
            &mut warnings,
        )?
    } else {
        Money::new(0.0, cash_currency)
    };
    let available_cash = if let Some(available_cash_node) = &waterfall_spec.available_cash_node {
        let cash = eval_value_or_formula(context, available_cash_node, &mut warnings)?;
        Some(money_from_expr(
            cash.max(0.0),
            cash_currency,
            available_cash_node,
        )?)
    } else {
        None
    };

    // --- Step 3: Build staged per-instrument state ---
    //
    // Execution order per standard loan documentation:
    //   1. Determine sweep amount (already computed above)
    //   2. Apply sweep as additional principal prepayment
    //   3. Update balance after sweep + scheduled amortization
    //   4. Capitalize PIK interest into the closing balance when appropriate
    let mut staged: Vec<StagedInstrumentFlow> = Vec::with_capacity(contractual_flows.len());
    for (instrument_id, breakdown) in contractual_flows {
        let currency = breakdown.interest_expense_cash.currency();
        let opening_balance = state.get_opening_balance(instrument_id, currency);
        let net_new_funding = state.get_period_new_funding(instrument_id, currency);
        // The balance available to repay principal this period is the opening
        // balance plus any in-period draws (a revolver can repay against cash it
        // just drew). Used to cap principal so the available-cash pool is never
        // over-deducted (see Step 5/6).
        let payable_balance = (opening_balance.amount() + net_new_funding.amount()).max(0.0);

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
                    return Err(crate::error::Error::currency_mismatch(
                        currency,
                        shortfall.currency(),
                    ));
                }
                staged_breakdown.interest_expense_cash = staged_breakdown
                    .interest_expense_cash
                    .checked_add(shortfall)?;
            }
        }
        if let Some(shortfall) = state
            .principal_shortfall
            .shift_remove(instrument_id.as_str())
        {
            if shortfall.amount() > 0.0 {
                if shortfall.currency() != currency {
                    return Err(crate::error::Error::currency_mismatch(
                        currency,
                        shortfall.currency(),
                    ));
                }
                staged_breakdown.principal_payment =
                    staged_breakdown.principal_payment.checked_add(shortfall)?;
            }
        }
        if let Some(shortfall) = state.fee_shortfall.shift_remove(instrument_id.as_str()) {
            if shortfall.amount() > 0.0 {
                if shortfall.currency() != currency {
                    return Err(crate::error::Error::currency_mismatch(
                        currency,
                        shortfall.currency(),
                    ));
                }
                staged_breakdown.fees = staged_breakdown.fees.checked_add(shortfall)?;
            }
        }
        // Clamp scheduled principal to the payable balance so Step 5 never
        // deducts more cash than can actually be applied (over-amortization or
        // carried principal shortfalls can push the claim above the balance;
        // paying more principal than is owed would destroy cash that should
        // have flowed to equity).
        let scheduled_principal = Money::new(
            staged_breakdown
                .principal_payment
                .amount()
                .clamp(0.0, payable_balance),
            currency,
        );
        staged_breakdown.principal_payment = scheduled_principal;
        staged.push(StagedInstrumentFlow {
            instrument_id: instrument_id.clone(),
            breakdown: staged_breakdown,
            opening_balance,
            net_new_funding,
            extra_principal: Money::new(0.0, currency),
            scheduled_principal,
            toggled_pik_moved: Money::new(0.0, currency),
        });
    }

    // --- Step 4: Distribute sweep across instruments ---
    let mut remaining_sweep = if equity_priority < extra_principal_priority {
        Money::new(0.0, sweep_amount.currency())
    } else {
        sweep_amount
    };

    // Note: no separate fee, interest-priority, or amortization deduction from
    // `remaining_sweep` here. When an ECF sweep is configured,
    // `calculate_ecf_sweep` already deducts cash interest plus any fees and
    // scheduled principal that rank ahead of the prepayment priority. When no
    // ECF sweep is configured, `sweep_amount` is zero, so `remaining_sweep`
    // starts at zero and any subtraction would be a no-op.

    let target_instrument_id = waterfall_spec
        .ecf_sweep
        .as_ref()
        .and_then(|spec| spec.target_instrument_id.as_deref());
    let mut extra_capacity: IndexMap<String, f64> = IndexMap::new();
    let mut total_extra_capacity = 0.0;
    for s in &staged {
        let eligible = if let Some(target_id) = target_instrument_id {
            target_id == s.instrument_id
        } else {
            true
        };
        if !eligible || extra_principal_priority == usize::MAX {
            extra_capacity.insert(s.instrument_id.clone(), 0.0);
            continue;
        }

        let capacity = (s.opening_balance.amount() - s.scheduled_principal.amount()).max(0.0);

        total_extra_capacity += capacity;
        extra_capacity.insert(s.instrument_id.clone(), capacity);
    }

    // Two-pass approach: compute all proportional shares first, then apply.
    // This avoids the bug where mutating remaining_sweep during iteration
    // gives incorrect proportions to instruments after the first.
    let sweep_currency = remaining_sweep.currency();
    let sweep_total = remaining_sweep.amount();
    let staged_len = staged.len();
    let mut sweep_allocations: Vec<f64> = vec![0.0; staged_len];

    for (idx, s) in staged.iter().enumerate() {
        let currency = s.breakdown.interest_expense_cash.currency();

        sweep_allocations[idx] =
            if extra_principal_priority == usize::MAX || sweep_currency != currency {
                0.0
            } else if let Some(target_id) = target_instrument_id {
                if target_id == s.instrument_id {
                    let capacity = *extra_capacity.get(s.instrument_id.as_str()).unwrap_or(&0.0);
                    sweep_total.min(capacity)
                } else {
                    0.0
                }
            } else {
                let capacity = *extra_capacity.get(s.instrument_id.as_str()).unwrap_or(&0.0);
                if total_extra_capacity <= 0.0 || capacity <= 0.0 {
                    0.0
                } else {
                    let proportional = sweep_total * (capacity / total_extra_capacity);
                    proportional.min(capacity)
                }
            };
    }

    // Cascade the unallocated residual across instruments with remaining
    // capacity until the sweep is exhausted or no capacity remains. The
    // residual arises when proportional shares are capped at capacity; a
    // single-instrument assignment would silently drop cash whenever the
    // (arbitrary) last instrument lacked headroom. Only floating-point
    // rounding residue may remain after the cascade.
    let mut residual = sweep_total - sweep_allocations.iter().sum::<f64>();
    for _ in 0..staged_len {
        if residual <= MONEY_TOLERANCE {
            break;
        }
        let remaining_capacity: f64 = staged
            .iter()
            .enumerate()
            .map(|(idx, s)| {
                let capacity = *extra_capacity.get(s.instrument_id.as_str()).unwrap_or(&0.0);
                (capacity - sweep_allocations[idx]).max(0.0)
            })
            .sum();
        if remaining_capacity <= 0.0 {
            break;
        }
        let distributable = residual.min(remaining_capacity);
        for (idx, s) in staged.iter().enumerate() {
            let capacity = *extra_capacity.get(s.instrument_id.as_str()).unwrap_or(&0.0);
            let headroom = (capacity - sweep_allocations[idx]).max(0.0);
            if headroom <= 0.0 {
                continue;
            }
            let share = (distributable * (headroom / remaining_capacity)).min(headroom);
            sweep_allocations[idx] += share;
            residual -= share;
        }
    }

    // Second pass: apply computed shares
    for (idx, s) in staged.iter_mut().enumerate() {
        let currency = s.breakdown.interest_expense_cash.currency();
        s.extra_principal = Money::new(sweep_allocations[idx], currency);
        remaining_sweep = remaining_sweep.checked_sub(s.extra_principal)?;
        s.breakdown.principal_payment = s.scheduled_principal.checked_add(s.extra_principal)?;
    }

    // --- Step 4b: Apply PIK mode (pre-cap) ---
    // When the PIK toggle is active, the full contractual coupon is moved
    // into the PIK bucket unconditionally BEFORE the available-cash caps in
    // Step 5. PIK'd coupons must never consume cash, and the full contractual
    // coupon capitalizes at period close regardless of where Interest ranks
    // relative to the prepayment priorities. The moved amount is recorded so
    // toggle-driven capitalization can be tracked in state at close.
    for s in &mut staged {
        let currency = s.breakdown.interest_expense_cash.currency();
        if is_pik_enabled(state, &s.instrument_id) {
            let moved = s.breakdown.interest_expense_cash;
            s.toggled_pik_moved = moved;
            s.breakdown.interest_expense_pik += moved;
            s.breakdown.interest_expense_cash = Money::new(0.0, currency);
        }
    }

    // --- Step 5: Available cash caps ---
    //
    // The three prepayment priorities (MandatoryPrepayment, VoluntaryPrepayment,
    // Sweep) all share the single `extra_principal` bucket populated from the
    // ECF sweep in Step 4. Because there is only one bucket, the first of the
    // three that appears in `priority_of_payments` consumes the cash cap for
    // that bucket; later entries are no-ops. Modelers who need strict ordering
    // across distinct prepayment types should populate separate buckets
    // upstream (not currently supported by this engine) and distinguish them
    // via separate `target_instrument_id`s.
    let mut shortfalls: IndexMap<String, Money> = IndexMap::new();
    let mut equity_distribution: Option<Money> = None;
    if let Some(mut remaining_cash) = available_cash {
        // Snapshot planned interest and fees so unpaid amounts can be carried
        // forward instead of silently evaporating when cash runs out.
        let planned_interest: Vec<f64> = staged
            .iter()
            .map(|s| s.breakdown.interest_expense_cash.amount().max(0.0))
            .collect();
        let planned_fees: Vec<f64> = staged
            .iter()
            .map(|s| s.breakdown.fees.amount().max(0.0))
            .collect();
        let planned_scheduled_principal: Vec<f64> = staged
            .iter()
            .map(|s| s.scheduled_principal.amount().max(0.0))
            .collect();
        let mut extra_principal_capped = false;
        for priority in &waterfall_spec.priority_of_payments {
            match priority {
                PaymentPriority::Fees => {
                    apply_cash_cap_to_category(
                        &mut staged,
                        &mut remaining_cash,
                        *_period_id,
                        "fees",
                        &mut warnings,
                        |s| &mut s.breakdown.fees,
                    );
                }
                PaymentPriority::Interest => {
                    apply_cash_cap_to_category(
                        &mut staged,
                        &mut remaining_cash,
                        *_period_id,
                        "interest",
                        &mut warnings,
                        |s| &mut s.breakdown.interest_expense_cash,
                    );
                }
                PaymentPriority::Amortization => {
                    let planned: Vec<f64> = staged
                        .iter()
                        .map(|s| s.scheduled_principal.amount().max(0.0))
                        .collect();
                    let allocations = allocate_pro_rata(&planned, &mut remaining_cash);
                    for (s, allocated) in staged.iter_mut().zip(allocations.into_iter()) {
                        s.scheduled_principal =
                            Money::new(allocated, s.scheduled_principal.currency());
                    }
                }
                PaymentPriority::MandatoryPrepayment
                | PaymentPriority::VoluntaryPrepayment
                | PaymentPriority::Sweep => {
                    if extra_principal_capped {
                        continue;
                    }
                    let planned: Vec<f64> = staged
                        .iter()
                        .map(|s| s.extra_principal.amount().max(0.0))
                        .collect();
                    let allocations = allocate_pro_rata(&planned, &mut remaining_cash);
                    for (s, allocated) in staged.iter_mut().zip(allocations.into_iter()) {
                        s.extra_principal = Money::new(allocated, s.extra_principal.currency());
                    }
                    extra_principal_capped = true;
                }
                PaymentPriority::Equity => {}
            }
        }

        // Per-instrument shortfall: planned − allocated. Unpaid debt service
        // does not evaporate — it is carried as a claim in the next period.
        // Interest and fees are tracked in *separate* buckets so a fee arrears
        // re-enters the fee category (not demoted into interest, which ranks
        // one rung lower and would also misclassify the accrual).
        for (idx, s) in staged.iter().enumerate() {
            let unpaid_interest =
                planned_interest[idx] - s.breakdown.interest_expense_cash.amount().max(0.0);
            if unpaid_interest > MONEY_TOLERANCE {
                let currency = s.breakdown.interest_expense_cash.currency();
                shortfalls.insert(
                    s.instrument_id.clone(),
                    Money::new(unpaid_interest, currency),
                );
                warnings.push(EvalWarning::CapitalStructureCashflowIgnored {
                    period: *_period_id,
                    kind: format!(
                        "interest_shortfall(instrument={}, amount={unpaid_interest:.4})",
                        s.instrument_id
                    ),
                    cashflow_date: _period_id.to_string(),
                });
                tracing::warn!(
                    instrument = s.instrument_id.as_str(),
                    shortfall = unpaid_interest,
                    period = _period_id.to_string(),
                    "Available cash insufficient for planned interest; \
                     shortfall accrued and carried into the next period's interest claim."
                );
            }

            let unpaid_fees = planned_fees[idx] - s.breakdown.fees.amount().max(0.0);
            if unpaid_fees > MONEY_TOLERANCE {
                let currency = s.breakdown.fees.currency();
                shortfalls.insert(
                    format!("fees::{}", s.instrument_id),
                    Money::new(unpaid_fees, currency),
                );
                warnings.push(EvalWarning::CapitalStructureCashflowIgnored {
                    period: *_period_id,
                    kind: format!(
                        "fee_shortfall(instrument={}, amount={unpaid_fees:.4})",
                        s.instrument_id
                    ),
                    cashflow_date: _period_id.to_string(),
                });
                tracing::warn!(
                    instrument = s.instrument_id.as_str(),
                    shortfall = unpaid_fees,
                    period = _period_id.to_string(),
                    "Available cash insufficient for planned fees; \
                     shortfall carried into the next period's fee claim."
                );
            }

            let unpaid_principal =
                planned_scheduled_principal[idx] - s.scheduled_principal.amount().max(0.0);
            if unpaid_principal > MONEY_TOLERANCE {
                let currency = s.scheduled_principal.currency();
                shortfalls.insert(
                    format!("principal::{}", s.instrument_id),
                    Money::new(unpaid_principal, currency),
                );
                warnings.push(EvalWarning::CapitalStructureCashflowIgnored {
                    period: *_period_id,
                    kind: format!(
                        "principal_shortfall(instrument={}, amount={unpaid_principal:.4})",
                        s.instrument_id
                    ),
                    cashflow_date: _period_id.to_string(),
                });
                tracing::warn!(
                    instrument = s.instrument_id.as_str(),
                    shortfall = unpaid_principal,
                    period = _period_id.to_string(),
                    "Available cash insufficient for planned scheduled principal; \
                     shortfall carried into the next period's amortization claim."
                );
            }
        }

        // Whatever cash survives the priority stack flows to equity.
        equity_distribution = Some(remaining_cash);
    }

    // --- Step 6: Period close ---
    //
    // For each instrument:
    //   (a) principal_payment = scheduled + extra, capped at the payable
    //       balance (opening + in-period draws). If the cap truncates the sum,
    //       reduce `extra_principal` first (discretionary sweep is netted
    //       before scheduled amortization) so downstream accounting stays
    //       consistent.
    //   (b) post_sweep_balance = opening + draws - principal_payment (with a
    //       small dust floor to avoid micro-residuals). The draw term keeps a
    //       revolver's in-period funding from being wiped at close.
    //   (c) PIK capitalization bookkeeping: the coupon was already moved into
    //       the PIK bucket in Step 4b when the toggle is active. The moved
    //       amount is accumulated into `state.cumulative_toggled_pik` so the
    //       period-flow scale clamp can exclude toggle-driven compounding.
    //       PIK interest accrues on the pre-waterfall opening balance and
    //       capitalizes at period close even when the principal was fully
    //       paid down during the period: the coupon still economically
    //       exists and gets rolled into the closing balance.
    //   (d) closing_balance = post_sweep_balance + PIK capitalized.
    //   (e) accrued_interest: cleared to zero when PIK capitalization
    //       absorbed the contractual coupon into principal, or when the
    //       debt was paid off and no further contractual accrual applies.
    //       Otherwise the field retains the contractual pre-waterfall
    //       accrual. Any cash shortfall from Step 5 is then added on top
    //       and carried into the next period's interest claim.
    for s in staged {
        let StagedInstrumentFlow {
            instrument_id,
            mut breakdown,
            opening_balance,
            net_new_funding,
            extra_principal,
            scheduled_principal,
            toggled_pik_moved,
        } = s;
        let currency = breakdown.interest_expense_cash.currency();

        // (a) Principal cap. The payable balance is the opening balance plus
        // any in-period draws (a revolver can repay against cash it just drew).
        // `extra_principal` (the discretionary sweep bucket) is netted against
        // any overshoot before scheduled amortization is reduced, so the
        // aggregate `principal_payment` is never > payable_balance.
        let payable_balance = (opening_balance.amount() + net_new_funding.amount()).max(0.0);
        let desired = scheduled_principal.checked_add(extra_principal)?;
        let principal_payment = if desired.amount() > payable_balance {
            Money::new(payable_balance, currency)
        } else {
            desired
        };
        breakdown.principal_payment = principal_payment;

        // (b) Post-payment balance = opening + draws - principal. Including the
        // draw term is what preserves in-period funding: recomputing closing as
        // `opening - principal` would silently wipe a revolver's new draws.
        let post_pay_amount =
            opening_balance.amount() + net_new_funding.amount() - principal_payment.amount();
        // Dust floor: collapse sub-cent residuals on full paydown. Currency
        // agnostic fallback; modelers in JPY should override via explicit
        // rounding upstream.
        let post_sweep_balance = if post_pay_amount.abs() < 0.005 {
            Money::new(0.0, currency)
        } else {
            Money::new(post_pay_amount, currency)
        };
        let fully_paid = post_sweep_balance.amount() == 0.0;

        // (c) PIK bookkeeping at close. The coupon was already moved into the
        // PIK bucket in Step 4b whenever the toggle is active; here the moved
        // amount is accumulated into state so the period-flow scale clamp can
        // exclude toggle-driven compounding from its basis.
        let pik_capitalized_this_step = is_pik_enabled(state, &instrument_id);
        if toggled_pik_moved.amount() != 0.0 {
            let current = state
                .cumulative_toggled_pik
                .get(instrument_id.as_str())
                .copied()
                .unwrap_or_else(|| Money::new(0.0, currency));
            state.cumulative_toggled_pik.insert(
                instrument_id.clone(),
                current.checked_add(toggled_pik_moved)?,
            );
        }

        // (d) Closing balance. PIK capitalizes into the post-sweep balance.
        let closing_balance = post_sweep_balance.checked_add(breakdown.interest_expense_pik)?;
        state.set_closing_balance(instrument_id.to_string(), closing_balance);
        breakdown.debt_balance = closing_balance;

        // (e) Accrued interest bookkeeping after waterfall mutation.
        // The pre-waterfall `accrued_interest` was the contractual schedule's
        // accrual. It is cleared when the coupon was moved into PIK (the
        // accrual has been capitalized into principal) or when the debt was
        // fully paid off and there is no remaining balance to accrue on.
        if fully_paid || pik_capitalized_this_step {
            breakdown.accrued_interest = Money::new(0.0, currency);
        }

        // Unpaid interest from the available-cash cap accrues and is carried as
        // a claim in the next period's interest category.
        if let Some(shortfall) = shortfalls.get(instrument_id.as_str()) {
            breakdown.accrued_interest = breakdown.accrued_interest.checked_add(*shortfall)?;
            state
                .interest_shortfall
                .insert(instrument_id.clone(), *shortfall);
        }
        if let Some(shortfall) = shortfalls.get(format!("principal::{instrument_id}").as_str()) {
            state
                .principal_shortfall
                .insert(instrument_id.clone(), *shortfall);
        }
        // Unpaid fees are carried in their own bucket so they re-enter the fee
        // category next period rather than being demoted into interest.
        if let Some(shortfall) = shortfalls.get(format!("fees::{instrument_id}").as_str()) {
            state
                .fee_shortfall
                .insert(instrument_id.clone(), *shortfall);
        }

        breakdown.validate_currency_invariant().map_err(|e| {
            crate::error::Error::capital_structure(format!(
                "Currency invariant violated after waterfall mutation for {instrument_id}: {e}"
            ))
        })?;

        update_cumulative_metrics(state, &instrument_id, &breakdown, currency)?;

        result.insert(instrument_id.to_string(), breakdown);
    }

    Ok(WaterfallPeriodResult {
        flows: result,
        warnings,
        equity_distribution,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capital_structure::{
        CapitalStructureState, CashflowBreakdown, EcfSweepSpec, PaymentPriority, PikToggleSpec,
        WaterfallSpec,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::PeriodId;
    use finstack_quant_core::money::Money;
    use indexmap::IndexMap;

    fn build_context(period: PeriodId, values: &[(&str, f64)]) -> EvaluationContext {
        let mut node_to_column = IndexMap::new();
        for (idx, (name, _)) in values.iter().enumerate() {
            node_to_column.insert(crate::types::NodeId::new(*name), idx);
        }
        let mut ctx = EvaluationContext::new(
            period,
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
        let period = PeriodId::quarter(2025, 1);
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
        tl_breakdown.principal_payment = Money::new(100_000.0, Currency::USD);
        contractual_flows.insert("TL-1".to_string(), tl_breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::new(10_000_000.0, Currency::USD));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: None,
            ecf_sweep: Some(EcfSweepSpec {
                ebitda_node: "ebitda".into(),
                taxes_node: Some("taxes".into()),
                capex_node: Some("capex".into()),
                working_capital_node: None,
                cash_interest_node: None,
                sweep_percentage: 0.5,
                target_instrument_id: Some("TL-1".into()),
            }),
            pik_toggle: None,
        };

        let results = execute_waterfall(
            &period,
            &context,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
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
        assert_eq!(
            state
                .cumulative_principal
                .get("TL-1")
                .expect("cumulative principal")
                .amount(),
            425_000.0
        );
    }

    /// A non-finite `available_cash_node` must be a hard error, not a silent
    /// zero. `NaN.max(0.0)` evaluates to 0.0, which would short every creditor
    /// against an empty cash pool while reporting only downstream shortfall
    /// warnings — no diagnostic pointing at the broken formula.
    #[test]
    fn available_cash_node_rejects_non_finite_value() {
        let period = PeriodId::quarter(2025, 1);
        // `0 / 0` is NaN under the DSL's documented division-by-zero policy.
        let context = build_context(period, &[("cash", 0.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_expense_cash = Money::new(100.0, Currency::USD);
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::new(10_000.0, Currency::USD));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            available_cash_node: Some("cash / 0".into()),
            ecf_sweep: None,
            pik_toggle: None,
        };

        let err = execute_waterfall(
            &period,
            &context,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect_err("non-finite available cash must error rather than coerce to zero");
        let msg = err.to_string();
        assert!(
            msg.contains("non-finite") && msg.contains("cash / 0"),
            "error must name the offending expression: {msg}"
        );
    }

    /// A finite-but-astronomical cash value must error rather than panic.
    /// `Money::new` asserts a `rust_decimal`-representable amount (~7.9e28) and
    /// panics beyond it, which would abort evaluation from ordinary model data.
    #[test]
    fn available_cash_node_rejects_amount_beyond_decimal_range() {
        let period = PeriodId::quarter(2025, 1);
        let context = build_context(period, &[("cash", 1e30)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_expense_cash = Money::new(100.0, Currency::USD);
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::new(10_000.0, Currency::USD));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            available_cash_node: Some("cash".into()),
            ecf_sweep: None,
            pik_toggle: None,
        };

        let err = execute_waterfall(
            &period,
            &context,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
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
    fn inline_expression_warnings_are_not_dropped() {
        let period = PeriodId::quarter(2025, 1);
        let context = build_context(period, &[("cash", 500.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_expense_cash = Money::new(100.0, Currency::USD);
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::new(10_000.0, Currency::USD));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            // Guarded: the division by zero is swallowed by `coalesce`, so the
            // node evaluates to a perfectly finite 500 and the finiteness check
            // cannot catch it. Only the warning reveals the broken arithmetic.
            available_cash_node: Some("coalesce(cash / 0, cash)".into()),
            ecf_sweep: None,
            pik_toggle: None,
        };

        let result = execute_waterfall(
            &period,
            &context,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect("a guarded expression still evaluates");

        assert!(
            result
                .warnings
                .iter()
                .any(|w| matches!(w, EvalWarning::DivisionByZero { .. })),
            "the division inside the inline expression must be surfaced: {:?}",
            result.warnings
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
        let period = PeriodId::quarter(2025, 1);
        let context = build_context(period, &[("cash", 1_000.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_expense_cash = Money::new(100.0, Currency::USD);
        // A caller-built breakdown with one leg in the wrong currency.
        breakdown.interest_expense_pik = Money::new(50.0, Currency::EUR);
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::new(10_000.0, Currency::USD));

        // A firing PIK toggle reaches `interest_expense_pik += moved` in Step
        // 4b, where `Money`'s asserting `AddAssign` aborts on the mismatch.
        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            available_cash_node: Some("cash".into()),
            ecf_sweep: None,
            pik_toggle: Some(PikToggleSpec {
                liquidity_metric: "cash".into(),
                threshold: 1_000_000.0, // always below => PIK on
                min_periods_in_pik: 0,
                target_instrument_ids: Some(vec!["TL-1".to_string()]),
            }),
        };

        let err = execute_waterfall(
            &period,
            &context,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
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
        let period = PeriodId::quarter(2025, 1);
        let context = build_context(period, &[("liquidity", 0.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_expense_cash = Money::new(100.0, Currency::USD);
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::new(10_000.0, Currency::USD));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![PaymentPriority::Interest, PaymentPriority::Equity],
            available_cash_node: None,
            ecf_sweep: None,
            pik_toggle: Some(PikToggleSpec {
                liquidity_metric: "liquidity / 0".into(),
                threshold: 1.0,
                min_periods_in_pik: 0,
                target_instrument_ids: Some(vec!["TL-1".to_string()]),
            }),
        };

        let err = execute_waterfall(
            &period,
            &context,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect_err("non-finite PIK metric must error rather than read as 'not triggered'");
        assert!(
            err.to_string().contains("non-finite"),
            "error must flag the non-finite metric: {err}"
        );
    }

    #[test]
    fn test_ecf_sweep_deducts_fees_before_sweep_percentage() {
        let period = PeriodId::quarter(2025, 1);
        let context = build_context(period, &[("ebitda", 1_000.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.fees = Money::new(100.0, Currency::USD);
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::new(10_000.0, Currency::USD));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: None,
            ecf_sweep: Some(EcfSweepSpec {
                ebitda_node: "ebitda".into(),
                taxes_node: None,
                capex_node: None,
                working_capital_node: None,
                cash_interest_node: None,
                sweep_percentage: 0.5,
                target_instrument_id: Some("TL-1".into()),
            }),
            pik_toggle: None,
        };

        let result = execute_waterfall(
            &period,
            &context,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect("waterfall should execute");

        let tl = result.flows.get("TL-1").expect("TL-1");
        assert_eq!(
            tl.principal_payment.amount(),
            450.0,
            "fees paid ahead of sweep reduce ECF before the sweep percentage: (1000 - 100) * 50%"
        );
    }

    #[test]
    fn test_pik_toggle_updates_state() {
        let period = PeriodId::quarter(2025, 2);
        let mut context = build_context(period, &[("liquidity", 50.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        contractual_flows.insert(
            "TL-PIK".to_string(),
            CashflowBreakdown::with_currency(Currency::USD),
        );

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-PIK".to_string(), Money::new(5_000_000.0, Currency::USD));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: None,
            ecf_sweep: None,
            pik_toggle: Some(PikToggleSpec {
                liquidity_metric: "liquidity".into(),
                threshold: 100.0,
                target_instrument_ids: Some(vec!["TL-PIK".into()]),
                min_periods_in_pik: 0,
            }),
        };

        execute_waterfall(
            &period,
            &context,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect("waterfall should execute");

        assert_eq!(state.pik_mode.get("TL-PIK"), Some(&true));

        context
            .set_value("liquidity", 150.0)
            .expect("should update liquidity for second evaluation");
        execute_waterfall(
            &period,
            &context,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect("waterfall should execute");

        assert_eq!(state.pik_mode.get("TL-PIK"), Some(&false));
    }

    #[test]
    fn test_pik_hysteresis_holds_pik_active_for_min_periods() {
        let period = PeriodId::quarter(2025, 1);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut tl_breakdown = CashflowBreakdown::with_currency(Currency::USD);
        tl_breakdown.interest_expense_cash = Money::new(10_000.0, Currency::USD);
        contractual_flows.insert("TL-PIK".to_string(), tl_breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-PIK".to_string(), Money::new(5_000_000.0, Currency::USD));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: None,
            ecf_sweep: None,
            pik_toggle: Some(PikToggleSpec {
                liquidity_metric: "liquidity".into(),
                threshold: 100.0,
                target_instrument_ids: Some(vec!["TL-PIK".into()]),
                min_periods_in_pik: 3,
            }),
        };

        // Period 1: liquidity < threshold => PIK activates
        let ctx_low = build_context(period, &[("liquidity", 50.0)]);
        execute_waterfall(
            &period,
            &ctx_low,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect("waterfall should execute");

        assert_eq!(state.pik_mode.get("TL-PIK"), Some(&true));
        assert_eq!(state.pik_periods_active.get("TL-PIK"), Some(&1));

        // Period 2: liquidity recovers above threshold, but hysteresis holds PIK
        let ctx_high = build_context(period, &[("liquidity", 150.0)]);
        execute_waterfall(
            &period,
            &ctx_high,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect("waterfall should execute");

        assert_eq!(
            state.pik_mode.get("TL-PIK"),
            Some(&true),
            "PIK should remain active due to hysteresis (periods_active=1 < 3)"
        );
        assert_eq!(state.pik_periods_active.get("TL-PIK"), Some(&2));

        // Period 3: still above threshold, hysteresis still holds (periods_active=2 < 3)
        execute_waterfall(
            &period,
            &ctx_high,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect("waterfall should execute");

        assert_eq!(
            state.pik_mode.get("TL-PIK"),
            Some(&true),
            "PIK should remain active due to hysteresis (periods_active=2 < 3)"
        );
        assert_eq!(state.pik_periods_active.get("TL-PIK"), Some(&3));

        // Period 4: min_periods met (periods_active=3, which is not < 3), PIK releases
        execute_waterfall(
            &period,
            &ctx_high,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect("waterfall should execute");

        assert_eq!(
            state.pik_mode.get("TL-PIK"),
            Some(&false),
            "PIK should release after min_periods_in_pik completed"
        );
        assert_eq!(
            state.pik_periods_active.get("TL-PIK"),
            Some(&0),
            "counter should reset on PIK exit"
        );
    }

    #[test]
    fn test_execute_waterfall_conserves_sweep_across_multiple_instruments() {
        let period = PeriodId::quarter(2025, 1);
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
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::new(200_000.0, Currency::USD));
        state
            .opening_balances
            .insert("TL-2".to_string(), Money::new(300_000.0, Currency::USD));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: None,
            ecf_sweep: Some(EcfSweepSpec {
                ebitda_node: "ebitda".into(),
                taxes_node: Some("taxes".into()),
                capex_node: Some("capex".into()),
                working_capital_node: None,
                cash_interest_node: None,
                sweep_percentage: 0.5,
                target_instrument_id: None,
            }),
            pik_toggle: None,
        };

        let results = execute_waterfall(
            &period,
            &context,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
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
    fn test_priority_of_payments_changes_pik_sweep_order() {
        let period = PeriodId::quarter(2025, 1);
        let context = build_context(period, &[("ebitda", 2_100.0), ("liquidity", 50.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_expense_cash = Money::new(100.0, Currency::USD);
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut sweep_first_state = CapitalStructureState::new();
        sweep_first_state
            .opening_balances
            .insert("TL-1".to_string(), Money::new(1_000.0, Currency::USD));
        let mut interest_first_state = sweep_first_state.clone();

        let sweep_first = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Sweep,
                PaymentPriority::Interest,
                PaymentPriority::Equity,
            ],
            available_cash_node: None,
            ecf_sweep: Some(EcfSweepSpec {
                ebitda_node: "ebitda".into(),
                taxes_node: None,
                capex_node: None,
                working_capital_node: None,
                cash_interest_node: None,
                sweep_percentage: 0.5,
                target_instrument_id: Some("TL-1".into()),
            }),
            pik_toggle: Some(PikToggleSpec {
                liquidity_metric: "liquidity".into(),
                threshold: 100.0,
                target_instrument_ids: Some(vec!["TL-1".into()]),
                min_periods_in_pik: 0,
            }),
        };
        let interest_first = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Interest,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: None,
            ecf_sweep: sweep_first.ecf_sweep.clone(),
            pik_toggle: sweep_first.pik_toggle.clone(),
        };

        let sweep_first_result = execute_waterfall(
            &period,
            &context,
            &sweep_first,
            &mut sweep_first_state,
            &contractual_flows,
        )
        .expect("waterfall should execute");
        let interest_first_result = execute_waterfall(
            &period,
            &context,
            &interest_first,
            &mut interest_first_state,
            &contractual_flows,
        )
        .expect("waterfall should execute");

        let sweep_first_balance = sweep_first_result.flows["TL-1"].debt_balance.amount();
        let interest_first_balance = interest_first_result.flows["TL-1"].debt_balance.amount();
        assert_eq!(sweep_first_balance, 100.0);
        assert_eq!(interest_first_balance, 100.0);
    }

    #[test]
    fn test_available_cash_caps_scheduled_payments_by_priority() {
        let period = PeriodId::quarter(2025, 1);
        let context = build_context(period, &[("cash_available", 150.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.fees = Money::new(20.0, Currency::USD);
        breakdown.interest_expense_cash = Money::new(100.0, Currency::USD);
        breakdown.principal_payment = Money::new(200.0, Currency::USD);
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::new(1_000.0, Currency::USD));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: Some("cash_available".into()),
            ecf_sweep: None,
            pik_toggle: None,
        };

        let results = execute_waterfall(
            &period,
            &context,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect("waterfall should execute");

        let tl = results.flows.get("TL-1").expect("TL-1");
        assert_eq!(tl.fees.amount(), 20.0);
        assert_eq!(tl.interest_expense_cash.amount(), 100.0);
        assert_eq!(tl.principal_payment.amount(), 30.0);
    }

    // Regression (C4): over-amortization (scheduled principal > opening balance)
    // must not destroy cash. Before the fix, Step 5 deducted the full planned
    // scheduled principal from the available-cash pool while Step 6 clamped the
    // paid principal to the opening balance, so `fees + interest + principal +
    // equity` came out < available cash. This asserts strict conservation.
    #[test]
    fn test_over_amortization_conserves_available_cash() {
        let period = PeriodId::quarter(2025, 1);
        let available = 1_000.0;
        let context = build_context(period, &[("cash_available", available)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        // Scheduled principal (300) exceeds the opening balance (200).
        breakdown.principal_payment = Money::new(300.0, Currency::USD);
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::new(200.0, Currency::USD));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            available_cash_node: Some("cash_available".into()),
            ecf_sweep: None,
            pik_toggle: None,
        };

        let results = execute_waterfall(
            &period,
            &context,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect("waterfall should execute");

        let tl = results.flows.get("TL-1").expect("TL-1");
        // Principal is clamped to the opening balance (can't repay more than owed).
        assert_eq!(tl.principal_payment.amount(), 200.0);
        let equity = results.equity_distribution.expect("equity populated");
        // Conservation: uses == sources.
        let uses = tl.fees.amount()
            + tl.interest_expense_cash.amount()
            + tl.principal_payment.amount()
            + equity.amount();
        assert!(
            (uses - available).abs() < 1e-6,
            "waterfall must conserve cash: uses={uses} != available={available}"
        );
    }

    // Regression (C5): a revolver with an in-period draw must keep the drawn
    // balance at close. Before the fix, Step 6 recomputed closing as
    // `opening - principal` (no draw term), wiping the draw to zero.
    #[test]
    fn test_revolver_in_period_draw_preserved_at_close() {
        let period = PeriodId::quarter(2025, 1);
        let context = build_context(period, &[]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        // In-period repayment of 30k against a freshly-drawn balance.
        breakdown.principal_payment = Money::new(30_000.0, Currency::USD);
        contractual_flows.insert("REVOLVER".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        // Fully-swept revolver: opening balance is zero, but it draws 100k this
        // period (net new funding recorded by the contractual pass).
        state
            .opening_balances
            .insert("REVOLVER".to_string(), Money::new(0.0, Currency::USD));
        state
            .period_new_funding
            .insert("REVOLVER".to_string(), Money::new(100_000.0, Currency::USD));

        let waterfall = WaterfallSpec::default();

        let results = execute_waterfall(
            &period,
            &context,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect("waterfall should execute");

        let rev = results.flows.get("REVOLVER").expect("REVOLVER");
        // closing = opening(0) + draws(100k) - principal(30k) = 70k.
        assert_eq!(rev.debt_balance.amount(), 70_000.0);
        assert_eq!(
            state
                .get_closing_balance("REVOLVER", Currency::USD)
                .amount(),
            70_000.0
        );
    }

    #[test]
    fn test_sweep_before_amortization_does_not_produce_negative_balance() {
        let period = PeriodId::quarter(2025, 1);
        let context = build_context(period, &[("ebitda", 5_000.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.principal_payment = Money::new(300.0, Currency::USD);
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::new(500.0, Currency::USD));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Sweep,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            available_cash_node: None,
            ecf_sweep: Some(EcfSweepSpec {
                ebitda_node: "ebitda".into(),
                taxes_node: None,
                capex_node: None,
                working_capital_node: None,
                cash_interest_node: None,
                sweep_percentage: 1.0,
                target_instrument_id: Some("TL-1".into()),
            }),
            pik_toggle: None,
        };

        let results = execute_waterfall(
            &period,
            &context,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
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
        let period = PeriodId::quarter(2025, 1);
        let context = build_context(
            period,
            &[("ebitda", 1_000.0), ("taxes", 100.0), ("capex", 50.0)],
        );

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_expense_cash = Money::new(200.0, Currency::USD);
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::new(10_000.0, Currency::USD));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: None,
            ecf_sweep: Some(EcfSweepSpec {
                ebitda_node: "ebitda".into(),
                taxes_node: Some("taxes".into()),
                capex_node: Some("capex".into()),
                working_capital_node: None,
                cash_interest_node: None,
                sweep_percentage: 0.5,
                target_instrument_id: Some("TL-1".into()),
            }),
            pik_toggle: None,
        };

        let results = execute_waterfall(
            &period,
            &context,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect("waterfall should execute");

        let tl = results.flows.get("TL-1").expect("TL-1");
        assert_eq!(tl.principal_payment.amount(), 325.0);
    }

    #[test]
    fn test_ecf_negative_cash_interest_does_not_reduce_ecf() {
        let period = PeriodId::quarter(2025, 1);
        let context = build_context(
            period,
            &[("ebitda", 1_000.0), ("taxes", 100.0), ("capex", 50.0)],
        );

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_expense_cash = Money::new(-200.0, Currency::USD);
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::new(10_000.0, Currency::USD));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: None,
            ecf_sweep: Some(EcfSweepSpec {
                ebitda_node: "ebitda".into(),
                taxes_node: Some("taxes".into()),
                capex_node: Some("capex".into()),
                working_capital_node: None,
                cash_interest_node: None,
                sweep_percentage: 0.5,
                target_instrument_id: Some("TL-1".into()),
            }),
            pik_toggle: None,
        };

        let results = execute_waterfall(
            &period,
            &context,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect("waterfall should execute");

        // ECF = 1000 - 100 - 50 - max(0, -200) = 1000 - 100 - 50 - 0 = 850
        // Sweep = 850 * 0.5 = 425
        let tl = results.flows.get("TL-1").expect("TL-1");
        assert_eq!(tl.principal_payment.amount(), 425.0);
    }

    #[test]
    fn test_scheduled_amortization_exceeding_balance_is_clamped() {
        let period = PeriodId::quarter(2025, 1);
        let context = build_context(period, &[]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.principal_payment = Money::new(300.0, Currency::USD);
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::new(200.0, Currency::USD));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![PaymentPriority::Amortization, PaymentPriority::Equity],
            available_cash_node: None,
            ecf_sweep: None,
            pik_toggle: None,
        };

        let results = execute_waterfall(
            &period,
            &context,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect("waterfall should execute");

        let tl = results.flows.get("TL-1").expect("TL-1");
        assert_eq!(
            tl.principal_payment.amount(),
            200.0,
            "principal should be clamped to opening balance"
        );
        assert_eq!(
            tl.debt_balance.amount(),
            0.0,
            "balance should be zero, not negative"
        );
    }

    /// Unpaid interest under an available-cash cap must not evaporate: it
    /// accrues, raises a structured warning, and is carried as a claim in the
    /// next period's interest category.
    #[test]
    fn test_cash_shortfall_accrues_and_carries_forward() {
        let period = PeriodId::quarter(2025, 1);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_expense_cash = Money::new(100.0, Currency::USD);
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::new(1_000.0, Currency::USD));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: Some("cash_available".into()),
            ecf_sweep: None,
            pik_toggle: None,
        };

        // Period 1: only 60 of cash against a 100 coupon.
        let ctx_short = build_context(period, &[("cash_available", 60.0)]);
        let result_short = execute_waterfall(
            &period,
            &ctx_short,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect("waterfall should execute");

        let tl = result_short.flows.get("TL-1").expect("TL-1");
        assert_eq!(tl.interest_expense_cash.amount(), 60.0);
        assert_eq!(
            tl.accrued_interest.amount(),
            40.0,
            "unpaid coupon must accrue"
        );
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
        let result_recovered = execute_waterfall(
            &period,
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
        let period = PeriodId::quarter(2025, 1);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.principal_payment = Money::new(100.0, Currency::USD);
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::new(1_000.0, Currency::USD));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: Some("cash_available".into()),
            ecf_sweep: None,
            pik_toggle: None,
        };

        let ctx_short = build_context(period, &[("cash_available", 60.0)]);
        let result_short = execute_waterfall(
            &period,
            &ctx_short,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect("waterfall should execute");

        let tl = result_short.flows.get("TL-1").expect("TL-1");
        assert_eq!(tl.principal_payment.amount(), 60.0);
        assert_eq!(
            state
                .principal_shortfall
                .get("TL-1")
                .expect("principal shortfall recorded")
                .amount(),
            40.0
        );
        assert!(
            result_short
                .warnings
                .iter()
                .any(|warning| matches!(warning, EvalWarning::CapitalStructureCashflowIgnored { kind, .. } if kind.starts_with("principal_shortfall"))),
            "principal shortfall must surface a structured warning"
        );

        state.advance_period();
        let ctx_recovered = build_context(period, &[("cash_available", 200.0)]);
        let result_recovered = execute_waterfall(
            &period,
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
        let period = PeriodId::quarter(2025, 1);
        let context = build_context(period, &[("cash_available", 1_000.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.fees = Money::new(20.0, Currency::USD);
        breakdown.interest_expense_cash = Money::new(100.0, Currency::USD);
        breakdown.principal_payment = Money::new(200.0, Currency::USD);
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::new(5_000.0, Currency::USD));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: Some("cash_available".into()),
            ecf_sweep: None,
            pik_toggle: None,
        };

        let result = execute_waterfall(
            &period,
            &context,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
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

    /// With a priority stack lacking any prepayment entry, a PIK'd coupon must
    /// still be moved into the PIK bucket BEFORE the cash caps: it consumes no
    /// cash and the full contractual coupon capitalizes.
    #[test]
    fn test_pik_coupon_never_consumes_cash_without_prepayment_priority() {
        let period = PeriodId::quarter(2025, 1);
        let context = build_context(period, &[("cash_available", 50.0), ("liquidity", 10.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.interest_expense_cash = Money::new(100.0, Currency::USD);
        contractual_flows.insert("TL-PIK".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-PIK".to_string(), Money::new(1_000.0, Currency::USD));

        // No MandatoryPrepayment/VoluntaryPrepayment/Sweep in the stack.
        // (Amortization is listed because `available_cash_node` is set; it caps
        // to zero here since the instrument has no scheduled principal.)
        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            available_cash_node: Some("cash_available".into()),
            ecf_sweep: None,
            pik_toggle: Some(PikToggleSpec {
                liquidity_metric: "liquidity".into(),
                threshold: 100.0,
                target_instrument_ids: Some(vec!["TL-PIK".into()]),
                min_periods_in_pik: 0,
            }),
        };

        let result = execute_waterfall(
            &period,
            &context,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect("waterfall should execute");

        let tl = result.flows.get("TL-PIK").expect("TL-PIK");
        assert_eq!(
            tl.interest_expense_cash.amount(),
            0.0,
            "PIK'd coupon must not consume cash"
        );
        assert_eq!(
            tl.interest_expense_pik.amount(),
            100.0,
            "full contractual coupon must capitalize"
        );
        assert_eq!(tl.debt_balance.amount(), 1_100.0);
        assert_eq!(
            result
                .equity_distribution
                .expect("equity populated")
                .amount(),
            50.0,
            "cash untouched by the PIK'd coupon flows to equity"
        );
        assert_eq!(
            state
                .cumulative_toggled_pik
                .get("TL-PIK")
                .expect("toggled PIK tracked")
                .amount(),
            100.0
        );
    }

    #[test]
    fn test_sweep_plus_amortization_exceeding_balance_is_clamped() {
        let period = PeriodId::quarter(2025, 1);
        let context = build_context(period, &[("ebitda", 10_000.0)]);

        let mut contractual_flows: IndexMap<String, CashflowBreakdown> = IndexMap::new();
        let mut breakdown = CashflowBreakdown::with_currency(Currency::USD);
        breakdown.principal_payment = Money::new(500.0, Currency::USD);
        contractual_flows.insert("TL-1".to_string(), breakdown);

        let mut state = CapitalStructureState::new();
        state
            .opening_balances
            .insert("TL-1".to_string(), Money::new(400.0, Currency::USD));

        let waterfall = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Amortization,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            available_cash_node: None,
            ecf_sweep: Some(EcfSweepSpec {
                ebitda_node: "ebitda".into(),
                taxes_node: None,
                capex_node: None,
                working_capital_node: None,
                cash_interest_node: None,
                sweep_percentage: 1.0,
                target_instrument_id: Some("TL-1".into()),
            }),
            pik_toggle: None,
        };

        let results = execute_waterfall(
            &period,
            &context,
            &waterfall,
            &mut state,
            &contractual_flows,
        )
        .expect("waterfall should execute");

        let tl = results.flows.get("TL-1").expect("TL-1");
        assert_eq!(
            tl.principal_payment.amount(),
            400.0,
            "principal should be clamped to opening balance"
        );
        assert_eq!(
            tl.debt_balance.amount(),
            0.0,
            "balance should be zero after full paydown"
        );
    }
}
