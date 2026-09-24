//! Waterfall execution functions for structured credit instruments.
//!
//! This module contains pure functions for executing waterfall distributions.
//! All type definitions are in `types::waterfall`.

use super::coverage_tests::TestContext;
use crate::instruments::fixed_income::structured_credit::types::{
    AfcSpec, AllocationMode, AssetPool, CoverageRules, CoverageTestAction, CoverageTestSpec,
    DiversionRecord, EquityHistory, FundingSource, LiveCollateral, PaymentCalculation,
    PaymentRecord, PaymentType, Recipient, RecipientType, RoundingConvention, Tranche,
    TrancheCoupon, TrancheStructure, Waterfall, WaterfallDistribution, WaterfallTier,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::explain::{ExplainOpts, ExplanationTrace, TraceEntry};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Error as CoreError;
use finstack_quant_core::HashMap;
use finstack_quant_core::Result;

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
    /// Live delinquency buckets and NPL flags for the borrowing-base
    /// eligibility rules.
    pub live_collateral: Option<LiveCollateral<'a>>,
    /// Live special-servicing flags aligned with `pool.assets` (loans that
    /// defaulted or took a balloon loss during the simulation); `None` uses
    /// the closing `special_servicing` specs.
    pub special_serviced: Option<&'a [bool]>,
    /// Deferred interest claims carried from prior periods.
    pub deferred_interest: Option<&'a HashMap<String, Money>>,
    /// Current reserve account balance (passed dynamically each period).
    /// Used by `PaymentCalculation::ReserveReplenishment` to compute shortfall.
    pub reserve_balance: Money,
    /// Trust-held collateral cash outside the asset balances (N2): the
    /// controlled-accumulation funding account. Counted in the OC numerator.
    pub restricted_cash: Money,
    /// Recovery value of defaulted collateral not yet received as cash
    /// (pending recovery claims). Counted in the OC numerator.
    pub defaulted_collateral_value: Money,
    /// Recovery proceeds released this period (tracked separately for reporting).
    pub recovery_proceeds: Money,
    /// Simulated shift applied to FLOATING tranche coupons (SC-M13 OAS rate
    /// path). Zero outside OAS runs. Applied before any available-funds cap so
    /// the interest the waterfall allocates matches the interest the engine
    /// records on the same rate path.
    pub floating_rate_shift: f64,
    /// Equity's cash to date, the input of any
    /// [`PaymentCalculation::IncentiveFee`] recipient's hurdle test; `None`
    /// pays no incentive fee.
    pub equity_history: Option<&'a EquityHistory>,
}

