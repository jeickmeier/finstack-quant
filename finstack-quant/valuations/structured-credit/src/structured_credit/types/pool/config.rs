//! Reinvestment, representative-line, account-tally, and summary-statistics types.

use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::math::units::BASIS_POINTS_PER_UNIT;
use finstack_quant_core::money::Money;
use serde::{Deserialize, Serialize};

use super::super::enums::AssetType;

/// Deal-level reinvestment period and rules.
///
/// While the period is active (`is_active` and the payment date is on or
/// before `end_date`), principal proceeds are recycled into collateral
/// instead of repaying the notes. Every note is held flat except those listed
/// in `amortizing_tranches`, which are paid down first; only the principal
/// left after their paydown is reinvested.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ReinvestmentPeriod {
    /// Inclusive end date of the reinvestment period.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub end: Date,
    /// Whether reinvestment is currently active.
    pub is_active: bool,
    /// Eligibility criteria for purchases.
    pub criteria: ReinvestmentCriteria,
    /// Notes paid down from principal proceeds during the period, in the
    /// principal waterfall's order; every other note is held flat. Must name
    /// non-equity tranches of the deal.
    #[serde(default)]
    pub amortizing_tranches: Vec<String>,
    /// Terms of the replacement collateral. `None` clones the surviving pool
    /// pro rata; `Some` books each period's purchases as a synthetic
    /// `REINVEST-{n}` first-lien row with these terms.
    #[serde(default)]
    pub assumptions: Option<ReinvestmentAssumptions>,
}

/// Terms of the collateral bought with reinvested principal.
///
/// Each purchase creates a bullet first-lien row maturing `maturity_months`
/// after the purchase date, accruing ACT/360 at `forward_curve_id` plus `spread_bp`
/// (or at `spread_bp` as a fixed coupon when no index is given), bought at
/// `price_pct` percent of par. A discount price builds par.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ReinvestmentAssumptions {
    /// Spread over the index in basis points (the whole coupon in basis
    /// points when `forward_curve_id` is `None`).
    pub spread_bp: f64,
    /// Purchase price as percent of par (`99.0` buys `100/99` of par per unit
    /// of cash); must not exceed `ReinvestmentCriteria::max_price_pct`.
    pub price_pct: f64,
    /// Months from the purchase date to the bullet maturity.
    pub maturity_months: u32,
    /// Floating-rate index id (a forward curve in the market context), or
    /// `None` for a fixed coupon.
    pub forward_curve_id: Option<String>,
    /// Business-day fixing lag of the purchased rows (see
    /// `PoolAsset::reset_lag_days`); required when `forward_curve_id` is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_lag_days: Option<i32>,
    /// Minimum all-in annual coupon in basis points (a floor on the resolved
    /// index plus spread, 100 = 1%), if any.
    pub all_in_floor_bp: Option<f64>,
}

/// Criteria for reinvestment during revolving period
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ReinvestmentCriteria {
    /// Maximum purchase price in percent of par (`100.0` = par).
    pub max_price_pct: f64,
    /// Minimum annual decimal current yield: replacement coupon divided by
    /// purchase-price fraction. Surviving assets below it are skipped when
    /// the pool is replicated pro rata; a synthetic purchase below it is not
    /// made.
    pub min_yield: f64,
}

impl Default for ReinvestmentCriteria {
    fn default() -> Self {
        Self {
            max_price_pct: 100.0, // 100% of par
            min_yield: 0.0,
        }
    }
}

/// AssetPool-level performance statistics
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PoolStats {
    /// Weighted-average coupon (annual decimal) of the performing fixed-rate
    /// collateral; `None` when there is none. See [`AssetPool::wac`](super::AssetPool::wac).
    pub wac: Option<f64>,
    /// Weighted-average spread in basis points of the performing collateral
    /// that carries an explicit `spread_bp`; `None` when there is none. See
    /// [`AssetPool::weighted_avg_spread_bp`](super::AssetPool::weighted_avg_spread_bp).
    pub weighted_avg_spread_bp: Option<f64>,
    /// Weighted average maturity (WAM) in years; `None` when the pool balance is
    /// zero.
    ///
    /// For weighted average life, which weights principal cashflows rather
    /// than maturities, use the `wal` metric.
    pub weighted_avg_maturity: Option<f64>,
    /// Diversity score (Moody's methodology) of the assets that name an obligor;
    /// `None` when none does. See [`AssetPool::diversity_score`](super::AssetPool::diversity_score).
    pub diversity_score: Option<f64>,
    /// Number of distinct `PoolAsset::obligor_id` values. Assets naming no obligor
    /// are not counted.
    pub num_obligors: usize,
    /// Number of distinct `PoolAsset::industry` values. Assets naming no industry
    /// are not counted.
    pub num_industries: usize,
    /// Balance of assets currently carried as defaulted, in percent points
    /// of the current total pool balance (defaulted assets included in the
    /// denominator; `10.0` = 10%).
    pub defaulted_balance_pct: f64,
    /// Undrawn commitment across revolving and delayed-draw collateral
    /// (`commitment − balance`, performing assets only).
    #[serde(default)]
    pub undrawn_commitment: f64,
}

