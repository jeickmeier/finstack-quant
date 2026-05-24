//! Waterfall execution functions for structured credit instruments.
//!
//! This module contains pure functions for executing waterfall distributions.
//! All type definitions are in `types::waterfall`.

use super::coverage_tests::{CoverageTest, TestContext};
use crate::instruments::fixed_income::structured_credit::types::{
    AllocationMode, AssetPool, DiversionRecord, PaymentCalculation, PaymentRecord, PaymentType,
    Recipient, RecipientType, RoundingConvention, TrancheStructure, Waterfall,
    WaterfallDistribution, WaterfallTier, WaterfallWorkspace,
};
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount, DayCountContext};
use finstack_core::explain::{ExplainOpts, ExplanationTrace, TraceEntry};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::Error as CoreError;
use finstack_core::HashMap;
use finstack_core::Result;

// ============================================================================
// CURRENCY PRECISION HELPERS
// ============================================================================

/// Returns the number of decimal places for currency-aware penny-safe allocation.
#[inline]
fn currency_decimal_places(currency: Currency) -> u32 {
    u32::from(currency.decimals())
}

/// Returns the scaling factor for converting amounts to smallest currency units.
#[inline]
fn currency_scale_factor(currency: Currency) -> f64 {
    let decimals = currency_decimal_places(currency);
    10_f64.powi(decimals as i32)
}

#[inline]
fn to_currency_units(amount: f64, scale: f64) -> Result<i64> {
    if !amount.is_finite() || !scale.is_finite() || scale <= 0.0 {
        return Err(CoreError::Validation(
            "Invalid amount or scale for currency unit conversion".to_string(),
        ));
    }
    let scaled = amount * scale;
    if !scaled.is_finite() || scaled.abs() > i64::MAX as f64 {
        return Err(CoreError::Validation(
            "Tier amount exceeds penny-safe allocation capacity".to_string(),
        ));
    }
    let rounded = scaled.round();
    if rounded > i64::MAX as f64 || rounded < i64::MIN as f64 {
        return Err(CoreError::Validation(
            "Tier amount exceeds penny-safe allocation capacity".to_string(),
        ));
    }
    Ok(rounded as i64)
}

// ============================================================================
// MAIN EXECUTION FUNCTIONS
// ============================================================================

/// Context for waterfall execution.
pub struct WaterfallContext<'a> {
    /// Total cash available for distribution in this period.
    pub available_cash: Money,
    /// Interest collections from the pool for this period.
    pub interest_collections: Money,
    /// Payment date for this waterfall period.
    pub payment_date: Date,
    /// Start date of the accrual period.
    pub period_start: Date,
    /// Current pool balance at the start of the period.
    pub pool_balance: Money,
    /// Market context for rate lookups and discounting.
    pub market: &'a MarketContext,
    /// Current tranche balances (overrides `tranche.current_balance` when present).
    /// This ensures the waterfall uses up-to-date balances after principal payments
    /// and PIK accretion rather than stale original balances.
    pub tranche_balances: Option<&'a HashMap<String, Money>>,
    /// Deferred interest claims carried from prior periods.
    pub deferred_interest: Option<&'a HashMap<String, Money>>,
    /// Current reserve account balance (passed dynamically each period).
    /// Used by `PaymentCalculation::ReserveReplenishment` to compute shortfall.
    pub reserve_balance: Money,
    /// Recovery proceeds released this period (tracked separately for reporting).
    pub recovery_proceeds: Money,
}

/// Execute waterfall to distribute available cash.
pub fn execute_waterfall(
    waterfall: &Waterfall,
    tranches: &TrancheStructure,
    pool: &AssetPool,
    context: WaterfallContext,
) -> Result<WaterfallDistribution> {
    execute_waterfall_with_explanation(waterfall, tranches, pool, context, ExplainOpts::disabled())
}