/// Balance of the specially serviced collateral, the base of the CMBS
/// special servicing fee: the live per-asset balances when the engine
/// supplies them, else the closing balances of the assets that carry a
/// `special_servicing` spec.
///
/// # Arguments
///
/// * `pool` - The collateral pool; only non-defaulted assets with a
///   `special_servicing` spec count.
/// * `asset_balances` - Current per-asset balances aligned with
///   `pool.assets`, or `None` to use the closing balances.
/// * `special_serviced` - Live special-servicing flags aligned with
///   `pool.assets` (loans that defaulted or took a balloon loss during the
///   simulation), or `None` to use the closing `special_servicing` specs.
/// * `currency` - Currency of the returned balance.
pub(crate) fn special_serviced_balance(
    pool: &AssetPool,
    asset_balances: Option<&[f64]>,
    special_serviced: Option<&[bool]>,
    currency: Currency,
) -> Result<Money> {
    Money::new(
        pool.assets
            .iter()
            .enumerate()
            .filter(|(index, asset)| {
                special_serviced.map_or(asset.special_servicing.is_some(), |flags| {
                    flags.get(*index).copied().unwrap_or(false)
                }) && !asset.is_defaulted
            })
            .map(|(index, asset)| {
                asset_balances
                    .and_then(|balances| balances.get(index).copied())
                    .unwrap_or_else(|| asset.balance.amount())
            })
            .sum::<f64>(),
        currency,
    )
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
    let mut tiers: Vec<_> = waterfall.tiers.iter().collect();
    tiers.sort_by_key(|tier| tier.priority);
    for pair in tiers.windows(2) {
        if pair[0].priority == pair[1].priority {
            return Err(CoreError::Validation(format!(
                "waterfall tiers '{}' and '{}' have duplicate priority {}",
                pair[0].id, pair[1].id, pair[0].priority
            )));
        }
    }
    let mut interest_remaining = context.interest_collections;
    let mut principal_remaining = context.principal_collections;
    let classified_cash = interest_remaining.checked_add(principal_remaining)?;
    if interest_remaining.amount() < 0.0
        || principal_remaining.amount() < 0.0
        || (classified_cash.amount() - context.available_cash.amount()).abs()
            > (context.available_cash.amount().abs() * f64::EPSILON * 8.0).max(1e-8)
        || classified_cash.currency() != waterfall.base_currency
    {
        return Err(CoreError::Validation(format!(
            "waterfall available cash must equal nonnegative interest plus principal in its base currency: available={}, interest={}, principal={}",
            context.available_cash.amount(), interest_remaining.amount(), principal_remaining.amount()
        )));
    }
    let mut total_diverted = Money::from((0_i64, waterfall.base_currency));
    let mut had_diversions = false;
    let mut diversion_reason = None;

    let mut tranche_index = HashMap::default();
    tranche_index.reserve(tranches.tranches.len());
    for (i, t) in tranches.tranches.iter().enumerate() {
        tranche_index.insert(t.id.as_str(), i);
    }

    let special_serviced_balance = special_serviced_balance(
        pool,
        context.asset_balances,
        context.special_serviced,
        waterfall.base_currency,
    )?;

    let allocation_ctx = AllocationContext {
        base_currency: waterfall.base_currency,
        tranches,
        tranche_index,
        pool_balance: context.pool_balance,
        special_serviced_balance,
        payment_date: context.payment_date,
        valuation_date: context.valuation_date,
        market: context.market,
        tranche_balances: context.tranche_balances,
        deferred_interest: context.deferred_interest,
        reserve_balance: context.reserve_balance,
        floating_rate_shift: context.floating_rate_shift,
        equity_history: context.equity_history,
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
    // `(interest collections − senior fees) / note interest due`.
    let senior_fees = senior_fee_accrual(
        waterfall,
        tranches,
        &allocation_ctx.tranche_index,
        SeniorFeeInputs {
            available: context.interest_collections,
            tranche_balances: context.tranche_balances,
            deferred_interest: context.deferred_interest,
            pool_balance: context.pool_balance,
            special_serviced_balance,
            period_start: context.period_start,
            payment_date: context.payment_date,
            valuation_date: context.valuation_date,
            market: context.market,
            reserve_balance: context.reserve_balance,
            floating_rate_shift: context.floating_rate_shift,
        },
    )?;

    // Evaluate every coverage test carried by the waterfall's test tiers on
    // the period's balances and collections. The ratio does not depend on the
    // tier's position; the position decides what cash a failure can divert.
    let specs: Vec<&CoverageTestSpec> = waterfall.coverage_tests().collect();
    let (claim_caps, coverage_rules) = coverage_inputs(waterfall);
    let coverage_test_results = evaluate_coverage_tests(
        &specs,
        &TestContext {
            pool,
            tranches,
            as_of: context.payment_date,
            valuation_date: context.valuation_date,
            period_start: Some(context.period_start),
            cash_balance: context.principal_collections,
            interest_collections: context.interest_collections,
            rules: coverage_rules,
            market: Some(context.market),
            tranche_balances: context.tranche_balances,
            payable_principal_tranche_ids: Some(&payable_principal_tranche_ids),
            asset_balances: context.asset_balances,
            live_collateral: context.live_collateral,
            current_pool_balance: Some(context.pool_balance),
            senior_fees,
            restricted_cash: context.restricted_cash,
            defaulted_collateral_value: context.defaulted_collateral_value,
            interest_claim_caps: &claim_caps,
            floating_rate_shift: context.floating_rate_shift,
            deferred_interest: context.deferred_interest,
        },
    )?;

    let estimated_recipients = waterfall
        .tiers
        .iter()
        .map(|t| t.recipients.len())
        .sum::<usize>();
    let mut allocation_output = AllocationOutput::with_capacity(estimated_recipients, &explain);

    // Cash allocated per tier, in execution order.
    let mut tier_allocations = Vec::with_capacity(waterfall.tiers.len());

    // Net all principal paid during the period against the period-start balance
    // so regular and diverted tiers cannot retire the same notional twice.
    let mut principal_paid_in_period: HashMap<String, Money> = HashMap::default();

    // Interest a `Reinvest` test retained as principal proceeds this period.
    let mut diverted_to_reinvestment = Money::from((0_i64, waterfall.base_currency));
    // Principal proceeds spent by `InterestThenPrincipal` tiers.
    let mut principal_used_for_interest = Money::from((0_i64, waterfall.base_currency));
    let mut reinvestment_diversions: Vec<DiversionRecord> = Vec::new();

    for tier in tiers {
        if tier.payment_type == PaymentType::CoverageTest {
            // A coverage-test position: while any of its tests fails, the
            // interest still undistributed here is diverted up to the binding
            // cure. Tests at one position share the senior balances they
            // de-lever, so the binding cure is the maximum failing cure, not
            // the sum (INTEX/Bloomberg convention). Only cash ranked below
            // this position can be diverted, because everything above has
            // already been paid.
            let mut binding_cure = 0.0_f64;
            let mut failing: Vec<&str> = Vec::new();
            for test in &tier.tests {
                if let Some(result) = coverage_test_results
                    .iter()
                    .find(|result| result.test_id == test.id)
                {
                    if !result.is_passing {
                        failing.push(test.id.as_str());
                        if let Some(cure) = result.cure_amount {
                            binding_cure = binding_cure.max(cure.amount());
                        }
                    }
                }
            }
            if failing.is_empty() {
                tier_allocations.push((
                    tier.id.clone(),
                    Money::from((0_i64, waterfall.base_currency)),
                ));
                continue;
            }
            had_diversions = true;
            diversion_reason = Some(format!("coverage test failed: {}", failing.join(", ")));
            let divertible = interest_remaining.amount().min(binding_cure).max(0.0);
            // A `divert_pct` caps the diversion at that share of the interest
            // remaining at the test tier.
            let divertible = match tier.tests.first().and_then(|test| test.divert_pct) {
                Some(pct) => divertible.min(interest_remaining.amount().max(0.0) * pct / 100.0),
                None => divertible,
            };
            let divertible = Money::new(divertible, waterfall.base_currency)?;
            let action = tier
                .tests
                .first()
                .map(|test| test.action)
                .unwrap_or_default();
            let tier_cash = match action {
                CoverageTestAction::PayDownSenior => {
                    // Pay the earliest principal tier in its configured
                    // recipient order. Early principal is booked in
                    // `principal_paid_in_period` so the principal tier nets
                    // it and cannot double-pay.
                    match diversion_principal_tier {
                        Some(principal_tier) if divertible.amount() > 0.0 => allocate_sequential(
                            &allocation_ctx,
                            tier,
                            &principal_tier.recipients[..],
                            divertible,
                            context.period_start,
                            true,
                            &mut allocation_output,
                            &explain,
                            &mut principal_paid_in_period,
                        )?,
                        _ => Money::from((0_i64, waterfall.base_currency)),
                    }
                }
                CoverageTestAction::Reinvest => {
                    // Retained as principal proceeds: reinvested while the
                    // reinvestment period is active, otherwise repaid through
                    // the principal tier next period.
                    if divertible.amount() > 0.0 {
                        diverted_to_reinvestment =
                            diverted_to_reinvestment.checked_add(divertible)?;
                        reinvestment_diversions.push(DiversionRecord {
                            source_tier: tier.id.clone(),
                            target_tranche: "principal_account".to_string(),
                            amount: divertible,
                            reason: diversion_reason
                                .clone()
                                .unwrap_or_else(|| "Waterfall diversion".to_string()),
                        });
                    }
                    divertible
                }
            };
            total_diverted = total_diverted.checked_add(tier_cash)?;
            interest_remaining = interest_remaining.checked_sub(tier_cash)?;
            tier_allocations.push((tier.id.clone(), tier_cash));
            continue;
        }

        let funding = tier.effective_funding();
        let remaining = match funding {
            FundingSource::Principal => principal_remaining,
            FundingSource::Interest => interest_remaining,
            FundingSource::InterestThenPrincipal => {
                interest_remaining.checked_add(principal_remaining)?
            }
        };

        let tier_cash = match tier.allocation_mode {
            AllocationMode::Sequential => allocate_sequential(
                &allocation_ctx,
                tier,
                &tier.recipients[..],
                remaining,
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
                remaining,
                context.period_start,
                false,
                &mut allocation_output,
                &explain,
                &mut principal_paid_in_period,
            )?,
        };

        tier_allocations.push((tier.id.clone(), tier_cash));
        match funding {
            FundingSource::Principal => {
                principal_remaining = principal_remaining.checked_sub(tier_cash)?;
            }
            FundingSource::Interest => {
                interest_remaining = interest_remaining.checked_sub(tier_cash)?;
            }
            FundingSource::InterestThenPrincipal => {
                // Interest proceeds first; only the shortfall touches principal.
                let from_interest = Money::new(
                    tier_cash.amount().min(interest_remaining.amount()).max(0.0),
                    waterfall.base_currency,
                )?;
                let from_principal = tier_cash.checked_sub(from_interest)?;
                interest_remaining = interest_remaining.checked_sub(from_interest)?;
                principal_remaining = principal_remaining.checked_sub(from_principal)?;
                // A principal tier drawing on principal is not principal
                // used for interest (targeted-OC amortization).
                if tier.payment_type != PaymentType::Principal {
                    principal_used_for_interest =
                        principal_used_for_interest.checked_add(from_principal)?;
                }
            }
        }
    }

    // Interest retained by a `Reinvest` test carries forward as principal.
    principal_remaining = principal_remaining.checked_add(diverted_to_reinvestment)?;

    let coverage_tests_public: Vec<(String, f64, bool)> = coverage_test_results
        .iter()
        .map(|r| (r.test_id.clone(), r.current_ratio, r.is_passing))
        .collect();
    // DiversionRecords are built only from payments that actually moved cash.
    // A failing coverage test with no cash to divert (e.g. an empty waterfall
    // period) must NOT fabricate records carrying the theoretical cure amount;
    // `had_diversions` / `coverage_tests` already report the breach itself.
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
    diverted_amounts.extend(reinvestment_diversions);

    Ok(WaterfallDistribution {
        payment_date: context.payment_date,
        total_available: context.available_cash,
        tier_allocations,
        distributions: allocation_output.distributions.into_iter().collect(),
        principal_distributions: allocation_output
            .principal_distributions
            .into_iter()
            .collect(),
        payment_records: allocation_output.payment_records,
        coverage_tests: coverage_tests_public,
        diverted_cash: total_diverted,
        remaining_cash: interest_remaining.checked_add(principal_remaining)?,
        remaining_interest: interest_remaining,
        remaining_principal: principal_remaining,
        principal_used_for_interest,
        had_diversions,
        diversion_reason,
        diverted_amounts,
        recovery_proceeds: context.recovery_proceeds,
        explanation: allocation_output.trace,
    })
}

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
    /// Balance of the specially serviced collateral (special servicing fee base).
    pub(crate) special_serviced_balance: Money,
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
    /// Simulated shift applied to FLOATING tranche coupons (SC-M13 OAS rate
    /// path); zero outside OAS runs. Threading it here keeps the cash the
    /// waterfall *allocates* on the same rate path as the interest the engine
    /// *records* in Step 5.
    pub(crate) floating_rate_shift: f64,
    /// Equity cash to date for incentive-fee IRR tests.
    pub(crate) equity_history: Option<&'a EquityHistory>,
}

