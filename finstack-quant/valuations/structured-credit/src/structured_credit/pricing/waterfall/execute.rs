//! Waterfall execution engine.

use super::super::coverage::{TestContext, TestResult};
use crate::structured_credit::types::{
    AllocationMode, AssetPool, CoverageRules, CoverageTestAction, CoverageTestSpec,
    DiversionRecord, EquityHistory, FundingSource, LiveCollateral, PaymentType, RecipientType,
    TrancheStructure, Waterfall, WaterfallDistribution,
};
use finstack_quant_core::Error;
use finstack_quant_core::FxHashMap;
use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

use super::allocation::{AllocationContext, AllocationOutput};
use super::allocation::{allocate_pro_rata, allocate_sequential};
use super::payments::{SeniorFeeInputs, interest_claim_caps, senior_fee_accrual};

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
    /// Contractual end of coupon and fee accrual, before payment adjustment.
    pub accrual_end: Date,
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
    pub tranche_balances: Option<&'a FxHashMap<String, Money>>,
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
    pub deferred_interest: Option<&'a FxHashMap<String, Money>>,
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
    /// Simulated shift applied to FLOATING tranche coupons (OAS rate
    /// path). Zero outside OAS runs. Applied before any available-funds cap so
    /// the interest the waterfall allocates matches the interest the engine
    /// records on the same rate path.
    pub floating_rate_shift: f64,
    /// Equity's cash to date, the input of any
    /// [`PaymentCalculation::IncentiveFee`](crate::structured_credit::PaymentCalculation::IncentiveFee)
    /// recipient's hurdle test; `None`
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
    Money::from_model_value(
        pool.assets
            .iter()
            .enumerate()
            .filter(|(index, asset)| {
                special_serviced.map_or(asset.special_servicing.is_some(), |flags| {
                    flags.get(*index).copied().unwrap_or(false)
                }) && !asset.defaulted
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
    let mut tiers: Vec<_> = waterfall.tiers.iter().collect();
    tiers.sort_by_key(|tier| tier.priority);
    for pair in tiers.windows(2) {
        if pair[0].priority == pair[1].priority {
            return Err(Error::Validation(format!(
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
        || classified_cash.currency() != waterfall.currency
    {
        return Err(Error::Validation(format!(
            "waterfall available cash must equal nonnegative interest plus principal in its base currency: available={}, interest={}, principal={}",
            context.available_cash.amount(),
            interest_remaining.amount(),
            principal_remaining.amount()
        )));
    }
    let mut total_diverted = Money::from((0_i64, waterfall.currency));
    let mut had_diversions = false;
    let mut diversion_reason = None;

    let mut tranche_index = FxHashMap::default();
    tranche_index.reserve(tranches.tranches.len());
    for (i, t) in tranches.tranches.iter().enumerate() {
        tranche_index.insert(t.id.as_str(), i);
    }

    let special_serviced_balance = special_serviced_balance(
        pool,
        context.asset_balances,
        context.special_serviced,
        waterfall.currency,
    )?;

    let allocation_ctx = AllocationContext {
        currency: waterfall.currency,
        tranches,
        tranche_index: &tranche_index,
        pool_balance: context.pool_balance,
        special_serviced_balance,
        payment_date: context.payment_date,
        accrual_end: context.accrual_end,
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
            if let RecipientType::Tranche(tranche_id) = &recipient.recipient_type
                && !payable_principal_tranche_ids.contains(&tranche_id.as_str())
            {
                payable_principal_tranche_ids.push(tranche_id.as_str());
            }
        }
    }

    // Senior fees rank ahead of every note, so IC uses
    // `(interest collections − senior fees) / note interest due`.
    let senior_fees = senior_fee_accrual(
        waterfall,
        tranches,
        allocation_ctx.tranche_index,
        SeniorFeeInputs {
            available: context.interest_collections,
            tranche_balances: context.tranche_balances,
            deferred_interest: context.deferred_interest,
            pool_balance: context.pool_balance,
            special_serviced_balance,
            period_start: context.period_start,
            payment_date: context.payment_date,
            accrual_end: context.accrual_end,
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
    let mut allocation_output = AllocationOutput::new(waterfall.currency, estimated_recipients);

    // Cash allocated per tier, in execution order.
    let mut tier_allocations = Vec::with_capacity(waterfall.tiers.len());

    // Interest a `Reinvest` test retained as principal proceeds this period.
    let mut diverted_to_reinvestment = Money::from((0_i64, waterfall.currency));
    // Principal proceeds spent by `InterestThenPrincipal` tiers.
    let mut principal_used_for_interest = Money::from((0_i64, waterfall.currency));
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
                    && !result.passing
                {
                    failing.push(test.id.as_str());
                    if let Some(cure) = result.cure_amount {
                        binding_cure = binding_cure.max(cure.amount());
                    }
                }
            }
            if failing.is_empty() {
                tier_allocations.push((tier.id.clone(), Money::from((0_i64, waterfall.currency))));
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
            let divertible = Money::from_model_value(divertible, waterfall.currency)?;
            let action = tier
                .tests
                .first()
                .map(|test| test.action)
                .unwrap_or_default();
            let tier_cash = match action {
                CoverageTestAction::PayDownSenior => {
                    // Pay the earliest principal tier in its configured
                    // recipient order. Early principal is booked in
                    // `allocation_output.principal_paid_in_period` so the principal tier nets
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
                        )?,
                        _ => Money::from((0_i64, waterfall.currency)),
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
            )?,
            AllocationMode::ProRata => allocate_pro_rata(
                &allocation_ctx,
                tier,
                &tier.recipients[..],
                remaining,
                context.period_start,
                false,
                &mut allocation_output,
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
                let from_interest = Money::from_model_value(
                    tier_cash.amount().min(interest_remaining.amount()).max(0.0),
                    waterfall.currency,
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

    // The executor's infinite ratio is its zero denominator: no claim is
    // outstanding for the test, so no ratio exists to report.
    let coverage_tests_public: Vec<(String, Option<f64>, bool)> = coverage_test_results
        .iter()
        .map(|r| {
            (
                r.test_id.clone(),
                r.ratio.is_finite().then_some(r.ratio),
                r.passing,
            )
        })
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
        payment_records: allocation_output.payment_records,
        coverage_tests: coverage_tests_public,
        diverted_cash: total_diverted,
        remaining_cash: interest_remaining.checked_add(principal_remaining)?,
        remaining_interest: interest_remaining,
        remaining_principal: principal_remaining,
        principal_used_for_interest,
        had_diversions,
        diverted_amounts,
        recovery_proceeds: context.recovery_proceeds,
    })
}

/// Interest claims and coverage rules a coverage test reads from the
/// waterfall it is evaluated in.
///
/// The waterfall spec defines each tranche's interest CLAIM (uncapped,
/// capped, or absent); the IC test must measure coverage of those claims,
/// not of raw coupons the structure never owes. Caps in this waterfall are
/// already resolved (the live AFC cap is baked in by `resolve::apply_afc_cap`),
/// so no AFC override applies here.
pub(crate) fn coverage_inputs(
    waterfall: &Waterfall,
) -> (FxHashMap<&str, Option<f64>>, Option<&CoverageRules>) {
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
pub(crate) fn evaluate_coverage_tests(
    specs: &[&CoverageTestSpec],
    context: &TestContext<'_>,
) -> Result<Vec<TestResult>> {
    specs.iter().map(|spec| spec.evaluate(context)).collect()
}