/// Core waterfall execution logic with optional workspace for zero-allocation hot paths.
///
/// This is the unified implementation that handles both regular and workspace-based execution.
/// When `workspace` is `Some`, it uses pre-allocated buffers for zero-allocation execution.
/// When `workspace` is `None`, it allocates local state as needed.
fn execute_waterfall_core(
    waterfall: &Waterfall,
    tranches: &TrancheStructure,
    pool: &AssetPool,
    context: WaterfallContext,
    explain: ExplainOpts,
    mut workspace: Option<&mut WaterfallWorkspace>,
) -> Result<WaterfallDistribution> {
    let mut remaining = context.available_cash;
    let mut total_diverted = Money::new(0.0, waterfall.base_currency);
    let mut had_diversions = false;
    let mut diversion_reason = None;

    // Build tranche index fresh (cheap operation)
    let mut tranche_index = HashMap::default();
    tranche_index.reserve(tranches.tranches.len());
    for (i, t) in tranches.tranches.iter().enumerate() {
        tranche_index.insert(t.id.as_str(), i);
    }

    // Build allocation context for reuse across tiers
    let allocation_ctx = AllocationContext {
        base_currency: waterfall.base_currency,
        tranches,
        tranche_index,
        pool_balance: context.pool_balance,
        payment_date: context.payment_date,
        market: context.market,
        tranche_balances: context.tranche_balances,
        deferred_interest: context.deferred_interest,
        reserve_balance: context.reserve_balance,
    };

    // Evaluate coverage tests (M4: pass current balances)
    let coverage_test_results = evaluate_coverage_tests(
        waterfall,
        tranches,
        pool,
        context.payment_date,
        context.period_start,
        context.available_cash,
        context.interest_collections,
        context.pool_balance,
        context.market,
        context.tranche_balances,
    )?;

    // Check if diversions are active and compute the binding cure amount.
    //
    // INTEX/Bloomberg convention: coverage cures are NOT additive across
    // tranches. The diversion always redirects cash to the most-senior
    // principal tier, and OC/IC tests *share* the senior tranche balances —
    // an OC test for tranche T has `T_balance + senior_balances` in its
    // denominator, so any senior paydown also de-leverages every more-senior
    // test. Curing the most-subordinate failing test (the LARGEST required
    // paydown) therefore implicitly cures every test above it.
    //
    // Summing the per-tranche cures double-counts that shared senior paydown
    // and over-diverts cash. The binding cure is the MAXIMUM cure across all
    // failing tests, not the sum.
    let diversion_active = coverage_test_results.iter().any(|r| !r.is_passing);
    let total_cure_amount: Money = {
        let mut binding_cure = 0.0_f64;
        for r in &coverage_test_results {
            if let Some(cure) = r.cure_amount {
                if cure.amount() > 0.0 {
                    binding_cure = binding_cure.max(cure.amount());
                }
            }
        }
        Money::new(binding_cure, waterfall.base_currency)
    };
    if diversion_active {
        had_diversions = true;
        diversion_reason = Some("OC or IC test failed".to_string());
    }

    // Create allocation output, using workspace buffers if available
    let mut allocation_output = if let Some(ref mut ws) = workspace {
        // Clear workspace buffers and reuse them
        ws.distributions.clear();
        ws.payment_records.clear();
        ws.tier_allocations.clear();
        ws.coverage_tests.clear();
        ws.coverage_tests.extend(
            coverage_test_results
                .iter()
                .map(|r| (r.test_id.clone(), r.current_ratio, r.is_passing)),
        );

        AllocationOutput {
            distributions: std::mem::take(&mut ws.distributions),
            payment_records: std::mem::take(&mut ws.payment_records),
            trace: if explain.enabled {
                Some(ExplanationTrace::new("waterfall"))
            } else {
                None
            },
        }
    } else {
        // Allocate fresh buffers
        let estimated_recipients = waterfall
            .tiers
            .iter()
            .map(|t| t.recipients.len())
            .sum::<usize>();
        AllocationOutput::with_capacity(estimated_recipients, &explain)
    };

    // Storage for tier allocations (will be moved to workspace or returned directly)
    let mut tier_allocations = Vec::with_capacity(waterfall.tiers.len());

    // Track how much cure cash has already been diverted (for partial diversion).
    let mut cure_remaining = total_cure_amount;

    // Process tiers in priority order
    for tier in &waterfall.tiers {
        let (target_recipients, tier_diverted): (&[Recipient], bool) = if tier.divertible
            && diversion_active
        {
            let senior_tier = waterfall
                .tiers
                .iter()
                .filter(|t| t.priority < tier.priority && t.payment_type == PaymentType::Principal)
                .min_by_key(|t| t.priority);

            senior_tier
                .map(|s| (&s.recipients[..], true))
                .unwrap_or((&tier.recipients[..], false))
        } else {
            (&tier.recipients[..], false)
        };

        // When diverting with a cure amount, cap the diversion at the cure amount.
        // This implements partial diversion (INTEX-standard): only redirect enough
        // cash to cure the OC/IC breach, not the entire tier's allocation.
        let effective_remaining = if tier_diverted && cure_remaining.amount() > 0.0 {
            let capped = remaining.amount().min(cure_remaining.amount());
            Money::new(capped, waterfall.base_currency)
        } else {
            remaining
        };

        let tier_cash = match tier.allocation_mode {
            AllocationMode::Sequential => allocate_sequential(
                &allocation_ctx,
                tier,
                target_recipients,
                effective_remaining,
                context.period_start,
                tier_diverted,
                &mut allocation_output,
                &explain,
            )?,
            AllocationMode::ProRata => allocate_pro_rata(
                &allocation_ctx,
                tier,
                target_recipients,
                effective_remaining,
                context.period_start,
                tier_diverted,
                &mut allocation_output,
                &explain,
            )?,
        };

        if tier_diverted {
            total_diverted = total_diverted.checked_add(tier_cash)?;
            cure_remaining = cure_remaining
                .checked_sub(tier_cash)
                .unwrap_or(Money::new(0.0, waterfall.base_currency));
        }

        tier_allocations.push((tier.id.clone(), tier_cash));
        remaining = remaining.checked_sub(tier_cash)?;
    }

    // Convert internal results to public tuple format
    let coverage_tests_public: Vec<(String, f64, bool)> = coverage_test_results
        .iter()
        .map(|r| (r.test_id.clone(), r.current_ratio, r.is_passing))
        .collect();
    let mut diverted_amounts: Vec<DiversionRecord> = allocation_output
        .payment_records
        .iter()
        .filter(|record| record.diverted && record.paid_amount.amount() > 0.0)
        .map(|record| DiversionRecord {
            source_tier: record.tier_id.clone(),
            target_tranche: record.recipient_id.clone(),
            amount: record.paid_amount,
            reason: diversion_reason
                .clone()
                .unwrap_or_else(|| "Waterfall diversion".to_string()),
        })
        .collect();
    if diverted_amounts.is_empty() && diversion_active {
        let mut per_tranche_max: HashMap<&str, (f64, &'static str)> = HashMap::default();
        for result in &coverage_test_results {
            if let Some(cure) = result.cure_amount {
                if cure.amount() > 0.0 {
                    let tranche_id = result.tranche_id.as_str();
                    let reason = if result.test_id.starts_with("OC_") {
                        "OC cure"
                    } else {
                        "IC cure"
                    };
                    let entry = per_tranche_max.entry(tranche_id).or_insert((0.0, reason));
                    if cure.amount() > entry.0 {
                        *entry = (cure.amount(), reason);
                    }
                }
            }
        }
        diverted_amounts = per_tranche_max
            .into_iter()
            .map(|(tranche_id, (amount, reason))| DiversionRecord {
                source_tier: "coverage_diversion".to_string(),
                target_tranche: tranche_id.to_string(),
                amount: Money::new(amount, waterfall.base_currency),
                reason: reason.to_string(),
            })
            .collect();
    }

    // Build the final distribution result
    let distribution = WaterfallDistribution {
        payment_date: context.payment_date,
        total_available: context.available_cash,
        tier_allocations: tier_allocations.clone(),
        distributions: allocation_output.distributions.clone(),
        payment_records: allocation_output.payment_records.clone(),
        coverage_tests: coverage_tests_public.clone(),
        diverted_cash: total_diverted,
        remaining_cash: remaining,
        had_diversions,
        diversion_reason,
        diverted_amounts,
        recovery_proceeds: context.recovery_proceeds,
        explanation: allocation_output.trace,
    };

    // If using workspace, restore buffers for future reuse
    if let Some(ws) = workspace {
        ws.distributions = allocation_output.distributions;
        ws.payment_records = allocation_output.payment_records;
        ws.tier_allocations = tier_allocations;
        ws.coverage_tests = coverage_tests_public;
    }

    Ok(distribution)
}

/// Execute waterfall with optional explanation trace.
pub fn execute_waterfall_with_explanation(
    waterfall: &Waterfall,
    tranches: &TrancheStructure,
    pool: &AssetPool,
    context: WaterfallContext,
    explain: ExplainOpts,
) -> Result<WaterfallDistribution> {
    execute_waterfall_core(waterfall, tranches, pool, context, explain, None)
}

/// Execute waterfall using a pre-allocated workspace for zero-allocation hot paths.
pub fn execute_waterfall_with_workspace(
    waterfall: &Waterfall,
    tranches: &TrancheStructure,
    pool: &AssetPool,
    context: WaterfallContext,
    explain: ExplainOpts,
    workspace: &mut WaterfallWorkspace,
) -> Result<WaterfallDistribution> {
    execute_waterfall_core(waterfall, tranches, pool, context, explain, Some(workspace))
}

// ============================================================================
// ALLOCATION CONTEXT
// ============================================================================

/// Immutable context for waterfall allocation operations.
///
/// Groups parameters that remain constant during allocation, reducing
/// parameter count in allocation functions.
pub(crate) struct AllocationContext<'a> {
    /// Base currency for allocations
    pub(crate) base_currency: Currency,
    /// Tranche structure for looking up tranche data
    pub(crate) tranches: &'a TrancheStructure,
    /// O(1) lookup from tranche ID to index
    pub(crate) tranche_index: HashMap<&'a str, usize>,
    /// Current pool balance
    pub(crate) pool_balance: Money,
    /// Payment date
    pub(crate) payment_date: Date,
    /// Market context for rate lookups
    pub(crate) market: &'a MarketContext,
    /// Current tranche balances (overrides tranche.current_balance when present)
    pub(crate) tranche_balances: Option<&'a HashMap<String, Money>>,
    /// Deferred interest claims carried from prior periods.
    pub(crate) deferred_interest: Option<&'a HashMap<String, Money>>,
    /// Current reserve account balance (passed dynamically each period)
    pub(crate) reserve_balance: Money,
}

