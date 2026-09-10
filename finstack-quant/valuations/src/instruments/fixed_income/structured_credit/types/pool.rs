//! Asset pool structures for structured credit instruments.

use crate::instruments::fixed_income::bond::Bond;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt, DayCount};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
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
    pub spread_bp: Option<f64>,
    /// Reference index identifier for floating-rate assets, such as SOFR-3M.
    pub index_id: Option<String>,
    /// Contractual maturity date of the asset.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
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
    /// Economic default date for the outstanding recovery claim; required for defaulted assets.
    #[serde(default, with = "finstack_quant_core::wire::optional_date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub default_date: Option<Date>,
    /// Acquisition price in the asset currency, used for trading gain/loss.
    pub purchase_price: Option<Money>,
    /// Date on which the pool acquired the asset, if known.
    #[serde(default, with = "finstack_quant_core::wire::optional_date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub acquisition_date: Option<Date>,
    /// Day-count convention used for coupon and accrual calculations.
    pub day_count: DayCount,
    /// Optional decimal Single Monthly Mortality override.
    #[serde(default)]
    pub smm_override: Option<f64>,
    /// Optional decimal Monthly Default Rate override.
    #[serde(default)]
    pub mdr_override: Option<f64>,
    /// Per-asset recovery fraction in [0, 1]; overrides the deal recovery model.
    #[serde(default)]
    pub recovery_rate: Option<f64>,
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
                    spec.schedule.day_count,
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

        let (rate, spread_bp, index_id, day_count) = economics(&bond.cashflow_spec)?;
        Ok(Self {
            id: bond.id.to_owned(),
            asset_type: AssetType::HighYieldBond {
                industry: industry.clone(),
            },
            balance: bond.notional,
            rate,
            spread_bp,
            index_id,
            maturity: bond.maturity,
            credit_quality: None,
            industry,
            obligor_id: None,
            is_defaulted: false,
            recovery_amount: None,
            default_date: None,
            purchase_price: bond
                .instrument_pricing_overrides
                .market_quotes
                .quoted_clean_price
                .map(|p| Money::new(p * bond.notional.amount() / 100.0, bond.notional.currency()))
                .transpose()?,
            acquisition_date: Some(bond.issue_date),
            day_count,
            smm_override: None,
            mdr_override: None,
            recovery_rate: None,
            contractual_payment: None,
        })
    }

    /// Create a floating rate loan asset with explicit spread tracking
    ///
    /// This helper ensures spread_bp is properly populated for WAS calculations.
    ///
    /// # Arguments
    /// * `id` - Unique asset identifier
    /// * `balance` - Current outstanding balance
    /// * `index_id` - Reference rate (e.g., "SOFR-3M", "LIBOR-3M")
    /// * `spread_bp` - Spread over index in basis points
    /// * `maturity` - Maturity date
    /// * `day_count` - Day count convention
    ///
    /// # Example
    /// ```text
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::{Date, DateExt, DayCount};
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_valuations::instruments::fixed_income::structured_credit::types::pool::PoolAsset;
    /// use time::Month;
    ///
    /// let maturity_date =
    ///     Date::from_calendar_date(2030, Month::January, 1).expect("valid date");
    ///
    /// let asset = PoolAsset::floating_rate_loan(
    ///     "LOAN001",
    ///     Money::from((10_000_000_i64, Currency::USD)),
    ///     "SOFR-3M",
    ///     450.0,  // 450bps spread
    ///     maturity_date,
    ///     DayCount::Act360,
    /// );
    /// // asset.rate will be 0.0 initially (set after index fixings)
    /// // asset.spread_bp will be Some(450.0) for WAS calculation
    /// ```
    pub fn floating_rate_loan(
        id: impl Into<InstrumentId>,
        balance: Money,
        index_id: impl Into<String>,
        spread_bp: f64,
        maturity: Date,
        day_count: DayCount,
    ) -> Self {
        Self {
            id: id.into(),
            asset_type: AssetType::FirstLienLoan { industry: None },
            balance,
            rate: spread_bp / BASIS_POINTS_DIVISOR, // Initialize with spread only
            spread_bp: Some(spread_bp),
            index_id: Some(index_id.into()),
            maturity,
            credit_quality: None,
            industry: None,
            obligor_id: None,
            is_defaulted: false,
            recovery_amount: None,
            default_date: None,
            purchase_price: None,
            acquisition_date: None,
            day_count,
            smm_override: None,
            mdr_override: None,
            recovery_rate: None,
            contractual_payment: None,
        }
    }

    /// Create a fixed rate bond asset
    ///
    /// For fixed rate assets, spread_bp is None (WAS falls back to rate).
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
            spread_bp: None, // Fixed rate - no separate spread
            index_id: None,
            maturity,
            credit_quality: None,
            industry: None,
            obligor_id: None,
            is_defaulted: false,
            recovery_amount: None,
            default_date: None,
            purchase_price: None,
            acquisition_date: None,
            day_count,
            smm_override: None,
            mdr_override: None,
            recovery_rate: None,
            contractual_payment: None,
        }
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

    /// Get spread component in basis points
    ///
    /// Returns the explicit spread if available, otherwise derives from rate.
    pub fn spread_bp(&self) -> f64 {
        self.spread_bp.unwrap_or(self.rate * BASIS_POINTS_DIVISOR)
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

    /// Record a default and its outstanding recovery claim, payable after the deal recovery lag.
    /// Repeating this operation replaces the current claim; it does not add another claim.
    ///
    /// # Arguments
    /// * `recovery_amount` - Unreceived recovery cash in the asset currency, from zero to defaulted par.
    /// * `default_date` - Economic default date used to schedule the recovery payment.
    pub fn default_with_recovery(&mut self, recovery_amount: Money, default_date: Date) {
        self.is_defaulted = true;
        self.recovery_amount = Some(recovery_amount);
        self.default_date = Some(default_date);
    }
}

