//! Asset pool structures for structured credit instruments.

use crate::instruments::fixed_income::bond::Bond;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt, DayCount};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{Bps, InstrumentId, Percentage, Rate};
use rust_decimal::prelude::ToPrimitive;

use finstack_quant_core::HashMap;

use serde::{Deserialize, Serialize};

use super::enums::{AssetType, DealType};
use crate::instruments::fixed_income::structured_credit::types::constants::BASIS_POINTS_DIVISOR;
use finstack_quant_core::types::CreditRating;

/// Individual asset held in a structured-credit collateral pool.
///
/// Monetary fields use the asset's native currency. Rates are annual decimal
/// rates unless a field explicitly says basis points.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PoolAsset {
    /// Stable identifier used to match the asset to diagnostics and scenarios.
    pub id: InstrumentId,
    /// Economic asset classification used by pool-level assumptions.
    pub asset_type: AssetType,
    /// Current outstanding principal balance in the asset currency.
    pub balance: Money,
    /// Current all-in coupon as an annual decimal rate.
    pub rate: f64,
    /// Spread over the reference index in basis points for floating-rate assets.
    /// Weighted-average-spread calculations use this field rather than the
    /// all-in coupon because the index component is not a credit spread.
    pub spread_bps: Option<f64>,
    /// Reference index identifier for floating-rate assets, such as SOFR-3M.
    pub index_id: Option<String>,
    /// Contractual maturity date of the asset.
    #[schemars(with = "String")]
    pub maturity: Date,
    /// Optional credit-quality classification of the obligor or asset.
    pub credit_quality: Option<CreditRating>,
    /// Optional industry classification used by concentration checks.
    pub industry: Option<String>,
    /// Optional obligor identifier used for single-name concentration limits.
    pub obligor_id: Option<String>,
    /// Whether the asset is currently treated as defaulted by the pool model.
    pub is_defaulted: bool,
    /// Realized or modeled recovery amount when the asset is defaulted.
    pub recovery_amount: Option<Money>,
    /// Acquisition price in the asset currency, used for trading gain/loss.
    pub purchase_price: Option<Money>,
    /// Date on which the pool acquired the asset, if known.
    #[schemars(with = "Option<String>")]
    pub acquisition_date: Option<Date>,
    /// Day-count convention used for coupon and accrual calculations.
    pub day_count: DayCount,
    /// Optional decimal Single Monthly Mortality override.
    #[serde(default)]
    pub smm_override: Option<f64>,
    /// Optional decimal Monthly Default Rate override.
    #[serde(default)]
    pub mdr_override: Option<f64>,
    /// Contractual periodic payment for level-pay assets. Required for exact
    /// seasoned-loan amortization; when absent it is inferred once from the
    /// current state and remaining contractual periods.
    #[serde(default)]
    pub contractual_payment: Option<Money>,
}

impl PoolAsset {
    /// Create new pool asset from existing bond
    pub fn from_bond(bond: &Bond, industry: Option<String>) -> finstack_quant_core::Result<Self> {
        fn economics(
            spec: &crate::instruments::fixed_income::bond::CashflowSpec,
        ) -> finstack_quant_core::Result<(f64, Option<f64>, Option<String>, DayCount)> {
            match spec {
                crate::instruments::fixed_income::bond::CashflowSpec::Fixed(spec) => Ok((
                    spec.rate.to_f64().ok_or_else(|| {
                        finstack_quant_core::Error::Validation(
                            "bond fixed coupon cannot be represented as f64".into(),
                        )
                    })?,
                    None,
                    None,
                    spec.schedule.dc,
                )),
                crate::instruments::fixed_income::bond::CashflowSpec::Floating(spec) => {
                    Err(finstack_quant_core::Error::Validation(format!(
                        "PoolAsset::from_bond cannot faithfully flatten floating-rate bond '{}' into the simplified pool asset schema: reset lag/frequency, fixing calendars, gearing, floors/caps, and overnight conventions would be lost; construct an explicit PoolAsset with supported pool-rate terms instead",
                        spec.rate_spec.index_id
                    )))
                }
                crate::instruments::fixed_income::bond::CashflowSpec::Amortizing {
                    base, ..
                } => economics(base),
                crate::instruments::fixed_income::bond::CashflowSpec::StepUp(_) => {
                    Err(finstack_quant_core::Error::Validation(
                        "PoolAsset cannot faithfully represent a step-up bond coupon schedule"
                            .into(),
                    ))
                }
            }
        }

        let (rate, spread_bps, index_id, day_count) = economics(&bond.cashflow_spec)?;
        Ok(Self {
            id: bond.id.to_owned(),
            asset_type: AssetType::HighYieldBond {
                industry: industry.clone(),
            },
            balance: bond.notional,
            rate,
            spread_bps,
            index_id,
            maturity: bond.maturity,
            credit_quality: None,
            industry,
            obligor_id: None,
            is_defaulted: false,
            recovery_amount: None,
            purchase_price: bond
                .instrument_pricing_overrides
                .market_quotes
                .quoted_clean_price
                .map(|p| Money::new(p * bond.notional.amount() / 100.0, bond.notional.currency())),
            acquisition_date: Some(bond.issue_date),
            day_count,
            smm_override: None,
            mdr_override: None,
            contractual_payment: None,
        })
    }

