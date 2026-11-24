//! Coverage test evaluator implementation.

use crate::structured_credit::types::{
    AssetPool, CoverageRules, CoverageTestSpec, CoverageTestType, LiveCollateral, PoolAsset,
    Tranche, TrancheStructure,
};
use finstack_quant_core::Error;
use finstack_quant_core::FxHashMap;
use finstack_quant_core::Result;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::units::BASIS_POINTS_PER_UNIT;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CreditRating;

use serde::{Deserialize, Serialize};

impl CoverageTestSpec {
    /// Evaluate the test on `context`: the ratio for the tested tranche and
    /// every class senior to it against `trigger_level`, and the cure when it
    /// fails.
    ///
    /// # Arguments
    ///
    /// * `context` - The period's pool, tranche balances, collections and
    ///   coverage rules.
    ///
    /// # Errors
    ///
    /// Returns an error when the tested tranche is missing, a borrowing-base
    /// test has no `coverage_rules.borrowing_base`, or a balance, rate or
    /// money computation fails.
    pub fn evaluate(&self, context: &TestContext) -> Result<TestResult> {
        match self.kind {
            CoverageTestType::Oc => oc_test(self, context),
            CoverageTestType::Ic => ic_test(self, context),
            CoverageTestType::BorrowingBase => borrowing_base_test(self, context),
        }
    }
}

/// Borrowing base over the tested stack: numerator from the live asset
/// balances (or the closing balances) under `rules.borrowing_base`, the
/// OC denominator; the cure is the paydown that restores the ratio.
fn borrowing_base_test(spec: &CoverageTestSpec, context: &TestContext) -> Result<TestResult> {
    let (test_id, tranche_id, trigger_level) = (
        spec.id.clone(),
        spec.tranche_id.as_str(),
        spec.trigger_level,
    );
    let rules = context
        .rules
        .and_then(|rules| rules.borrowing_base.as_ref())
        .ok_or_else(|| {
            Error::Validation(format!(
                "borrowing-base test {test_id} needs coverage_rules.borrowing_base"
            ))
        })?;
    let tranche = context.tranches.require(tranche_id)?;
    let tranche_balance = context
        .tranche_balances
        .and_then(|b| b.get(tranche.id.as_str()))
        .copied()
        .unwrap_or(tranche.current_balance);
    let senior_balance = if let Some(tb) = context.tranche_balances {
        context.tranches.senior_to(tranche_id).iter().try_fold(
            Money::from((0_i64, tranche_balance.currency())),
            |acc, t| {
                let bal = tb.get(t.id.as_str()).copied().unwrap_or(t.current_balance);
                acc.checked_add(bal)
            },
        )?
    } else {
        context.tranches.senior_balance(tranche_id)
    };
    // Cash held for the collateral counts at a 100% advance rate: the
    // principal funding account and this period's principal collections
    // (both sit in the trust's accounts until reinvested or applied).
    let numerator = rules
        .evaluate_live(
            context.pool,
            context.asset_balances,
            context.live_collateral,
        )?
        .borrowing_base
        .checked_add(context.restricted_cash)?
        .checked_add(context.cash_balance)?;
    let denominator = tranche_balance.checked_add(senior_balance)?;
    let ratio = if denominator.amount() > 0.0 {
        numerator.amount() / denominator.amount()
    } else {
        f64::INFINITY
    };
    let passing = ratio >= trigger_level;
    let cure_amount = if !passing && trigger_level > 0.0 {
        let paydown = (denominator.amount() - numerator.amount() / trigger_level)
            .max(0.0)
            .min(denominator.amount());
        Some(Money::from_model_value(paydown, denominator.currency())?)
    } else {
        None
    };
    Ok(TestResult {
        test_id,
        tranche_id: tranche_id.to_string(),
        ratio,
        passing,
        cure_amount,
    })
}

