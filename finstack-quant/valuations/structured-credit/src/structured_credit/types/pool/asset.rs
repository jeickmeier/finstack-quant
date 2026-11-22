//! Individual collateral asset definition and constructors.

use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::math::units::BASIS_POINTS_PER_UNIT;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CreditRating, CurveId, InstrumentId};
use finstack_quant_valuations_fixed_income::bond::Bond;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};

use super::super::cmbs::{BalloonSpec, PrepaymentPenalty, SpecialServicingSpec};
use super::super::enums::AssetType;
use super::super::npl::LiquidationSpec;

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
    /// Rates forward-curve identifier for floating-rate assets, such as SOFR-3M.
    pub forward_curve_id: Option<String>,
    /// Floor on the floating index in basis points (100 = 1%), applied before
    /// `spread_bp` is added, matching `FloatingRateSpec::index_floor_bp`;
    /// `None` for no floor. Ignored on fixed-rate rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_floor_bp: Option<f64>,
    /// Business days before each coupon period start on which the floating
    /// index fixes (positive T-minus on a weekends-only calendar; `2` for
    /// term-SOFR loans, `0` fixes on the period start). Required when
    /// `forward_curve_id` is set; must be absent on fixed-rate rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_lag_days: Option<i32>,
    /// Contractual maturity date of the asset.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub maturity: Date,
    /// Optional agency credit rating of the obligor or asset; drives WARF,
    /// the CCC bucket and `CoverageRules.rating_haircuts`.
    pub rating: Option<CreditRating>,
    /// Optional industry classification used by concentration checks.
    pub industry: Option<String>,
    /// Optional obligor identifier used for single-name concentration limits.
    pub obligor_id: Option<String>,
    /// Whether the asset has defaulted (optional on the wire, default `false`).
    #[serde(default)]
    pub defaulted: bool,
    /// Realized or modeled recovery amount when the asset is defaulted.
    pub recovery_amount: Option<Money>,
    /// Economic default date for the outstanding recovery claim; required for defaulted assets.
    #[serde(default)]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub default_date: Option<Date>,
    /// Acquisition price in the asset currency, used for trading gain/loss.
    pub purchase_price: Option<Money>,
    /// Date on which the pool acquired the asset, if known.
    #[serde(default)]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub acquisition_date: Option<Date>,
    /// Origination date of the loan or receivable, if known: the anchor for
    /// the seasoning-dependent prepayment/default curves (PSA, SDA, ABS,
    /// vector) and the amortization schedule. Falls back to
    /// `acquisition_date`, then to the deal closing date (new collateral).
    #[serde(default)]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub origination_date: Option<Date>,
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
    /// Total commitment for revolving or delayed-draw collateral; `balance` is
    /// the drawn part. `None` for fully funded assets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commitment: Option<Money>,
    /// Contractual periodic payment for level-pay assets. Required for exact
    /// seasoned-loan amortization; when absent it is inferred once from the
    /// current state and the amortization term (or the remaining contractual
    /// periods to maturity).
    #[serde(default)]
    pub contractual_payment: Option<Money>,
    /// Amortization schedule length in months from origination
    /// (`acquisition_date`, else the deal closing) for level-pay assets, e.g.
    /// `360` for a 30-year schedule on a 10-year loan: the level payment is
    /// sized over `term − age` and the unamortized balance pays as a balloon
    /// at maturity. `None` amortizes fully by maturity. Ignored when
    /// `contractual_payment` is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amortization_term_months: Option<u32>,
    /// Interest-only period in months from origination: no scheduled
    /// principal while the loan is younger than this, then the level payment
    /// over the remaining schedule. `None` for no interest-only period.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io_months: Option<u32>,
    /// Market price as percent of par (`60.0` = 60% of par). Read by the
    /// excess-CCC coverage rule; an asset without a price is carried at par.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub market_price_pct: Option<f64>,
    /// Delinquent balance per bucket (30, 60, 90 days past due, ...) at the
    /// valuation date, in the pool's currency. Requires
    /// `credit_model.delinquency` with the same number of buckets; the sum
    /// must not exceed `balance`. `None` for a fully current asset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delinquency_buckets: Option<Vec<Money>>,
    /// Balloon maturity behavior (commercial mortgages): the share that does
    /// not refinance at maturity is extended instead of paid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balloon: Option<BalloonSpec>,
    /// Prepayment penalty the borrower pays on voluntary prepayment; the
    /// premium is collected as interest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prepayment_penalty: Option<PrepaymentPenalty>,
    /// Special-servicing state: an appraisal reduction cuts the interest
    /// advanced on the loan, and the special servicing fee accrues on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub special_servicing: Option<SpecialServicingSpec>,
    /// Net operating income per annum of the property securing the loan,
    /// in the loan's currency. The only NOI input of the CMBS pool
    /// debt-service coverage ratio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub noi: Option<Money>,
    /// Non-performing loan resolution: the loan pays nothing until the
    /// resolution date, where a share liquidates (a default whose recovery
    /// is the net proceeds) and the rest re-performs. The timeline replaces
    /// the default flag: `defaulted` stays `false` and
    /// `recovery_amount`/`default_date` stay unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liquidation: Option<LiquidationSpec>,
}