    /// Create a floating rate loan asset with explicit spread tracking
    ///
    /// This helper ensures spread_bps is properly populated for WAS calculations.
    ///
    /// # Arguments
    /// * `id` - Unique asset identifier
    /// * `balance` - Current outstanding balance
    /// * `index_id` - Reference rate (e.g., "SOFR-3M", "LIBOR-3M")
    /// * `spread_bps` - Spread over index in basis points
    /// * `maturity` - Maturity date
    /// * `day_count` - Day count convention
    ///
    /// # Example
    /// ```text
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::{Date, DayCount};
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_valuations::instruments::fixed_income::structured_credit::types::pool::PoolAsset;
    /// use time::Month;
    ///
    /// let maturity_date =
    ///     Date::from_calendar_date(2030, Month::January, 1).expect("valid date");
    ///
    /// let asset = PoolAsset::floating_rate_loan(
    ///     "LOAN001",
    ///     Money::new(10_000_000.0, Currency::USD),
    ///     "SOFR-3M",
    ///     450.0,  // 450bps spread
    ///     maturity_date,
    ///     DayCount::Act360,
    /// );
    /// // asset.rate will be 0.0 initially (set after index fixings)
    /// // asset.spread_bps will be Some(450.0) for WAS calculation
    /// ```
    pub fn floating_rate_loan(
        id: impl Into<InstrumentId>,
        balance: Money,
        index_id: impl Into<String>,
        spread_bps: f64,
        maturity: Date,
        day_count: DayCount,
    ) -> Self {
        Self {
            id: id.into(),
            asset_type: AssetType::FirstLienLoan { industry: None },
            balance,
            rate: spread_bps / BASIS_POINTS_DIVISOR, // Initialize with spread only
            spread_bps: Some(spread_bps),
            index_id: Some(index_id.into()),
            maturity,
            credit_quality: None,
            industry: None,
            obligor_id: None,
            is_defaulted: false,
            recovery_amount: None,
            purchase_price: None,
            acquisition_date: None,
            day_count,
            smm_override: None,
            mdr_override: None,
            contractual_payment: None,
        }
    }

    /// Create a floating rate loan asset using a typed spread in basis points.
    pub fn floating_rate_loan_bps(
        id: impl Into<InstrumentId>,
        balance: Money,
        index_id: impl Into<String>,
        spread_bps: Bps,
        maturity: Date,
        day_count: DayCount,
    ) -> Self {
        Self::floating_rate_loan(
            id,
            balance,
            index_id,
            spread_bps.as_bps() as f64,
            maturity,
            day_count,
        )
    }

    /// Create a fixed rate bond asset
    ///
    /// For fixed rate assets, spread_bps is None (WAS falls back to rate).
    pub fn fixed_rate_bond(
        id: impl Into<InstrumentId>,
        balance: Money,
        rate: f64,
        maturity: Date,
        day_count: DayCount,
    ) -> Self {
        Self {
            id: id.into(),
            asset_type: AssetType::HighYieldBond { industry: None },
            balance,
            rate,
            spread_bps: None, // Fixed rate - no separate spread
            index_id: None,
            maturity,
            credit_quality: None,
            industry: None,
            obligor_id: None,
            is_defaulted: false,
            recovery_amount: None,
            purchase_price: None,
            acquisition_date: None,
            day_count,
            smm_override: None,
            mdr_override: None,
            contractual_payment: None,
        }
    }

