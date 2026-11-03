//! Whole-pool collateral report: statistics, WARF and concentration tables.
//!
//! [`pool_report`] is the one call a pool screen makes. Every number comes from
//! the Rust pool analytics, and every table is ordered here, so a host only
//! draws. Units are stated on each field and returned at run time by
//! `finstack_quant_valuations::pricer::pool_report_field_units`.

use finstack_quant_core::Result;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::pool::{
    AssetPool, PoolAsset, PoolStats, calculate_pool_stats, weighted_avg_rating_factor,
};

/// Balance held by one obligor, industry or rating, as a row of a [`PoolReport`]
/// concentration table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PoolConcentrationEntry {
    /// Obligor id, industry or rating wire form (`"BB+"`) this row groups, or
    /// `null` for the assets that carry no value on the dimension. A null key
    /// cannot collide with a real name.
    pub key: Option<String>,
    /// Current balance of the row's assets, defaulted assets included, in the
    /// pool's base currency.
    pub balance: Money,
    /// `balance` in percent points of [`PoolReport::total_balance`] (`20.0` =
    /// 20%), the convention of `ConcentrationLimit::max_pct` and
    /// `PoolStats::defaulted_balance_pct`, so a share compares to a limit
    /// directly. The rows of one table sum to `100.0`; `0.0` when the pool
    /// balance is not positive.
    pub balance_pct: f64,
    /// Number of asset rows grouped, zero-balance rows included.
    pub asset_count: usize,
}

/// Statistics, WARF and concentration of a collateral pool at one date; the
/// return value of [`pool_report`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PoolReport {
    /// Current balance of every asset row, defaulted assets included, in the
    /// pool's base currency: the denominator of each `balance_pct`.
    pub total_balance: Money,
    /// [`calculate_pool_stats`] of the same rows. Its fields mix units: `wac` is
    /// a decimal, `weighted_avg_spread_bp` basis points and
    /// `defaulted_balance_pct` percent points.
    pub stats: PoolStats,
    /// Moody's weighted-average rating factor, the number `CloWarfCalculator`
    /// and `PeriodDiagnostics::warf` report: weighted by balance over the
    /// performing rows only (defaulted and non-positive-balance rows are out of
    /// both numerator and denominator). An unrated asset takes the Moody's table
    /// factor for `NR`. `None` when no performing balance remains; the
    /// `clo_warf` metric, which must return a number, reports `0.0` there.
    pub warf: Option<f64>,
    /// Balance per `PoolAsset::obligor_id`.
    pub by_obligor: Vec<PoolConcentrationEntry>,
    /// Balance per `PoolAsset::industry`.
    pub by_industry: Vec<PoolConcentrationEntry>,
    /// Balance per `PoolAsset::rating` wire form. An asset with no rating is in
    /// the null-key row, not in an explicit `"NR"` row, although WARF weights
    /// both with the `NR` factor.
    pub by_rating: Vec<PoolConcentrationEntry>,
}

/// Report a collateral pool's statistics, WARF and concentration at a date.
///
/// Each concentration table covers every asset row, so its shares sum to 100%.
/// Defaulted assets count in the balances and in the denominator, as they do in
/// `PoolStats::defaulted_balance_pct`; only `warf` and the `PoolStats` fields
/// that say so are performing-only. Assets with no value on a dimension form one
/// row with a null key. Rows are ordered by balance descending, then key
/// ascending with the null key first, whatever the asset order.
///
/// Representative lines and instrument collateral are first normalized to asset
/// rows (see [`AssetPool::normalized`]); they carry no obligor, industry or
/// rating, so they land in the null-key rows and `warf` weights them as unrated.
///
/// # Arguments
///
/// * `pool` - Collateral pool to report on, in any of its three representations.
/// * `as_of` - Date remaining maturities are measured from. It also stands in
///   for the closing date when a pool is normalized: it ages representative
///   lines and counts the delayed draws already funded on instrument collateral.
///
/// # Returns
///
/// The [`PoolReport`]. An empty pool reports zero balances, empty tables and
/// `None` for each average.
///
/// # Errors
///
/// Returns an error when the pool cannot be normalized, an asset is not in the
/// pool's base currency, the weighted-average maturity cannot be measured, or
/// the Moody's table has no factor for a performing asset's rating.
pub fn pool_report(pool: &AssetPool, as_of: Date) -> Result<PoolReport> {
    let pool = pool.normalized_view(as_of)?;
    let assets = &pool.assets;
    let total_balance = pool.total_balance()?;
    let table = |key: fn(&PoolAsset) -> Option<String>| {
        concentration(assets, total_balance, pool.currency, key)
    };
    Ok(PoolReport {
        total_balance,
        stats: calculate_pool_stats(&pool, as_of)?,
        warf: weighted_avg_rating_factor(
            assets
                .iter()
                .filter(|asset| !asset.defaulted)
                .map(|asset| (asset.balance.amount(), asset.rating)),
        )?,
        by_obligor: table(|asset| asset.obligor_id.clone())?,
        by_industry: table(|asset| asset.industry.clone())?,
        by_rating: table(|asset| asset.rating.map(|rating| rating.to_string()))?,
    })
}

/// Group `assets` by `key` and order the rows; decimal sums make the balances
/// independent of the asset order.
fn concentration(
    assets: &[PoolAsset],
    total: Money,
    currency: finstack_quant_core::currency::Currency,
    key: fn(&PoolAsset) -> Option<String>,
) -> Result<Vec<PoolConcentrationEntry>> {
    let mut groups: BTreeMap<Option<String>, (Money, usize)> = BTreeMap::new();
    for asset in assets {
        let (balance, count) = groups
            .entry(key(asset))
            .or_insert((Money::from((0_i64, currency)), 0));
        *balance = balance.checked_add(asset.balance)?;
        *count += 1;
    }
    let mut rows: Vec<PoolConcentrationEntry> = groups
        .into_iter()
        .map(|(key, (balance, asset_count))| PoolConcentrationEntry {
            key,
            balance,
            balance_pct: if total.amount() > 0.0 {
                balance.amount() / total.amount() * 100.0
            } else {
                0.0
            },
            asset_count,
        })
        .collect();
    // Stable, and `groups` already ascends by key, so ties keep key order.
    rows.sort_by_key(|row| std::cmp::Reverse(row.balance.amount_decimal()));
    Ok(rows)
}
