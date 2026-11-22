//! Pool balance, WAC/WAS/WAM, diversity, WARF, and summary-statistics calculations.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CreditRating;
use std::collections::BTreeMap;

use super::{AssetPool, PoolAsset, PoolStats};

impl AssetPool {
    /// Balance-weighted collateral age from each asset's origination date
    /// (acquisition when origination is unknown); undated assets are new at
    /// closing.
    pub(crate) fn seasoning_months(&self, date: Date, closing_date: Date) -> u32 {
        let mut weighted = 0.0;
        let mut total = 0.0;
        for asset in &self.assets {
            let weight = asset.balance.amount().max(0.0);
            let start = asset.seasoning_anchor().unwrap_or(closing_date);
            let age = if start < date {
                start.months_until(date).unwrap_or(0)
            } else {
                0
            };
            weighted += f64::from(age) * weight;
            total += weight;
        }
        if total > 0.0 {
            (weighted / total).round() as u32
        } else {
            0
        }
    }

    /// Total pool balance
    pub fn total_balance(&self) -> finstack_quant_core::Result<Money> {
        self.validate_representation()?;
        if let Some(instruments) = &self.instruments
            && self.assets.is_empty()
            && !instruments.is_empty()
        {
            return instruments.total_balance(self.currency);
        }
        if let Some(lines) = &self.rep_lines
            && self.assets.is_empty()
        {
            return lines
                .iter()
                .try_fold(Money::from((0_i64, self.currency)), |sum, line| {
                    sum.checked_add(line.balance)
                });
        }
        self.assets
            .iter()
            .try_fold(Money::from((0_i64, self.currency)), |acc, asset| {
                self.validate_asset_currency(asset)?;
                acc.checked_add(asset.balance)
            })
    }

    /// Original (cut-off) pool balance: the explicit [`Self::original_balance`]
    /// when supplied, else the current total balance plus the cumulative
    /// defaults, prepayments and scheduled amortization received to date.
    ///
    /// This is the base of every "fraction of the original pool" quantity
    /// (cumulative-loss triggers, the clean-up call factor, cumulative-loss
    /// and timing default curves). For a new-issue pool it equals the current
    /// balance; for a seasoned pool it is the balance the tallies reconstruct.
    ///
    /// # Errors
    ///
    /// Returns the currency-mismatch or representation errors of
    /// [`Self::total_balance`].
    pub fn original_balance_or_reconstructed(&self) -> finstack_quant_core::Result<Money> {
        if let Some(original) = self.original_balance {
            return Ok(original);
        }
        self.total_balance()?
            .checked_add(self.cumulative_defaults)?
            .checked_add(self.cumulative_prepayments)?
            .checked_add(self.cumulative_scheduled_amortization)
    }

    /// Total pool balance excluding defaulted assets
    pub fn performing_balance(&self) -> finstack_quant_core::Result<Money> {
        self.validate_representation()?;
        if self.assets.is_empty() {
            return self.total_balance();
        }
        self.assets.iter().filter(|a| !a.defaulted).try_fold(
            Money::from((0_i64, self.currency)),
            |acc, asset| {
                self.validate_asset_currency(asset)?;
                acc.checked_add(asset.balance)
            },
        )
    }

    /// Weighted-average coupon (WAC) of the collateral, as an annual decimal.
    ///
    /// Indenture WAC: the balance-weighted all-in coupon (`rate`) over the
    /// performing (non-defaulted, positive-balance) fixed-rate collateral,
    /// asset rows and rep lines alike. A row is floating-rate, and so left
    /// out, when it carries both an `forward_curve_id` and a `spread_bp`; its spread
    /// is reported by [`Self::weighted_avg_spread_bp`] instead. This is the
    /// same population as the simulated `PeriodDiagnostics::wac`.
    ///
    /// # Returns
    ///
    /// The WAC as a decimal (`0.08` = 8%), or `None` when no performing
    /// fixed-rate balance remains: a floating-only pool has no WAC.
    pub fn wac(&self) -> Option<f64> {
        let is_fixed = |forward_curve_id: &Option<String>, spread_bp: Option<f64>| {
            forward_curve_id.is_none() || spread_bp.is_none()
        };
        let mut weighted = 0.0;
        let mut included = 0.0;
        for asset in &self.assets {
            let balance = asset.balance.amount();
            if asset.defaulted
                || balance <= 0.0
                || !is_fixed(&asset.forward_curve_id, asset.spread_bp)
            {
                continue;
            }
            weighted += asset.rate * balance;
            included += balance;
        }
        for line in self.rep_lines.iter().flatten() {
            let balance = line.balance.amount();
            if balance <= 0.0 || !is_fixed(&line.forward_curve_id, line.spread_bp) {
                continue;
            }
            weighted += line.rate * balance;
            included += balance;
        }
        (included > 0.0).then(|| weighted / included)
    }