    /// Create a fixed rate bond asset using a typed rate.
    pub fn fixed_rate_bond_rate(
        id: impl Into<InstrumentId>,
        balance: Money,
        rate: Rate,
        maturity: Date,
        day_count: DayCount,
    ) -> Self {
        Self::fixed_rate_bond(id, balance, rate.as_decimal(), maturity, day_count)
    }

    /// Set credit quality
    pub fn with_rating(mut self, rating: CreditRating) -> Self {
        self.credit_quality = Some(rating);
        self
    }

    /// Set industry classification
    pub fn with_industry(mut self, industry: impl Into<String>) -> Self {
        self.industry = Some(industry.into());
        self
    }

    /// Set obligor identifier
    pub fn with_obligor(mut self, obligor_id: impl Into<String>) -> Self {
        self.obligor_id = Some(obligor_id.into());
        self
    }

    /// Set day count convention
    pub fn with_day_count(mut self, day_count: DayCount) -> Self {
        self.day_count = day_count;
        self
    }

    /// Current yield of the asset
    pub fn current_yield(&self) -> f64 {
        self.rate
    }

    /// Get spread component in basis points
    ///
    /// Returns the explicit spread if available, otherwise derives from rate.
    pub fn spread_bps(&self) -> f64 {
        self.spread_bps.unwrap_or(self.rate * BASIS_POINTS_DIVISOR)
    }

    /// Remaining term to maturity in years
    pub fn remaining_term(
        &self,
        as_of: Date,
        day_count: DayCount,
    ) -> finstack_quant_core::Result<f64> {
        // Handle past maturity - return 0.0 instead of error
        if as_of >= self.maturity {
            return Ok(0.0);
        }
        day_count.year_fraction(
            as_of,
            self.maturity,
            finstack_quant_core::dates::DayCountContext::default(),
        )
    }

    /// Mark asset as defaulted with recovery
    pub fn default_with_recovery(&mut self, recovery_amount: Money, _default_date: Date) {
        self.is_defaulted = true;
        self.recovery_amount = Some(recovery_amount);
        // Could store default_date in additional field if needed
    }
}

/// Reinvestment period and rules
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReinvestmentPeriod {
    /// End date of reinvestment period
    #[schemars(with = "String")]
    pub end_date: Date,
    /// Whether reinvestment is currently active
    pub is_active: bool,
    /// Criteria for new investments
    pub criteria: ReinvestmentCriteria,
}

/// Criteria for reinvestment during revolving period
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReinvestmentCriteria {
    /// Maximum purchase price (% of par)
    pub max_price: f64,
    /// Minimum yield requirement
    pub min_yield: f64,
    /// Must maintain credit quality distribution
    pub maintain_credit_quality: bool,
    /// Must maintain weighted average life
    pub maintain_wal: bool,
    /// Must satisfy eligibility criteria
    pub apply_eligibility_criteria: bool,
}

impl Default for ReinvestmentCriteria {
    fn default() -> Self {
        Self {
            max_price: 100.0, // 100% of par
            min_yield: 0.0,
            maintain_credit_quality: true,
            maintain_wal: true,
            apply_eligibility_criteria: true,
        }
    }
}

/// AssetPool-level performance statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PoolStats {
    /// Weighted average coupon
    pub weighted_avg_coupon: f64,
    /// Weighted average spread
    pub weighted_avg_spread: f64,
    /// Weighted average life (approximation using WAM)
    /// For accurate WAL, use weighted_avg_life_from_cashflows()
    pub weighted_avg_life: f64,
    /// Weighted average maturity (WAM) in years
    #[serde(default)]
    pub weighted_avg_maturity: f64,
    /// Weighted average rating factor
    pub weighted_avg_rating_factor: f64,
    /// Diversity score (Moody's methodology)
    pub diversity_score: f64,
    /// Number of obligors
    pub num_obligors: usize,
    /// Number of industries
    pub num_industries: usize,
    /// Cumulative default rate
    pub cumulative_default_rate: f64,
    /// Recovery rate on defaults
    pub recovery_rate: f64,
    /// Prepayment rate (annualized)
    pub prepayment_rate: f64,
}