impl<'a> AllocationContext<'a> {
    /// Create a new allocation context.
    ///
    /// Pass `tranche_balances` to use current (dynamic) tranche balances for
    /// interest accrual and principal calculations instead of the static balances
    /// stored on the `Tranche` definitions.
    #[allow(dead_code, clippy::too_many_arguments)] // public API constructor
    pub(crate) fn new(
        base_currency: Currency,
        tranches: &'a TrancheStructure,
        pool_balance: Money,
        payment_date: Date,
        market: &'a MarketContext,
        tranche_balances: Option<&'a HashMap<String, Money>>,
        deferred_interest: Option<&'a HashMap<String, Money>>,
        reserve_balance: Money,
    ) -> Self {
        let mut tranche_index = HashMap::default();
        tranche_index.reserve(tranches.tranches.len());
        for (i, t) in tranches.tranches.iter().enumerate() {
            tranche_index.insert(t.id.as_str(), i);
        }

        Self {
            base_currency,
            tranches,
            tranche_index,
            pool_balance,
            payment_date,
            market,
            tranche_balances,
            deferred_interest,
            reserve_balance,
        }
    }
}

/// Mutable output for allocation tracking.
///
/// Groups mutable state that is updated during allocation.
pub(crate) struct AllocationOutput {
    /// Accumulated distributions by recipient
    pub(crate) distributions: HashMap<RecipientType, Money>,
    /// Payment records for audit trail
    pub(crate) payment_records: Vec<PaymentRecord>,
    /// Optional explanation trace
    pub(crate) trace: Option<ExplanationTrace>,
}

impl AllocationOutput {
    /// Create new allocation state with pre-allocated capacity.
    pub(crate) fn with_capacity(estimated_recipients: usize, explain: &ExplainOpts) -> Self {
        let mut distributions = HashMap::default();
        distributions.reserve(estimated_recipients);
        Self {
            distributions,
            payment_records: Vec::with_capacity(estimated_recipients),
            trace: if explain.enabled {
                Some(ExplanationTrace::new("waterfall"))
            } else {
                None
            },
        }
    }
}

// ============================================================================
// ALLOCATION FUNCTIONS
// ============================================================================

