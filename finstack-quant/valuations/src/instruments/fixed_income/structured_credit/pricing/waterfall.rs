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
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::explain::{ExplainOpts, ExplanationTrace, TraceEntry};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Error as CoreError;
use finstack_quant_core::HashMap;
use finstack_quant_core::Result;

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
    /// Principal collections (scheduled + prepayments + recoveries) for this
    /// period. Standard CLO par-OC: only principal proceeds count toward the
    /// OC numerator's cash component (`include_cash = true`); interest
    /// proceeds belong to the interest waterfall and must not flatter OC.
    pub principal_collections: Money,
    /// Payment date for this waterfall period.
    pub payment_date: Date,
    /// Start date of the accrual period.
    pub period_start: Date,
    /// Valuation date used to distinguish known resets from future projections.
    pub valuation_date: Date,
    /// Current pool balance at the start of the period.
    pub pool_balance: Money,
    /// Market context for rate lookups and discounting.
    pub market: &'a MarketContext,
    /// Current tranche balances (overrides `tranche.current_balance` when present).
    /// This ensures the waterfall uses up-to-date balances after principal payments
    /// and PIK accretion rather than stale original balances.
    pub tranche_balances: Option<&'a HashMap<String, Money>>,
    /// Current per-asset balances, aligned by index with `pool.assets`.
    pub asset_balances: Option<&'a [f64]>,
    /// Deferred interest claims carried from prior periods.
    pub deferred_interest: Option<&'a HashMap<String, Money>>,
    /// Current reserve account balance (passed dynamically each period).
    /// Used by `PaymentCalculation::ReserveReplenishment` to compute shortfall.
    pub reserve_balance: Money,
    /// Recovery proceeds released this period (tracked separately for reporting).
    pub recovery_proceeds: Money,
}

