//! Coverage test calculations for structured credit instruments.
//!
//! This module provides OC and IC test calculations for waterfall diversion.

use crate::instruments::fixed_income::structured_credit::types::{
    AssetPool, CoverageRules, CoverageTestSpec, CoverageTestType, LiveCollateral, PoolAsset,
    Tranche, TrancheStructure,
};
use crate::instruments::fixed_income::structured_credit::utils::frequency_periods_per_year;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CreditRating;
use finstack_quant_core::HashMap;
use finstack_quant_core::Result;
use finstack_quant_core::{Error as CoreError, InputError};

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
    let (test_id, tranche_id, required_ratio) = (
        spec.id.clone(),
        spec.tranche_id.as_str(),
        spec.trigger_level,
    );
    let rules = context
        .rules
        .and_then(|rules| rules.borrowing_base.as_ref())
        .ok_or_else(|| {
            CoreError::Validation(format!(
                "borrowing-base test {test_id} needs coverage_rules.borrowing_base"
            ))
        })?;
    let tranche = context
        .tranches
        .tranches
        .iter()
        .find(|t| t.id.as_str() == tranche_id)
        .ok_or_else(|| {
            CoreError::from(InputError::NotFound {
                id: format!("tranche:{}", tranche_id),
            })
        })?;
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
    let is_passing = ratio >= required_ratio;
    let cure_amount = if !is_passing && required_ratio > 0.0 {
        let paydown = (denominator.amount() - numerator.amount() / required_ratio)
            .max(0.0)
            .min(denominator.amount());
        Some(Money::new(paydown, denominator.currency())?)
    } else {
        None
    };
    Ok(TestResult {
        test_id,
        tranche_id: tranche_id.to_string(),
        current_ratio: ratio,
        is_passing,
        cure_amount,
    })
}

/// Overcollateralization: performing collateral (after the coverage rules)
/// plus the period's principal collections, trust-held cash and defaulted
/// collateral at its recovery value, over the tested class and its seniors.
fn oc_test(spec: &CoverageTestSpec, context: &TestContext) -> Result<TestResult> {
    let (test_id, tranche_id, required_ratio) = (
        spec.id.clone(),
        spec.tranche_id.as_str(),
        spec.trigger_level,
    );
    let tranche = context
        .tranches
        .tranches
        .iter()
        .find(|t| t.id.as_str() == tranche_id)
        .ok_or_else(|| {
            CoreError::from(InputError::NotFound {
                id: format!("tranche:{}", tranche_id),
            })
        })?;

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
                Money::new(current.amount() * factor, current.currency())?
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

    let is_passing = ratio >= required_ratio;

    // Note paydown needed to restore OC. The cure is paid from INTEREST
    // proceeds (the executor diverts the interest ranked below the test
    // position), which never enter the OC numerator, so the diversion
    // only shrinks the denominator:
    //   numerator / (denominator - X) >= required_ratio
    //   => X >= denominator - numerator / required_ratio
    // A diversion cannot retire more than the OC stack, so cap the cure at
    // the denominator.
    let cure_amount = if !is_passing && required_ratio > 0.0 {
        let paydown_needed = denominator.amount() - numerator.amount() / required_ratio;
        let capped = paydown_needed.max(0.0).min(denominator.amount());
        Some(Money::new(capped, denominator.currency())?)
    } else {
        None
    };

    Ok(TestResult {
        test_id,
        tranche_id: tranche_id.to_string(),
        current_ratio: ratio,
        is_passing,
        cure_amount,
    })
}

