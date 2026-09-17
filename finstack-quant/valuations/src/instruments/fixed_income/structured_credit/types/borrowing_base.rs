//! Borrowing-base rules: advance rates by collateral class, eligibility and
//! concentration limits, evaluated on the live collateral balances by the
//! `CoverageTestType::BorrowingBase` coverage test (and by the asset-backed
//! facility instrument that synthesizes that test).

use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;
use serde::{Deserialize, Serialize};

use super::pool::{AssetPool, PoolAsset};

/// Advance rate applied to one collateral class.
///
/// `asset_class` is the wire name of the [`AssetType`](super::AssetType)
/// variant (`"first_lien_loan"`, `"new_auto_loan"`, ...) or `"*"` for every
/// class without a more specific entry; collateral with no matching entry is
/// ineligible.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AdvanceRate {
    /// Collateral class the rate applies to (`AssetType` wire name or `"*"`).
    pub asset_class: String,
    /// Advance rate as a decimal share of eligible balance (`0.8` = 80%).
    pub rate: f64,
    /// Eligibility criteria the collateral must meet to count.
    #[serde(default)]
    pub eligibility: EligibilityRule,
}

/// Eligibility criteria for collateral under an advance rate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EligibilityRule {
    /// Defaulted collateral is ineligible (the default).
    #[serde(default = "default_true")]
    pub exclude_defaulted: bool,
    /// Collateral maturing after this date is ineligible.
    #[serde(default, with = "finstack_quant_core::wire::optional_date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub max_maturity: Option<Date>,
}

fn default_true() -> bool {
    true
}

impl Default for EligibilityRule {
    fn default() -> Self {
        Self {
            exclude_defaulted: true,
            max_maturity: None,
        }
    }
}

/// Dimension a concentration limit is measured on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ConcentrationScope {
    /// Per `PoolAsset::obligor_id`.
    Obligor,
    /// Per `PoolAsset::industry`.
    Industry,
    /// Per `AssetType` wire name.
    AssetClass,
}

/// Cap on the eligible collateral any one obligor, industry or asset class
/// may contribute; balance above the cap is excluded from the borrowing base.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ConcentrationLimit {
    /// Dimension the cap is measured on.
    pub scope: ConcentrationScope,
    /// Maximum share of the eligible collateral, in percent (`20.0` = 20%).
    pub max_pct: f64,
}

/// Advance rates and concentration limits that define a borrowing base.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BorrowingBaseRules {
    /// Advance rate per collateral class (first match wins, `"*"` last).
    pub advance_rates: Vec<AdvanceRate>,
    /// Concentration limits applied to the eligible collateral before the
    /// advance rates, in order.
    #[serde(default)]
    pub concentration_limits: Vec<ConcentrationLimit>,
}

/// Borrowing base of a collateral pool at one point in time.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BorrowingBaseReport {
    /// Collateral balance that meets the eligibility criteria, before
    /// concentration limits.
    pub eligible_collateral: Money,
    /// Eligible balance excluded by the concentration limits.
    pub concentration_excess: Money,
    /// Advance-rate-weighted eligible collateral after the limits.
    pub borrowing_base: Money,
}