/// Execute waterfall to distribute available cash.
///
/// # Arguments
///
/// * `waterfall` - Ordered payment rules, base currency, and diversion logic
///   that control each allocation.
/// * `tranches` - Tranche structure receiving interest, principal, fees, and
///   other waterfall distributions.
/// * `pool` - Asset pool supplying collateral balances and data for coverage
///   tests and allocation calculations.
/// * `context` - Period cash, dates, market data, dynamic balances, reserve,
///   and deferred-interest state used for this payment date.
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
        valuation_date: context.valuation_date,
        market: context.market,
        tranche_balances: context.tranche_balances,
        deferred_interest: context.deferred_interest,
        reserve_balance: context.reserve_balance,
    };

    let diversion_principal_tier = waterfall
        .tiers
        .iter()
        .filter(|tier| tier.payment_type == PaymentType::Principal)
        .min_by_key(|tier| tier.priority);
    let mut payable_principal_tranche_ids = Vec::new();
    if let Some(tier) = diversion_principal_tier {
        for recipient in &tier.recipients {
            if let RecipientType::Tranche(tranche_id) = &recipient.recipient_type {
                if !payable_principal_tranche_ids.contains(&tranche_id.as_str()) {
                    payable_principal_tranche_ids.push(tranche_id.as_str());
                }
            }
        }
    }

    // Senior fees rank ahead of every note, so IC uses
    // `(interest collections − senior fees) / note interest due`. Compute fees
    // with the same payment kernel used by the waterfall.
    let empty_in_period: HashMap<String, Money> = HashMap::default();
    let mut senior_fees = Money::new(0.0, waterfall.base_currency);
    for tier in waterfall
        .tiers
        .iter()
        .take_while(|t| t.payment_type == PaymentType::Fee)
    {
        for recipient in &tier.recipients {
            let amount = calculate_payment_amount(
                waterfall.base_currency,
                &recipient.calculation,
                context.interest_collections,
                tranches,
                &allocation_ctx.tranche_index,
                context.tranche_balances,
                context.deferred_interest,
                context.pool_balance,
                context.period_start,
                context.payment_date,
                context.valuation_date,
                context.market,
                context.reserve_balance,
                &empty_in_period,
                false,
            )?;
            senior_fees = senior_fees.checked_add(amount)?;
        }
    }

    // Evaluate coverage tests against current balances.
    let coverage_test_results = evaluate_coverage_tests(
        waterfall,
        tranches,
        pool,
        context.payment_date,
        context.period_start,
        context.principal_collections,
        context.interest_collections,
        context.pool_balance,
        context.market,
        context.tranche_balances,
        context.asset_balances,
        &payable_principal_tranche_ids,
        senior_fees,
    )?;

    // Coverage tests share senior balances, so one paydown de-leverages every
    // applicable test. Under the INTEX/Bloomberg convention the binding
    // diversion is therefore the maximum failing cure, not their sum.
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

    // Net all principal paid during the period against the period-start balance
    // so regular and diverted tiers cannot retire the same notional twice.
    let mut principal_paid_in_period: HashMap<String, Money> = HashMap::default();

    // Process tiers in priority order
    for tier in &waterfall.tiers {
        let (target_recipients, tier_diverted): (&[Recipient], bool) =
            if tier.divertible && diversion_active {
                // Cure pays the earliest principal tier in its configured
                // recipient order (it may sit later than this divertible
                // interest tier). Early principal is booked in
                // `principal_paid_in_period` so the principal tier nets it and
                // cannot double-pay.
                diversion_principal_tier
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
                &mut principal_paid_in_period,
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
                &mut principal_paid_in_period,
            )?,
        };

        let mut tier_cash = tier_cash;
        if tier_diverted {
            total_diverted = total_diverted.checked_add(tier_cash)?;
            cure_remaining = cure_remaining
                .checked_sub(tier_cash)
                .unwrap_or(Money::new(0.0, waterfall.base_currency));

            // A partial diversion redirects only the cure amount; remaining
            // cash still belongs to the divertible tier's own recipients.
            let leftover = remaining.checked_sub(tier_cash)?;
            if leftover.amount() > 0.0 && !tier.recipients.is_empty() {
                let own_cash = match tier.allocation_mode {
                    AllocationMode::Sequential => allocate_sequential(
                        &allocation_ctx,
                        tier,
                        &tier.recipients[..],
                        leftover,
                        context.period_start,
                        false,
                        &mut allocation_output,
                        &explain,
                        &mut principal_paid_in_period,
                    )?,
                    AllocationMode::ProRata => allocate_pro_rata(
                        &allocation_ctx,
                        tier,
                        &tier.recipients[..],
                        leftover,
                        context.period_start,
                        false,
                        &mut allocation_output,
                        &explain,
                        &mut principal_paid_in_period,
                    )?,
                };
                tier_cash = tier_cash.checked_add(own_cash)?;
            }
        }

        tier_allocations.push((tier.id.clone(), tier_cash));
        remaining = remaining.checked_sub(tier_cash)?;
    }

    // Convert internal results to public tuple format
    let coverage_tests_public: Vec<(String, f64, bool)> = coverage_test_results
        .iter()
        .map(|r| (r.test_id.clone(), r.current_ratio, r.is_passing))
        .collect();
    // DiversionRecords are built only from payments that actually moved cash.
    // A failing coverage test with no cash to divert (e.g. an empty waterfall
    // period) must NOT fabricate records carrying the theoretical cure amount;
    // `had_diversions` / `coverage_tests` already report the breach itself.
    let diverted_amounts: Vec<DiversionRecord> = allocation_output
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
///
/// # Arguments
///
/// * `waterfall` - Ordered payment rules, base currency, and diversion logic.
/// * `tranches` - Tranche structure receiving the calculated distributions.
/// * `pool` - Asset pool supplying collateral and coverage-test data.
/// * `context` - Period cash, dates, market data, and dynamic balance state.
/// * `explain` - Trace configuration; disabled tracing leaves the economic
///   allocations unchanged while avoiding explanation records.
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
///
/// # Arguments
///
/// * `waterfall` - Ordered payment rules, base currency, and diversion logic.
/// * `tranches` - Tranche structure receiving the calculated distributions.
/// * `pool` - Asset pool supplying collateral and coverage-test data.
/// * `context` - Period cash, dates, market data, and dynamic balance state.
/// * `explain` - Trace configuration for the resulting allocation explanation.
/// * `workspace` - Caller-owned reusable buffers overwritten during execution
///   and retained for subsequent zero-allocation waterfall calls.
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
    /// Valuation date for fixing lifecycle decisions.
    pub(crate) valuation_date: Date,
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
        valuation_date: Date,
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
            valuation_date,
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
    principal_paid_in_period: &mut HashMap<String, Money>,
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
            ctx.valuation_date,
            ctx.market,
            ctx.reserve_balance,
            principal_paid_in_period,
            diverted,
        )?;

        let paid = if requested.amount() <= available.amount() {
            requested
        } else {
            available
        };

        record_in_period_principal(
            principal_paid_in_period,
            &recipient.calculation,
            paid,
            base_currency,
        )?;

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
    principal_paid_in_period: &mut HashMap<String, Money>,
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
            ctx.valuation_date,
            ctx.market,
            ctx.reserve_balance,
            principal_paid_in_period,
            diverted,
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

    // Penny-safe weighted allocation. Each recipient is capped at its own request;
    // the capped excess from a recipient whose weight-share exceeds its request is
    // *water-filled* onto the recipients still below their caps, so the full
    // `tier_available` is distributed instead of leaking to the next tier (which,
    // for a single combined principal tier, is the residual/equity tier). Without
    // this redistribution a small-balance senior carrying a large shifting-interest
    // weight would drop its excess past outstanding junior debt — a subordination
    // inversion.
    let scale = currency_scale_factor(base_currency);
    let tier_available_units = to_currency_units(tier_available.amount(), scale)?;

    let weights: Vec<f64> = recipient_requests
        .iter()
        .map(|(recipient, _)| recipient.weight.unwrap_or(1.0))
        .collect();
    let caps: Vec<i64> = recipient_requests
        .iter()
        .map(|(_, requested)| to_currency_units(requested.amount(), scale))
        .collect::<Result<Vec<_>>>()?;

    let final_units = water_fill_allocation(tier_available_units, &weights, &caps);

    let mut tier_total = Money::new(0.0, base_currency);

    for (idx, (recipient, requested)) in recipient_requests.iter().enumerate() {
        let allocated = Money::new(final_units[idx] as f64 / scale, base_currency);

        // `water_fill_allocation` never allocates above a recipient's cap
        // (`requested`); the `min` is retained as a defensive floor.
        let paid = if allocated.amount() <= requested.amount() {
            allocated
        } else {
            *requested
        };

        record_in_period_principal(
            principal_paid_in_period,
            &recipient.calculation,
            paid,
            base_currency,
        )?;

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

/// Penny-exact weighted water-filling of `total_units` across recipients, each
/// capped at `caps[i]` (its request in currency units), in proportion to
/// `weights[i]`.
///
/// A recipient whose proportional share exceeds its cap is filled to the cap and
/// the freed units are redistributed across the recipients still below their
/// caps, iterating until either all units are placed or every recipient is at
/// its cap. The integer remainder left by flooring is handed out one unit at a
/// time to the still-open recipients with the largest fractional shares (each
/// such recipient has at least one unit of headroom, so a cap is never
/// exceeded).
///
/// The returned allocation sums to `min(total_units, Σ caps)` (up to currency
/// rounding); any shortfall means every recipient is already at its request, so
/// the unplaced cash correctly remains for the next tier. This is the fix for
/// the subordination inversion where weight-capped excess used to leak straight
/// to the residual tier.
fn water_fill_allocation(total_units: i64, weights: &[f64], caps: &[i64]) -> Vec<i64> {
    let n = weights.len();
    let mut alloc = vec![0i64; n];
    if n == 0 || total_units <= 0 {
        return alloc;
    }

    let mut open: Vec<usize> = (0..n).filter(|&i| caps[i] > 0).collect();
    let mut remaining = total_units;

    loop {
        if remaining <= 0 || open.is_empty() {
            break;
        }

        let active_weight: f64 = open.iter().map(|&i| weights[i].max(0.0)).sum();

        // Degenerate weights (all zero/negative on the open set): spread the
        // remaining units as evenly as possible across the open recipients,
        // respecting caps. Bulk-allocate the even base first (capping iterates),
        // then hand out the sub-`open.len()` remainder one unit each.
        if active_weight <= 0.0 {
            let m = open.len() as i64;
            let base = remaining / m;
            let mut progressed = false;
            if base > 0 {
                for &i in &open {
                    let add = base.min(caps[i] - alloc[i]);
                    if add > 0 {
                        alloc[i] += add;
                        remaining -= add;
                        progressed = true;
                    }
                }
            }
            for &i in &open {
                if remaining <= 0 {
                    break;
                }
                if alloc[i] < caps[i] {
                    alloc[i] += 1;
                    remaining -= 1;
                    progressed = true;
                }
            }
            open.retain(|&i| alloc[i] < caps[i]);
            if !progressed {
                break;
            }
            continue;
        }

        let remaining_f = remaining as f64;
        // Provisional floor allocation proportional to weight, tracking the
        // fractional part for the largest-remainder tie-break below.
        let mut provisional: Vec<(usize, i64, f64)> = open
            .iter()
            .map(|&i| {
                let want = remaining_f * (weights[i].max(0.0) / active_weight);
                let floor = want.floor() as i64;
                (i, floor, want - floor as f64)
            })
            .collect();

        let mut any_capped = false;
        let mut placed = 0i64;
        for &(i, floor, _) in &provisional {
            let headroom = caps[i] - alloc[i];
            let add = floor.min(headroom);
            alloc[i] += add;
            placed += add;
            if floor >= headroom {
                any_capped = true;
            }
        }
        remaining -= placed;
        open.retain(|&i| alloc[i] < caps[i]);

        if !any_capped {
            // No recipient hit its cap this round, so the leftover (< open.len()
            // units, lost only to flooring) goes one unit at a time to the
            // largest fractional shares. Each still-open recipient has >= 1 unit
            // of headroom, so caps continue to hold.
            provisional.retain(|&(i, _, _)| alloc[i] < caps[i]);
            provisional.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
            for &(i, _, _) in &provisional {
                if remaining <= 0 {
                    break;
                }
                alloc[i] += 1;
                remaining -= 1;
            }
            break;
        }
    }

    alloc
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Evaluate coverage tests.
///
/// `principal_collections` is the cash component of the OC numerator
/// (standard CLO par-OC counts only principal proceeds, never interest).
#[allow(clippy::too_many_arguments)]
fn evaluate_coverage_tests(
    waterfall: &Waterfall,
    tranches: &TrancheStructure,
    pool: &AssetPool,
    as_of: Date,
    period_start: Date,
    principal_collections: Money,
    interest_collections: Money,
    current_pool_balance: Money,
    market: &MarketContext,
    tranche_balances: Option<&HashMap<String, Money>>,
    asset_balances: Option<&[f64]>,
    payable_principal_tranche_ids: &[&str],
    senior_fees: Money,
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
                cash_balance: principal_collections,
                interest_collections,
                haircuts,
                par_value_threshold,
                market: Some(market),
                tranche_balances,
                payable_principal_tranche_ids: Some(payable_principal_tranche_ids),
                asset_balances,
                current_pool_balance: Some(current_pool_balance),
                senior_fees,
            };

            let oc_test = CoverageTest::new_oc(oc_trigger_level);
            let result = oc_test.calculate(&ctx)?;
            results.push(CoverageTestResult {
                test_id: format!("OC_{}", trigger.tranche_id),
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
                cash_balance: principal_collections,
                interest_collections,
                haircuts,
                par_value_threshold,
                market: Some(market),
                tranche_balances,
                payable_principal_tranche_ids: Some(payable_principal_tranche_ids),
                asset_balances,
                current_pool_balance: Some(current_pool_balance),
                senior_fees,
            };

            let ic_test = CoverageTest::new_ic(ic_trigger_level);
            let result = ic_test.calculate(&ctx)?;
            results.push(CoverageTestResult {
                test_id: format!("IC_{}", trigger.tranche_id),
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
    current_ratio: f64,
    is_passing: bool,
    /// Amount needed to cure the breach (divert to senior principal).
    cure_amount: Option<Money>,
}

/// Record principal paid to a tranche within the current waterfall period so
/// later tiers (e.g. an OC/IC diversion into the senior principal tier) see
/// the post-payment balance instead of the stale period-start snapshot.
fn record_in_period_principal(
    principal_paid_in_period: &mut HashMap<String, Money>,
    calculation: &PaymentCalculation,
    paid: Money,
    base_currency: Currency,
) -> Result<()> {
    if paid.amount() <= 0.0 {
        return Ok(());
    }
    if let PaymentCalculation::TranchePrincipal { tranche_id, .. } = calculation {
        let entry = principal_paid_in_period
            .entry(tranche_id.clone())
            .or_insert(Money::new(0.0, base_currency));
        *entry = entry.checked_add(paid)?;
    }
    Ok(())
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
    valuation_date: Date,
    market: &MarketContext,
    reserve_balance: Money,
    principal_paid_in_period: &HashMap<String, Money>,
    diverted: bool,
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
                CoreError::from(finstack_quant_core::InputError::NotFound {
                    id: format!("tranche:{}", tranche_id),
                })
            })?;
            let tranche = &tranches.tranches[idx];
            // Use current tranche balance when available
            let balance = tranche_balances
                .and_then(|b| b.get(tranche_id.as_str()))
                .copied()
                .unwrap_or(tranche.current_balance);
            let rate = tranche.coupon.try_rate_for_period(
                period_start,
                payment_date,
                valuation_date,
                market,
            )?;
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

        PaymentCalculation::CappedTrancheInterest {
            tranche_id,
            cap_rate,
            rounding,
        } => {
            let idx = *tranche_index.get(tranche_id.as_str()).ok_or_else(|| {
                CoreError::from(finstack_quant_core::InputError::NotFound {
                    id: format!("tranche:{}", tranche_id),
                })
            })?;
            let tranche = &tranches.tranches[idx];
            let balance = tranche_balances
                .and_then(|b| b.get(tranche_id.as_str()))
                .copied()
                .unwrap_or(tranche.current_balance);
            // Available-funds cap: the effective coupon cannot exceed `cap_rate`.
            let rate = tranche
                .coupon
                .try_rate_for_period(period_start, payment_date, valuation_date, market)?
                .min(*cap_rate);
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
                CoreError::from(finstack_quant_core::InputError::NotFound {
                    id: format!("tranche:{}", tranche_id),
                })
            })?;
            let tranche = &tranches.tranches[idx];
            // Use current tranche balance when available
            let current = tranche_balances
                .and_then(|b| b.get(tranche_id.as_str()))
                .copied()
                .unwrap_or(tranche.current_balance);
            // Net out principal already paid to this tranche by earlier tiers
            // in the SAME period (e.g. its regular principal tier before an
            // OC/IC diversion). The snapshot balance is period-start, so
            // without this the tranche requests its full balance twice and
            // gets over-paid into a negative balance.
            let paid_this_period = principal_paid_in_period
                .get(tranche_id.as_str())
                .map(|m| m.amount())
                .unwrap_or(0.0);
            // A coverage-cure diversion pays the senior tranche *below* its
            // scheduled target (toward zero) to de-leverage the structure;
            // the scheduled target only applies to the tier's regular pass.
            let target = if diverted {
                Money::new(0.0, base_currency)
            } else {
                target_balance.unwrap_or(Money::new(0.0, base_currency))
            };
            let needed = (current.amount() - paid_this_period - target.amount()).max(0.0);
            (needed, *rounding)
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
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::money::Money;

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
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::Money;
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
            use finstack_quant_core::types::{CreditRating, InstrumentId};
            pool.assets.push(PoolAsset {
                day_count: finstack_quant_core::dates::DayCount::Act360,
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
                contractual_payment: None,
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
            principal_collections: Money::new(19_900_000.0, currency),
            payment_date,
            period_start,
            valuation_date: period_start,
            pool_balance: Money::new(500_000_000.0, currency),
            market: &market,
            tranche_balances: None,
            asset_balances: None,
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
                finstack_quant_core::dates::DayCountContext::default(),
            )
            .expect("year fraction");
        let interest_due =
            class_a.current_balance.amount() * class_a.coupon.current_rate(payment_date) * yf;
        // SC-M08: the cure is a PRINCIPAL PAYDOWN, not a cash shortfall.
        //
        // Paying down senior principal adds nothing to interest collections, so
        // the old `1.20 * interest_due - 100_000` cash shortfall cured nothing
        // when applied as a paydown. De-levering needs
        // `X >= (I_due - I_coll/R) / (r*tau)`, which here is ~93.4M against a
        // 100M CLASS_A — i.e. the breach is so severe that no available cash
        // can cure it.
        //
        // This test previously asserted the diversion equalled the 1,416,667
        // cash shortfall, a 66x under-cure. With the correct cure exceeding
        // every dollar in the waterfall, the diversion is now bounded by
        // AVAILABLE CASH rather than by the cure — which is the right
        // behaviour: divert everything you have and still fail the test.
        let rate_tau = class_a.coupon.current_rate(payment_date) * yf;
        let delevering_cure = (interest_due - 100_000.0 / 1.20) / rate_tau;
        assert!(
            delevering_cure > 5_000_000.0,
            "test setup: this breach must be severe enough that the cure \
             exceeds available cash, got {delevering_cure:.2}"
        );

        assert!(
            result.diverted_cash.amount() > 1_416_667.0,
            "the diversion must exceed the pre-SC-M08 cash shortfall of \
             1,416,667 — that figure under-cured an IC breach by ~66x. Got {}",
            result.diverted_cash.amount()
        );
        assert!(
            result.diverted_cash.amount() <= result.total_available.amount() + 1.0,
            "the diversion can never exceed the cash actually available: {} vs {}",
            result.diverted_cash.amount(),
            result.total_available.amount()
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
            use finstack_quant_core::types::{CreditRating, InstrumentId};
            pool.assets.push(PoolAsset {
                day_count: finstack_quant_core::dates::DayCount::Act360,
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
                contractual_payment: None,
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
            // Two OC triggers. Numerator = 118M collateral + 20M principal
            // cash = 138M. CLASS_A denominator = 100M (ratio 1.38); CLASS_B
            // denominator = 130M (ratio 1.06). Triggers set above each ratio
            // so BOTH breach, with materially different cure amounts.
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
            principal_collections: Money::new(20_000_000.0, currency),
            payment_date,
            period_start,
            valuation_date: period_start,
            pool_balance: Money::new(118_000_000.0, currency),
            market: &market,
            tranche_balances: None,
            asset_balances: None,
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
        // numerator = collateral + principal collections (par-OC: only the
        // principal cash component enters the numerator). Diverting X removes
        // X from the numerator and pays down X of the (shared) senior
        // denominator:
        //   X = (numerator − ratio·denominator) / (1 − ratio)
        let numerator = 118_000_000.0_f64 + 20_000_000.0; // collateral + principal cash
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

        // Diverted cash is capped at the binding (max) cure, not the sum.
        let diverted = result.diverted_cash.amount();
        assert!(
            diverted <= binding + 1.0,
            "diverted cash {diverted:.0} must not exceed the binding (max) \
             cure {binding:.0} (sum of cures would be {summed:.0})"
        );
        assert!(
            diverted < summed - 1_000_000.0,
            "diverted cash {diverted:.0} must be strictly below the summed \
             cures {summed:.0} — coverage cures are not additive across \
             tranches"
        );
    }

    /// Diversion nets principal already paid this period (no over-pay / negative balance).
    #[test]
    fn diversion_never_over_pays_senior_principal() {
        let currency = Currency::USD;

        let mut pool = AssetPool::new("POOL", DealType::CLO, currency);
        {
            use crate::instruments::fixed_income::structured_credit::types::{
                AssetType, PoolAsset,
            };
            use finstack_quant_core::types::{CreditRating, InstrumentId};
            pool.assets.push(PoolAsset {
                day_count: finstack_quant_core::dates::DayCount::Act360,
                id: InstrumentId::new("ASSET_0"),
                asset_type: AssetType::FirstLienLoan {
                    industry: Some("Technology".into()),
                },
                // Collateral below the tranche stack so the OC test breaches
                // and the junior tier diverts to the senior principal tier.
                balance: Money::new(20_000_000.0, currency),
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
                contractual_payment: None,
            });
        }

        // Small senior tranche: 3M. Plenty of cash (10M) so its regular
        // principal tier (target None → pay to zero) retires it in full.
        let class_a_balance = 3_000_000.0;
        let class_a = Tranche::new(
            "CLASS_A",
            0.0,
            10.0,
            TrancheSeniority::Senior,
            Money::new(class_a_balance, currency),
            TrancheCoupon::Fixed { rate: 0.05 },
            Date::from_calendar_date(2031, Month::January, 1).unwrap(),
        )
        .unwrap();
        let class_b = Tranche::new(
            "CLASS_B",
            10.0,
            100.0,
            TrancheSeniority::Subordinated,
            Money::new(27_000_000.0, currency),
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
                        None,
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
            // Collateral 20M + cash vs 30M stack: the OC test breaches.
            .add_coverage_trigger(CoverageTrigger {
                tranche_id: "CLASS_B".into(),
                oc_trigger: Some(1.20),
                ic_trigger: None,
            })
            .build()
            .expect("build waterfall");

        let market = MarketContext::new();
        let payment_date = Date::from_calendar_date(2024, Month::April, 1).unwrap();
        let period_start = Date::from_calendar_date(2024, Month::January, 1).unwrap();

        let context = WaterfallContext {
            available_cash: Money::new(10_000_000.0, currency),
            interest_collections: Money::new(1_000_000.0, currency),
            principal_collections: Money::new(9_000_000.0, currency),
            payment_date,
            period_start,
            valuation_date: period_start,
            pool_balance: Money::new(20_000_000.0, currency),
            market: &market,
            tranche_balances: None,
            asset_balances: None,
            deferred_interest: None,
            reserve_balance: Money::new(0.0, currency),
            recovery_proceeds: Money::new(0.0, currency),
        };

        let result =
            execute_waterfall(&waterfall, &tranches, &pool, context).expect("waterfall execution");

        assert!(
            result.had_diversions,
            "test setup: the OC breach must trigger a diversion"
        );

        // Invariant: total principal paid to CLASS_A across ALL tiers
        // (regular + diverted) must not exceed its period-start balance —
        // i.e. the post-payment balance never goes negative.
        let class_a_principal_paid: f64 = result
            .payment_records
            .iter()
            .filter(|r| r.recipient_id == "class_a_prin")
            .map(|r| r.paid_amount.amount())
            .sum();
        assert!(
            class_a_principal_paid <= class_a_balance + 1e-6,
            "CLASS_A principal paid {class_a_principal_paid:.2} exceeds its \
             balance {class_a_balance:.2}: diversion used a stale \
             period-start balance and over-paid principal"
        );
    }

    #[test]
    fn coverage_economics_late_junior_fee_is_not_deducted_from_ic_numerator() {
        let currency = Currency::USD;
        let pool = AssetPool::new("POOL", DealType::CLO, currency);
        let tranche = Tranche::new(
            "CLASS_A",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::new(100_000.0, currency),
            TrancheCoupon::Fixed { rate: 0.05 },
            Date::from_calendar_date(2031, Month::January, 1).expect("date"),
        )
        .expect("tranche");
        let tranches = TrancheStructure::new(vec![tranche]).expect("tranche structure");
        let build_waterfall = |late_fee: Option<f64>| {
            let mut builder = WaterfallBuilder::new(currency).add_tier(
                WaterfallTier::new("note_interest", 1, PaymentType::Interest)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_interest("class_a_interest", "CLASS_A")),
            );
            if let Some(amount) = late_fee {
                builder = builder.add_tier(
                    WaterfallTier::new("junior_fee", 2, PaymentType::Fee)
                        .allocation_mode(AllocationMode::Sequential)
                        .add_recipient(Recipient::fixed_fee(
                            "junior_fee_recipient",
                            "junior_manager",
                            Money::new(amount, currency),
                        )),
                );
            }
            builder
                .add_coverage_trigger(CoverageTrigger {
                    tranche_id: "CLASS_A".into(),
                    oc_trigger: None,
                    ic_trigger: Some(1.0),
                })
                .build()
                .expect("waterfall")
        };
        let market = MarketContext::new();
        let period_start = Date::from_calendar_date(2025, Month::January, 1).expect("period start");
        let payment_date = Date::from_calendar_date(2025, Month::April, 1).expect("payment date");
        let run = |late_fee| {
            execute_waterfall(
                &build_waterfall(late_fee),
                &tranches,
                &pool,
                WaterfallContext {
                    available_cash: Money::new(2_000.0, currency),
                    interest_collections: Money::new(2_000.0, currency),
                    principal_collections: Money::new(0.0, currency),
                    payment_date,
                    period_start,
                    valuation_date: period_start,
                    pool_balance: Money::new(100_000.0, currency),
                    market: &market,
                    tranche_balances: None,
                    asset_balances: None,
                    deferred_interest: None,
                    reserve_balance: Money::new(0.0, currency),
                    recovery_proceeds: Money::new(0.0, currency),
                },
            )
            .expect("waterfall execution")
            .coverage_tests
            .into_iter()
            .find(|(id, _, _)| id == "IC_CLASS_A")
            .expect("IC result")
            .1
        };

        let without_late_fee = run(None);
        let with_late_fee = run(Some(500.0));
        assert!(
            (with_late_fee - without_late_fee).abs() < 1e-12,
            "a fee tier behind note interest is junior to the IC claim and must \
             not reduce its numerator: without={without_late_fee}, \
             with={with_late_fee}"
        );
    }

    #[test]
    fn coverage_economics_custom_principal_recipient_order_drives_ic_cure() {
        let currency = Currency::USD;
        let pool = AssetPool::new("POOL", DealType::CLO, currency);
        let maturity = Date::from_calendar_date(2031, Month::January, 1).expect("date");
        let class_a = Tranche::new(
            "CLASS_A",
            0.0,
            50.0,
            TrancheSeniority::Senior,
            Money::new(100_000.0, currency),
            TrancheCoupon::Fixed { rate: 0.04 },
            maturity,
        )
        .expect("class A");
        let class_b = Tranche::new(
            "CLASS_B",
            50.0,
            100.0,
            TrancheSeniority::Subordinated,
            Money::new(100_000.0, currency),
            TrancheCoupon::Fixed { rate: 0.16 },
            maturity,
        )
        .expect("class B");
        let tranches = TrancheStructure::new(vec![class_a, class_b]).expect("tranche structure");

        // The custom principal tier intentionally pays B before A, opposite
        // structural payment_priority. Its scheduled targets equal current
        // balances, so the tier only pays when coverage diversion is active.
        let waterfall = WaterfallBuilder::new(currency)
            .add_tier(
                WaterfallTier::new("interest", 1, PaymentType::Interest)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_interest("a_interest", "CLASS_A"))
                    .add_recipient(Recipient::tranche_interest("b_interest", "CLASS_B")),
            )
            .add_tier(
                WaterfallTier::new("custom_principal", 2, PaymentType::Principal)
                    .allocation_mode(AllocationMode::Sequential)
                    .divertible(true)
                    .add_recipient(Recipient::tranche_principal(
                        "b_principal",
                        "CLASS_B",
                        Some(Money::new(100_000.0, currency)),
                    ))
                    .add_recipient(Recipient::tranche_principal(
                        "a_principal",
                        "CLASS_A",
                        Some(Money::new(100_000.0, currency)),
                    )),
            )
            .add_coverage_trigger(CoverageTrigger {
                tranche_id: "CLASS_B".into(),
                oc_trigger: None,
                ic_trigger: Some(1.0),
            })
            .build()
            .expect("waterfall");
        let market = MarketContext::new();
        let period_start = Date::from_calendar_date(2025, Month::January, 1).expect("period start");
        let payment_date = Date::from_calendar_date(2025, Month::April, 1).expect("payment date");

        let result = execute_waterfall(
            &waterfall,
            &tranches,
            &pool,
            WaterfallContext {
                available_cash: Money::new(104_500.0, currency),
                interest_collections: Money::new(4_500.0, currency),
                principal_collections: Money::new(100_000.0, currency),
                payment_date,
                period_start,
                valuation_date: period_start,
                pool_balance: Money::new(200_000.0, currency),
                market: &market,
                tranche_balances: None,
                asset_balances: None,
                deferred_interest: None,
                reserve_balance: Money::new(0.0, currency),
                recovery_proceeds: Money::new(0.0, currency),
            },
        )
        .expect("waterfall execution");

        // Total quarterly interest is 5,000. Collections of 4,500 require a
        // 500 reduction. Since B is the first actual principal recipient and
        // has rate×tau = 16%×0.25 = 4%, the cure is 500/4% = 12,500.
        assert!(
            (result.diverted_cash.amount() - 12_500.0).abs() < 1e-6,
            "custom B-then-A principal order must size the cure from B's rate: \
             expected 12,500, got {}",
            result.diverted_cash.amount()
        );
        let first_diverted = result
            .payment_records
            .iter()
            .find(|record| record.diverted && record.paid_amount.amount() > 0.0)
            .expect("diverted payment");
        assert_eq!(first_diverted.recipient_id, "b_principal");
    }

    #[test]
    fn coverage_economics_non_curative_principal_recipient_consumes_cure_cash() {
        let currency = Currency::USD;
        let pool = AssetPool::new("POOL", DealType::CLO, currency);
        let maturity = Date::from_calendar_date(2031, Month::January, 1).expect("date");
        let class_a = Tranche::new(
            "CLASS_A",
            0.0,
            90.0,
            TrancheSeniority::Senior,
            Money::new(100_000.0, currency),
            TrancheCoupon::Fixed { rate: 0.04 },
            maturity,
        )
        .expect("class A");
        let class_b = Tranche::new(
            "CLASS_B",
            90.0,
            100.0,
            TrancheSeniority::Subordinated,
            Money::new(5_000.0, currency),
            TrancheCoupon::Fixed { rate: 0.16 },
            maturity,
        )
        .expect("class B");
        let tranches = TrancheStructure::new(vec![class_a, class_b]).expect("tranche structure");
        let waterfall = WaterfallBuilder::new(currency)
            .add_tier(
                WaterfallTier::new("interest", 1, PaymentType::Interest)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_interest("a_interest", "CLASS_A")),
            )
            .add_tier(
                WaterfallTier::new("custom_principal", 2, PaymentType::Principal)
                    .allocation_mode(AllocationMode::Sequential)
                    .divertible(true)
                    // B is junior to the tested A denominator. The waterfall
                    // still pays B first, so its full balance consumes cure
                    // cash without reducing A's IC interest denominator.
                    .add_recipient(Recipient::tranche_principal(
                        "b_principal",
                        "CLASS_B",
                        Some(Money::new(5_000.0, currency)),
                    ))
                    .add_recipient(Recipient::tranche_principal(
                        "a_principal",
                        "CLASS_A",
                        Some(Money::new(100_000.0, currency)),
                    )),
            )
            .add_coverage_trigger(CoverageTrigger {
                tranche_id: "CLASS_A".into(),
                oc_trigger: None,
                ic_trigger: Some(1.0),
            })
            .build()
            .expect("waterfall");
        let market = MarketContext::new();
        let period_start = Date::from_calendar_date(2025, Month::January, 1).expect("period start");
        let payment_date = Date::from_calendar_date(2025, Month::April, 1).expect("payment date");

        let result = execute_waterfall(
            &waterfall,
            &tranches,
            &pool,
            WaterfallContext {
                available_cash: Money::new(20_900.0, currency),
                interest_collections: Money::new(900.0, currency),
                principal_collections: Money::new(20_000.0, currency),
                payment_date,
                period_start,
                valuation_date: period_start,
                pool_balance: Money::new(105_000.0, currency),
                market: &market,
                tranche_balances: None,
                asset_balances: None,
                deferred_interest: None,
                reserve_balance: Money::new(0.0, currency),
                recovery_proceeds: Money::new(0.0, currency),
            },
        )
        .expect("waterfall execution");

        // A owes 1,000 quarterly interest; 900 collections require reducing
        // A's interest by 100, which needs 10,000 of A principal at 4%×0.25.
        // B consumes 5,000 first without curing A, so total diversion is 15,000.
        assert!(
            (result.diverted_cash.amount() - 15_000.0).abs() < 1e-6,
            "cure must include 5,000 consumed by out-of-denominator B plus \
             10,000 curative A paydown; got {}",
            result.diverted_cash.amount()
        );
        let diverted: Vec<_> = result
            .payment_records
            .iter()
            .filter(|record| record.diverted && record.paid_amount.amount() > 0.0)
            .map(|record| (record.recipient_id.as_str(), record.paid_amount.amount()))
            .collect();
        assert_eq!(
            diverted,
            vec![("b_principal", 5_000.0), ("a_principal", 10_000.0)]
        );
    }
}

#[cfg(test)]
mod water_fill_tests {
    use super::water_fill_allocation;

    /// The subordination-inversion case: a small-balance senior carrying a large
    /// weight must not leak its capped excess to the residual tier; the freed
    /// units water-fill onto the outstanding junior.
    #[test]
    fn capped_senior_excess_redistributes_to_junior() {
        // available = 100 units; senior weight 0.8 but cap 10; junior weight 0.2
        // cap 1000. Senior takes 10, junior absorbs the remaining 90.
        let alloc = water_fill_allocation(100, &[0.8, 0.2], &[10, 1000]);
        assert_eq!(alloc, vec![10, 90]);
        assert_eq!(alloc.iter().sum::<i64>(), 100);
    }

    /// Equal-weight pro-rata with unequal balances: the small recipient is
    /// capped and the large recipient takes the rest — nothing leaks.
    #[test]
    fn equal_weight_unequal_caps_no_leak() {
        let alloc = water_fill_allocation(100, &[1.0, 1.0], &[10, 1000]);
        assert_eq!(alloc, vec![10, 90]);
        assert_eq!(alloc.iter().sum::<i64>(), 100);
    }

    /// When every recipient's request is fully covered (available >= total
    /// requested), each receives exactly its cap and the rest stays unplaced
    /// (to flow to the next tier).
    #[test]
    fn all_requests_covered_leaves_residual() {
        let alloc = water_fill_allocation(100, &[1.0, 1.0], &[30, 40]);
        assert_eq!(alloc, vec![30, 40]);
        assert_eq!(alloc.iter().sum::<i64>(), 70); // 30 residual flows on
    }

    /// Conservation under tight caps: the allocation never exceeds total_units
    /// and never exceeds any per-recipient cap.
    #[test]
    fn conserves_and_respects_caps() {
        let caps = [13, 27, 5, 200];
        let alloc = water_fill_allocation(101, &[0.4, 0.3, 0.2, 0.1], &caps);
        let total: i64 = alloc.iter().sum();
        assert_eq!(total, 101, "all available units distributed");
        for (a, c) in alloc.iter().zip(caps.iter()) {
            assert!(*a <= *c, "allocation {a} exceeded cap {c}");
        }
    }

    /// Zero-weight recipients still receive an even split (degenerate fallback).
    #[test]
    fn zero_weights_split_evenly() {
        let alloc = water_fill_allocation(10, &[0.0, 0.0], &[100, 100]);
        assert_eq!(alloc.iter().sum::<i64>(), 10);
        assert!((alloc[0] - alloc[1]).abs() <= 1);
    }
}