/// Representative line for aggregated pool modeling
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RepLine {
    /// Unique identifier for the rep line
    pub id: String,
    /// Contractual asset classification; determines level-pay amortization.
    pub asset_type: AssetType,
    /// Aggregated balance
    pub balance: Money,
    /// Weighted average coupon
    pub rate: f64,
    /// Weighted average spread (for floating rate)
    pub spread_bp: Option<f64>,
    /// Rates forward-curve identifier (if floating)
    pub forward_curve_id: Option<String>,
    /// Floor on the floating index in basis points (100 = 1%), applied before
    /// `spread_bp`; `None` for no floor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_floor_bp: Option<f64>,
    /// Business-day fixing lag before each coupon period start (see
    /// `PoolAsset::reset_lag_days`); required when `forward_curve_id` is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_lag_days: Option<i32>,
    /// Weighted average maturity date
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub maturity: Date,
    /// Weighted average seasoning in months
    pub seasoning_months: u32,
    /// Day count convention
    pub day_count: DayCount,
    /// Optional CPR override for this line
    pub cpr: Option<f64>,
    /// Optional CDR override for this line
    pub cdr: Option<f64>,
    /// Optional recovery rate override
    pub recovery_rate: Option<f64>,
    /// Contractual periodic payment for level-pay lines (see
    /// `PoolAsset::contractual_payment`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contractual_payment: Option<Money>,
    /// Amortization schedule length in months from origination (see
    /// `PoolAsset::amortization_term_months`); with `seasoning_months` the
    /// line amortizes over `term − seasoning`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amortization_term_months: Option<u32>,
    /// Interest-only months from origination (see `PoolAsset::io_months`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io_months: Option<u32>,
}

impl RepLine {
    /// Create a representative collateral line with explicit amortization type.
    ///
    /// # Arguments
    /// * `id` - Stable identifier used in pool diagnostics.
    /// * `balance` - Aggregate outstanding principal in the pool currency.
    /// * `rate` - Annual decimal coupon, or current all-in rate for floating assets.
    /// * `maturity` - Contractual final repayment date.
    /// * `day_count` - Coupon accrual convention.
    /// * `asset_type` - Contractual asset type, including level-pay versus bullet behavior.
    pub fn new(
        id: impl Into<String>,
        balance: Money,
        rate: f64,
        maturity: Date,
        day_count: DayCount,
        asset_type: AssetType,
    ) -> Self {
        Self {
            id: id.into(),
            asset_type,
            balance,
            rate,
            spread_bp: None,
            forward_curve_id: None,
            index_floor_bp: None,
            reset_lag_days: None,
            maturity,
            seasoning_months: 0,
            day_count,
            cpr: None,
            cdr: None,
            recovery_rate: None,
            contractual_payment: None,
            amortization_term_months: None,
            io_months: None,
        }
    }

    /// Set CPR override
    pub fn with_cpr(mut self, cpr: f64) -> Self {
        self.cpr = Some(cpr);
        self
    }

    /// Set CDR override
    pub fn with_cdr(mut self, cdr: f64) -> Self {
        self.cdr = Some(cdr);
        self
    }

    /// Set recovery rate override
    pub fn with_recovery_rate(mut self, recovery_rate: f64) -> Self {
        self.recovery_rate = Some(recovery_rate);
        self
    }

    /// Get effective spread in basis points
    pub fn spread_bp(&self) -> f64 {
        self.spread_bp.unwrap_or(self.rate * BASIS_POINTS_PER_UNIT)
    }
}

/// Account balances and performance tallies of a seasoned pool, set together
/// by [`AssetPool::with_accounts`](super::AssetPool::with_accounts).
///
/// Every field is optional: an omitted field keeps the pool's current value.
/// All amounts are in the pool currency.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::money::Money;
/// use finstack_quant_valuations_structured_credit::structured_credit::{
///     AssetPool, DealType, PoolAccounts,
/// };
///
/// let collected = Money::new(250_000.0, Currency::USD)?;
/// let pool = AssetPool::new("POOL", DealType::Abs, Currency::USD).with_accounts(PoolAccounts {
///     collection_account: Some(collected),
///     ..PoolAccounts::default()
/// });
/// assert_eq!(pool.collection_account, collected);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PoolAccounts {
    /// Cumulative defaults to date
    /// ([`AssetPool::cumulative_defaults`](super::AssetPool::cumulative_defaults)).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cumulative_defaults: Option<Money>,
    /// Cumulative recoveries on defaulted assets
    /// ([`AssetPool::cumulative_recoveries`](super::AssetPool::cumulative_recoveries)).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cumulative_recoveries: Option<Money>,
    /// Cumulative voluntary prepayments
    /// ([`AssetPool::cumulative_prepayments`](super::AssetPool::cumulative_prepayments)).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cumulative_prepayments: Option<Money>,
    /// Cumulative scheduled amortization
    /// ([`AssetPool::cumulative_scheduled_amortization`](super::AssetPool::cumulative_scheduled_amortization)).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cumulative_scheduled_amortization: Option<Money>,
    /// Collection account balance
    /// ([`AssetPool::collection_account`](super::AssetPool::collection_account)).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection_account: Option<Money>,
    /// Excess spread account balance
    /// ([`AssetPool::excess_spread_account`](super::AssetPool::excess_spread_account)).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excess_spread_account: Option<Money>,
    /// Original (cut-off) pool balance
    /// ([`AssetPool::original_balance`](super::AssetPool::original_balance)).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_balance: Option<Money>,
}