    /// Calculate weighted average maturity (WAM)
    ///
    /// This calculates the balance-weighted average time to maturity, in
    /// Act/365F years (the time basis shared by every structured-credit
    /// metric, so WAM and WAL are on one clock); rows past maturity count 0.
    /// A row's accrual day count does not apply: ACT/ACT ICMA, for one, cannot
    /// measure a span without the coupon schedule a pool row does not carry.
    /// Note: This is NOT the same as Weighted Average Life (WAL).
    /// WAL requires cashflow schedules and is calculated from principal payments.
    ///
    /// # Arguments
    ///
    /// * `as_of` - Date from which each row's remaining term is measured.
    ///
    /// # Returns
    ///
    /// Balance-weighted remaining term in Act/365F years; `None` when the pool
    /// balance is zero.
    ///
    /// # Errors
    ///
    /// Returns an error when the pool balance cannot be summed (a row in a
    /// currency other than the pool's base currency).
    pub fn weighted_avg_maturity(&self, as_of: Date) -> finstack_quant_core::Result<Option<f64>> {
        let total_balance = self.total_balance()?.amount();
        if total_balance == 0.0 {
            return Ok(None);
        }
        let basis = crate::structured_credit::metrics::METRIC_TIME_BASIS;
        let mut weighted_sum = 0.0;
        for asset in &self.assets {
            weighted_sum += asset.remaining_term(as_of, basis)? * asset.balance.amount();
        }
        for line in self.rep_lines.iter().flatten() {
            let term = basis.year_fraction(
                as_of.min(line.maturity),
                line.maturity,
                finstack_quant_core::dates::DayCountContext::default(),
            )?;
            weighted_sum += term * line.balance.amount();
        }
        Ok(Some(weighted_sum / total_balance))
    }

    /// Calculate diversity score (simplified Moody's approach)
    ///
    /// Obligor balances are summed in obligor-id order, so the score is the
    /// same bits on every build target and for any asset order.
    ///
    /// # Returns
    ///
    /// The score over the assets that name an obligor, or `None` when the pool
    /// balance is zero or no asset names an obligor, so no diversity is measured.
    ///
    /// # Errors
    ///
    /// Returns a currency-mismatch error when the pool balance cannot be summed
    /// (a row in a currency other than the pool's base currency).
    pub fn diversity_score(&self) -> finstack_quant_core::Result<Option<f64>> {
        if self.total_balance()?.amount() == 0.0 {
            return Ok(None);
        }

        // Group balances by obligor in key order: the two f64 sums below are
        // order-sensitive, and a hash map's iteration order differs between
        // native and wasm32 builds.
        let mut obligor_balances: BTreeMap<&str, f64> = BTreeMap::new();

        // Group by obligor
        for asset in &self.assets {
            if let Some(ref obligor) = asset.obligor_id {
                *obligor_balances.entry(obligor.as_str()).or_insert(0.0) += asset.balance.amount();
            }
        }

        // Calculate diversity score = (sum of balances)^2 / sum of (balance^2)
        let sum_balances: f64 = obligor_balances.values().sum();
        let sum_squares: f64 = obligor_balances.values().map(|b| b * b).sum();

        Ok((sum_squares > 0.0).then(|| (sum_balances * sum_balances) / sum_squares))
    }

    /// Base currency of the pool.
    pub fn get_currency(&self) -> Currency {
        self.currency
    }

    /// Get assets by industry
    pub fn assets_by_industry(&self, industry: &str) -> Vec<&PoolAsset> {
        self.assets
            .iter()
            .filter(|a| a.industry.as_deref() == Some(industry))
            .collect()
    }

    /// Get assets by obligor
    pub fn assets_by_obligor(&self, obligor_id: &str) -> Vec<&PoolAsset> {
        self.assets
            .iter()
            .filter(|a| a.obligor_id.as_deref() == Some(obligor_id))
            .collect()
    }