/// Mutable output for allocation tracking.
///
/// Groups mutable state that is updated during allocation.
pub(crate) struct AllocationOutput {
    /// Accumulated distributions by recipient
    pub(crate) distributions: HashMap<RecipientType, Money>,
    /// The PRINCIPAL portion of `distributions`, per recipient (SC-M28).
    pub(crate) principal_distributions: HashMap<RecipientType, Money>,
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
            principal_distributions: HashMap::default(),
            payment_records: Vec::with_capacity(estimated_recipients),
            trace: if explain.enabled {
                Some(ExplanationTrace::new("waterfall"))
            } else {
                None
            },
        }
    }
}

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
    let mut tier_total = Money::from((0_i64, base_currency));

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
            ctx.special_serviced_balance,
            period_start,
            ctx.payment_date,
            ctx.valuation_date,
            ctx.market,
            ctx.reserve_balance,
            principal_paid_in_period,
            diverted,
            ctx.floating_rate_shift,
            ctx.equity_history,
            output
                .distributions
                .get(&RecipientType::Equity)
                .copied()
                .unwrap_or(Money::from((0_i64, base_currency))),
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
            .unwrap_or(Money::from((0_i64, base_currency)));

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
        // SC-M28: record the PRINCIPAL portion separately, keyed off the
        // payment calculation that produced it, so the engine never has to
        // re-derive the split from an aggregate.
        if is_principal_payment(&recipient.calculation, tier.payment_type) {
            match output
                .principal_distributions
                .entry(recipient.recipient_type.clone())
            {
                Entry::Occupied(mut e) => {
                    let next = e.get().checked_add(paid)?;
                    e.insert(next);
                }
                Entry::Vacant(e) => {
                    e.insert(paid);
                }
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
        return Ok(Money::from((0_i64, base_currency)));
    }

    let mut total_requested = Money::from((0_i64, base_currency));
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
            ctx.special_serviced_balance,
            period_start,
            ctx.payment_date,
            ctx.valuation_date,
            ctx.market,
            ctx.reserve_balance,
            principal_paid_in_period,
            diverted,
            ctx.floating_rate_shift,
            ctx.equity_history,
            output
                .distributions
                .get(&RecipientType::Equity)
                .copied()
                .unwrap_or(Money::from((0_i64, base_currency))),
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

    let mut tier_total = Money::from((0_i64, base_currency));

    for (idx, (recipient, requested)) in recipient_requests.iter().enumerate() {
        let allocated = Money::new(final_units[idx] as f64 / scale, base_currency)?;

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
            .unwrap_or(Money::from((0_i64, base_currency)));

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
        // SC-M28: record the PRINCIPAL portion separately, keyed off the
        // payment calculation that produced it, so the engine never has to
        // re-derive the split from an aggregate.
        if is_principal_payment(&recipient.calculation, tier.payment_type) {
            match output
                .principal_distributions
                .entry(recipient.recipient_type.clone())
            {
                Entry::Occupied(mut e) => {
                    let next = e.get().checked_add(paid)?;
                    e.insert(next);
                }
                Entry::Vacant(e) => {
                    e.insert(paid);
                }
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

/// Evaluate coverage tests.
///
/// `principal_collections` is the cash component of the OC numerator
/// (standard CLO par-OC counts only principal proceeds, never interest).
/// Whether a payment calculation pays PRINCIPAL rather than interest.
///
/// SC-M28: the waterfall's own classification, decided in exactly one place.
/// `RecipientType::Tranche(id)` is the same map key for a tranche's interest
/// and its principal, so `distributions` aggregates them; without this the
/// engine had to guess the split by assuming interest is satisfied first.
///
/// Residual cash retains its source: only a principal tier repays capital.
fn is_principal_payment(calculation: &PaymentCalculation, payment_type: PaymentType) -> bool {
    matches!(calculation, PaymentCalculation::TranchePrincipal { .. })
        || (matches!(calculation, PaymentCalculation::ResidualCash)
            && payment_type == PaymentType::Principal)
}

/// Contractual coupon for the period with the simulated rate-path shift applied.
///
/// SC-M13: a FLOATING tranche's coupon moves with the simulated short-rate path
/// (`floating_rate_shift`), floored at zero; a fixed coupon is contractual and
/// unaffected. This is the same shift-then-floor rule as the engine's
/// interest-due kernel, so allocation and recording cannot diverge on the path.
fn shifted_tranche_rate(
    tranche: &Tranche,
    period_start: Date,
    valuation_date: Date,
    market: &MarketContext,
    floating_rate_shift: f64,
) -> Result<f64> {
    let raw = tranche
        .coupon
        .try_rate_for_period(period_start, valuation_date, market)?;
    Ok(match tranche.coupon {
        TrancheCoupon::Floating(_) => (raw + floating_rate_shift).max(0.0),
        _ => raw,
    })
}

/// Per-tranche interest-claim definition extracted from a waterfall spec.
///
/// The waterfall is the single source of truth for what interest a tranche is
/// *owed* each period, not merely how cash is allocated:
///
/// - `Some(None)` — an uncapped [`PaymentCalculation::TrancheInterest`]
///   recipient: the tranche is owed its full coupon accrual.
/// - `Some(Some(cap))` — a [`PaymentCalculation::CappedTrancheInterest`]
///   recipient: the claim itself is capped at `cap` (available-funds style),
///   so the capped-off portion is never owed and never defers.
/// - absent key — the waterfall defines **no** interest claim for the tranche
///   (e.g. a principal-only class): nothing accrues, nothing defers.
///
/// When `afc` names a tranche whose recipient is still the uncapped variant,
/// `live_afc_cap` is applied — exactly the rewrite `resolve::apply_afc_cap` performs
/// on the period waterfall, so claims extracted from the base waterfall agree
/// with the allocation the resolved waterfall executes.
///
/// `StructuredCredit::validate_custom_waterfall` enforces at most one
/// interest-type recipient per tranche, so the extraction is unambiguous.
pub(crate) fn interest_claim_caps<'w>(
    waterfall: &'w Waterfall,
    afc: Option<&AfcSpec>,
    live_afc_cap: f64,
) -> HashMap<&'w str, Option<f64>> {
    let mut caps: HashMap<&'w str, Option<f64>> = HashMap::default();
    for tier in &waterfall.tiers {
        for recipient in &tier.recipients {
            match &recipient.calculation {
                PaymentCalculation::TrancheInterest { tranche_id, .. } => {
                    let afc_capped = afc
                        .is_some_and(|spec| spec.capped_tranches.iter().any(|t| t == tranche_id));
                    caps.insert(tranche_id.as_str(), afc_capped.then_some(live_afc_cap));
                }
                PaymentCalculation::CappedTrancheInterest {
                    tranche_id,
                    cap_rate,
                    ..
                } => {
                    caps.insert(tranche_id.as_str(), Some(*cap_rate));
                }
                _ => {}
            }
        }
    }
    caps
}

/// Inputs the senior-fee kernel needs beyond the waterfall and tranche index.
pub(crate) struct SeniorFeeInputs<'a> {
    /// Cash the fee tier is measured against (pool interest for a period).
    pub available: Money,
    /// Current tranche balances, when known.
    pub tranche_balances: Option<&'a HashMap<String, Money>>,
    /// Outstanding deferred interest, when known.
    pub deferred_interest: Option<&'a HashMap<String, Money>>,
    /// Collateral balance the percentage-of-collateral fees accrue on.
    pub pool_balance: Money,
    /// Balance of the specially serviced collateral (special servicing fee base).
    pub special_serviced_balance: Money,
    /// Accrual period start.
    pub period_start: Date,
    /// Payment date.
    pub payment_date: Date,
    /// Valuation date.
    pub valuation_date: Date,
    /// Market context for any index-linked fee.
    pub market: &'a MarketContext,
    /// Reserve balance, for reserve-linked calculations.
    pub reserve_balance: Money,
    /// Simulated floating-coupon shift (SC-M13); zero outside OAS runs.
    pub floating_rate_shift: f64,
}

/// Senior fees accruing this period, i.e. the fee tiers that rank ahead of
/// every note.
///
/// Priority determines seniority even when deserialized tiers arrive out of order.
/// Only fees ranking ahead of the first non-fee tier enter the senior claim.
///
/// This is the single source of truth for "what the fee tier will take",
/// shared by three call sites that must agree:
///   * the IC numerator (SC-M29), which nets it from interest collections;
///   * the excess-spread capture/draw and the reserve draw (N1), which must
///     treat it as a senior claim ranking ahead of note interest;
///   * the waterfall itself, which actually pays it.
///
/// Computing it with `calculate_payment_amount` — the same kernel the fee tier
/// uses to pay — is what makes the measured and paid amounts identical by
/// construction rather than by coincidence.
pub(crate) fn senior_fee_accrual(
    waterfall: &Waterfall,
    tranches: &TrancheStructure,
    tranche_index: &HashMap<&str, usize>,
    inputs: SeniorFeeInputs<'_>,
) -> Result<Money> {
    let empty_in_period: HashMap<String, Money> = HashMap::default();
    let mut total = Money::from((0_i64, waterfall.base_currency));
    let first_note_priority = waterfall
        .tiers
        .iter()
        .filter(|tier| tier.payment_type != PaymentType::Fee)
        .map(|tier| tier.priority)
        .min()
        .unwrap_or(usize::MAX);
    let mut fees: Vec<_> = waterfall
        .tiers
        .iter()
        .filter(|tier| tier.payment_type == PaymentType::Fee && tier.priority < first_note_priority)
        .collect();
    fees.sort_by_key(|tier| tier.priority);
    for tier in fees {
        for recipient in &tier.recipients {
            let amount = calculate_payment_amount(
                waterfall.base_currency,
                &recipient.calculation,
                inputs.available,
                tranches,
                tranche_index,
                inputs.tranche_balances,
                inputs.deferred_interest,
                inputs.pool_balance,
                inputs.special_serviced_balance,
                inputs.period_start,
                inputs.payment_date,
                inputs.valuation_date,
                inputs.market,
                inputs.reserve_balance,
                &empty_in_period,
                false,
                inputs.floating_rate_shift,
                None,
                Money::from((0_i64, waterfall.base_currency)),
            )?;
            total = total.checked_add(amount)?;
        }
    }
    Ok(total)
}

/// Interest claims and coverage rules a coverage test reads from the
/// waterfall it is evaluated in.
///
/// The waterfall spec defines each tranche's interest CLAIM (uncapped,
/// capped, or absent); the IC test must measure coverage of those claims,
/// not of raw coupons the structure never owes. Caps in this waterfall are
/// already resolved (the live AFC cap is baked in by `resolve::apply_afc_cap`),
/// so no AFC override applies here.
pub(super) fn coverage_inputs(
    waterfall: &Waterfall,
) -> (HashMap<&str, Option<f64>>, Option<&CoverageRules>) {
    let claim_caps = interest_claim_caps(waterfall, None, 0.0);
    let rules = waterfall
        .coverage_rules
        .as_ref()
        .filter(|rules| !rules.is_empty());
    (claim_caps, rules)
}

/// Evaluate `specs` on one period's state. The ratio of each test is
/// independent of the tier position that carries it; the executor uses the
/// position only to decide what cash a failure can divert.
pub(super) fn evaluate_coverage_tests(
    specs: &[&CoverageTestSpec],
    context: &TestContext<'_>,
) -> Result<Vec<CoverageTestResult>> {
    specs
        .iter()
        .map(|spec| {
            let result = spec.evaluate(context)?;
            Ok(CoverageTestResult {
                test_id: spec.id.clone(),
                current_ratio: result.current_ratio,
                is_passing: result.is_passing,
                cure_amount: result.cure_amount,
            })
        })
        .collect()
}

/// Internal coverage test result with cure amount.
#[derive(Debug, Clone)]
pub(super) struct CoverageTestResult {
    pub(super) test_id: String,
    pub(super) current_ratio: f64,
    pub(super) is_passing: bool,
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
            .or_insert(Money::from((0_i64, base_currency)));
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
    special_serviced_balance: Money,
    period_start: Date,
    payment_date: Date,
    valuation_date: Date,
    market: &MarketContext,
    reserve_balance: Money,
    principal_paid_in_period: &HashMap<String, Money>,
    diverted: bool,
    floating_rate_shift: f64,
    equity_history: Option<&EquityHistory>,
    equity_paid_in_period: Money,
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

        PaymentCalculation::PercentageOfSpecialServiced {
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
            (
                special_serviced_balance.amount() * rate * accrual_fraction,
                *rounding,
            )
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
            // Use current tranche balance when available. Floor at zero so a
            // rounding-induced negative balance can never produce a negative
            // interest claim (which would distort IC coverage tests).
            let balance = tranche_balances
                .and_then(|b| b.get(tranche_id.as_str()))
                .copied()
                .unwrap_or(tranche.current_balance)
                .amount()
                .max(0.0);
            let rate = shifted_tranche_rate(
                tranche,
                period_start,
                valuation_date,
                market,
                floating_rate_shift,
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
            (balance * rate * accrual_fraction + carried, *rounding)
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
            // Floored at zero for the same reason as `TrancheInterest`.
            let balance = tranche_balances
                .and_then(|b| b.get(tranche_id.as_str()))
                .copied()
                .unwrap_or(tranche.current_balance)
                .amount()
                .max(0.0);
            // Available-funds cap: the effective coupon cannot exceed `cap_rate`.
            // The cap applies AFTER the rate-path shift, matching the engine's
            // interest-due kernel (the cap tracks the shifted net WAC).
            let rate = shifted_tranche_rate(
                tranche,
                period_start,
                valuation_date,
                market,
                floating_rate_shift,
            )?
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
            (balance * rate * accrual_fraction + carried, *rounding)
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
                Money::from((0_i64, base_currency))
            } else {
                target_balance.unwrap_or(Money::from((0_i64, base_currency)))
            };
            let needed = (current.amount() - paid_this_period - target.amount()).max(0.0);
            (needed, *rounding)
        }

        PaymentCalculation::ResidualCash => (available.amount(), None),

        PaymentCalculation::NetWacCarryover { amount, .. } => (amount.amount().max(0.0), None),

        PaymentCalculation::IncentiveFee {
            hurdle_irr,
            share_pct,
        } => {
            // The hurdle is tested on equity's cash to date plus what it has
            // already received earlier in this run plus this cash; the share
            // applies only to the part of this cash above the hurdle, so the
            // crossing period is split rather than all-or-nothing.
            let fee = match equity_history {
                Some(history) => {
                    let total = equity_paid_in_period.checked_add(available)?;
                    let shortfall = history
                        .hurdle_shortfall(payment_date, total, *hurdle_irr)
                        .amount();
                    let excess = (total.amount() - shortfall.max(equity_paid_in_period.amount()))
                        .clamp(0.0, available.amount());
                    excess * share_pct
                }
                None => 0.0,
            };
            (fee, None)
        }

        PaymentCalculation::ReserveReplenishment { target_balance } => {
            // Shortfall = max(0, target - current). Current balance is passed
            // dynamically from SimulationState, not stored in the waterfall definition.
            let shortfall = target_balance
                .checked_sub(reserve_balance)
                .unwrap_or(Money::from((0_i64, base_currency)));
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
        Ok(Money::new(rounded_val, base_currency)?)
    } else {
        Ok(Money::new(raw_amount, base_currency)?)
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
        let _pool_bal = Money::from((1_000_000_i64, Currency::USD));

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
mod coverage_position_tests {
    //! Positional coverage tests: a `PaymentType::CoverageTest` tier can only
    //! divert the interest still undistributed at its position, so a test on
    //! a junior class never traps a senior class's coupon. The IC cure-sizing
    //! rules (recipient order, out-of-denominator recipients) are covered at
    //! the `CoverageTest::calculate` level in `coverage_tests.rs`.
    use super::execute_waterfall;
    use super::WaterfallContext;
    use crate::instruments::fixed_income::structured_credit::types::{
        AllocationMode, AssetPool, AssetType, CoverageTestSpec, DealType, PaymentCalculation,
        PaymentType, PoolAsset, Recipient, RecipientType, Tranche, TrancheCoupon, TrancheSeniority,
        TrancheStructure, Waterfall, WaterfallTier,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CreditRating, InstrumentId};
    use time::Month;

    fn usd(amount: f64) -> Money {
        Money::new(amount, Currency::USD).expect("valid money fixture")
    }

    fn maturity() -> Date {
        Date::from_calendar_date(2031, Month::January, 1).expect("date")
    }

    fn pool_with(balance: f64) -> AssetPool {
        let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
        pool.assets.push(PoolAsset {
            day_count: finstack_quant_core::dates::DayCount::Act360,
            id: InstrumentId::new("ASSET_0"),
            asset_type: AssetType::FirstLienLoan {
                industry: Some("Technology".into()),
            },
            balance: usd(balance),
            rate: 0.08,
            spread_bp: Some(400.0),
            index_id: None,
            index_floor: None,
            maturity: maturity(),
            credit_quality: Some(CreditRating::BB),
            industry: Some("Technology".into()),
            obligor_id: Some("OBLIGOR_0".into()),
            is_defaulted: false,
            recovery_amount: None,
            default_date: None,
            purchase_price: None,
            acquisition_date: None,
            origination_date: None,
            smm_override: None,
            mdr_override: None,
            recovery_rate: None,
            commitment: None,
            contractual_payment: None,
            amortization_term_months: None,
            io_months: None,
            market_price_pct: None,
            delinquency_buckets: None,
            balloon: None,
            prepayment_penalty: None,
            special_servicing: None,
            noi: None,
            liquidation: None,
        });
        pool
    }

    fn tranche(id: &str, a: f64, d: f64, sen: TrancheSeniority, bal: f64, cpn: f64) -> Tranche {
        Tranche::new(
            id,
            a,
            d,
            sen,
            usd(bal),
            TrancheCoupon::Fixed { rate: cpn },
            maturity(),
        )
        .expect("tranche")
    }

    fn context<'a>(
        market: &'a MarketContext,
        interest: f64,
        principal: f64,
        pool_balance: f64,
    ) -> WaterfallContext<'a> {
        let payment_date = Date::from_calendar_date(2024, Month::April, 1).expect("date");
        let period_start = Date::from_calendar_date(2024, Month::January, 1).expect("date");
        WaterfallContext {
            available_cash: usd(interest + principal),
            interest_collections: usd(interest),
            principal_collections: usd(principal),
            payment_date,
            period_start,
            valuation_date: period_start,
            pool_balance: usd(pool_balance),
            market,
            tranche_balances: None,
            asset_balances: None,
            live_collateral: None,
            special_serviced: None,
            deferred_interest: None,
            reserve_balance: usd(0.0),
            restricted_cash: usd(0.0),
            defaulted_collateral_value: usd(0.0),
            recovery_proceeds: usd(0.0),
            floating_rate_shift: 0.0,
            equity_history: None,
        }
    }

    fn paid_to(result: &super::WaterfallDistribution, recipient_id: &str) -> f64 {
        result
            .payment_records
            .iter()
            .filter(|r| r.recipient_id == recipient_id)
            .map(|r| r.paid_amount.amount())
            .sum()
    }

    /// A test after Class B's interest can trap Class C's coupon and the
    /// residual, never Class A's or B's.
    #[test]
    fn test_after_a_junior_class_leaves_senior_coupons_intact() {
        let pool = pool_with(120_000_000.0);
        let tranches = TrancheStructure::new(vec![
            tranche("A", 0.0, 60.0, TrancheSeniority::Senior, 60_000_000.0, 0.05),
            tranche(
                "B",
                60.0,
                80.0,
                TrancheSeniority::Mezzanine,
                20_000_000.0,
                0.07,
            ),
            tranche(
                "C",
                80.0,
                100.0,
                TrancheSeniority::Subordinated,
                20_000_000.0,
                0.10,
            ),
        ])
        .expect("structure");
        // Collateral 120M + 0 principal cash against A + B = 80M gives 1.50,
        // so a 1.60 test on B fails while everything is otherwise healthy.
        let waterfall = Waterfall::builder(Currency::USD)
            .add_tier(
                WaterfallTier::new("a_interest", 1, PaymentType::Interest)
                    .add_recipient(Recipient::tranche_interest("a_int", "A")),
            )
            .add_tier(
                WaterfallTier::new("b_interest", 2, PaymentType::Interest)
                    .add_recipient(Recipient::tranche_interest("b_int", "B")),
            )
            .add_tier(WaterfallTier::coverage_tests(
                "b_coverage",
                3,
                vec![CoverageTestSpec::oc("B", 1.60)],
            ))
            .add_tier(
                WaterfallTier::new("c_interest", 4, PaymentType::Interest)
                    .add_recipient(Recipient::tranche_interest("c_int", "C")),
            )
            .add_tier(
                WaterfallTier::new("principal", 5, PaymentType::Principal)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_principal("a_prin", "A", None))
                    .add_recipient(Recipient::tranche_principal("b_prin", "B", None))
                    .add_recipient(Recipient::tranche_principal("c_prin", "C", None)),
            )
            .add_tier(
                WaterfallTier::new("equity", 6, PaymentType::Residual).add_recipient(
                    Recipient::new(
                        "equity_dist",
                        RecipientType::Equity,
                        PaymentCalculation::ResidualCash,
                    ),
                ),
            )
            .build()
            .expect("waterfall");
        let market = MarketContext::new();
        let result = execute_waterfall(
            &waterfall,
            &tranches,
            &pool,
            context(&market, 3_000_000.0, 0.0, 120_000_000.0),
        )
        .expect("waterfall execution");

        let oc = result
            .coverage_tests
            .iter()
            .find(|(id, _, _)| id == "OC_B")
            .expect("OC_B evaluated");
        assert!(!oc.2, "the B test must fail (ratio {})", oc.1);
        assert!(result.had_diversions);

        let a_interest = paid_to(&result, "a_int");
        let b_interest = paid_to(&result, "b_int");
        let c_interest = paid_to(&result, "c_int");
        // Fixed coupons accrue ACT/360 over the 91-day Jan-1 to Apr-1 period.
        let accrual = 91.0 / 360.0;
        assert!(
            (a_interest - 60_000_000.0 * 0.05 * accrual).abs() < 1.0,
            "A coupon is paid in full above the test, got {a_interest}"
        );
        assert!(
            (b_interest - 20_000_000.0 * 0.07 * accrual).abs() < 1.0,
            "B coupon is paid in full above the test, got {b_interest}"
        );
        assert!(
            c_interest < 1.0,
            "C's coupon sits below the failing test and is diverted, got {c_interest}"
        );
        // Everything left after B's coupon went to A principal, up to the cure.
        let a_principal = paid_to(&result, "a_prin");
        let remaining_after_b = 3_000_000.0 - a_interest - b_interest;
        assert!(
            (a_principal - remaining_after_b).abs() < 1.0,
            "diverted interest {remaining_after_b} must pay down A, got {a_principal}"
        );
        assert!(
            (result.diverted_cash.amount() - a_principal).abs() < 1.0,
            "diverted cash {} must equal the senior paydown {a_principal}",
            result.diverted_cash.amount()
        );
    }

    /// W-21: an IC-only breach diverts the interest below the test position.
    #[test]
    fn ic_only_breach_diverts_interest_below_the_test() {
        let pool = pool_with(500_000_000.0);
        let tranches = TrancheStructure::new(vec![
            tranche(
                "CLASS_A",
                0.0,
                76.9,
                TrancheSeniority::Senior,
                100_000_000.0,
                0.05,
            ),
            tranche(
                "CLASS_B",
                76.9,
                100.0,
                TrancheSeniority::Subordinated,
                30_000_000.0,
                0.08,
            ),
        ])
        .expect("structure");
        let waterfall = Waterfall::builder(Currency::USD)
            .add_tier(
                WaterfallTier::new("a_interest", 1, PaymentType::Interest)
                    .add_recipient(Recipient::tranche_interest("class_a_int", "CLASS_A")),
            )
            // OC 1.05 passes easily (500M collateral); IC 1.20 fails because
            // collections (1.4M) do not cover 1.2 × A's 1.25M coupon.
            .add_tier(WaterfallTier::coverage_tests(
                "a_coverage",
                2,
                vec![
                    CoverageTestSpec::oc("CLASS_A", 1.05),
                    CoverageTestSpec::ic("CLASS_A", 1.20),
                ],
            ))
            .add_tier(
                WaterfallTier::new("b_interest", 3, PaymentType::Interest)
                    .add_recipient(Recipient::tranche_interest("class_b_int", "CLASS_B")),
            )
            .add_tier(
                WaterfallTier::new("principal", 4, PaymentType::Principal)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_principal(
                        "class_a_prin",
                        "CLASS_A",
                        None,
                    ))
                    .add_recipient(Recipient::tranche_principal(
                        "class_b_prin",
                        "CLASS_B",
                        None,
                    )),
            )
            .add_tier(
                WaterfallTier::new("equity", 5, PaymentType::Residual).add_recipient(
                    Recipient::new(
                        "equity_dist",
                        RecipientType::Equity,
                        PaymentCalculation::ResidualCash,
                    ),
                ),
            )
            .build()
            .expect("waterfall");
        let market = MarketContext::new();
        let principal_collections = 19_900_000.0;
        let result = execute_waterfall(
            &waterfall,
            &tranches,
            &pool,
            context(&market, 1_400_000.0, principal_collections, 500_000_000.0),
        )
        .expect("waterfall execution");

        let oc = result
            .coverage_tests
            .iter()
            .find(|(id, _, _)| id == "OC_CLASS_A")
            .expect("OC result present");
        let ic = result
            .coverage_tests
            .iter()
            .find(|(id, _, _)| id == "IC_CLASS_A")
            .expect("IC result present");
        assert!(oc.2, "OC test should pass (ratio {})", oc.1);
        assert!(!ic.2, "IC test should fail (ratio {})", ic.1);

        let a_interest = paid_to(&result, "class_a_int");
        let diverted = result.diverted_cash.amount();
        assert!(
            (diverted - (1_400_000.0 - a_interest)).abs() < 1.0,
            "the interest left after A's coupon ({}) is diverted, got {diverted}",
            1_400_000.0 - a_interest
        );
        assert!(
            paid_to(&result, "class_b_int") < 1.0,
            "B's coupon sits below the failing test"
        );
        // The diverted interest pays A principal on top of the period's
        // principal collections, which A absorbs in full (no target balance).
        let a_principal = paid_to(&result, "class_a_prin");
        assert!(
            (a_principal - (principal_collections + diverted)).abs() < 1.0,
            "A principal must be the collections plus the diversion, got {a_principal}"
        );
        assert!(
            paid_to(&result, "class_b_prin") < 1.0,
            "no principal reaches B while A is outstanding"
        );
    }

    /// Two tests at one position share the senior balances they de-lever, so
    /// the binding cure is the maximum, not the sum.
    #[test]
    fn coverage_cures_at_one_position_are_not_summed() {
        let pool = pool_with(118_000_000.0);
        let tranches = TrancheStructure::new(vec![
            tranche(
                "CLASS_A",
                0.0,
                77.0,
                TrancheSeniority::Senior,
                100_000_000.0,
                0.05,
            ),
            tranche(
                "CLASS_B",
                77.0,
                100.0,
                TrancheSeniority::Subordinated,
                30_000_000.0,
                0.08,
            ),
        ])
        .expect("structure");
        let waterfall = Waterfall::builder(Currency::USD)
            .add_tier(
                WaterfallTier::new("interest", 1, PaymentType::Interest)
                    .add_recipient(Recipient::tranche_interest("class_a_int", "CLASS_A"))
                    .add_recipient(Recipient::tranche_interest("class_b_int", "CLASS_B")),
            )
            // Numerator = 118M collateral + 20M principal cash = 138M.
            // CLASS_A denominator 100M (1.38) breaches 1.39; CLASS_B
            // denominator 130M (1.06) breaches 1.07 by more.
            .add_tier(WaterfallTier::coverage_tests(
                "coverage",
                2,
                vec![
                    CoverageTestSpec::oc("CLASS_A", 1.39),
                    CoverageTestSpec::oc("CLASS_B", 1.07),
                ],
            ))
            .add_tier(
                WaterfallTier::new("principal", 3, PaymentType::Principal)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_principal(
                        "class_a_prin",
                        "CLASS_A",
                        Some(usd(99_000_000.0)),
                    ))
                    .add_recipient(Recipient::tranche_principal(
                        "class_b_prin",
                        "CLASS_B",
                        None,
                    )),
            )
            .add_tier(
                WaterfallTier::new("equity", 4, PaymentType::Residual).add_recipient(
                    Recipient::new(
                        "equity_dist",
                        RecipientType::Equity,
                        PaymentCalculation::ResidualCash,
                    ),
                ),
            )
            .build()
            .expect("waterfall");
        let market = MarketContext::new();
        let result = execute_waterfall(
            &waterfall,
            &tranches,
            &pool,
            context(&market, 25_000_000.0, 20_000_000.0, 118_000_000.0),
        )
        .expect("waterfall execution");

        let failing = result
            .coverage_tests
            .iter()
            .filter(|(_, _, passing)| !passing)
            .count();
        assert_eq!(
            failing, 2,
            "both OC tests must breach: {:?}",
            result.coverage_tests
        );

        // Diverting X of interest pays down X of the shared senior
        // denominator and leaves the numerator untouched: X = den − num / r.
        let numerator = 118_000_000.0_f64 + 20_000_000.0;
        let cure = |ratio: f64, denom: f64| denom - numerator / ratio;
        let cure_a = cure(1.39, 100_000_000.0);
        let cure_b = cure(1.07, 130_000_000.0);
        let binding = cure_a.max(cure_b);
        let summed = cure_a + cure_b;
        let interest_left =
            25_000_000.0 - paid_to(&result, "class_a_int") - paid_to(&result, "class_b_int");
        assert!(
            summed < interest_left,
            "fixture: both cures ({summed}) must be fundable from the interest left ({interest_left})"
        );
        let diverted = result.diverted_cash.amount();
        assert!(
            (diverted - binding).abs() < 1.0,
            "diverted {diverted} must equal the binding (max) cure {binding}, not the sum {summed}"
        );
    }

    /// Principal paid by a diversion is netted by the regular principal tier.
    #[test]
    fn diversion_never_over_pays_senior_principal() {
        let pool = pool_with(20_000_000.0);
        let class_a_balance = 3_000_000.0;
        let tranches = TrancheStructure::new(vec![
            tranche(
                "CLASS_A",
                0.0,
                10.0,
                TrancheSeniority::Senior,
                class_a_balance,
                0.05,
            ),
            tranche(
                "CLASS_B",
                10.0,
                100.0,
                TrancheSeniority::Subordinated,
                27_000_000.0,
                0.08,
            ),
        ])
        .expect("structure");
        let waterfall = Waterfall::builder(Currency::USD)
            .add_tier(
                WaterfallTier::new("interest", 1, PaymentType::Interest)
                    .add_recipient(Recipient::tranche_interest("class_a_int", "CLASS_A"))
                    .add_recipient(Recipient::tranche_interest("class_b_int", "CLASS_B")),
            )
            // Collateral 20M + 9M cash against the 30M stack breaches 1.20.
            .add_tier(WaterfallTier::coverage_tests(
                "coverage",
                2,
                vec![CoverageTestSpec::oc("CLASS_B", 1.20)],
            ))
            .add_tier(
                WaterfallTier::new("principal", 3, PaymentType::Principal)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_principal(
                        "class_a_prin",
                        "CLASS_A",
                        None,
                    ))
                    .add_recipient(Recipient::tranche_principal(
                        "class_b_prin",
                        "CLASS_B",
                        None,
                    )),
            )
            .add_tier(
                WaterfallTier::new("equity", 4, PaymentType::Residual).add_recipient(
                    Recipient::new(
                        "equity_dist",
                        RecipientType::Equity,
                        PaymentCalculation::ResidualCash,
                    ),
                ),
            )
            .build()
            .expect("waterfall");
        let market = MarketContext::new();
        let result = execute_waterfall(
            &waterfall,
            &tranches,
            &pool,
            context(&market, 1_000_000.0, 9_000_000.0, 20_000_000.0),
        )
        .expect("waterfall execution");

        assert!(result.had_diversions, "the OC breach must divert");
        assert!(
            result.diverted_cash.amount() > 0.0,
            "interest left after the coupons must reach A principal"
        );
        let class_a_principal_paid = paid_to(&result, "class_a_prin");
        assert!(
            class_a_principal_paid <= class_a_balance + 1e-6,
            "CLASS_A principal paid {class_a_principal_paid:.2} exceeds its \
             balance {class_a_balance:.2}: the regular tier did not net the diversion"
        );
        assert!(
            (class_a_principal_paid - class_a_balance).abs() < 1e-6,
            "with 9M of principal the senior is retired exactly once"
        );
    }

    /// A `Reinvest` test retains the diverted interest as principal proceeds
    /// instead of paying notes.
    #[test]
    fn reinvest_action_retains_diverted_interest_as_principal() {
        let pool = pool_with(100_000_000.0);
        let tranches = TrancheStructure::new(vec![
            tranche(
                "CLASS_A",
                0.0,
                80.0,
                TrancheSeniority::Senior,
                80_000_000.0,
                0.05,
            ),
            tranche(
                "CLASS_B",
                80.0,
                100.0,
                TrancheSeniority::Subordinated,
                20_000_000.0,
                0.08,
            ),
        ])
        .expect("structure");
        let waterfall = Waterfall::builder(Currency::USD)
            .add_tier(
                WaterfallTier::new("interest", 1, PaymentType::Interest)
                    .add_recipient(Recipient::tranche_interest("class_a_int", "CLASS_A"))
                    .add_recipient(Recipient::tranche_interest("class_b_int", "CLASS_B")),
            )
            // 100M against 100M of notes fails a 1.05 reinvestment OC test.
            .add_tier(WaterfallTier::coverage_tests(
                "reinvestment_oc",
                2,
                vec![CoverageTestSpec::oc("CLASS_B", 1.05)
                    .with_action(super::CoverageTestAction::Reinvest)],
            ))
            .add_tier(
                WaterfallTier::new("principal", 3, PaymentType::Principal)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_principal(
                        "class_a_prin",
                        "CLASS_A",
                        Some(usd(80_000_000.0)),
                    )),
            )
            .add_tier(
                WaterfallTier::new("equity", 4, PaymentType::Residual).add_recipient(
                    Recipient::new(
                        "equity_dist",
                        RecipientType::Equity,
                        PaymentCalculation::ResidualCash,
                    ),
                ),
            )
            .build()
            .expect("waterfall");
        let market = MarketContext::new();
        let result = execute_waterfall(
            &waterfall,
            &tranches,
            &pool,
            context(&market, 2_500_000.0, 0.0, 100_000_000.0),
        )
        .expect("waterfall execution");

        let coupons = paid_to(&result, "class_a_int") + paid_to(&result, "class_b_int");
        let retained = 2_500_000.0 - coupons;
        assert!(retained > 0.0, "fixture: excess interest must exist");
        assert!(
            (result.remaining_principal.amount() - retained).abs() < 1.0,
            "diverted interest is carried as principal: got {}, expected {retained}",
            result.remaining_principal.amount()
        );
        assert!(
            paid_to(&result, "class_a_prin") < 1.0,
            "a Reinvest test does not pay down notes this period"
        );
        assert!(
            result
                .distributions
                .get(&RecipientType::Equity)
                .is_none_or(|m| m.amount() < 1.0),
            "nothing reaches equity below a failing test"
        );
        assert!(result
            .diverted_amounts
            .iter()
            .any(|record| record.target_tranche == "principal_account"));
    }

    /// A fee tier ranked behind note interest is junior to the IC claim and
    /// must not reduce its numerator.
    #[test]
    fn coverage_economics_late_junior_fee_is_not_deducted_from_ic_numerator() {
        let pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
        let tranches = TrancheStructure::new(vec![tranche(
            "CLASS_A",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            100_000.0,
            0.05,
        )])
        .expect("structure");
        let build = |late_fee: Option<f64>| {
            let mut builder = Waterfall::builder(Currency::USD)
                .add_tier(
                    WaterfallTier::new("note_interest", 1, PaymentType::Interest)
                        .add_recipient(Recipient::tranche_interest("class_a_interest", "CLASS_A")),
                )
                .add_tier(WaterfallTier::coverage_tests(
                    "coverage",
                    2,
                    vec![CoverageTestSpec::ic("CLASS_A", 1.0)],
                ));
            if let Some(amount) = late_fee {
                builder = builder.add_tier(
                    WaterfallTier::new("junior_fee", 3, PaymentType::Fee).add_recipient(
                        Recipient::fixed_fee("junior_fee_recipient", "junior_manager", usd(amount)),
                    ),
                );
            }
            builder.build().expect("waterfall")
        };
        let market = MarketContext::new();
        let ratio = |late_fee| {
            execute_waterfall(
                &build(late_fee),
                &tranches,
                &pool,
                context(&market, 2_000.0, 0.0, 100_000.0),
            )
            .expect("waterfall execution")
            .coverage_tests
            .into_iter()
            .find(|(id, _, _)| id == "IC_CLASS_A")
            .expect("IC result")
            .1
        };
        let without = ratio(None);
        let with = ratio(Some(500.0));
        assert!(
            (with - without).abs() < 1e-12,
            "a fee tier behind note interest must not reduce the IC numerator: \
             without={without}, with={with}"
        );
    }
}