/// Main asset pool structure
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssetPool {
    /// AssetPool identifier
    pub id: InstrumentId,

    /// Deal type classification
    pub deal_type: DealType,

    /// Base currency for every asset and pool-level account.
    pub base_currency: Currency,

    /// Underlying assets
    pub assets: Vec<PoolAsset>,

    /// Performance tracking
    /// Cumulative defaults to date
    pub cumulative_defaults: Money,
    /// Cumulative recoveries on defaulted assets
    pub cumulative_recoveries: Money,
    /// Cumulative prepayments (voluntary early repayment)
    pub cumulative_prepayments: Money,
    /// Cumulative scheduled amortization (level-pay principal for amortizing assets).
    /// `None` means not tracked (legacy data); treated as zero in loss calculations.
    #[serde(default)]
    pub cumulative_scheduled_amortization: Option<Money>,

    /// Reinvestment management
    /// Reinvestment period configuration (if applicable)
    pub reinvestment_period: Option<ReinvestmentPeriod>,

    /// AssetPool-level accounts
    /// Collection account balance (collected but not yet distributed)
    pub collection_account: Money,
    /// Reserve account balance (for credit enhancement)
    pub reserve_account: Money,
    /// Excess spread account (accumulated excess interest)
    pub excess_spread_account: Money,

    /// Aggregated representative lines (optional optimization)
    /// If present, pricing engine will use these instead of individual assets.
    pub rep_lines: Option<Vec<RepLine>>,
}

/// Representative line for aggregated pool modeling
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepLine {
    /// Unique identifier for the rep line
    pub id: String,
    /// Aggregated balance
    pub balance: Money,
    /// Weighted average coupon
    pub rate: f64,
    /// Weighted average spread (for floating rate)
    pub spread_bps: Option<f64>,
    /// Reference index (if floating)
    pub index_id: Option<String>,
    /// Weighted average maturity date
    #[schemars(with = "String")]
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
}

impl RepLine {
    /// Create a new rep line
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        balance: Money,
        rate: f64,
        spread_bps: Option<f64>,
        index_id: Option<String>,
        maturity: Date,
        seasoning_months: u32,
        day_count: DayCount,
    ) -> Self {
        Self {
            id: id.into(),
            balance,
            rate,
            spread_bps,
            index_id,
            maturity,
            seasoning_months,
            day_count,
            cpr: None,
            cdr: None,
            recovery_rate: None,
        }
    }

    /// Set CPR override
    pub fn with_cpr(mut self, cpr: f64) -> Self {
        self.cpr = Some(cpr);
        self
    }

    /// Set CPR override using a typed percentage.
    pub fn with_cpr_pct(mut self, cpr: Percentage) -> Self {
        self.cpr = Some(cpr.as_decimal());
        self
    }

    /// Set CDR override
    pub fn with_cdr(mut self, cdr: f64) -> Self {
        self.cdr = Some(cdr);
        self
    }

    /// Set CDR override using a typed percentage.
    pub fn with_cdr_pct(mut self, cdr: Percentage) -> Self {
        self.cdr = Some(cdr.as_decimal());
        self
    }

    /// Set recovery rate override
    pub fn with_recovery_rate(mut self, recovery_rate: f64) -> Self {
        self.recovery_rate = Some(recovery_rate);
        self
    }

    /// Set recovery rate override using a typed percentage.
    pub fn with_recovery_rate_pct(mut self, recovery_rate: Percentage) -> Self {
        self.recovery_rate = Some(recovery_rate.as_decimal());
        self
    }

    /// Get effective spread in basis points
    pub fn spread_bps(&self) -> f64 {
        self.spread_bps.unwrap_or(self.rate * BASIS_POINTS_DIVISOR)
    }
}

impl AssetPool {
    /// Create new asset pool
    pub fn new(id: impl Into<InstrumentId>, deal_type: DealType, base_currency: Currency) -> Self {
        let zero_money = Money::new(0.0, base_currency);
        Self {
            id: id.into(),
            deal_type,
            base_currency,
            assets: Vec::new(),
            cumulative_defaults: zero_money,
            cumulative_recoveries: zero_money,
            cumulative_prepayments: zero_money,
            cumulative_scheduled_amortization: None,
            reinvestment_period: None,
            collection_account: zero_money,
            reserve_account: zero_money,
            excess_spread_account: zero_money,
            rep_lines: None,
        }
    }

