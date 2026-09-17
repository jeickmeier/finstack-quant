//! Public structured-credit deal, pool, tranche, and waterfall types.
//!
use super::cmbs::{BalloonSpec, PrepaymentPenalty};
use super::npl::LiquidationSpec;
use super::pool::{AssetPool, PoolAsset};
use finstack_quant_core::dates::{Date, DayCount};

/// Structure of Arrays (SoA) layout for pool assets to improve cache locality
/// and enable vectorization during pricing.
#[derive(Debug, Clone)]
pub(crate) struct PoolState {
    /// Asset identifiers
    pub(crate) ids: Vec<String>,
    /// Current outstanding balances
    pub(crate) balances: Vec<f64>,
    /// Interest rates (decimal)
    pub(crate) rates: Vec<f64>,
    /// Spread over index (basis points)
    pub(crate) spread_bp: Vec<Option<f64>>,
    /// Maturity dates
    pub(crate) maturities: Vec<Date>,
    /// Day count conventions
    pub(crate) day_counts: Vec<DayCount>,
    /// Default status
    pub(crate) is_defaulted: Vec<bool>,
    /// SMM overrides
    pub(crate) smm_overrides: Vec<Option<f64>>,
    /// MDR overrides
    pub(crate) mdr_overrides: Vec<Option<f64>>,
    /// Explicit per-asset recovery fractions.
    pub(crate) recovery_rates: Vec<Option<f64>>,
    /// Integer indices for curve lookups (optimization)
    pub(crate) curve_indices: Vec<Option<usize>>,
    /// Unique curve identifiers (referenced by curve_indices)
    pub(crate) unique_curves: Vec<String>,
    /// Whether each asset amortizes through level payments (mortgages, auto, etc.)
    pub(crate) is_amortizing: Vec<bool>,
    /// Frozen contractual level payment per amortizing asset.
    ///
    /// `None` until the first period the asset is amortized; then populated
    /// with the level payment computed from the balance and remaining term at
    /// that point, and reused for every subsequent period. A level-pay loan's
    /// scheduled payment is fixed at origination — prepayments shorten the
    /// loan, they do NOT reduce the scheduled payment. Recomputing it each
    /// period off the (shrinking) remaining balance understates scheduled
    /// principal after every prepayment.
    pub(crate) level_payments: Vec<Option<f64>>,
    /// Minimum all-in coupon per asset (a floor on the resolved floating
    /// rate); `None` for every asset except reinvestment purchases with a
    /// `coupon_floor`.
    pub(crate) rate_floors: Vec<Option<f64>>,
    /// Delinquent balance per bucket for each asset (empty until a
    /// delinquency model sizes it); the asset's `balances` entry includes
    /// these amounts.
    pub(crate) delinquent: Vec<Vec<f64>>,
    /// Balloon terms per asset, taken when the extension happens.
    pub(crate) balloon: Vec<Option<BalloonSpec>>,
    /// Prepayment penalty per asset.
    pub(crate) prepayment_penalty: Vec<Option<PrepaymentPenalty>>,
    /// Appraisal reduction per asset as a decimal fraction (0 when not
    /// specially serviced).
    pub(crate) appraisal_reduction: Vec<f64>,
    /// Pending NPL resolution per asset, taken when the loan resolves.
    pub(crate) liquidation: Vec<Option<LiquidationSpec>>,
}

impl PoolState {
    /// Append one asset row (a reinvestment purchase made during the
    /// simulation), registering its index curve when it is new.
    ///
    /// # Arguments
    ///
    /// * `asset` - Purchased asset; its balance, coupon, spread, index,
    ///   maturity, day count and amortization type seed the new row.
    /// * `rate_floor` - Minimum all-in annual coupon (decimal) applied when
    ///   the row's floating rate is resolved, if any.
    pub(crate) fn push_asset(&mut self, asset: &PoolAsset, rate_floor: Option<f64>) {
        self.ids.push(asset.id.to_string());
        self.balances.push(asset.balance.amount());
        self.rates.push(asset.rate);
        self.spread_bp.push(asset.spread_bp);
        self.maturities.push(asset.maturity);
        self.day_counts.push(asset.day_count);
        self.is_defaulted.push(asset.is_defaulted);
        self.smm_overrides.push(asset.smm_override);
        self.mdr_overrides.push(asset.mdr_override);
        self.recovery_rates.push(asset.recovery_rate);
        self.level_payments
            .push(asset.contractual_payment.map(|m| m.amount()));
        self.is_amortizing.push(asset.asset_type.is_amortizing());
        self.delinquent.push(Vec::new());
        self.balloon.push(asset.balloon);
        self.prepayment_penalty.push(asset.prepayment_penalty);
        self.appraisal_reduction.push(
            asset
                .special_servicing
                .map_or(0.0, |spec| spec.appraisal_reduction_pct / 100.0),
        );
        self.liquidation.push(asset.liquidation);
        self.rate_floors.push(rate_floor);
        let curve_index = asset.index_id.as_ref().map(|id| {
            self.unique_curves
                .iter()
                .position(|curve| curve == id)
                .unwrap_or_else(|| {
                    self.unique_curves.push(id.clone());
                    self.unique_curves.len() - 1
                })
        });
        self.curve_indices.push(curve_index);
    }