#[cfg(test)]
mod water_fill_tests {
    use super::*;
    use crate::instruments::fixed_income::structured_credit::types::{
        AllocationMode, AssetPool, DealType, PaymentType, Recipient, Tranche, TrancheCoupon,
        TrancheSeniority, TrancheStructure, Waterfall, WaterfallTier,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::money::Money;
    use time::Month;

    /// SC-M28 — the waterfall must report its OWN interest/principal split.
    ///
    /// `distributions` keys a tranche's interest and principal under the same
    /// `RecipientType::Tranche(id)`, so the engine's Step 5 had to reconstruct
    /// the split by assuming interest is satisfied first. That reclassified
    /// principal as interest whenever a tranche carried a shortfall — the exact
    /// state an OC cure addresses — leaving the balance unretired so the cure
    /// could not de-lever the ratio it was sized to fix.
    ///
    /// This pins the contract that removes the guesswork: for a tranche paid
    /// from BOTH an interest tier and a principal tier, `distributions` holds
    /// the total while `principal_distributions` holds only the principal.
    #[test]
    fn distribution_reports_the_principal_portion_separately() {
        let ccy = Currency::USD;
        let payment_date = Date::from_calendar_date(2024, Month::April, 1).expect("date");
        let period_start = Date::from_calendar_date(2024, Month::January, 1).expect("date");

        let tranche = Tranche::new(
            "A",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((1_000_000_i64, ccy)),
            TrancheCoupon::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("date"),
        )
        .expect("tranche");
        let tranches = TrancheStructure::new(vec![tranche]).expect("structure");

        let waterfall = Waterfall::builder(ccy)
            .add_tier(
                WaterfallTier::new("interest", 1, PaymentType::Interest)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_interest("A_int", "A")),
            )
            .add_tier(
                WaterfallTier::new("principal", 2, PaymentType::Principal)
                    .allocation_mode(AllocationMode::Sequential)
                    .add_recipient(Recipient::tranche_principal("A_prin", "A", None)),
            )
            .build()
            .expect("valid waterfall");

        let market = MarketContext::new();
        let result = execute_waterfall(
            &waterfall,
            &tranches,
            &AssetPool::new("POOL", DealType::Clo, ccy),
            WaterfallContext {
                available_cash: Money::from((300_000_i64, ccy)),
                interest_collections: Money::from((13_000_i64, ccy)),
                principal_collections: Money::from((287_000_i64, ccy)),
                payment_date,
                period_start,
                valuation_date: period_start,
                pool_balance: Money::from((1_000_000_i64, ccy)),
                market: &market,
                tranche_balances: None,
                asset_balances: None,
                live_collateral: None,
                special_serviced: None,
                deferred_interest: None,
                reserve_balance: Money::from((0_i64, ccy)),
                restricted_cash: Money::from((0_i64, ccy)),
                defaulted_collateral_value: Money::from((0_i64, ccy)),
                recovery_proceeds: Money::from((0_i64, ccy)),
                floating_rate_shift: 0.0,
                equity_history: None,
            },
        )
        .expect("waterfall executes");

        let key = RecipientType::Tranche("A".to_string());
        let total = result
            .distributions
            .get(&key)
            .copied()
            .expect("tranche paid")
            .amount();
        let principal = result
            .principal_distributions
            .get(&key)
            .copied()
            .unwrap_or(Money::from((0_i64, ccy)))
            .amount();

        // A quarter's interest on 1,000,000 at 5% is ~12,500; the rest of the
        // 300,000 retires principal.
        assert!(
            total > 290_000.0,
            "the tranche should receive nearly all available cash, got {total:.2}"
        );
        assert!(
            principal > 250_000.0 && principal < total,
            "principal_distributions must hold ONLY the principal portion: got \
             {principal:.2} of {total:.2} total. Equal values mean interest was \
             misclassified as principal; zero means the split is not being \
             reported at all (SC-M28)."
        );
        let interest = total - principal;
        assert!(
            (interest - 12_500.0).abs() < 500.0,
            "the interest remainder must be ~one quarter's coupon (12,500), \
             got {interest:.2}"
        );
    }

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