    /// Aggregate assets into representative lines based on key characteristics.
    ///
    /// Groups assets by:
    /// - Asset Type
    /// - Index ID (for floating rate)
    /// - Day Count
    ///
    /// Assets within groups are aggregated by summing balances and weighting rates/spreads.
    /// Maturity is weighted average.
    pub fn aggregate_to_rep_lines(&mut self, as_of: Date) {
        if self.assets.is_empty() {
            return;
        }

        let mut groups: HashMap<String, Vec<&PoolAsset>> = HashMap::default();

        for asset in &self.assets {
            let key = format!(
                "{:?}|{:?}|{:?}",
                asset.asset_type, asset.index_id, asset.day_count
            );
            groups.entry(key).or_default().push(asset);
        }

        let mut rep_lines = Vec::with_capacity(groups.len());
        let base_ccy = self.base_currency();

        for (i, (_, group_assets)) in groups.into_iter().enumerate() {
            let total_balance: f64 = group_assets.iter().map(|a| a.balance.amount()).sum();

            if total_balance <= 0.0 {
                continue;
            }

            let mut weighted_rate = 0.0;
            let mut weighted_spread = 0.0;
            let mut weighted_maturity_days = 0.0;
            let mut weighted_seasoning = 0.0;

            let first = group_assets[0];
            let index_id = first.index_id.clone();
            let day_count = first.day_count;

            for asset in &group_assets {
                let weight = asset.balance.amount() / total_balance;
                weighted_rate += asset.rate * weight;
                weighted_spread += asset.spread_bps() * weight;

                let days_to_maturity = (asset.maturity - as_of).whole_days().max(0) as f64;
                weighted_maturity_days += days_to_maturity * weight;

                if let Some(acq_date) = asset.acquisition_date {
                    if as_of > acq_date {
                        let months = acq_date.months_until(as_of) as f64;
                        weighted_seasoning += months * weight;
                    }
                }
            }

            let maturity_date = as_of + time::Duration::days(weighted_maturity_days as i64);
            let spread_opt = if index_id.is_some() {
                Some(weighted_spread)
            } else {
                None
            };

            let rep_line = RepLine::new(
                format!("REP_{}", i),
                Money::new(total_balance, base_ccy),
                weighted_rate,
                spread_opt,
                index_id,
                maturity_date,
                weighted_seasoning.round() as u32,
                day_count,
            );

            rep_lines.push(rep_line);
        }

        self.rep_lines = Some(rep_lines);
    }

    /// Add asset from existing bond
    pub fn add_bond(
        &mut self,
        bond: &Bond,
        industry: Option<String>,
    ) -> finstack_quant_core::Result<&mut Self> {
        let asset = PoolAsset::from_bond(bond, industry)?;
        self.assets.push(asset);
        Ok(self)
    }

    /// Total pool balance
    pub fn total_balance(&self) -> finstack_quant_core::Result<Money> {
        self.assets
            .iter()
            .try_fold(Money::new(0.0, self.base_currency), |acc, asset| {
                self.validate_asset_currency(asset)?;
                acc.checked_add(asset.balance)
            })
    }

    /// Total pool balance excluding defaulted assets
    pub fn performing_balance(&self) -> finstack_quant_core::Result<Money> {
        self.assets.iter().filter(|a| !a.is_defaulted).try_fold(
            Money::new(0.0, self.base_currency),
            |acc, asset| {
                self.validate_asset_currency(asset)?;
                acc.checked_add(asset.balance)
            },
        )
    }

    /// Calculate weighted average coupon
    pub fn weighted_avg_coupon(&self) -> f64 {
        let total_balance = match self.total_balance() {
            Ok(b) => b.amount(),
            Err(_) => return 0.0,
        };

        if total_balance == 0.0 {
            return 0.0;
        }

        let weighted_sum = self
            .assets
            .iter()
            .map(|a| a.rate * a.balance.amount())
            .sum::<f64>();

        weighted_sum / total_balance
    }

    /// Calculate weighted average maturity (WAM)
    ///
    /// This calculates the balance-weighted average time to maturity.
    /// Note: This is NOT the same as Weighted Average Life (WAL).
    /// WAL requires cashflow schedules and is calculated from principal payments.
    pub fn weighted_avg_maturity(&self, as_of: Date) -> f64 {
        let total_balance = match self.total_balance() {
            Ok(b) => b.amount(),
            Err(_) => return 0.0,
        };

        if total_balance == 0.0 {
            return 0.0;
        }

        let weighted_sum = self
            .assets
            .iter()
            .filter_map(|a| {
                a.remaining_term(as_of, a.day_count)
                    .ok()
                    .map(|term| term * a.balance.amount())
            })
            .sum::<f64>();

        weighted_sum / total_balance
    }