    /// Create a new PoolState from a AssetPool (AoS to SoA conversion).
    pub(crate) fn from_pool(pool: &AssetPool) -> Self {
        let n = pool.assets.len();
        let mut ids = Vec::with_capacity(n);
        let mut balances = Vec::with_capacity(n);
        let mut rates = Vec::with_capacity(n);
        let mut spread_bp = Vec::with_capacity(n);
        let mut index_ids = Vec::with_capacity(n);
        let mut maturities = Vec::with_capacity(n);
        let mut day_counts: Vec<DayCount> = Vec::with_capacity(n);
        let mut is_defaulted = Vec::with_capacity(n);
        let mut smm_overrides = Vec::with_capacity(n);
        let mut mdr_overrides = Vec::with_capacity(n);
        let mut recovery_rates = Vec::with_capacity(n);
        let mut level_payments = Vec::with_capacity(n);

        let mut is_amortizing = Vec::with_capacity(n);
        let mut delinquent = Vec::with_capacity(n);
        let mut balloon = Vec::with_capacity(n);
        let mut prepayment_penalty = Vec::with_capacity(n);
        let mut appraisal_reduction = Vec::with_capacity(n);
        let mut liquidation = Vec::with_capacity(n);

        for asset in &pool.assets {
            ids.push(asset.id.to_string());
            balances.push(if asset.is_defaulted {
                0.0
            } else {
                asset.balance.amount()
            });
            rates.push(asset.rate);
            spread_bp.push(asset.spread_bp);
            index_ids.push(asset.index_id.clone());
            maturities.push(asset.maturity);
            day_counts.push(asset.day_count);
            is_defaulted.push(asset.is_defaulted);
            smm_overrides.push(asset.smm_override);
            mdr_overrides.push(asset.mdr_override);
            recovery_rates.push(asset.recovery_rate);
            level_payments.push(asset.contractual_payment.map(|m| m.amount()));
            is_amortizing.push(asset.asset_type.is_amortizing());
            delinquent.push(
                asset
                    .delinquency_buckets
                    .as_ref()
                    .map(|buckets| buckets.iter().map(|m| m.amount()).collect())
                    .unwrap_or_default(),
            );
            balloon.push(asset.balloon);
            prepayment_penalty.push(asset.prepayment_penalty);
            appraisal_reduction.push(
                asset
                    .special_servicing
                    .map_or(0.0, |spec| spec.appraisal_reduction_pct / 100.0),
            );
            liquidation.push(asset.liquidation);
        }

        let mut unique_curves = Vec::new();
        let mut curve_map = finstack_quant_core::HashMap::default();
        let mut curve_indices = Vec::with_capacity(n);

        for id_opt in &index_ids {
            if let Some(id) = id_opt {
                if !curve_map.contains_key(id) {
                    curve_map.insert(id.clone(), unique_curves.len());
                    unique_curves.push(id.clone());
                }
                curve_indices.push(Some(curve_map[id]));
            } else {
                curve_indices.push(None);
            }
        }

        Self {
            ids,
            balances,
            rates,
            spread_bp,
            maturities,
            day_counts,
            is_defaulted,
            smm_overrides,
            mdr_overrides,
            recovery_rates,
            curve_indices,
            unique_curves,
            is_amortizing,
            level_payments,
            rate_floors: vec![None; n],
            delinquent,
            balloon,
            prepayment_penalty,
            appraisal_reduction,
            liquidation,
        }
    }

    /// Get the number of assets in the pool.
    pub(crate) fn len(&self) -> usize {
        self.balances.len()
    }
}