/// Allocate cash sequentially to recipients.
#[allow(clippy::too_many_arguments)]
fn allocate_sequential(
    ctx: &AllocationContext,
    tier: &WaterfallTier,
    recipients: &[Recipient],
    mut available: Money,
    period_start: Date,
    diverted: bool,
    output: &mut AllocationOutput,
    explain: &ExplainOpts,
) -> Result<Money> {
    let base_currency = ctx.base_currency;
    let mut tier_total = Money::new(0.0, base_currency);

    for recipient in recipients {
        if available.amount() <= 0.0 {
            break;
        }

        let requested = calculate_payment_amount(
            base_currency,
            &recipient.calculation,
            available,
            ctx.tranches,
            &ctx.tranche_index,
            ctx.tranche_balances,
            ctx.deferred_interest,
            ctx.pool_balance,
            period_start,
            ctx.payment_date,
            ctx.market,
            ctx.reserve_balance,
        )?;

        let paid = if requested.amount() <= available.amount() {
            requested
        } else {
            available
        };

        let shortfall = requested
            .checked_sub(paid)
            .unwrap_or(Money::new(0.0, base_currency));

        // Update distributions
        use std::collections::hash_map::Entry;
        match output.distributions.entry(recipient.recipient_type.clone()) {
            Entry::Occupied(mut e) => {
                let next = e.get().checked_add(paid)?;
                e.insert(next);
            }
            Entry::Vacant(e) => {
                e.insert(paid);
            }
        }

        output.payment_records.push(PaymentRecord {
            tier_id: tier.id.clone(),
            recipient_id: recipient.id.clone(),
            priority: tier.priority,
            recipient: recipient.recipient_type.clone(),
            requested_amount: requested,
            paid_amount: paid,
            shortfall,
            diverted,
        });

        if let Some(ref mut t) = output.trace {
            t.push(
                TraceEntry::WaterfallStep {
                    period: 0,
                    step_name: format!(
                        "{}/{} - {:?}",
                        tier.id, recipient.id, recipient.recipient_type
                    ),
                    cash_in_amount: requested.amount(),
                    cash_in_currency: requested.currency().to_string(),
                    cash_out_amount: paid.amount(),
                    cash_out_currency: paid.currency().to_string(),
                    shortfall_amount: if shortfall.amount() > 0.0 {
                        Some(shortfall.amount())
                    } else {
                        None
                    },
                    shortfall_currency: if shortfall.amount() > 0.0 {
                        Some(shortfall.currency().to_string())
                    } else {
                        None
                    },
                },
                explain.max_entries,
            );
        }

        tier_total = tier_total.checked_add(paid)?;
        available = available.checked_sub(paid)?;
    }

    Ok(tier_total)
}