    /// Calculate true weighted average life from cashflow schedule
    ///
    /// This is the market-standard calculation that should be used when
    /// full cashflow schedules are available.
    pub fn weighted_avg_life_from_cashflows(
        &self,
        cashflows: &[(Date, Money)],
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        crate::cashflow::builder::schedule::weighted_average_life_from_principal(
            cashflows.iter().copied(),
            as_of,
        )
    }

    /// Calculate diversity score (simplified Moody's approach)
    pub fn diversity_score(&self) -> f64 {
        let total_balance = match self.total_balance() {
            Ok(b) => b.amount(),
            Err(_) => return 0.0,
        };

        if total_balance == 0.0 {
            return 0.0;
        }

        // Collect obligor balances
        // Optimization: Sort and scan to avoid HashMap allocation if possible,
        // but since we need to aggregate by string ID, a HashMap is often cleanest.
        // However, to avoid allocating a new HashMap every time, we could pass a workspace.
        // For now, we'll stick to the HashMap but pre-allocate capacity.
        // A better optimization for the future would be to integerize obligor IDs.

        let mut obligor_balances: HashMap<&str, f64> = {
            let mut m = HashMap::default();
            m.reserve(self.assets.len());
            m
        };

        // Group by obligor
        for asset in &self.assets {
            if let Some(ref obligor) = asset.obligor_id {
                *obligor_balances.entry(obligor.as_str()).or_insert(0.0) += asset.balance.amount();
            }
        }

        // Calculate diversity score = (sum of balances)^2 / sum of (balance^2)
        let sum_balances: f64 = obligor_balances.values().sum();
        let sum_squares: f64 = obligor_balances.values().map(|b| b * b).sum();

        if sum_squares > 0.0 {
            (sum_balances * sum_balances) / sum_squares
        } else {
            0.0
        }
    }

    /// Base currency of the pool.
    pub fn base_currency(&self) -> Currency {
        self.base_currency
    }

    fn validate_asset_currency(&self, asset: &PoolAsset) -> finstack_quant_core::Result<()> {
        let actual = asset.balance.currency();
        if actual != self.base_currency {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: self.base_currency,
                actual,
            });
        }
        if let Some(payment) = asset.contractual_payment {
            if payment.currency() != self.base_currency {
                return Err(finstack_quant_core::Error::CurrencyMismatch {
                    expected: self.base_currency,
                    actual: payment.currency(),
                });
            }
        }
        Ok(())
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
    ///   `spread_bps` are skipped rather than counting their all-in coupon as
    ///   spread (the old `rate × 10⁴` fallback inflated WAS one-sidedly).
    ///
    /// The denominator is the balance of the INCLUDED assets, so a pool of
    /// only fixed-rate or defaulted assets returns 0.
    pub fn weighted_avg_spread(&self) -> f64 {
        let mut weighted_spread = 0.0;
        let mut included_balance = 0.0;
        for asset in &self.assets {
            if asset.is_defaulted {
                continue;
            }
            let Some(spread_bps) = asset.spread_bps else {
                continue;
            };
            weighted_spread += spread_bps * asset.balance.amount();
            included_balance += asset.balance.amount();
        }

        if included_balance == 0.0 {
            return 0.0;
        }
        weighted_spread / included_balance
    }
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
pub fn calculate_pool_stats(pool: &AssetPool, as_of: Date) -> PoolStats {
    // Count unique obligors and industries
    let mut obligors = finstack_quant_core::HashSet::default();
    let mut industries = finstack_quant_core::HashSet::default();

    for asset in &pool.assets {
        if let Some(ref obligor) = asset.obligor_id {
            obligors.insert(obligor.clone());
        }
        if let Some(ref industry) = asset.industry {
            industries.insert(industry.clone());
        }
    }

    // Calculate default rate
    let total_balance = pool.total_balance().map(|b| b.amount()).unwrap_or(0.0);
    let defaulted_balance: f64 = pool
        .assets
        .iter()
        .filter(|a| a.is_defaulted)
        .map(|a| a.balance.amount())
        .sum();

    let cumulative_default_rate = if total_balance > 0.0 {
        defaulted_balance / total_balance * 100.0
    } else {
        0.0
    };

    PoolStats {
        weighted_avg_coupon: pool.weighted_avg_coupon(),
        weighted_avg_spread: pool.weighted_avg_spread(),
        // Maintain historical behavior: WAL field carries WAM proxy unless cashflows provided externally
        weighted_avg_life: pool.weighted_avg_maturity(as_of),
        weighted_avg_maturity: pool.weighted_avg_maturity(as_of),
        weighted_avg_rating_factor: 0.0, // Computed separately if needed
        diversity_score: pool.diversity_score(),
        num_obligors: obligors.len(),
        num_industries: industries.len(),
        cumulative_default_rate,
        recovery_rate: 0.0,   // Computed separately if needed
        prepayment_rate: 0.0, // Computed separately if needed
    }
}