impl BorrowingBaseRules {
    /// Validate rates, limits and class names.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when there is no advance rate, a rate is
    /// outside `[0, 1]`, a class name is empty, or a limit is outside
    /// `(0, 100]`.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        let invalid = |msg: String| finstack_quant_core::Error::Validation(msg);
        if self.advance_rates.is_empty() {
            return Err(invalid(
                "borrowing base needs at least one advance rate".to_string(),
            ));
        }
        for advance in &self.advance_rates {
            if advance.asset_class.trim().is_empty() {
                return Err(invalid(
                    "advance rate asset_class must not be empty".to_string(),
                ));
            }
            if !advance.rate.is_finite() || !(0.0..=1.0).contains(&advance.rate) {
                return Err(invalid(format!(
                    "advance rate for {} ({}) must be a decimal in [0, 1]",
                    advance.asset_class, advance.rate
                )));
            }
        }
        for limit in &self.concentration_limits {
            if !limit.max_pct.is_finite() || limit.max_pct <= 0.0 || limit.max_pct > 100.0 {
                return Err(invalid(format!(
                    "concentration limit ({}) must be a percent in (0, 100]",
                    limit.max_pct
                )));
            }
        }
        Ok(())
    }

    /// The advance rate that applies to `asset`: the first entry naming its
    /// class, else the `"*"` entry, else `None` (ineligible).
    pub fn advance_rate_for(&self, asset: &PoolAsset) -> Option<&AdvanceRate> {
        let class = asset.asset_type.wire_name();
        self.advance_rates
            .iter()
            .find(|advance| advance.asset_class == class)
            .or_else(|| {
                self.advance_rates
                    .iter()
                    .find(|advance| advance.asset_class == "*")
            })
    }

    /// Evaluate the borrowing base on `pool`.
    ///
    /// # Arguments
    ///
    /// * `pool` - Collateral pool; asset rows supply class, obligor, industry,
    ///   default state and maturity.
    /// * `balances` - Live per-asset balances aligned with `pool.assets`, or
    ///   `None` to use the closing balances.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when `balances` does not match the pool's
    /// asset count.
    pub fn evaluate(
        &self,
        pool: &AssetPool,
        balances: Option<&[f64]>,
    ) -> finstack_quant_core::Result<BorrowingBaseReport> {
        let currency = pool.get_base_currency();
        if let Some(balances) = balances {
            if balances.len() != pool.assets.len() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "borrowing base balances ({}) do not match the pool's assets ({})",
                    balances.len(),
                    pool.assets.len()
                )));
            }
        }

        // Eligible balance and advance rate per asset.
        let mut eligible: Vec<f64> = Vec::with_capacity(pool.assets.len());
        let mut rates: Vec<f64> = Vec::with_capacity(pool.assets.len());
        for (index, asset) in pool.assets.iter().enumerate() {
            let balance = balances
                .and_then(|b| b.get(index).copied())
                .unwrap_or_else(|| asset.balance.amount())
                .max(0.0);
            let advance = self.advance_rate_for(asset);
            let is_eligible = advance.is_some_and(|advance| {
                let rule = &advance.eligibility;
                !(rule.exclude_defaulted && asset.is_defaulted)
                    && rule
                        .max_maturity
                        .is_none_or(|max_maturity| asset.maturity <= max_maturity)
            });
            eligible.push(if is_eligible { balance } else { 0.0 });
            rates.push(advance.map_or(0.0, |advance| advance.rate));
        }
        let eligible_total: f64 = eligible.iter().sum();

        // Concentration limits: scale each over-cap group down to its cap,
        // measured against the eligible total before any exclusion.
        let mut adjusted = eligible.clone();
        for limit in &self.concentration_limits {
            let cap = eligible_total * limit.max_pct / 100.0;
            let mut groups: HashMap<String, f64> = HashMap::default();
            for (index, asset) in pool.assets.iter().enumerate() {
                if let Some(key) = concentration_key(limit.scope, asset) {
                    *groups.entry(key).or_insert(0.0) += adjusted[index];
                }
            }
            for (index, asset) in pool.assets.iter().enumerate() {
                let Some(key) = concentration_key(limit.scope, asset) else {
                    continue;
                };
                let group_total = groups.get(&key).copied().unwrap_or(0.0);
                if group_total > cap && group_total > 0.0 {
                    adjusted[index] *= cap / group_total;
                }
            }
        }
        let adjusted_total: f64 = adjusted.iter().sum();
        let base: f64 = adjusted
            .iter()
            .zip(&rates)
            .map(|(balance, rate)| balance * rate)
            .sum();

        Ok(BorrowingBaseReport {
            eligible_collateral: Money::new(eligible_total, currency)?,
            concentration_excess: Money::new((eligible_total - adjusted_total).max(0.0), currency)?,
            borrowing_base: Money::new(base, currency)?,
        })
    }
}

/// Grouping key of `asset` under `scope`, or `None` when the asset carries
/// no value on that dimension (it then escapes the limit).
fn concentration_key(scope: ConcentrationScope, asset: &PoolAsset) -> Option<String> {
    match scope {
        ConcentrationScope::Obligor => asset.obligor_id.clone(),
        ConcentrationScope::Industry => asset.industry.clone(),
        ConcentrationScope::AssetClass => Some(asset.asset_type.wire_name().to_string()),
    }
}