/// Interest coverage: interest collections net of senior fees over the
/// interest due to the tested class and its seniors.
fn ic_test(spec: &CoverageTestSpec, context: &TestContext) -> Result<TestResult> {
    let (test_id, tranche_id, required_ratio) = (
        spec.id.clone(),
        spec.tranche_id.as_str(),
        spec.trigger_level,
    );
    let tranche = context
        .tranches
        .tranches
        .iter()
        .find(|t| t.id.as_str() == tranche_id)
        .ok_or_else(|| {
            CoreError::from(InputError::NotFound {
                id: format!("tranche:{}", tranche_id),
            })
        })?;

    // Coverage tests use live tranche balances when available.
    let tranche_bal = context
        .tranche_balances
        .and_then(|b| b.get(tranche.id.as_str()))
        .copied()
        .unwrap_or(tranche.current_balance);

    let (tranche_rate, accrual_factor) = rate_and_accrual(tranche, context)?;

    let interest_due = Money::new(
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
            {
                if !payable_tranches
                    .iter()
                    .any(|(existing, _): &(&Tranche, bool)| existing.id == payable.id)
                {
                    let reduces_test_interest = delevering_set
                        .iter()
                        .any(|candidate| candidate.id == payable.id);
                    payable_tranches.push((payable, reduces_test_interest));
                }
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
            let interest = Money::new(
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

    let is_passing = ratio >= required_ratio;

    // Size the cure as principal paydown in the diversion tier's recipient
    // order. Every recipient consumes cure cash, while only tested-or-senior
    // notes with positive `rate × accrual` reduce interest due toward
    // `net_collections / required_ratio`. Exhausted stacks return the
    // principal consumed; an absent payable stack reports the cash shortfall.
    let cure_amount = if !is_passing {
        let cash_shortfall =
            (required_ratio * total_interest_due.amount() - net_collections).max(0.0);
        let cure = if required_ratio > 0.0 {
            let target_due = net_collections / required_ratio;
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
        Some(Money::new(cure, context.interest_collections.currency())?)
    } else {
        None
    };

    Ok(TestResult {
        test_id,
        tranche_id: tranche_id.to_string(),
        current_ratio: ratio,
        is_passing,
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
    let rate = if let Some(market) = context.market {
        if let Some(period_start) = context.period_start {
            tranche
                .coupon
                .try_rate_for_period(period_start, context.valuation_date, market)?
        } else {
            tranche
                .coupon
                .try_rate_for_period(context.as_of, context.valuation_date, market)?
        }
    } else {
        tranche.coupon.current_rate(context.as_of)
    };
    // A floating coupon rides the simulated rate path (shift-then-floor,
    // matching the engine's interest-due kernel). Fixed coupons are contractual.
    let rate = match tranche.coupon {
        crate::instruments::fixed_income::structured_credit::types::TrancheCoupon::Floating(_) => {
            (rate + context.floating_rate_shift).max(0.0)
        }
        _ => rate,
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
    let accrual = if let Some(period_start) = context.period_start {
        tranche
            .day_count
            .year_fraction(
                period_start,
                context.as_of,
                finstack_quant_core::dates::DayCountContext::default(),
            )
            .unwrap_or_else(|_| 1.0 / frequency_periods_per_year(tranche.frequency))
    } else {
        1.0 / frequency_periods_per_year(tranche.frequency)
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
    pub tranche_balances: Option<&'a HashMap<String, Money>>,
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
    /// N2: during accumulation, collected principal leaves the asset balances
    /// and is diverted into the funding account, so it appeared in NEITHER the
    /// collateral term NOR the cash term of the test. The OC ratio decayed by
    /// exactly the accumulated amount while the denominator stayed flat,
    /// producing a breach that does not exist — which then overrode the
    /// accumulation lockout, since a diverted pass zeroes principal targets.
    /// Real indentures count principal-collection-account cash in par-value
    /// tests.
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
    pub interest_claim_caps: &'a HashMap<&'a str, Option<f64>>,
    /// Simulated floating-coupon shift (OAS rate path); zero outside
    /// OAS runs. Keeps the IC due on the same rate path as collections.
    pub floating_rate_shift: f64,
    /// Outstanding non-PIK deferred interest by tranche id.
    ///
    /// The waterfall interest claim is current coupon plus these arrears.
    /// IC uses the same denominator so a carried shortfall cannot make the
    /// test look healthier than the cash the structure actually owes.
    pub deferred_interest: Option<&'a HashMap<String, Money>>,
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
    pub current_ratio: f64,
    /// Whether test is currently passing.
    pub is_passing: bool,
    /// Cure amount if failing. For OC tests this is the note paydown, funded
    /// from the interest ranked below the test, that restores the ratio
    /// (`denominator − numerator / required_ratio`); for IC tests it is the
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
            return Err(CoreError::Validation(format!(
                "current asset balance count {} does not match pool asset count {}",
                balances.len(),
                pool.assets.len()
            )));
        }
        if balances
            .iter()
            .any(|balance| !balance.is_finite() || *balance < 0.0)
        {
            return Err(CoreError::Validation(
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
        if asset.is_defaulted {
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
            let haircut = match asset.credit_quality {
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
        if ccc.is_some() && asset.credit_quality.is_some_and(is_ccc_or_below) {
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

    Money::new(total.max(0.0), pool.get_base_currency())
}

#[cfg(test)]
/// The engine always supplies a claim map. Tests that want "every tranche
/// owes its full uncapped coupon" state that explicitly with this helper.
fn uncapped_claims(ids: &[&'static str]) -> HashMap<&'static str, Option<f64>> {
    ids.iter().map(|id| (*id, None)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::structured_credit::types::{
        AssetPool, DealType, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use time::Month;

    #[test]
    fn test_oc_test_creation() {
        let test = CoverageTestSpec::oc("A", 1.15);
        assert_eq!(test.trigger_level, 1.15);
    }

    #[test]
    fn test_oc_test_calculation() {
        let pool = AssetPool::new("TEST", DealType::Clo, Currency::USD);
        let test = CoverageTestSpec::oc("TEST_TRANCHE", 1.25);

        let tranche = Tranche::new(
            "TEST_TRANCHE",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((100_000_i64, Currency::USD)),
            TrancheCoupon::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("Valid date"),
        )
        .expect("Valid tranche");

        let tranches = TrancheStructure::new(vec![tranche]).expect("Valid tranche structure");

        let context = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            valuation_date: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            period_start: None,
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::from((0_i64, Currency::USD)),
            rules: None,
            market: None,
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: None,
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["TEST_TRANCHE"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = test.evaluate(&context).expect("calculation should succeed");

        assert_eq!(result.current_ratio, 0.0);
        assert!(!result.is_passing);
    }

    #[test]
    fn test_ic_test_calculation() {
        let pool = AssetPool::new("TEST", DealType::Clo, Currency::USD);
        let test = CoverageTestSpec::ic("TEST_TRANCHE", 1.20);

        let tranche = Tranche::new(
            "TEST_TRANCHE",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((100_000_i64, Currency::USD)),
            TrancheCoupon::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("Valid date"),
        )
        .expect("Valid tranche");

        let tranches = TrancheStructure::new(vec![tranche]).expect("Valid tranche structure");

        let context = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            valuation_date: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            period_start: None,
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::from((1_500_i64, Currency::USD)),
            rules: None,
            market: None,
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: None,
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["TEST_TRANCHE"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = test.evaluate(&context).expect("calculation should succeed");

        assert!((result.current_ratio - 1.2).abs() < 0.01);
        assert!(result.is_passing);
    }

    /// IC must cover the same claim the waterfall pays: current coupon plus
    /// unpaid non-PIK arrears. Omitting deferred from the denominator makes a
    /// deal with carried shortfalls look healthier than the cash it owes.
    #[test]
    fn ic_denominator_includes_deferred_arrears() {
        let pool = AssetPool::new("TEST", DealType::Clo, Currency::USD);
        let test = CoverageTestSpec::ic("TEST_TRANCHE", 1.20);
        let tranche = Tranche::new(
            "TEST_TRANCHE",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((100_000_i64, Currency::USD)),
            TrancheCoupon::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("Valid date"),
        )
        .expect("Valid tranche");
        let tranches = TrancheStructure::new(vec![tranche]).expect("Valid tranche structure");
        let mut deferred = HashMap::default();
        deferred.insert(
            "TEST_TRANCHE".to_string(),
            Money::from((1_250_i64, Currency::USD)),
        );
        let context = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            valuation_date: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            period_start: None,
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::from((1_500_i64, Currency::USD)),
            rules: None,
            market: None,
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: None,
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["TEST_TRANCHE"]),
            floating_rate_shift: 0.0,
            deferred_interest: Some(&deferred),
        };

        let result = test.evaluate(&context).expect("calculation should succeed");
        // Coupon ≈ 100k × 5% / 4 = 1,250; plus 1,250 deferred => 2,500 due.
        // Collections 1,500 / 2,500 = 0.60, which fails a 1.20 test.
        // Without deferred the ratio would be 1.20 and the test would pass.
        assert!(
            (result.current_ratio - 0.60).abs() < 0.02,
            "IC must include deferred arrears; got {}",
            result.current_ratio
        );
        assert!(!result.is_passing);
    }

    /// PIK shortfalls live in balance, not the deferred map. A stale deferred
    /// entry on a PIK tranche must not inflate the IC denominator.
    #[test]
    fn ic_denominator_ignores_deferred_on_pik_tranche() {
        let pool = AssetPool::new("TEST", DealType::Clo, Currency::USD);
        let test = CoverageTestSpec::ic("TEST_TRANCHE", 1.20);
        let mut tranche = Tranche::new(
            "TEST_TRANCHE",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((100_000_i64, Currency::USD)),
            TrancheCoupon::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("Valid date"),
        )
        .expect("Valid tranche");
        tranche.pik_enabled = true;
        let tranches = TrancheStructure::new(vec![tranche]).expect("Valid tranche structure");
        let mut deferred = HashMap::default();
        deferred.insert(
            "TEST_TRANCHE".to_string(),
            Money::from((1_250_i64, Currency::USD)),
        );
        let context = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            valuation_date: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            period_start: None,
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::from((1_500_i64, Currency::USD)),
            rules: None,
            market: None,
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: None,
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["TEST_TRANCHE"]),
            floating_rate_shift: 0.0,
            deferred_interest: Some(&deferred),
        };

        let result = test.evaluate(&context).expect("calculation should succeed");
        // Coupon ≈ 100k × 5% / 4 = 1,250; stale deferred is ignored.
        // Collections 1,500 / 1,250 = 1.20.
        assert!(
            (result.current_ratio - 1.20).abs() < 0.02,
            "PIK IC denominator must be coupon only; got {}",
            result.current_ratio
        );
        assert!(result.is_passing);
    }

    /// W-22: the OC cure amount must account for the cash term leaving the
    /// numerator when it is diverted to pay down notes. Diverting the computed
    /// cure must bring the OC ratio to exactly the required ratio.
    #[test]
    fn test_oc_cure_with_cash_in_numerator_restores_exact_ratio() {
        // Numerator = collateral (90k, stays) + cash (30k, leaves on diversion).
        // Denominator = 100k. Ratio = 120k / 100k = 1.20, breaches a 1.25 trigger.
        let pool = AssetPool::new("TEST", DealType::Clo, Currency::USD);
        let required_ratio = 1.25_f64;
        let test = CoverageTestSpec::oc("SENIOR", required_ratio);

        let tranche = Tranche::new(
            "SENIOR",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((100_000_i64, Currency::USD)),
            TrancheCoupon::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("Valid date"),
        )
        .expect("Valid tranche");
        let tranches = TrancheStructure::new(vec![tranche]).expect("Valid tranche structure");

        let collateral = 90_000.0_f64;
        let cash = 30_000.0_f64;
        let context = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            valuation_date: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            period_start: None,
            cash_balance: Money::new(cash, Currency::USD).expect("valid money fixture"),
            interest_collections: Money::from((0_i64, Currency::USD)),
            rules: None,
            market: None,
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: Some(
                Money::new(collateral, Currency::USD).expect("valid money fixture"),
            ),
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["SENIOR"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = test.evaluate(&context).expect("calculation should succeed");
        assert!(
            !result.is_passing,
            "OC test should breach (ratio 1.20 < 1.25)"
        );

        let cure = result.cure_amount.expect("breach must yield a cure amount");
        let x = cure.amount();
        assert!(
            x <= cash + 1e-6,
            "cure {x} should be fundable from available cash {cash}"
        );

        // numerator = collateral + cash, denominator = 100k.
        let numerator = collateral + cash;
        let denominator = 100_000.0_f64;
        // The cure is diverted interest, which is not in the numerator: it
        // only pays down the denominator. The cured ratio must equal the
        // required ratio exactly.
        let cured_ratio = numerator / (denominator - x);
        assert!(
            (cured_ratio - required_ratio).abs() < 1e-6,
            "cured ratio {cured_ratio} should equal required {required_ratio}; cure X={x}"
        );
    }

    /// Item 11 — a near-1.0 OC trigger must NOT produce an exploding cure.
    ///
    /// The cure formula carries a `1/(1 − required_ratio)` factor. With a
    /// trigger like 1.001 that factor is ~1000×, so an unbounded cure would be
    /// orders of magnitude larger than any principal the structure holds. The
    /// cure must be capped at the OC denominator (test tranche + senior
    /// balances) — the most a coverage diversion could ever pay down.
    #[test]
    fn test_oc_cure_is_capped_at_denominator_for_near_one_trigger() {
        let pool = AssetPool::new("TEST", DealType::Clo, Currency::USD);
        // Trigger just above 1.0 — the pathological regime for the cure.
        let required_ratio = 1.001_f64;
        let test = CoverageTestSpec::oc("SENIOR", required_ratio);

        let tranche = Tranche::new(
            "SENIOR",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((100_000_i64, Currency::USD)),
            TrancheCoupon::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("Valid date"),
        )
        .expect("Valid tranche");
        let tranches = TrancheStructure::new(vec![tranche]).expect("Valid tranche structure");

        // Collateral well below par so the OC ratio breaches 1.001.
        let collateral = 50_000.0_f64;
        let denominator = 100_000.0_f64; // single senior tranche balance
        let context = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            valuation_date: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            period_start: None,
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::from((0_i64, Currency::USD)),
            rules: None,
            market: None,
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: Some(
                Money::new(collateral, Currency::USD).expect("valid money fixture"),
            ),
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["SENIOR"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = test.evaluate(&context).expect("calculation should succeed");
        assert!(
            !result.is_passing,
            "OC test must breach (ratio 0.5 < 1.001)"
        );

        let cure = result.cure_amount.expect("breach must yield a cure amount");
        // The raw formula would give a cure of roughly
        //   (50k − 1.001·100k)/(1 − 1.001) ≈ 50.1M — absurdly large.
        // The capped cure must never exceed the OC denominator.
        assert!(
            cure.amount() <= denominator + 1e-6,
            "near-1.0-trigger OC cure {} must be capped at the denominator \
             {denominator} — the raw formula explodes to tens of millions",
            cure.amount()
        );
        assert!(
            cure.amount() > 0.0,
            "a breaching OC test must still report a positive cure"
        );
    }

    /// C4 regression: a real CLO/ABS can carry several notes at one seniority
    /// level (Class A-1, A-2, A-3 all `Senior`). `payment_priority` must rank
    /// every note distinctly by structural seniority, NOT collapse them onto a
    /// per-`TrancheSeniority` constant. If they collapse, `senior_to` returns
    /// `[]` for same-seniority notes and the OC denominator omits them.
    #[test]
    fn test_same_seniority_notes_get_distinct_priorities_and_oc_denominator() {
        use finstack_quant_core::dates::Date as D;
        let mat = D::from_calendar_date(2034, Month::January, 1).expect("date");
        let cpn = || TrancheCoupon::Fixed { rate: 0.05 };
        // Three Senior notes at distinct attachment points + an Equity note.
        // Capital stack (most senior first, lowest attachment): A-1 0-25,
        // A-2 25-50, A-3 50-75, Equity 75-100. Passed in seniority order.
        let a1 = Tranche::new(
            "A-1",
            0.0,
            25.0,
            TrancheSeniority::Senior,
            Money::from((25_000_i64, Currency::USD)),
            cpn(),
            mat,
        )
        .expect("tranche");
        let a2 = Tranche::new(
            "A-2",
            25.0,
            50.0,
            TrancheSeniority::Senior,
            Money::from((25_000_i64, Currency::USD)),
            cpn(),
            mat,
        )
        .expect("tranche");
        let a3 = Tranche::new(
            "A-3",
            50.0,
            75.0,
            TrancheSeniority::Senior,
            Money::from((25_000_i64, Currency::USD)),
            cpn(),
            mat,
        )
        .expect("tranche");
        let equity = Tranche::new(
            "EQUITY",
            75.0,
            100.0,
            TrancheSeniority::Equity,
            Money::from((25_000_i64, Currency::USD)),
            cpn(),
            mat,
        )
        .expect("tranche");

        let tranches = TrancheStructure::new(vec![a1, a2, a3, equity]).expect("structure");

        // senior_to(A-2) must include A-1 only — not A-2 itself, not A-3/Equity.
        let senior_to_a2: Vec<&str> = tranches
            .senior_to("A-2")
            .iter()
            .map(|t| t.id.as_str())
            .collect();
        assert_eq!(
            senior_to_a2,
            vec!["A-1"],
            "A-1 is the only note senior to A-2"
        );

        // The OC denominator for a junior tranche (A-3) = its own balance plus
        // ALL senior notes (A-1 + A-2). All three senior notes must contribute.
        let oc_denominator = tranches.senior_balance("A-3").amount() + 25_000.0;
        assert_eq!(
            oc_denominator, 75_000.0,
            "A-3 OC denominator must include A-1 + A-2 + A-3 balances"
        );

        // A-2's OC denominator reflects A-1's balance (senior_balance non-zero).
        assert_eq!(
            tranches.senior_balance("A-2").amount(),
            25_000.0,
            "A-2 senior_balance must equal A-1's balance"
        );

        // Priorities must be distinct and strictly increasing by seniority.
        let p = |id: &str| {
            tranches
                .tranches
                .iter()
                .find(|t| t.id.as_str() == id)
                .expect("tranche")
                .payment_priority
        };
        assert!(p("A-1") < p("A-2"));
        assert!(p("A-2") < p("A-3"));
        assert!(p("A-3") < p("EQUITY"));
    }

    /// W-21: an IC-test breach must produce a non-`None` cure amount equal to
    /// the senior interest shortfall, so IC-only breaches actually divert cash.
    #[test]
    fn test_ic_breach_yields_senior_interest_shortfall_cure() {
        let pool = AssetPool::new("TEST", DealType::Clo, Currency::USD);
        let required_ratio = 1.20_f64;
        let test = CoverageTestSpec::ic("TEST_TRANCHE", required_ratio);

        let tranche = Tranche::new(
            "TEST_TRANCHE",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((100_000_i64, Currency::USD)),
            TrancheCoupon::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("Valid date"),
        )
        .expect("Valid tranche");
        let tranches = TrancheStructure::new(vec![tranche]).expect("Valid tranche structure");

        // Interest collections far below interest due => IC test breaches.
        let context = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            valuation_date: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            period_start: None,
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::from((100_i64, Currency::USD)),
            rules: None,
            market: None,
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: None,
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["TEST_TRANCHE"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = test.evaluate(&context).expect("calculation should succeed");
        assert!(!result.is_passing, "IC test should breach");
        let cure = result
            .cure_amount
            .expect("an IC breach must yield a non-None cure amount");
        assert!(
            cure.amount() > 0.0,
            "IC cure must be positive (the interest shortfall), got {}",
            cure.amount()
        );
    }
}

#[cfg(test)]
mod haircut_tests {
    use super::*;
    use crate::cashflow::builder::FloatingRateSpec;
    use crate::instruments::fixed_income::structured_credit::types::{
        AssetPool, DealType, PoolAsset, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::fixings::fixing_series_id;
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use rust_decimal::Decimal;
    use std::collections::BTreeMap;
    use time::Month;

    fn maturity() -> Date {
        Date::from_calendar_date(2034, Month::January, 1).expect("date")
    }

    /// Pool of two 500k assets: one AAA, one CCC.
    fn rated_pool() -> AssetPool {
        let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
        for (id, rating) in [("AAA1", CreditRating::AAA), ("CCC1", CreditRating::CCC)] {
            let mut asset = PoolAsset::fixed_rate_bond(
                id,
                Money::from((500_000_i64, Currency::USD)),
                0.07,
                maturity(),
                DayCount::Thirty360,
            );
            asset.credit_quality = Some(rating);
            pool.assets.push(asset);
        }
        pool
    }

    fn single_tranche() -> TrancheStructure {
        TrancheStructure::new(vec![Tranche::new(
            "A",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((500_000_i64, Currency::USD)),
            TrancheCoupon::Fixed { rate: 0.05 },
            maturity(),
        )
        .expect("tranche")])
        .expect("structure")
    }

    /// Carry CCC collateral at 50% of par — the standard CLO convention.
    fn ccc_haircuts() -> BTreeMap<CreditRating, f64> {
        let mut m = BTreeMap::new();
        m.insert(CreditRating::CCC, 0.5);
        m
    }

    /// Senior fees must be deducted from the IC numerator.
    ///
    /// Fees rank AHEAD of every note, so cash spent on them is not available to
    /// cover note interest. Market convention is
    /// `(collections − senior fees) / interest due`. Using raw collections
    /// overstates IC and lets a deal pass a test it should fail.
    ///
    /// With no fee tier modelled the deduction is zero, which hid the defect.
    #[test]
    fn senior_fees_tighten_the_ic_test() {
        let pool = rated_pool();
        let tranches = single_tranche();
        let market = MarketContext::new();
        let as_of = Date::from_calendar_date(2025, Month::April, 1).expect("date");
        let period_start = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let mut balances = HashMap::default();
        balances.insert("A".to_string(), Money::from((500_000_i64, Currency::USD)));

        let ratio_with_fees = |fees: f64| {
            let ctx = TestContext {
                pool: &pool,
                tranches: &tranches,
                as_of,
                valuation_date: as_of,
                period_start: Some(period_start),
                cash_balance: Money::from((0_i64, Currency::USD)),
                interest_collections: Money::from((10_000_i64, Currency::USD)),
                rules: None,
                market: Some(&market),
                tranche_balances: Some(&balances),
                payable_principal_tranche_ids: None,
                asset_balances: None,
                live_collateral: None,
                current_pool_balance: Some(Money::from((400_000_i64, Currency::USD))),
                senior_fees: Money::new(fees, Currency::USD).expect("valid money fixture"),
                restricted_cash: Money::from((0_i64, Currency::USD)),
                defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
                interest_claim_caps: &uncapped_claims(&["A"]),
                floating_rate_shift: 0.0,
                deferred_interest: None,
            };
            CoverageTestSpec::ic("A", 1.20)
                .evaluate(&ctx)
                .expect("ic test")
                .current_ratio
        };

        let without = ratio_with_fees(0.0);
        let with = ratio_with_fees(4_000.0);

        assert!(
            with < without,
            "senior fees must REDUCE the IC ratio: {with:.4} with a 4,000 fee \
             vs {without:.4} without. Equal values mean the numerator is still \
             raw collections."
        );
        // 10,000 collections less 4,000 of fees is 60% of the un-netted figure.
        assert!(
            (with / without - 0.6).abs() < 1e-9,
            "the ratio must fall exactly in proportion to the fee deduction: \
             expected 0.60x, got {:.4}x",
            with / without
        );
    }

    /// The IC cure must be a PRINCIPAL paydown, since that is how the
    /// diversion applies it.
    ///
    /// The old cure was the cash shortfall `R*I_due - I_coll`, which answers
    /// "how much extra interest cash would clear the test" — but the diversion
    /// pays down senior PRINCIPAL, which adds nothing to interest collections.
    /// Restoring IC by de-levering needs `X >= (I_due - I_coll/R) / (r*tau)`,
    /// which is larger by roughly `1/(r*tau)` — two orders of magnitude at a
    /// quarterly 5% coupon.
    #[test]
    fn ic_cure_is_a_delevering_paydown_not_a_cash_shortfall() {
        let pool = rated_pool();
        let market = MarketContext::new();
        let as_of = Date::from_calendar_date(2025, Month::April, 1).expect("date");
        let period_start = Date::from_calendar_date(2025, Month::January, 1).expect("date");

        // Single senior note, 5% coupon, quarterly.
        let tranches = single_tranche();
        let mut balances = HashMap::default();
        balances.insert("A".to_string(), Money::from((500_000_i64, Currency::USD)));

        let ctx = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of,
            valuation_date: as_of,
            period_start: Some(period_start),
            cash_balance: Money::from((0_i64, Currency::USD)),
            // Collections far below what the coupon demands => a hard breach.
            interest_collections: Money::from((100_i64, Currency::USD)),
            rules: None,
            market: Some(&market),
            tranche_balances: Some(&balances),
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: Some(Money::from((400_000_i64, Currency::USD))),
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["A"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = CoverageTestSpec::ic("A", 1.20)
            .evaluate(&ctx)
            .expect("ic test");
        assert!(!result.is_passing, "the IC test must breach");

        let cure = result
            .cure_amount
            .expect("a breach must produce a cure")
            .amount();

        // The cash shortfall for comparison: R*I_due - I_coll. With a 500k
        // balance at 5% over ~a quarter, I_due is on the order of 6k, so the
        // shortfall is a few thousand. De-levering must be far larger, because
        // it has to retire principal whose COUPON is the problem.
        let interest_due = 500_000.0 * 0.05 * 0.25;
        let cash_shortfall = 1.20 * interest_due - 100.0;
        assert!(
            cure > cash_shortfall * 10.0,
            "the de-levering cure {cure:.2} must be far larger than the cash \
             shortfall {cash_shortfall:.2} — they are different quantities, and \
             using the shortfall under-cures an IC breach by roughly 1/(r*tau) \
            "
        );

        // Sanity: paying down exactly `cure` of principal must clear the test.
        let remaining_balance = (500_000.0 - cure).max(0.0);
        let due_after = remaining_balance * 0.05 * 0.25;
        assert!(
            due_after <= 100.0 / 1.20 + 1e-6,
            "after paying down the cure the interest due {due_after:.4} must be \
             coverable by collections at the required ratio"
        );
    }

    #[test]
    fn coverage_economics_ic_cure_crosses_into_second_rate_piecewise() {
        let pool = rated_pool();
        let as_of = Date::from_calendar_date(2025, Month::April, 1).expect("date");
        let period_start = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let maturity = maturity();
        let senior = Tranche::new(
            "A",
            0.0,
            50.0,
            TrancheSeniority::Senior,
            Money::from((100_000_i64, Currency::USD)),
            TrancheCoupon::Fixed { rate: 0.04 },
            maturity,
        )
        .expect("senior tranche");
        let test_tranche = Tranche::new(
            "B",
            50.0,
            100.0,
            TrancheSeniority::Subordinated,
            Money::from((100_000_i64, Currency::USD)),
            TrancheCoupon::Fixed { rate: 0.16 },
            maturity,
        )
        .expect("test tranche");
        let tranches =
            TrancheStructure::new(vec![senior, test_tranche]).expect("tranche structure");
        let collections = 100.0;
        let required_ratio = 1.20;
        let ctx = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of,
            valuation_date: as_of,
            period_start: Some(period_start),
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::new(collections, Currency::USD)
                .expect("valid money fixture"),
            rules: None,
            market: None,
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: None,
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["B"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = CoverageTestSpec::ic("B", required_ratio)
            .evaluate(&ctx)
            .expect("IC test");
        let actual = result.cure_amount.expect("breach cure").amount();
        let senior = &tranches.tranches[0];
        let junior = &tranches.tranches[1];
        let senior_tau = senior
            .day_count
            .year_fraction(
                period_start,
                as_of,
                finstack_quant_core::dates::DayCountContext::default(),
            )
            .expect("senior accrual");
        let junior_tau = junior
            .day_count
            .year_fraction(
                period_start,
                as_of,
                finstack_quant_core::dates::DayCountContext::default(),
            )
            .expect("junior accrual");
        let total_due = 100_000.0 * 0.04 * senior_tau + 100_000.0 * 0.16 * junior_tau;
        let target_due = collections / required_ratio;
        let reduction_after_senior = total_due - target_due - 100_000.0 * 0.04 * senior_tau;
        let expected = 100_000.0 + reduction_after_senior / (0.16 * junior_tau);

        assert!(
            (actual - expected).abs() < 1e-6,
            "cure must exhaust the 100k senior balance, then use the junior \
             tranche's 16% rate × accrual for the remaining reduction: \
             expected {expected:.6}, got {actual:.6}"
        );
    }

    #[test]
    fn coverage_economics_floating_cure_uses_denominator_market_rate() {
        let pool = rated_pool();
        let as_of = Date::from_calendar_date(2025, Month::April, 1).expect("date");
        let index_id = CurveId::new("USD-3M");
        let forward = ForwardCurve::builder(index_id.clone(), 0.25)
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.04), (1.0, 0.04)])
            .build()
            .expect("forward curve");
        let fixing = ScalarTimeSeries::new(fixing_series_id("USD-3M"), vec![(as_of, 0.04)], None)
            .expect("fixing series");
        let market = MarketContext::new().insert(forward).insert_series(fixing);
        let floating_coupon = TrancheCoupon::Floating(FloatingRateSpec {
            index_id,
            spread_bp: Decimal::from(100),
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_cap_bp: None,
            all_in_floor_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_frequency: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
        });
        let tranche = Tranche::new(
            "A",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((100_000_i64, Currency::USD)),
            floating_coupon,
            maturity(),
        )
        .expect("floating tranche");
        let tranches = TrancheStructure::new(vec![tranche]).expect("tranche structure");
        let collections = 100.0;
        let required_ratio = 1.20;
        let ctx = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of,
            valuation_date: as_of,
            period_start: None,
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::new(collections, Currency::USD)
                .expect("valid money fixture"),
            rules: None,
            market: Some(&market),
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: None,
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["A"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = CoverageTestSpec::ic("A", required_ratio)
            .evaluate(&ctx)
            .expect("IC test");
        let actual = result.cure_amount.expect("breach cure").amount();
        let all_in_rate = tranches.tranches[0]
            .coupon
            .try_current_rate_with_index(as_of, &market)
            .expect("market-aware coupon");
        let tau = 1.0 / frequency_periods_per_year(tranches.tranches[0].frequency);
        let total_due = 100_000.0 * all_in_rate * tau;
        let expected = (total_due - collections / required_ratio) / (all_in_rate * tau);

        assert!(
            (actual - expected).abs() < 1e-6,
            "floating cure must use the denominator's market-aware all-in rate: \
             expected {expected:.6}, got {actual:.6}"
        );

        let payment = Date::from_calendar_date(2025, Month::August, 1).unwrap();
        let start = Date::from_calendar_date(2025, Month::May, 1).unwrap();
        let projected = TestContext {
            as_of: payment,
            valuation_date: as_of,
            period_start: Some(start),
            ..ctx
        };
        let result = CoverageTestSpec::ic("A", required_ratio)
            .evaluate(&projected)
            .expect("future coverage dates must not require future fixings");
        let rate = tranches.tranches[0]
            .coupon
            .try_rate_for_period(start, as_of, &market)
            .unwrap();
        let accrual = tranches.tranches[0]
            .day_count
            .year_fraction(
                start,
                payment,
                finstack_quant_core::dates::DayCountContext::default(),
            )
            .unwrap();
        assert!((result.current_ratio - collections / (100_000.0 * rate * accrual)).abs() < 1e-12);
        let expected_cure = 100_000.0 - collections / (required_ratio * rate * accrual);
        assert!((result.cure_amount.unwrap().amount() - expected_cure).abs() < 1e-6);

        let missing_fixing = TestContext {
            valuation_date: start,
            ..projected
        };
        let error = CoverageTestSpec::ic("A", required_ratio)
            .evaluate(&missing_fixing)
            .expect_err("a reset on the valuation date still requires its exact fixing");
        assert!(error.to_string().contains("2025-05-01"), "{error}");
    }

    /// A configured haircut must track the CURRENT pool balance, not
    /// freeze at the closing-date snapshot.
    ///
    /// `context.pool` is `&instrument.pool`, never mutated during simulation.
    /// Before this fix, setting any haircut bypassed the `current_pool_balance`
    /// override entirely, so the OC numerator stayed at its closing value while
    /// the denominator amortized — the ratio inflated monotonically and the
    /// test could never breach.
    #[test]
    fn haircut_applies_to_the_current_pool_balance() {
        let pool = rated_pool();
        let tranches = single_tranche();
        let market = MarketContext::new();
        let rules = CoverageRules {
            rating_haircuts: ccc_haircuts(),
            ..Default::default()
        };
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");

        // The pool has amortized from 1,000,000 to 400,000.
        let current = Money::from((400_000_i64, Currency::USD));
        let ctx = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of,
            valuation_date: as_of,
            period_start: None,
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::from((0_i64, Currency::USD)),
            rules: Some(&rules),
            market: Some(&market),
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: Some(current),
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["A"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = CoverageTestSpec::oc("A", 1.0)
            .evaluate(&ctx)
            .expect("oc test");

        // Haircut factor: (500k + 0.5*500k) / 1,000k = 0.75.
        // Numerator must be 400k * 0.75 = 300k against a 500k tranche => 0.60.
        assert!(
            (result.current_ratio - 0.60).abs() < 1e-9,
            "the haircut must scale the CURRENT balance: expected 0.60 \
             (400k x 0.75 / 500k), got {}. A ratio near 1.5 means the numerator \
             is still the frozen closing pool.",
            result.current_ratio
        );
        assert!(
            !result.is_passing,
            "a 0.60 OC ratio against a 1.0 requirement must breach"
        );
    }

    #[test]
    fn coverage_economics_oc_haircuts_use_uneven_live_asset_balances() {
        let pool = rated_pool();
        let tranches = single_tranche();
        let rules = CoverageRules {
            rating_haircuts: ccc_haircuts(),
            ..Default::default()
        };
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        // Closing balances are 500k AAA / 500k CCC. Live balances have become
        // 100k AAA / 300k CCC, so a 50% CCC haircut produces 250k, not the
        // 300k implied by scaling the 400k aggregate with the stale 75% mix.
        let live_asset_balances = [100_000.0, 300_000.0];
        let ctx = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of,
            valuation_date: as_of,
            period_start: None,
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::from((0_i64, Currency::USD)),
            rules: Some(&rules),
            market: None,
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: Some(&live_asset_balances),
            live_collateral: None,
            current_pool_balance: Some(Money::from((400_000_i64, Currency::USD))),
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["A"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = CoverageTestSpec::oc("A", 1.0)
            .evaluate(&ctx)
            .expect("OC test");
        assert!(
            (result.current_ratio - 0.50).abs() < 1e-9,
            "live per-asset haircut should be (100k + 50% × 300k) / 500k = \
             0.50, got {}",
            result.current_ratio
        );
    }

    /// Without haircuts the current-balance override is used directly,
    /// so this path is unchanged.
    #[test]
    fn absent_haircuts_use_the_current_balance_unscaled() {
        let pool = rated_pool();
        let tranches = single_tranche();
        let market = MarketContext::new();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");

        let ctx = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of,
            valuation_date: as_of,
            period_start: None,
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::from((0_i64, Currency::USD)),
            rules: None,
            market: Some(&market),
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: Some(Money::from((400_000_i64, Currency::USD))),
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["A"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = CoverageTestSpec::oc("A", 1.0)
            .evaluate(&ctx)
            .expect("oc test");
        assert!(
            (result.current_ratio - 0.80).abs() < 1e-9,
            "without haircuts the ratio is 400k / 500k = 0.80, got {}",
            result.current_ratio
        );
    }
}