/// Overcollateralization: performing collateral (after the coverage rules)
/// plus the period's principal collections, trust-held cash and defaulted
/// collateral at its recovery value, over the tested class and its seniors.
fn oc_test(spec: &CoverageTestSpec, context: &TestContext) -> Result<TestResult> {
    let (test_id, tranche_id, trigger_level) = (
        spec.id.clone(),
        spec.tranche_id.as_str(),
        spec.trigger_level,
    );
    let tranche = context.tranches.require(tranche_id)?;

    // Coverage tests use live tranche balances when available.
    let tranche_balance = context
        .tranche_balances
        .and_then(|b| b.get(tranche.id.as_str()))
        .copied()
        .unwrap_or(tranche.current_balance);

    let senior_balance = if let Some(tb) = context.tranche_balances {
        context.tranches.senior_to(tranche_id).iter().try_fold(
            Money::from((0_i64, tranche_balance.currency())),
            |acc, t| {
                let bal = tb.get(t.id.as_str()).copied().unwrap_or(t.current_balance);
                acc.checked_add(bal)
            },
        )?
    } else {
        context.tranches.senior_balance(tranche_id)
    };

    // Apply rating haircuts to live per-asset notionals. If only an
    // aggregate current balance is available, preserve that balance and
    // approximate composition with the closing pool's haircut factor.
    let mut numerator = if let Some(asset_balances) = context.asset_balances {
        collateral_value(context.pool, context.rules, Some(asset_balances))?
    } else {
        match context.current_pool_balance {
            Some(current) if context.rules.is_some_and(CoverageRules::adjusts_collateral) => {
                let gross = collateral_value(context.pool, None, None)?;
                let haircut = collateral_value(context.pool, context.rules, None)?;
                let factor = if gross.amount() > 0.0 {
                    (haircut.amount() / gross.amount()).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                Money::from_model_value(current.amount() * factor, current.currency())?
            }
            Some(current) => current,
            None => collateral_value(context.pool, context.rules, None)?,
        }
    };

    // This period's principal collections and the trust-held collateral
    // cash (the funding account, an accumulated balance) both secure the
    // notes.
    numerator = numerator.checked_add(context.cash_balance)?;
    numerator = numerator.checked_add(context.restricted_cash)?;
    // Defaulted collateral is carried at its modeled recovery value until
    // the recovery cash arrives (CLO indenture convention: defaulted
    // obligations at the lower of market value and the assumed recovery),
    // so a default reduces par-OC by the expected loss, not by the whole
    // defaulted par.
    numerator = numerator.checked_add(context.defaulted_collateral_value)?;

    // OC denominator = test tranche balance + all senior tranche balances
    // i.e., Sum(all tranche balances at this seniority level and above)
    let denominator = tranche_balance.checked_add(senior_balance)?;

    let ratio = if denominator.amount() > 0.0 {
        numerator.amount() / denominator.amount()
    } else {
        f64::INFINITY
    };

    let passing = ratio >= trigger_level;

    // Note paydown needed to restore OC. The cure is paid from INTEREST
    // proceeds (the executor diverts the interest ranked below the test
    // position), which never enter the OC numerator, so the diversion
    // only shrinks the denominator:
    //   numerator / (denominator - X) >= trigger_level
    //   => X >= denominator - numerator / trigger_level
    // A diversion cannot retire more than the OC stack, so cap the cure at
    // the denominator.
    let cure_amount = if !passing && trigger_level > 0.0 {
        let paydown_needed = denominator.amount() - numerator.amount() / trigger_level;
        let capped = paydown_needed.max(0.0).min(denominator.amount());
        Some(Money::from_model_value(capped, denominator.currency())?)
    } else {
        None
    };

    Ok(TestResult {
        test_id,
        tranche_id: tranche_id.to_string(),
        ratio,
        passing,
        cure_amount,
    })
}

/// Interest coverage: interest collections net of senior fees over the
/// interest due to the tested class and its seniors.
fn ic_test(spec: &CoverageTestSpec, context: &TestContext) -> Result<TestResult> {
    let (test_id, tranche_id, trigger_level) = (
        spec.id.clone(),
        spec.tranche_id.as_str(),
        spec.trigger_level,
    );
    let tranche = context.tranches.require(tranche_id)?;

    // Coverage tests use live tranche balances when available.
    let tranche_bal = context
        .tranche_balances
        .and_then(|b| b.get(tranche.id.as_str()))
        .copied()
        .unwrap_or(tranche.current_balance);

    let (tranche_rate, accrual_factor) = rate_and_accrual(tranche, context)?;

    let interest_due = Money::from_model_value(
        tranche_bal.amount() * tranche_rate * accrual_factor
            + carried_deferred(context, tranche.id.as_str()),
        tranche_bal.currency(),
    )?;

    let senior_tranches = context.tranches.senior_to(tranche_id);

    let mut delevering_set = vec![tranche];
    delevering_set.extend(senior_tranches.iter().copied());
    let mut payable_tranches = Vec::with_capacity(context.tranches.tranches.len());
    if let Some(payable_ids) = context.payable_principal_tranche_ids {
        for payable_id in payable_ids {
            if let Some(payable) = context
                .tranches
                .tranches
                .iter()
                .find(|candidate| candidate.id.as_str() == *payable_id)
                && !payable_tranches
                    .iter()
                    .any(|(existing, _): &(&Tranche, bool)| existing.id == payable.id)
            {
                let reduces_test_interest = delevering_set
                    .iter()
                    .any(|candidate| candidate.id == payable.id);
                payable_tranches.push((payable, reduces_test_interest));
            }
        }
    } else {
        payable_tranches.extend(
            delevering_set
                .iter()
                .copied()
                .map(|candidate| (candidate, true)),
        );
        payable_tranches.sort_by_key(|(candidate, _)| candidate.payment_priority);
    }

    let senior_interest_due = senior_tranches.iter().try_fold(
        Money::from((0_i64, interest_due.currency())),
        |acc, t| {
            let (rate, accrual) = rate_and_accrual(t, context)?;
            let t_bal = context
                .tranche_balances
                .and_then(|b| b.get(t.id.as_str()))
                .copied()
                .unwrap_or(t.current_balance);
            let interest = Money::from_model_value(
                t_bal.amount() * rate * accrual + carried_deferred(context, t.id.as_str()),
                t_bal.currency(),
            )?;
            acc.checked_add(interest)
        },
    )?;

    let total_interest_due = interest_due.checked_add(senior_interest_due)?;

    // Senior fees rank ahead of every note and reduce the IC numerator.
    let net_collections =
        (context.interest_collections.amount() - context.senior_fees.amount()).max(0.0);

    let ratio = if total_interest_due.amount() > 0.0 {
        net_collections / total_interest_due.amount()
    } else {
        f64::INFINITY
    };

    let passing = ratio >= trigger_level;

    // Size the cure as principal paydown in the diversion tier's recipient
    // order. Every recipient consumes cure cash, while only tested-or-senior
    // notes with positive `rate × accrual` reduce interest due toward
    // `net_collections / trigger_level`. Exhausted stacks return the
    // principal consumed; an absent payable stack reports the cash shortfall.
    let cure_amount = if !passing {
        let cash_shortfall =
            (trigger_level * total_interest_due.amount() - net_collections).max(0.0);
        let cure = if trigger_level > 0.0 {
            let target_due = net_collections / trigger_level;
            let mut remaining_reduction = (total_interest_due.amount() - target_due).max(0.0);
            let mut principal_cure = 0.0;
            let mut found_payable_principal = false;
            for (payable, reduces_test_interest) in &payable_tranches {
                let balance = context
                    .tranche_balances
                    .and_then(|balances| balances.get(payable.id.as_str()))
                    .copied()
                    .unwrap_or(payable.current_balance)
                    .amount()
                    .max(0.0);
                if balance <= 0.0 {
                    continue;
                }
                found_payable_principal = true;
                if !reduces_test_interest {
                    principal_cure += balance;
                    continue;
                }
                let (rate, accrual) = rate_and_accrual(payable, context)?;
                let rate_accrual = rate * accrual;
                if rate_accrual <= 1e-12 {
                    principal_cure += balance;
                    continue;
                }
                let paydown = (remaining_reduction / rate_accrual).min(balance);
                principal_cure += paydown;
                remaining_reduction = (remaining_reduction - paydown * rate_accrual).max(0.0);
                if remaining_reduction <= 1e-12 {
                    break;
                }
            }
            if found_payable_principal {
                principal_cure
            } else {
                cash_shortfall
            }
        } else {
            cash_shortfall
        };
        Some(Money::from_model_value(
            cure,
            context.interest_collections.currency(),
        )?)
    } else {
        None
    };

    Ok(TestResult {
        test_id,
        tranche_id: tranche_id.to_string(),
        ratio,
        passing,
        cure_amount,
    })
}

fn carried_deferred(context: &TestContext<'_>, tranche_id: &str) -> f64 {
    // Match the waterfall interest claim: current coupon plus unpaid arrears.
    // A tranche with no interest recipient (absent claim-cap key) owes nothing,
    // including prior deferrals that the structure cannot pay.
    if !context.interest_claim_caps.contains_key(tranche_id) {
        return 0.0;
    }
    // PIK shortfalls accrete into balance (OC denominator), not the deferred
    // map. A stale deferred entry must not inflate the IC denominator.
    if context
        .tranches
        .tranches
        .iter()
        .any(|tranche| tranche.id.as_str() == tranche_id && tranche.pik_enabled)
    {
        return 0.0;
    }
    context
        .deferred_interest
        .and_then(|deferred| deferred.get(tranche_id))
        .map(|amount| amount.amount().max(0.0))
        .unwrap_or(0.0)
}

fn rate_and_accrual(tranche: &Tranche, context: &TestContext<'_>) -> Result<(f64, f64)> {
    // Use the contractual all-in rate when market context is available.
    // Missing historical fixings invalidate the coverage test rather than
    // silently reverting to spread-only.
    let period_start = context.period_start.unwrap_or(context.as_of);
    let period_end = if context.period_start.is_some() {
        context.as_of
    } else {
        // Point-in-time IC uses the next configured coupon interval to express
        // the projected annual index in the tranche's contractual convention.
        tranche.frequency.add_to_date(
            period_start,
            None,
            finstack_quant_core::dates::BusinessDayConvention::Unadjusted,
        )?
    };
    let projection_accrual = tranche.day_count.year_fraction(
        period_start,
        period_end,
        finstack_quant_core::dates::DayCountContext::default(),
    )?;
    let rate = if let Some(market) = context.market {
        tranche.coupon.try_rate_for_period(
            period_start,
            period_end,
            projection_accrual,
            context.valuation_date,
            market,
        )?
    } else {
        match &tranche.coupon {
            finstack_quant_cashflows::builder::RateSpec::Fixed { rate } => *rate,
            finstack_quant_cashflows::builder::RateSpec::Floating(spec) => {
                use rust_decimal::prelude::ToPrimitive;
                spec.spread_bp.to_f64().unwrap_or_default() / BASIS_POINTS_PER_UNIT
            }
        }
    };
    // A floating coupon rides the simulated rate path (shift-then-floor,
    // matching the engine's interest-due kernel). Fixed coupons are contractual.
    let rate = match tranche.coupon {
        finstack_quant_cashflows::builder::RateSpec::Floating(_) => {
            (rate + context.floating_rate_shift).max(0.0)
        }
        finstack_quant_cashflows::builder::RateSpec::Fixed { .. } => rate,
    };
    // The waterfall spec defines the CLAIM the test measures coverage of:
    // capped claims accrue at the capped rate (applied after the shift, like
    // the AFC cap), and a tranche with no interest recipient owes nothing.
    let rate = match context.interest_claim_caps.get(tranche.id.as_str()) {
        None => 0.0,
        Some(None) => rate,
        Some(Some(cap)) => rate.min(*cap),
    };
    // Use actual day-count accrual when period_start is available (m3 fix);
    // fall back to periods-per-year approximation as default behavior.
    let accrual = if context.period_start.is_some() {
        projection_accrual
    } else {
        1.0 / tranche.frequency.payments_per_year()?
    };
    Ok((rate, accrual))
}

/// The period state a coverage test is evaluated on; the tested tranche and
/// trigger level come from the [`CoverageTestSpec`].
pub struct TestContext<'a> {
    /// AssetPool reference.
    pub pool: &'a AssetPool,
    /// Tranche structure reference.
    pub tranches: &'a TrancheStructure,
    /// As-of date.
    pub as_of: finstack_quant_core::dates::Date,
    #[doc = "Valuation date determining fixing availability, independent of the coverage payment date in `as_of`."]
    pub valuation_date: finstack_quant_core::dates::Date,
    /// Period start date for day-count accrual.
    pub period_start: Option<finstack_quant_core::dates::Date>,
    /// Cash balance.
    pub cash_balance: Money,
    /// Interest collections.
    pub interest_collections: Money,
    /// Collateral valuation rules for the OC numerator (rating haircuts,
    /// discount obligations, excess CCC); `None` values collateral at par.
    pub rules: Option<&'a CoverageRules>,
    /// Optional market context for floating rate index lookups in IC tests.
    pub market: Option<&'a MarketContext>,
    /// Current tranche balances (overrides `tranche.current_balance` when present).
    pub tranche_balances: Option<&'a FxHashMap<String, Money>>,
    /// Ordered tranche IDs paid by the waterfall's coverage-diversion principal tier.
    ///
    /// Direct callers may omit this to use structural `payment_priority` order.
    pub payable_principal_tranche_ids: Option<&'a [&'a str]>,
    /// Current per-asset balances, aligned by index with `pool.assets`.
    pub asset_balances: Option<&'a [f64]>,
    /// Live delinquency buckets and NPL flags for the eligibility rules.
    pub live_collateral: Option<LiveCollateral<'a>>,
    /// Aggregate current pool balance fallback when asset-level balances are unavailable.
    pub current_pool_balance: Option<Money>,
    /// Senior fees payable ahead of every note and deducted from the IC numerator.
    pub senior_fees: Money,
    /// Trust-held collateral cash outside the asset balances — currently the
    /// controlled-accumulation funding account.
    ///
    /// During accumulation, collected principal leaves the asset balances for
    /// the funding account while the notes stay outstanding; without this term
    /// the OC ratio would fall by the accumulated amount and report a breach
    /// that does not exist. Indentures count principal-collection-account cash
    /// in par-value tests.
    pub restricted_cash: Money,
    /// Recovery value of defaulted collateral whose recovery cash has not yet
    /// arrived (the pending recovery claims). Counted in the OC numerator so
    /// a default costs the test its expected loss rather than the full
    /// defaulted par.
    pub defaulted_collateral_value: Money,
    /// Per-tranche interest-claim caps extracted from the waterfall spec
    /// (see `pricing::waterfall::interest_claim_caps`): value `None` =
    /// uncapped claim, `Some(cap)` = coupon capped at `cap`, absent key = the
    /// waterfall defines no interest claim for the tranche.
    ///
    /// Always supplied from the waterfall spec, so IC measures coverage of
    /// what the structure actually owes.
    pub interest_claim_caps: &'a FxHashMap<&'a str, Option<f64>>,
    /// Simulated floating-coupon shift (OAS rate path); zero outside
    /// OAS runs. Keeps the IC due on the same rate path as collections.
    pub floating_rate_shift: f64,
    /// Outstanding non-PIK deferred interest by tranche id.
    ///
    /// The waterfall interest claim is current coupon plus these arrears.
    /// IC uses the same denominator so a carried shortfall cannot make the
    /// test look healthier than the cash the structure actually owes.
    pub deferred_interest: Option<&'a FxHashMap<String, Money>>,
}