impl PoolAsset {
    /// Date the asset's seasoning is measured from: `origination_date`, else
    /// `acquisition_date`; `None` when the row is undated (new at closing).
    #[must_use]
    pub fn seasoning_anchor(&self) -> Option<Date> {
        self.origination_date.or(self.acquisition_date)
    }

    /// Create new pool asset from existing bond
    pub fn from_bond(bond: &Bond, industry: Option<String>) -> finstack_quant_core::Result<Self> {
        fn economics(
            spec: &finstack_quant_valuations_fixed_income::bond::CashflowSpec,
        ) -> finstack_quant_core::Result<(f64, Option<f64>, Option<String>, DayCount)> {
            match spec {
                finstack_quant_valuations_fixed_income::bond::CashflowSpec::Fixed(spec) => Ok((
                    spec.rate.to_f64().ok_or_else(|| {
                        finstack_quant_core::Error::Validation(
                            "bond fixed coupon cannot be represented as f64".into(),
                        )
                    })?,
                    None,
                    None,
                    spec.schedule.day_count,
                )),
                finstack_quant_valuations_fixed_income::bond::CashflowSpec::Floating(spec) => {
                    Err(finstack_quant_core::Error::Validation(format!(
                        "PoolAsset::from_bond cannot faithfully flatten floating-rate bond '{}' into the simplified pool asset schema: reset lag/frequency, fixing calendars, gearing, floors/caps, and overnight conventions would be lost; construct an explicit PoolAsset with supported pool-rate terms instead",
                        spec.rate_spec.forward_curve_id
                    )))
                }
                finstack_quant_valuations_fixed_income::bond::CashflowSpec::Amortizing {
                    base,
                    ..
                } => economics(base),
                finstack_quant_valuations_fixed_income::bond::CashflowSpec::StepUp(_) => {
                    Err(finstack_quant_core::Error::Validation(
                        "PoolAsset cannot faithfully represent a step-up bond coupon schedule"
                            .into(),
                    ))
                }
            }
        }

        let (rate, spread_bp, forward_curve_id, day_count) = economics(&bond.cashflow_spec)?;
        Ok(Self {
            id: bond.id.to_owned(),
            asset_type: AssetType::HighYieldBond {},
            balance: bond.notional,
            rate,
            spread_bp,
            forward_curve_id,
            maturity: bond.maturity,
            index_floor_bp: None,
            reset_lag_days: None,
            rating: None,
            industry,
            obligor_id: None,
            defaulted: false,
            recovery_amount: None,
            default_date: None,
            purchase_price: bond
                .instrument_pricing_overrides
                .market_quotes
                .quoted_clean_price_pct
                .map(|p| Money::new(p * bond.notional.amount() / 100.0, bond.notional.currency()))
                .transpose()?,
            acquisition_date: None,
            origination_date: Some(bond.issue_date),
            day_count,
            smm_override: None,
            mdr_override: None,
            recovery_rate: None,
            commitment: None,
            contractual_payment: None,
            amortization_term_months: None,
            io_months: None,
            market_price_pct: None,
            delinquency_buckets: None,
            balloon: None,
            prepayment_penalty: None,
            special_servicing: None,
            noi: None,
            liquidation: None,
        })
    }