/// Reinvestment period and rules
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ReinvestmentPeriod {
    /// End date of reinvestment period
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub end_date: Date,
    /// Whether reinvestment is currently active
    pub is_active: bool,
    /// Criteria for new investments
    pub criteria: ReinvestmentCriteria,
}

/// Criteria for reinvestment during revolving period
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ReinvestmentCriteria {
    /// Maximum purchase price (% of par)
    pub max_price: f64,
    /// Minimum annual decimal current yield: replacement coupon divided by purchase-price fraction.
    pub min_yield: f64,
    /// Require the surviving credit-quality distribution; pro-rata replacement always preserves it.
    pub maintain_credit_quality: bool,
    /// Require the surviving principal-payment profile; pro-rata replacement always preserves it.
    pub maintain_wal: bool,
}

impl Default for ReinvestmentCriteria {
    fn default() -> Self {
        Self {
            max_price: 100.0, // 100% of par
            min_yield: 0.0,
            maintain_credit_quality: true,
            maintain_wal: true,
        }
    }
}

/// AssetPool-level performance statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PoolStats {
    /// Weighted average coupon
    pub weighted_avg_coupon: f64,
    /// Weighted average spread
    pub weighted_avg_spread: f64,
    /// Weighted average maturity (WAM) in years.
    ///
    /// For weighted average life use
    /// [`AssetPool::weighted_avg_life_from_cashflows`].
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
    ///
    /// Part of the original-balance denominator in
    /// [`current_loss_percentage`](super::StructuredCredit::current_loss_percentage),
    /// so it must be stated: omitting it understates the denominator and
    /// overstates the reported loss rate.
    pub cumulative_scheduled_amortization: Money,

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
    /// Must be used with an empty `assets` vector; normalized into the same engine.
    pub rep_lines: Option<Vec<RepLine>>,
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
    /// Reference index (if floating)
    pub index_id: Option<String>,
    /// Weighted average maturity date
    #[serde(with = "finstack_quant_core::wire::date")]
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
            index_id: None,
            maturity,
            seasoning_months: 0,
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
        self.spread_bp.unwrap_or(self.rate * BASIS_POINTS_DIVISOR)
    }
}