/// Result of a coverage test calculation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct TestResult {
    /// Test identifier.
    pub test_id: String,
    /// Tranche identifier associated with the test.
    #[serde(default)]
    pub tranche_id: String,
    /// Current calculated ratio.
    pub ratio: f64,
    /// Whether test is currently passing.
    pub passing: bool,
    /// Cure amount if failing. For OC tests this is the note paydown, funded
    /// from the interest ranked below the test, that restores the ratio
    /// (`denominator − numerator / trigger_level`); for IC tests it is the
    /// senior principal paydown needed to reduce the interest denominator
    /// enough for the test to clear.
    pub cure_amount: Option<Money>,
}

/// Whether an unrated row is unrated by construction rather than by credit:
/// a synthetic reinvestment purchase or a materialized instrument row.
fn unrated_by_construction(pool: &AssetPool, asset: &PoolAsset) -> bool {
    pool.instruments.is_some() || asset.id.as_str().starts_with("REINVEST-")
}

/// Whether `rating` is CCC+ or lower.
fn is_ccc_or_below(rating: CreditRating) -> bool {
    matches!(
        rating,
        CreditRating::CCCPlus
            | CreditRating::CCC
            | CreditRating::CCCMinus
            | CreditRating::CC
            | CreditRating::C
            | CreditRating::D
    )
}