    /// Calculate weighted average spread (WAS) in basis points
    ///
    /// Market standard (CLO indenture convention):
    /// - performing assets only — defaulted assets are excluded from both the
    ///   numerator and the denominator;
    /// - spread component only — fixed-rate assets without an explicit
    ///   `spread_bp` are skipped rather than counting their all-in coupon as
    ///   spread (the old `rate × 10⁴` fallback inflated WAS one-sidedly).
    ///
    /// The denominator is the balance of the INCLUDED assets, so a pool of
    /// only fixed-rate or defaulted assets returns `None`.
    pub fn weighted_avg_spread_bp(&self) -> Option<f64> {
        let mut weighted_spread = 0.0;
        let mut included_balance = 0.0;
        for asset in &self.assets {
            if asset.defaulted {
                continue;
            }
            let Some(spread_bp) = asset.spread_bp else {
                continue;
            };
            weighted_spread += spread_bp * asset.balance.amount();
            included_balance += asset.balance.amount();
        }

        for line in self.rep_lines.iter().flatten() {
            if let Some(spread) = line.spread_bp {
                weighted_spread += spread * line.balance.amount();
                included_balance += line.balance.amount();
            }
        }
        (included_balance != 0.0).then(|| weighted_spread / included_balance)
    }
}

/// Moody's weighted-average rating factor of `rows`, the only WARF formula in
/// the crate: weighted by balance over the rows with a positive balance, with
/// an unrated row taking the Moody's table factor for `NR`.
///
/// # Arguments
///
/// * `rows` - `(balance, rating)` of each performing row, in pool-currency
///   units; the caller leaves defaulted rows out.
///
/// # Returns
///
/// The rating factor, or `None` when no row has a positive balance.
///
/// # Errors
///
/// Returns an error when the Moody's table has no factor for a counted rating.
pub(crate) fn weighted_avg_rating_factor(
    rows: impl IntoIterator<Item = (f64, Option<CreditRating>)>,
) -> finstack_quant_core::Result<Option<f64>> {
    let mut weighted = 0.0;
    let mut total = 0.0;
    for (balance, rating) in rows {
        if balance <= 0.0 {
            continue;
        }
        let factor =
            finstack_quant_models::credit::moodys_warf_factor(rating.unwrap_or(CreditRating::NR))?;
        weighted += balance * factor;
        total += balance;
    }
    Ok((total > 0.0).then(|| weighted / total))
}

/// Calculate current pool statistics.
///
/// This function computes all pool statistics on-demand without caching.
/// This ensures statistics are always up-to-date and eliminates cache invalidation bugs.
///
/// # Arguments
///
/// * `pool` - Asset pool whose active/defaulted balances, obligors, industries,
///   coupons, and collateral attributes are summarized.
/// * `as_of` - Reporting date used to classify asset state and calculate
///   date-dependent pool measures.
///
/// # Errors
///
/// Returns an error when the pool balance cannot be summed or the weighted
/// average maturity cannot be measured (see
/// [`AssetPool::weighted_avg_maturity`]).
pub fn calculate_pool_stats(
    pool: &AssetPool,
    as_of: Date,
) -> finstack_quant_core::Result<PoolStats> {
    // Count unique obligors and industries
    let mut obligors = finstack_quant_core::FxHashSet::default();
    let mut industries = finstack_quant_core::FxHashSet::default();

    for asset in &pool.assets {
        if let Some(ref obligor) = asset.obligor_id {
            obligors.insert(obligor.clone());
        }
        if let Some(ref industry) = asset.industry {
            industries.insert(industry.clone());
        }
    }

    let total_balance = pool.total_balance()?.amount();
    let defaulted_balance: f64 = pool
        .assets
        .iter()
        .filter(|a| a.defaulted)
        .map(|a| a.balance.amount())
        .sum();

    let defaulted_balance_pct = if total_balance > 0.0 {
        defaulted_balance / total_balance * 100.0
    } else {
        0.0
    };

    Ok(PoolStats {
        wac: pool.wac(),
        weighted_avg_spread_bp: pool.weighted_avg_spread_bp(),
        weighted_avg_maturity: pool.weighted_avg_maturity(as_of)?,
        diversity_score: pool.diversity_score()?,
        num_obligors: obligors.len(),
        num_industries: industries.len(),
        defaulted_balance_pct,
        undrawn_commitment: pool
            .assets
            .iter()
            .filter(|a| !a.defaulted)
            .filter_map(|a| {
                a.commitment
                    .map(|c| (c.amount() - a.balance.amount()).max(0.0))
            })
            .sum(),
    })
}