impl AssetPool {
    /// Create new asset pool
    pub fn new(id: impl Into<InstrumentId>, deal_type: DealType, base_currency: Currency) -> Self {
        let zero_money = Money::from((0_i64, base_currency));
        Self {
            id: id.into(),
            deal_type,
            base_currency,
            assets: Vec::new(),
            cumulative_defaults: zero_money,
            cumulative_recoveries: zero_money,
            cumulative_prepayments: zero_money,
            cumulative_scheduled_amortization: zero_money,
            reinvestment_period: None,
            collection_account: zero_money,
            reserve_account: zero_money,
            excess_spread_account: zero_money,
            rep_lines: None,
        }
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

    /// Normalize representative collateral into the canonical asset engine.
    pub(crate) fn normalized(&self, closing_date: Date) -> finstack_quant_core::Result<Self> {
        self.validate_representation()?;
        let mut pool = self.clone();
        if let Some(lines) = pool.rep_lines.take() {
            for line in lines {
                for (name, value) in [
                    ("CPR", line.cpr),
                    ("CDR", line.cdr),
                    ("recovery", line.recovery_rate),
                ] {
                    if let Some(value) = value {
                        finstack_quant_core::validation::validate_f64_unit_interval(value, name)?;
                    }
                }
                if line.seasoning_months > Date::MIN.months_until(closing_date) {
                    return Err(finstack_quant_core::Error::Validation(
                        "representative seasoning precedes the supported date range".into(),
                    ));
                }
                let months = i32::try_from(line.seasoning_months).map_err(|_| {
                    finstack_quant_core::Error::Validation(
                        "representative seasoning exceeds the supported date range".into(),
                    )
                })?;
                let acquisition = closing_date.add_months(-months);
                pool.assets.push(PoolAsset {
                    id: line.id.into(),
                    asset_type: line.asset_type,
                    balance: line.balance,
                    rate: line.rate,
                    spread_bp: line.spread_bp,
                    index_id: line.index_id,
                    maturity: line.maturity,
                    credit_quality: None,
                    industry: None,
                    obligor_id: None,
                    is_defaulted: false,
                    recovery_amount: None,
                    default_date: None,
                    purchase_price: None,
                    acquisition_date: Some(acquisition),
                    day_count: line.day_count,
                    smm_override: line.cpr.map(|cpr| 1.0 - (1.0 - cpr).powf(1.0 / 12.0)),
                    mdr_override: line.cdr.map(|cdr| 1.0 - (1.0 - cdr).powf(1.0 / 12.0)),
                    recovery_rate: line.recovery_rate,
                    contractual_payment: None,
                });
            }
        }
        for asset in &pool.assets {
            pool.validate_asset_currency(asset)?;
            for (name, value) in [
                ("SMM", asset.smm_override),
                ("MDR", asset.mdr_override),
                ("recovery", asset.recovery_rate),
            ] {
                if let Some(value) = value {
                    finstack_quant_core::validation::validate_f64_unit_interval(value, name)?;
                }
            }
        }
        Ok(pool)
    }

    fn validate_representation(&self) -> finstack_quant_core::Result<()> {
        if !self.assets.is_empty()
            && self
                .rep_lines
                .as_ref()
                .is_some_and(|lines| !lines.is_empty())
        {
            return Err(finstack_quant_core::Error::Validation(
                "asset pool must contain assets or representative lines, not both".into(),
            ));
        }
        Ok(())
    }

    /// Balance-weighted collateral age, using acquisition as the supplied
    /// origination proxy; undated assets are new at closing.
    pub(crate) fn weighted_average_seasoning(&self, date: Date, closing_date: Date) -> u32 {
        let mut weighted = 0.0;
        let mut total = 0.0;
        for asset in &self.assets {
            let weight = asset.balance.amount().max(0.0);
            let start = asset.acquisition_date.unwrap_or(closing_date);
            let age = if start < date {
                start.months_until(date)
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
        if let Some(lines) = &self.rep_lines {
            if self.assets.is_empty() {
                return lines
                    .iter()
                    .try_fold(Money::from((0_i64, self.base_currency)), |sum, line| {
                        sum.checked_add(line.balance)
                    });
            }
        }
        self.assets
            .iter()
            .try_fold(Money::from((0_i64, self.base_currency)), |acc, asset| {
                self.validate_asset_currency(asset)?;
                acc.checked_add(asset.balance)
            })
    }

    /// Total pool balance excluding defaulted assets
    pub fn performing_balance(&self) -> finstack_quant_core::Result<Money> {
        self.validate_representation()?;
        if self.assets.is_empty() {
            return self.total_balance();
        }
        self.assets.iter().filter(|a| !a.is_defaulted).try_fold(
            Money::from((0_i64, self.base_currency)),
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
            .chain(
                self.rep_lines
                    .iter()
                    .flatten()
                    .map(|line| line.rate * line.balance.amount()),
            )
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
            .chain(self.rep_lines.iter().flatten().filter_map(|line| {
                line.day_count
                    .year_fraction(
                        as_of.min(line.maturity),
                        line.maturity,
                        finstack_quant_core::dates::DayCountContext::default(),
                    )
                    .ok()
                    .map(|term| term * line.balance.amount())
            }))
            .sum::<f64>();

        weighted_sum / total_balance
    }

    /// Calculate true weighted average life from cashflow schedule
    ///
    /// This is the market-standard calculation that should be used when
    /// full cashflow schedules are available.
    ///
    /// # Arguments
    ///
    /// * `cashflows` - Principal payment dates and amounts used as the WAL
    ///   weights. Interest-only rows should be omitted; the helper treats
    ///   each amount as principal.
    /// * `as_of` - Origin date for the year-fraction clock. Payments on or
    ///   before this date do not contribute to WAL.
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
    pub fn get_base_currency(&self) -> Currency {
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
    ///   `spread_bp` are skipped rather than counting their all-in coupon as
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
        let pool = AssetPool::new("TEST_POOL", DealType::Clo, Currency::USD);
        assert_eq!(pool.id.as_str(), "TEST_POOL");
        assert_eq!(pool.deal_type, DealType::Clo);
        assert_eq!(pool.get_base_currency(), Currency::USD);
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
            Money::from((100_i64, Currency::USD)),
            0.05,
            maturity,
            DayCount::Act365F,
        );

        // Asset B: Thirty360 -> 1.0 years (360/360)
        let asset_b = PoolAsset::fixed_rate_bond(
            "B",
            Money::from((100_i64, Currency::USD)),
            0.05,
            maturity,
            DayCount::Thirty360,
        );

        let mut pool = AssetPool::new("POOL", DealType::Abs, Currency::USD);
        pool.assets.push(asset_a);
        pool.assets.push(asset_b);

        let wam = pool.weighted_avg_maturity(as_of);

        // Both should be exactly 1.0
        assert!((wam - 1.0).abs() < 1e-10);
    }
}