/// Value of the collateral for an OC numerator: current par (or the closing
/// balances) after the rating haircuts, discount-obligation and excess-CCC
/// rules in `rules`; plain par when no rule adjusts collateral.
fn collateral_value(
    pool: &AssetPool,
    rules: Option<&CoverageRules>,
    current_balances: Option<&[f64]>,
) -> Result<Money> {
    if let Some(balances) = current_balances {
        if balances.len() != pool.assets.len() {
            return Err(Error::Validation(format!(
                "current asset balance count {} does not match pool asset count {}",
                balances.len(),
                pool.assets.len()
            )));
        }
        if balances
            .iter()
            .any(|balance| !balance.is_finite() || *balance < 0.0)
        {
            return Err(Error::Validation(
                "current asset balances must be finite and non-negative".to_string(),
            ));
        }
    }

    let adjusts = rules.is_some_and(CoverageRules::adjusts_collateral);
    if current_balances.is_none() && !adjusts {
        return pool.performing_balance();
    }

    let haircuts = rules.map(|r| &r.rating_haircuts);
    let discount = rules.and_then(|r| r.discount_obligation);
    let ccc = rules.and_then(|r| r.ccc_bucket);

    let mut total = 0.0_f64;
    let mut performing_par = 0.0_f64;
    let mut ccc_par = 0.0_f64;
    let mut ccc_market_value = 0.0_f64;
    for (index, asset) in pool.assets.iter().enumerate() {
        if asset.defaulted {
            continue;
        }
        let balance = current_balances
            .and_then(|balances| balances.get(index))
            .copied()
            .unwrap_or_else(|| asset.balance.amount());
        performing_par += balance;

        // Carry each asset at the lowest of par, haircut par and the discount
        // obligation's purchase price. Haircuts key by rating bucket; the NR
        // entry covers unrated rows the caller supplied, not rows that are
        // unrated by construction (reinvestment purchases, instrument rows).
        let mut carried = balance;
        if let Some(map) = haircuts {
            let haircut = match asset.rating {
                Some(rating) => map
                    .get(&rating.bucket())
                    .or_else(|| map.get(&rating))
                    .copied(),
                None if unrated_by_construction(pool, asset) => None,
                None => map.get(&CreditRating::NR).copied(),
            }
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
            carried = carried.min(balance * (1.0 - haircut));
        }
        if let (Some(rule), Some(price)) = (discount, asset.purchase_price) {
            let closing_par = asset.balance.amount();
            if closing_par > 0.0 {
                let price_pct = 100.0 * price.amount() / closing_par;
                if price_pct < rule.price_threshold_pct {
                    carried = carried.min(balance * price_pct / 100.0);
                }
            }
        }
        if ccc.is_some() && asset.rating.is_some_and(is_ccc_or_below) {
            ccc_par += balance;
            ccc_market_value += balance * asset.market_price_pct.unwrap_or(100.0) / 100.0;
        }
        total += carried;
    }

    // Excess CCC: the par above the bucket is carried at the CCC assets'
    // average market price (or excluded), never above par.
    if let Some(rule) = ccc {
        let allowed = performing_par * rule.threshold_pct / 100.0;
        let excess = (ccc_par - allowed).max(0.0);
        if excess > 0.0 {
            let market_fraction = if ccc_par > 0.0 {
                (ccc_market_value / ccc_par).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let carried_fraction = if rule.carry_at_market_value {
                market_fraction
            } else {
                0.0
            };
            total -= excess * (1.0 - carried_fraction);
        }
    }

    Money::from_model_value(total.max(0.0), pool.get_currency())
}