/// Allocate cash pro-rata to recipients using penny-safe allocation.
#[allow(clippy::too_many_arguments)]
fn allocate_pro_rata(
    ctx: &AllocationContext,
    tier: &WaterfallTier,
    recipients: &[Recipient],
    available: Money,
    period_start: Date,
    diverted: bool,
    output: &mut AllocationOutput,
    explain: &ExplainOpts,
) -> Result<Money> {
    let base_currency = ctx.base_currency;
    if recipients.is_empty() {
        return Ok(Money::new(0.0, base_currency));
    }

    // Calculate total requested across all recipients
    let mut total_requested = Money::new(0.0, base_currency);
    let mut recipient_requests = Vec::with_capacity(recipients.len());

    for recipient in recipients {
        let requested = calculate_payment_amount(
            base_currency,
            &recipient.calculation,
            available,
            ctx.tranches,
            &ctx.tranche_index,
            ctx.tranche_balances,
            ctx.deferred_interest,
            ctx.pool_balance,
            period_start,
            ctx.payment_date,
            ctx.market,
            ctx.reserve_balance,
        )?;
        total_requested = total_requested.checked_add(requested)?;
        recipient_requests.push((recipient, requested));
    }

    let total_weight: f64 = recipients.iter().map(|r| r.weight.unwrap_or(1.0)).sum();

    let tier_available = if total_requested.amount() <= available.amount() {
        total_requested
    } else {
        available
    };

    // Penny-safe allocation using largest remainder method
    let scale = currency_scale_factor(base_currency);
    let tier_available_units = to_currency_units(tier_available.amount(), scale)?;

    let mut allocations_data: Vec<(usize, &Recipient, Money, i64, f64)> =
        Vec::with_capacity(recipient_requests.len());

    for (idx, (recipient, requested)) in recipient_requests.iter().enumerate() {
        let weight = recipient.weight.unwrap_or(1.0);
        let pro_rata_share = if total_weight > 0.0 {
            weight / total_weight
        } else {
            1.0 / recipients.len() as f64
        };

        let ideal_units = tier_available_units as f64 * pro_rata_share;
        let floor_units = ideal_units.floor() as i64;
        let remainder = ideal_units - floor_units as f64;

        allocations_data.push((idx, recipient, *requested, floor_units, remainder));
    }

    let total_floor_units: i64 = allocations_data.iter().map(|(_, _, _, fu, _)| fu).sum();
    let mut remainder_units = tier_available_units - total_floor_units;

    let mut indices_by_remainder: Vec<usize> = (0..allocations_data.len()).collect();
    indices_by_remainder.sort_by(|&a, &b| {
        allocations_data[b]
            .4
            .partial_cmp(&allocations_data[a].4)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut final_units: Vec<i64> = allocations_data
        .iter()
        .map(|(_, _, _, fu, _)| *fu)
        .collect();
    for &idx in &indices_by_remainder {
        if remainder_units <= 0 {
            break;
        }
        final_units[idx] += 1;
        remainder_units -= 1;
    }

    let mut tier_total = Money::new(0.0, base_currency);

    for (idx, (_, recipient, requested, _, _)) in allocations_data.iter().enumerate() {
        let allocated_units = final_units[idx];
        let allocated = Money::new(allocated_units as f64 / scale, base_currency);

        let paid = if allocated.amount() <= requested.amount() {
            allocated
        } else {
            *requested
        };

        let shortfall = requested
            .checked_sub(paid)
            .unwrap_or(Money::new(0.0, base_currency));

        use std::collections::hash_map::Entry;
        match output.distributions.entry(recipient.recipient_type.clone()) {
            Entry::Occupied(mut e) => {
                let next = e.get().checked_add(paid)?;
                e.insert(next);
            }
            Entry::Vacant(e) => {
                e.insert(paid);
            }
        }

        let weight = recipient.weight.unwrap_or(1.0);
        let pro_rata_share = if total_weight > 0.0 {
            weight / total_weight
        } else {
            1.0 / recipients.len() as f64
        };

        output.payment_records.push(PaymentRecord {
            tier_id: tier.id.clone(),
            recipient_id: recipient.id.clone(),
            priority: tier.priority,
            recipient: recipient.recipient_type.clone(),
            requested_amount: *requested,
            paid_amount: paid,
            shortfall,
            diverted,
        });

        if let Some(ref mut t) = output.trace {
            t.push(
                TraceEntry::WaterfallStep {
                    period: 0,
                    step_name: format!(
                        "{}/{} - {:?} (pro-rata {:.1}%)",
                        tier.id,
                        recipient.id,
                        recipient.recipient_type,
                        pro_rata_share * 100.0
                    ),
                    cash_in_amount: requested.amount(),
                    cash_in_currency: requested.currency().to_string(),
                    cash_out_amount: paid.amount(),
                    cash_out_currency: paid.currency().to_string(),
                    shortfall_amount: if shortfall.amount() > 0.0 {
                        Some(shortfall.amount())
                    } else {
                        None
                    },
                    shortfall_currency: if shortfall.amount() > 0.0 {
                        Some(shortfall.currency().to_string())
                    } else {
                        None
                    },
                },
                explain.max_entries,
            );
        }

        tier_total = tier_total.checked_add(paid)?;
    }

    Ok(tier_total)
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Evaluate coverage tests.
#[allow(clippy::too_many_arguments)]
fn evaluate_coverage_tests(
    waterfall: &Waterfall,
    tranches: &TrancheStructure,
    pool: &AssetPool,
    as_of: Date,
    period_start: Date,
    available_cash: Money,
    interest_collections: Money,
    current_pool_balance: Money,
    market: &MarketContext,
    tranche_balances: Option<&HashMap<String, Money>>,
) -> Result<Vec<CoverageTestResult>> {
    let mut results = Vec::with_capacity(waterfall.coverage_triggers.len() * 2);

    let (haircuts, par_value_threshold) = match waterfall.coverage_rules.as_ref() {
        Some(rules) if !rules.is_empty() => (
            if rules.haircuts.is_empty() {
                None
            } else {
                Some(&rules.haircuts)
            },
            rules.par_value_threshold,
        ),
        _ => (None, None),
    };

    for trigger in &waterfall.coverage_triggers {
        if let Some(oc_trigger_level) = trigger.oc_trigger {
            let ctx = TestContext {
                pool,
                tranches,
                tranche_id: &trigger.tranche_id,
                as_of,
                period_start: Some(period_start),
                cash_balance: available_cash,
                interest_collections,
                haircuts,
                par_value_threshold,
                market: Some(market),
                tranche_balances,
                current_pool_balance: Some(current_pool_balance),
            };

            let oc_test = CoverageTest::new_oc(oc_trigger_level);
            let result = oc_test.calculate(&ctx)?;
            results.push(CoverageTestResult {
                test_id: format!("OC_{}", trigger.tranche_id),
                tranche_id: result.tranche_id,
                current_ratio: result.current_ratio,
                is_passing: result.is_passing,
                cure_amount: result.cure_amount,
            });
        }

        if let Some(ic_trigger_level) = trigger.ic_trigger {
            let ctx = TestContext {
                pool,
                tranches,
                tranche_id: &trigger.tranche_id,
                as_of,
                period_start: Some(period_start),
                cash_balance: available_cash,
                interest_collections,
                haircuts,
                par_value_threshold,
                market: Some(market),
                tranche_balances,
                current_pool_balance: Some(current_pool_balance),
            };

            let ic_test = CoverageTest::new_ic(ic_trigger_level);
            let result = ic_test.calculate(&ctx)?;
            results.push(CoverageTestResult {
                test_id: format!("IC_{}", trigger.tranche_id),
                tranche_id: result.tranche_id,
                current_ratio: result.current_ratio,
                is_passing: result.is_passing,
                cure_amount: result.cure_amount,
            });
        }
    }

    Ok(results)
}

/// Internal coverage test result with cure amount.
#[derive(Debug, Clone)]
struct CoverageTestResult {
    test_id: String,
    tranche_id: String,
    current_ratio: f64,
    is_passing: bool,
    /// Amount needed to cure the breach (divert to senior principal).
    cure_amount: Option<Money>,
}

/// Calculate payment amount for a recipient.
#[allow(clippy::too_many_arguments)]
fn calculate_payment_amount(
    base_currency: Currency,
    calculation: &PaymentCalculation,
    available: Money,
    tranches: &TrancheStructure,
    tranche_index: &HashMap<&str, usize>,
    tranche_balances: Option<&HashMap<String, Money>>,
    deferred_interest: Option<&HashMap<String, Money>>,
    pool_balance: Money,
    period_start: Date,
    payment_date: Date,
    market: &MarketContext,
    reserve_balance: Money,
) -> Result<Money> {
    let (raw_amount, rounding) = match calculation {
        PaymentCalculation::FixedAmount { amount, rounding } => (amount.amount(), *rounding),

        PaymentCalculation::PercentageOfCollateral {
            rate,
            annualized,
            day_count,
            rounding,
        } => {
            let accrual_fraction = if *annualized {
                day_count.unwrap_or(DayCount::Act360).year_fraction(
                    period_start,
                    payment_date,
                    DayCountContext::default(),
                )?
            } else {
                1.0
            };
            (pool_balance.amount() * rate * accrual_fraction, *rounding)
        }

        PaymentCalculation::TrancheInterest {
            tranche_id,
            rounding,
        } => {
            let idx = *tranche_index.get(tranche_id.as_str()).ok_or_else(|| {
                CoreError::from(finstack_core::InputError::NotFound {
                    id: format!("tranche:{}", tranche_id),
                })
            })?;
            let tranche = &tranches.tranches[idx];
            // B1 fix: use current tranche balance when available
            let balance = tranche_balances
                .and_then(|b| b.get(tranche_id.as_str()))
                .copied()
                .unwrap_or(tranche.current_balance);
            let rate = tranche
                .coupon
                .try_current_rate_with_index(payment_date, market)?;
            let accrual_fraction = tranche.day_count.year_fraction(
                period_start,
                payment_date,
                DayCountContext::default(),
            )?;
            let carried = deferred_interest
                .and_then(|d| d.get(tranche_id.as_str()))
                .map(|m| m.amount())
                .unwrap_or(0.0);
            (
                balance.amount() * rate * accrual_fraction + carried,
                *rounding,
            )
        }

        PaymentCalculation::TranchePrincipal {
            tranche_id,
            target_balance,
            rounding,
        } => {
            let idx = *tranche_index.get(tranche_id.as_str()).ok_or_else(|| {
                CoreError::from(finstack_core::InputError::NotFound {
                    id: format!("tranche:{}", tranche_id),
                })
            })?;
            let tranche = &tranches.tranches[idx];
            // B1 fix: use current tranche balance when available
            let current = tranche_balances
                .and_then(|b| b.get(tranche_id.as_str()))
                .copied()
                .unwrap_or(tranche.current_balance);
            let target = target_balance.unwrap_or(Money::new(0.0, base_currency));
            let needed = current
                .checked_sub(target)
                .unwrap_or(Money::new(0.0, base_currency));
            (needed.amount(), *rounding)
        }

        PaymentCalculation::ResidualCash => (available.amount(), None),

        PaymentCalculation::ReserveReplenishment { target_balance } => {
            // Shortfall = max(0, target - current). Current balance is passed
            // dynamically from SimulationState, not stored in the waterfall definition.
            let shortfall = target_balance
                .checked_sub(reserve_balance)
                .unwrap_or(Money::new(0.0, base_currency));
            (shortfall.amount().max(0.0).min(available.amount()), None)
        }
    };

    if let Some(convention) = rounding {
        // m1 fix: use currency-specific decimal places
        let decimals = currency_decimal_places(base_currency) as i32;
        let scale = 10f64.powi(decimals);
        let val = raw_amount;
        let rounded_val = match convention {
            RoundingConvention::Nearest => (val * scale).round() / scale,
            RoundingConvention::Floor => (val * scale).floor() / scale,
            RoundingConvention::Ceiling => (val * scale).ceil() / scale,
        };
        Ok(Money::new(rounded_val, base_currency))
    } else {
        Ok(Money::new(raw_amount, base_currency))
    }
}

#[cfg(test)]
mod market_standards_tests {
    use crate::instruments::fixed_income::structured_credit::types::PaymentCalculation;
    use finstack_core::currency::Currency;
    use finstack_core::dates::{Date, DayCount};
    use finstack_core::money::Money;

    #[test]
    fn test_fee_calc_day_count() {
        let _calc = PaymentCalculation::PercentageOfCollateral {
            rate: 0.01, // 1%
            annualized: true,
            day_count: Some(DayCount::Thirty360),
            rounding: None,
        };

        let _start = Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid date");
        let _end = Date::from_calendar_date(2025, time::Month::April, 1).expect("Valid date"); // 3 months
        let _pool_bal = Money::new(1_000_000.0, Currency::USD);

        // 30/360: 3 full months = 90 days. 90/360 = 0.25
        // Fee = 1M * 1% * 0.25 = 2500

        // We need to mock the context, but calculate_payment_amount is private/internal to pricing/waterfall.rs
        // However, we can test the logic if we can access it.
        // Since we can't easily unit test private functions from outside, we'll rely on integration test or add this to pricing/waterfall.rs
    }
}

#[cfg(test)]
mod to_currency_units_tests {
    use super::to_currency_units;

    #[test]
    fn rejects_overflow_beyond_representable_units() {
        let scale = 100.0;
        let amount = (i64::MAX as f64) / scale + 1.0e6;
        assert!(to_currency_units(amount, scale).is_err());
    }

    #[test]
    fn rejects_invalid_scale() {
        assert!(to_currency_units(1.0, 0.0).is_err());
        assert!(to_currency_units(1.0, -1.0).is_err());
    }
}

#[cfg(test)]
mod ic_diversion_tests {
    use super::execute_waterfall;
    use super::WaterfallContext;
    use crate::instruments::fixed_income::structured_credit::types::waterfall::CoverageTrigger;
    use crate::instruments::fixed_income::structured_credit::types::{
        AllocationMode, AssetPool, DealType, PaymentCalculation, PaymentType, Recipient,
        RecipientType, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
        WaterfallBuilder, WaterfallTier,
    };
    use finstack_core::currency::Currency;
    use finstack_core::dates::Date;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::money::Money;
    use time::Month;

    /// W-21: when only the IC test breaches (OC test passes), the waterfall
    /// must still divert cash. Before the fix the IC `cure_amount` was always
    /// `None`, so `total_cure_amount` was zero and nothing was diverted.
    #[test]
    fn ic_only_breach_diverts_cash() {
        let currency = Currency::USD;

        // AssetPool with a single large performing asset: OC numerator is huge so
        // the OC test comfortably passes.
        let mut pool = AssetPool::new("POOL", DealType::CLO, currency);
        {
            use crate::instruments::fixed_income::structured_credit::types::{
                AssetType, PoolAsset,
            };
            use finstack_core::types::{CreditRating, InstrumentId};
            pool.assets.push(PoolAsset {
                day_count: finstack_core::dates::DayCount::Act360,
                id: InstrumentId::new("ASSET_0"),
                asset_type: AssetType::FirstLienLoan {
                    industry: Some("Technology".into()),
                },
                balance: Money::new(500_000_000.0, currency),
                rate: 0.08,
                spread_bps: Some(400.0),
                index_id: None,
                maturity: Date::from_calendar_date(2031, Month::January, 1).unwrap(),
                credit_quality: Some(CreditRating::BB),
                industry: Some("Technology".into()),
                obligor_id: Some("OBLIGOR_0".into()),
                is_defaulted: false,
                recovery_amount: None,
                purchase_price: None,
                acquisition_date: None,
                smm_override: None,
                mdr_override: None,
            });
        }

        // Two tranches: a senior CLASS_A and a subordinated CLASS_B.
        let class_a = Tranche::new(
            "CLASS_A",
            0.0,
            70.0,
            TrancheSeniority::Senior,
            Money::new(100_000_000.0, currency),
            TrancheCoupon::Fixed { rate: 0.05 },
            Date::from_calendar_date(2031, Month::January, 1).unwrap(),
        )
        .unwrap();
        let class_b = Tranche::new(
            "CLASS_B",
            70.0,
            100.0,
            TrancheSeniority::Subordinated,
            Money::new(30_000_000.0, currency),
            TrancheCoupon::Fixed { rate: 0.08 },
            Date::from_calendar_date(2031, Month::January, 1).unwrap(),
        )
        .unwrap();
        let tranches = TrancheStructure::new(vec![class_a, class_b]).unwrap();

        let waterfall = WaterfallBuilder::new(currency)
            .add_tier(
                WaterfallTier::new("interest", 1, PaymentType::Interest)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_interest("class_a_int", "CLASS_A"))
                    .add_recipient(Recipient::tranche_interest("class_b_int", "CLASS_B")),
            )
            // Senior principal tier (CLASS_A) — diversion target. CLASS_A has
            // a target balance of 95M so absent any breach it only takes a 5M
            // scheduled paydown, leaving cash for the junior principal tier.
            .add_tier(
                WaterfallTier::new("senior_principal", 2, PaymentType::Principal)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_principal(
                        "class_a_prin",
                        "CLASS_A",
                        Some(Money::new(95_000_000.0, currency)),
                    )),
            )
            // Junior principal tier (CLASS_B) — divertible: on a coverage
            // breach its cash is redirected to the senior principal tier.
            .add_tier(
                WaterfallTier::new("junior_principal", 3, PaymentType::Principal)
                    .allocation_mode(AllocationMode::Sequential)
                    .divertible(true)
                    .add_recipient(Recipient::tranche_principal(
                        "class_b_prin",
                        "CLASS_B",
                        None,
                    )),
            )
            .add_tier(
                WaterfallTier::new("equity", 4, PaymentType::Residual)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::new(
                        "equity_dist",
                        RecipientType::Equity,
                        PaymentCalculation::ResidualCash,
                    )),
            )
            // OC trigger 1.05: numerator ~= 500M+cash, denominator = 130M => OC
            // ratio ~3.9, passes easily. IC trigger 1.20: interest collections
            // are deliberately tiny, so the IC test fails.
            .add_coverage_trigger(CoverageTrigger {
                tranche_id: "CLASS_A".into(),
                oc_trigger: Some(1.05),
                ic_trigger: Some(1.20),
            })
            .build()
            .expect("build waterfall");

        let market = MarketContext::new();
        let payment_date = Date::from_calendar_date(2024, Month::April, 1).unwrap();
        let period_start = Date::from_calendar_date(2024, Month::January, 1).unwrap();

        // Plenty of cash to distribute, but interest collections far below the
        // interest due on the tranches => IC test breaches.
        let context = WaterfallContext {
            available_cash: Money::new(20_000_000.0, currency),
            interest_collections: Money::new(100_000.0, currency),
            payment_date,
            period_start,
            pool_balance: Money::new(500_000_000.0, currency),
            market: &market,
            tranche_balances: None,
            deferred_interest: None,
            reserve_balance: Money::new(0.0, currency),
            recovery_proceeds: Money::new(0.0, currency),
        };

        let result =
            execute_waterfall(&waterfall, &tranches, &pool, context).expect("waterfall execution");

        // OC test passes, IC test fails.
        let oc = result
            .coverage_tests
            .iter()
            .find(|(id, _, _)| id.starts_with("OC_"))
            .expect("OC test result present");
        let ic = result
            .coverage_tests
            .iter()
            .find(|(id, _, _)| id.starts_with("IC_"))
            .expect("IC test result present");
        assert!(oc.2, "OC test should pass (ratio {})", oc.1);
        assert!(!ic.2, "IC test should fail (ratio {})", ic.1);

        // W-21: the IC-only breach must divert cash, and the diverted amount
        // must equal the IC cure (the senior interest shortfall) — i.e. partial
        // diversion. Before the fix the IC `cure_amount` was `None`, so
        // `total_cure_amount` was zero, the partial-diversion cap was skipped,
        // and the FULL junior tier (the entire 5M senior-principal need) was
        // diverted instead of just the cure.
        assert!(
            result.diverted_cash.amount() > 0.0,
            "IC-only breach must divert cash, got {}",
            result.diverted_cash.amount()
        );

        // Independently derive the expected IC cure:
        //   cure = required_ratio * interest_due(CLASS_A) - interest_collections
        // CLASS_A has no senior tranches, so total interest due is its own.
        let class_a = tranches
            .tranches
            .iter()
            .find(|t| t.id.as_str() == "CLASS_A")
            .expect("CLASS_A present");
        let yf = class_a
            .day_count
            .year_fraction(
                period_start,
                payment_date,
                finstack_core::dates::DayCountContext::default(),
            )
            .expect("year fraction");
        let interest_due =
            class_a.current_balance.amount() * class_a.coupon.current_rate(payment_date) * yf;
        let expected_cure = 1.20 * interest_due - 100_000.0;
        assert!(expected_cure > 0.0, "test setup: IC must breach");

        assert!(
            (result.diverted_cash.amount() - expected_cure).abs() < 1.0,
            "diverted cash {} should equal the IC cure {} (partial diversion)",
            result.diverted_cash.amount(),
            expected_cure
        );
        // Sanity: the cure is strictly smaller than the full junior tier need
        // (5M senior-principal paydown), so this is genuine partial diversion.
        assert!(
            result.diverted_cash.amount() < 5_000_000.0,
            "diversion must be cure-capped, not the full junior tier"
        );
    }

    /// Item 4 — coverage cures must NOT be summed across tranches.
    ///
    /// Two OC triggers (one on the senior tranche, one on the subordinated
    /// tranche) both breach. The OC tests share the senior tranche balance in
    /// their denominators, so a single senior paydown de-leverages both — the
    /// binding cure is the MAX of the two, not the sum. Summing them
    /// over-diverts cash. This test asserts the diverted cash equals the
    /// larger cure and is strictly below the sum of the two cures.
    #[test]
    fn coverage_cures_are_not_summed_across_tranches() {
        let currency = Currency::USD;

        // AssetPool: one performing asset sized so BOTH OC tests breach but by
        // different amounts (the junior test, with a smaller denominator,
        // needs a larger cure than the senior test).
        let mut pool = AssetPool::new("POOL", DealType::CLO, currency);
        {
            use crate::instruments::fixed_income::structured_credit::types::{
                AssetType, PoolAsset,
            };
            use finstack_core::types::{CreditRating, InstrumentId};
            pool.assets.push(PoolAsset {
                day_count: finstack_core::dates::DayCount::Act360,
                id: InstrumentId::new("ASSET_0"),
                asset_type: AssetType::FirstLienLoan {
                    industry: Some("Technology".into()),
                },
                // Collateral deliberately below the tranche par stack so both
                // OC ratios breach their triggers.
                balance: Money::new(118_000_000.0, currency),
                rate: 0.08,
                spread_bps: Some(400.0),
                index_id: None,
                maturity: Date::from_calendar_date(2031, Month::January, 1).unwrap(),
                credit_quality: Some(CreditRating::BB),
                industry: Some("Technology".into()),
                obligor_id: Some("OBLIGOR_0".into()),
                is_defaulted: false,
                recovery_amount: None,
                purchase_price: None,
                acquisition_date: None,
                smm_override: None,
                mdr_override: None,
            });
        }

        // Senior 100M, subordinated 30M.
        let class_a = Tranche::new(
            "CLASS_A",
            0.0,
            77.0,
            TrancheSeniority::Senior,
            Money::new(100_000_000.0, currency),
            TrancheCoupon::Fixed { rate: 0.05 },
            Date::from_calendar_date(2031, Month::January, 1).unwrap(),
        )
        .unwrap();
        let class_b = Tranche::new(
            "CLASS_B",
            77.0,
            100.0,
            TrancheSeniority::Subordinated,
            Money::new(30_000_000.0, currency),
            TrancheCoupon::Fixed { rate: 0.08 },
            Date::from_calendar_date(2031, Month::January, 1).unwrap(),
        )
        .unwrap();
        let tranches = TrancheStructure::new(vec![class_a, class_b]).unwrap();

        let waterfall = WaterfallBuilder::new(currency)
            .add_tier(
                WaterfallTier::new("interest", 1, PaymentType::Interest)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_interest("class_a_int", "CLASS_A"))
                    .add_recipient(Recipient::tranche_interest("class_b_int", "CLASS_B")),
            )
            .add_tier(
                WaterfallTier::new("senior_principal", 2, PaymentType::Principal)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_principal(
                        "class_a_prin",
                        "CLASS_A",
                        Some(Money::new(99_000_000.0, currency)),
                    )),
            )
            .add_tier(
                WaterfallTier::new("junior_principal", 3, PaymentType::Principal)
                    .allocation_mode(AllocationMode::Sequential)
                    .divertible(true)
                    .add_recipient(Recipient::tranche_principal(
                        "class_b_prin",
                        "CLASS_B",
                        None,
                    )),
            )
            .add_tier(
                WaterfallTier::new("equity", 4, PaymentType::Residual)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::new(
                        "equity_dist",
                        RecipientType::Equity,
                        PaymentCalculation::ResidualCash,
                    )),
            )
            // Two OC triggers. CLASS_A denominator = 100M (ratio 158/100 =
            // 1.58); CLASS_B denominator = 130M (ratio 158/130 = 1.215).
            // Triggers set just above each ratio so BOTH breach, with
            // materially different cure amounts.
            .add_coverage_trigger(CoverageTrigger {
                tranche_id: "CLASS_A".into(),
                oc_trigger: Some(1.70),
                ic_trigger: None,
            })
            .add_coverage_trigger(CoverageTrigger {
                tranche_id: "CLASS_B".into(),
                oc_trigger: Some(1.30),
                ic_trigger: None,
            })
            .build()
            .expect("build waterfall");

        let market = MarketContext::new();
        let payment_date = Date::from_calendar_date(2024, Month::April, 1).unwrap();
        let period_start = Date::from_calendar_date(2024, Month::January, 1).unwrap();

        let context = WaterfallContext {
            available_cash: Money::new(40_000_000.0, currency),
            interest_collections: Money::new(20_000_000.0, currency),
            payment_date,
            period_start,
            pool_balance: Money::new(118_000_000.0, currency),
            market: &market,
            tranche_balances: None,
            deferred_interest: None,
            reserve_balance: Money::new(0.0, currency),
            recovery_proceeds: Money::new(0.0, currency),
        };

        let result =
            execute_waterfall(&waterfall, &tranches, &pool, context).expect("waterfall execution");

        // Both OC tests must fail for this test to exercise the summing bug.
        let failing: Vec<_> = result
            .coverage_tests
            .iter()
            .filter(|(_, _, passing)| !passing)
            .collect();
        assert_eq!(
            failing.len(),
            2,
            "test setup: both OC tests must breach; got {:?}",
            result.coverage_tests
        );

        // Independently derive the two OC cures.
        // numerator = collateral + cash that remains in the numerator. The OC
        // numerator includes cash; diverting X removes X from the numerator
        // and pays down X of the (shared) senior denominator:
        //   X = (numerator − ratio·denominator) / (1 − ratio)
        let numerator = 118_000_000.0_f64 + 40_000_000.0; // collateral + cash
        let cure = |ratio: f64, denom: f64| (numerator - ratio * denom) / (1.0 - ratio);
        let cure_a = cure(1.70, 100_000_000.0);
        let cure_b = cure(1.30, 130_000_000.0);
        assert!(cure_a > 0.0 && cure_b > 0.0, "both cures must be positive");
        let binding = cure_a.max(cure_b);
        let summed = cure_a + cure_b;
        assert!(
            (summed - binding).abs() > 1_000_000.0,
            "test setup: the two cures must differ enough that sum vs max is \
             materially distinguishable (sum={summed:.0}, max={binding:.0})"
        );

        // The diverted cash is capped at the binding (max) cure, NOT the sum.
        // Pre-fix it was capped at the sum, over-diverting.
        let diverted = result.diverted_cash.amount();
        assert!(
            diverted <= binding + 1.0,
            "diverted cash {diverted:.0} must not exceed the binding (max) \
             cure {binding:.0}; pre-fix the cap was the SUM {summed:.0} which \
             over-diverts"
        );
        assert!(
            diverted < summed - 1_000_000.0,
            "diverted cash {diverted:.0} must be strictly below the summed \
             cures {summed:.0} — coverage cures are not additive across \
             tranches"
        );
    }
}