    /// Create a floating rate loan asset with explicit spread tracking
    ///
    /// This helper ensures spread_bp is properly populated for WAS calculations.
    ///
    /// # Arguments
    /// * `id` - Unique asset identifier
    /// * `balance` - Current outstanding balance
    /// * `forward_curve_id` - Reference rate (e.g., "SOFR-3M", "LIBOR-3M")
    /// * `spread_bp` - Spread over index in basis points
    /// * `maturity` - Maturity date
    /// * `day_count` - Day count convention
    /// * `reset_lag_days` - Business days before each coupon period start on
    ///   which the index fixes (positive T-minus; `2` for term-SOFR loans,
    ///   `0` fixes on the period start).
    ///
    /// # Example
    /// ```text
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::{Date, DateExt, DayCount};
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_valuations_structured_credit::structured_credit::types::pool::PoolAsset;
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
    ///     2,      // fixes two business days before each period
    /// );
    /// // asset.rate will be 0.0 initially (set after index fixings)
    /// // asset.spread_bp will be Some(450.0) for WAS calculation
    /// ```
    pub fn floating_rate_loan(
        id: impl Into<InstrumentId>,
        balance: Money,
        forward_curve_id: impl Into<CurveId>,
        spread_bp: f64,
        maturity: Date,
        day_count: DayCount,
        reset_lag_days: i32,
    ) -> Self {
        Self {
            id: id.into(),
            asset_type: AssetType::FirstLienLoan {},
            balance,
            rate: spread_bp / BASIS_POINTS_PER_UNIT, // Initialize with spread only
            spread_bp: Some(spread_bp),
            forward_curve_id: Some(forward_curve_id.into().as_str().to_string()),
            maturity,
            index_floor_bp: None,
            reset_lag_days: Some(reset_lag_days),
            rating: None,
            industry: None,
            obligor_id: None,
            defaulted: false,
            recovery_amount: None,
            default_date: None,
            purchase_price: None,
            acquisition_date: None,
            origination_date: None,
            day_count,
            smm_override: None,
            mdr_override: None,
            recovery_rate: None,
            commitment: None,
            contractual_payment: None,
            amortization_term_months: None,
            io_months: None,
            market_price_pct: None,
            delinquency_buckets: None,
            balloon: None,
            prepayment_penalty: None,
            special_servicing: None,
            noi: None,
            liquidation: None,
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
            asset_type: AssetType::HighYieldBond {},
            balance,
            rate,
            spread_bp: None, // Fixed rate - no separate spread
            forward_curve_id: None,
            maturity,
            index_floor_bp: None,
            reset_lag_days: None,
            rating: None,
            industry: None,
            obligor_id: None,
            defaulted: false,
            recovery_amount: None,
            default_date: None,
            purchase_price: None,
            acquisition_date: None,
            origination_date: None,
            day_count,
            smm_override: None,
            mdr_override: None,
            recovery_rate: None,
            commitment: None,
            contractual_payment: None,
            amortization_term_months: None,
            io_months: None,
            market_price_pct: None,
            delinquency_buckets: None,
            balloon: None,
            prepayment_penalty: None,
            special_servicing: None,
            noi: None,
            liquidation: None,
        }
    }

    /// Set credit quality
    pub fn with_rating(mut self, rating: CreditRating) -> Self {
        self.rating = Some(rating);
        self
    }

    /// Get spread component in basis points
    ///
    /// Returns the explicit spread if available, otherwise derives from rate.
    pub fn spread_bp(&self) -> f64 {
        self.spread_bp.unwrap_or(self.rate * BASIS_POINTS_PER_UNIT)
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
}