/// Result of concentration limit checking
/// Result of concentration limit check
#[derive(Debug, Clone)]
pub struct ConcentrationCheckResult {
    /// List of concentration limit violations found
    pub violations: Vec<ConcentrationViolation>,
}

impl ConcentrationCheckResult {
    /// Check if any limits are violated
    pub fn has_violations(&self) -> bool {
        !self.violations.is_empty()
    }
}

/// Individual concentration limit violation
#[derive(Debug, Clone)]
pub struct ConcentrationViolation {
    /// Type of violation (e.g., "Issuer", "Industry", "Rating")
    pub violation_type: String,
    /// Identifier of violating entity (e.g., issuer name)
    pub identifier: String,
    /// Current concentration level as percentage
    pub current_level: f64,
    /// Maximum allowed concentration level
    pub limit: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;

    #[test]
    fn test_pool_creation() {
        let pool = AssetPool::new("TEST_POOL", DealType::CLO, Currency::USD);
        assert_eq!(pool.id.as_str(), "TEST_POOL");
        assert_eq!(pool.deal_type, DealType::CLO);
        assert_eq!(pool.base_currency(), Currency::USD);
    }

    #[test]
    fn test_rep_line_aggregation() {
        let mut pool = AssetPool::new("TEST_POOL", DealType::RMBS, Currency::USD);
        let as_of = Date::from_calendar_date(2023, time::Month::January, 1).expect("valid date");

        // Add 3 identical assets
        for i in 0..3 {
            pool.assets.push(PoolAsset::fixed_rate_bond(
                format!("ASSET_{}", i),
                Money::new(100_000.0, Currency::USD),
                0.05,
                as_of + time::Duration::days(360 * 10), // 10 years
                finstack_quant_core::dates::DayCount::Thirty360,
            ));
        }

        // Add 2 different assets
        for i in 3..5 {
            pool.assets.push(PoolAsset::fixed_rate_bond(
                format!("ASSET_{}", i),
                Money::new(200_000.0, Currency::USD),
                0.06,
                as_of + time::Duration::days(360 * 5), // 5 years
                finstack_quant_core::dates::DayCount::Thirty360,
            ));
        }

        pool.aggregate_to_rep_lines(as_of);

        assert!(pool.rep_lines.is_some());
        let rep_lines = pool
            .rep_lines
            .as_ref()
            .expect("rep_lines should be set after aggregation");

        // Should have 1 rep line (grouped by type/index/dc)
        assert_eq!(rep_lines.len(), 1);
        let rep = &rep_lines[0];

        // Total balance: 3*100k + 2*200k = 700k
        assert_eq!(rep.balance.amount(), 700_000.0);

        // Weighted rate: (300k * 0.05 + 400k * 0.06) / 700k = 0.0557...
        assert!((rep.rate - 0.055714).abs() < 0.0001);
    }
}

#[cfg(test)]
mod market_standards_tests {
    use super::*;
    use finstack_quant_core::dates::DayCount;

    #[test]
    fn test_wam_mixed_day_counts() {
        let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid date");
        let maturity = Date::from_calendar_date(2026, time::Month::January, 1).expect("Valid date");

        // Asset A: Act365F (Standard) -> 1.0 years
        let asset_a = PoolAsset::fixed_rate_bond(
            "A",
            Money::new(100.0, Currency::USD),
            0.05,
            maturity,
            DayCount::Act365F,
        );

        // Asset B: Thirty360 -> 1.0 years (360/360)
        let asset_b = PoolAsset::fixed_rate_bond(
            "B",
            Money::new(100.0, Currency::USD),
            0.05,
            maturity,
            DayCount::Thirty360,
        );

        let mut pool = AssetPool::new("POOL", DealType::ABS, Currency::USD);
        pool.assets.push(asset_a);
        pool.assets.push(asset_b);

        let wam = pool.weighted_avg_maturity(as_of);

        // Both should be exactly 1.0
        assert!((wam - 1.0).abs() < 1e-10);
    }
}
