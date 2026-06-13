//! Inflation-Linked Bond (ILB) types and implementation.

use crate::cashflow::traits::{CashflowProvider, DatedFlows};
use crate::instruments::common_impl::dependencies::MarketDependencies;
use crate::instruments::common_impl::traits::Attributes;
use finstack_core::currency::Currency;
use finstack_core::dates::{
    BusinessDayConvention, Date, DateExt, DayCount, DayCountContext, StubKind, Tenor,
};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::{InflationIndex, InflationInterpolation, InflationLag};
use finstack_core::market_data::term_structures::InflationCurve;
use finstack_core::money::Money;
use finstack_core::types::CalendarId;
use finstack_core::types::CurveId;
use finstack_core::types::InstrumentId;
use finstack_core::Result;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::sync::Arc;
use time::Duration;

use super::parameters::InflationLinkedBondParams;
use crate::impl_instrument_base;

/// Indexation method for inflation adjustment
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub enum IndexationMethod {
    /// Canadian model (real yield, indexed principal and coupons)
    Canadian,
    /// US TIPS model (real yield, indexed principal and coupons)
    TIPS,
    /// UK model (nominal yield; indexed principal and coupons, no deflation floor)
    UK,
    /// French OATi/OAT€i model
    French,
    /// Japanese JGBi model
    Japanese,
}

impl std::fmt::Display for IndexationMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexationMethod::Canadian => write!(f, "canadian"),
            IndexationMethod::TIPS => write!(f, "tips"),
            IndexationMethod::UK => write!(f, "uk"),
            IndexationMethod::French => write!(f, "french"),
            IndexationMethod::Japanese => write!(f, "japanese"),
        }
    }
}

impl std::str::FromStr for IndexationMethod {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "canadian" => Ok(IndexationMethod::Canadian),
            "tips" | "us" => Ok(IndexationMethod::TIPS),
            "uk" => Ok(IndexationMethod::UK),
            "french" => Ok(IndexationMethod::French),
            "japanese" | "jgb" => Ok(IndexationMethod::Japanese),
            other => Err(format!("Unknown indexation method: {}", other)),
        }
    }
}

impl IndexationMethod {
    /// Get the standard lag for this indexation method.
    ///
    /// # UK Gilt Convention Notes
    ///
    /// For `IndexationMethod::UK`, this returns the **legacy 8-month lag** which applies to
    /// UK Index-Linked Gilts issued **before September 2005**. For modern Gilts issued
    /// **on or after September 2005**, use [`IndexationMethod::standard_lag_modern`] which
    /// returns the 3-month lag consistent with international standards.
    ///
    /// | Issue Date | Indexation Lag | Interpolation |
    /// |------------|----------------|---------------|
    /// | Before Sep 2005 | 8 months | Step (monthly) |
    /// | Sep 2005 onwards | 3 months | Linear (daily) |
    ///
    /// # Production Recommendation
    ///
    /// When pricing UK Index-Linked Gilts, verify the bond's issue date:
    /// - Use [`new_uk_linker`](InflationLinkedBond::new_uk_linker) for legacy (8-month lag)
    /// - Use [`new_uk_linker_modern`](InflationLinkedBond::new_uk_linker_modern) for modern (3-month lag)
    pub fn standard_lag(&self) -> InflationLag {
        match self {
            IndexationMethod::UK => InflationLag::Months(8), // Legacy UK Gilts (pre-Sep 2005)
            IndexationMethod::Canadian
            | IndexationMethod::TIPS
            | IndexationMethod::French
            | IndexationMethod::Japanese => InflationLag::Months(3),
        }
    }

    /// Get the modern indexation lag for markets that have transitioned.
    ///
    /// # UK Gilt Modern Convention
    ///
    /// UK Index-Linked Gilts issued **on or after September 2005** use a 3-month lag
    /// with daily linear interpolation, aligning with TIPS and other international linkers.
    ///
    /// For legacy UK Gilts (pre-September 2005), use [`standard_lag`](Self::standard_lag).
    pub fn standard_lag_modern(&self) -> InflationLag {
        match self {
            IndexationMethod::UK => InflationLag::Months(3), // Modern UK Gilts (Sep 2005+)
            _ => self.standard_lag(),
        }
    }

    /// Whether this method uses daily interpolation.
    ///
    /// # UK Gilt Note
    ///
    /// For UK Gilts, this returns `false` (step interpolation) which applies to legacy bonds.
    /// Modern UK Index-Linked Gilts (post-Sep 2005) use daily linear interpolation;
    /// use [`uses_daily_interpolation_modern`](Self::uses_daily_interpolation_modern) for those.
    pub fn uses_daily_interpolation(&self) -> bool {
        matches!(self, IndexationMethod::Canadian | IndexationMethod::TIPS)
    }

    /// Whether modern issuances of this method use daily interpolation.
    ///
    /// Modern UK Index-Linked Gilts (September 2005 onwards) switched to daily linear
    /// interpolation, matching TIPS and other international linkers.
    pub fn uses_daily_interpolation_modern(&self) -> bool {
        matches!(
            self,
            IndexationMethod::Canadian
                | IndexationMethod::TIPS
                | IndexationMethod::UK
                | IndexationMethod::French
        )
    }
}

/// Deflation protection type
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub enum DeflationProtection {
    /// No deflation protection
    None,
    /// Protection at maturity only (principal floor at par)
    MaturityOnly,
    /// Protection on all payments (floor at par)
    AllPayments,
}

impl std::fmt::Display for DeflationProtection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeflationProtection::None => write!(f, "none"),
            DeflationProtection::MaturityOnly => write!(f, "maturity_only"),
            DeflationProtection::AllPayments => write!(f, "all_payments"),
        }
    }
}

impl std::str::FromStr for DeflationProtection {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "none" => Ok(DeflationProtection::None),
            "maturity_only" | "maturity" => Ok(DeflationProtection::MaturityOnly),
            "all_payments" | "all" => Ok(DeflationProtection::AllPayments),
            other => Err(format!("Unknown deflation protection: {}", other)),
        }
    }
}

#[derive(Clone)]
enum InflationSource {
    Index(Arc<InflationIndex>),
    Curve(Arc<InflationCurve>),
}

impl InflationSource {
    fn from_market(curves: &MarketContext, id: &CurveId) -> Result<Self> {
        if let Ok(index) = curves.get_inflation_index(id.as_str()) {
            Ok(Self::Index(index))
        } else {
            let curve = curves.get_inflation_curve(id.as_str())?;
            Ok(Self::Curve(curve))
        }
    }

    fn ratio(&self, bond: &InflationLinkedBond, date: Date) -> Result<f64> {
        match self {
            Self::Index(index) => bond.index_ratio(date, index.as_ref()),
            Self::Curve(curve) => bond.index_ratio_from_curve(date, curve.as_ref()),
        }
    }
}

/// Inflation-Linked Bond instrument
#[derive(
    Clone,
    Debug,
    finstack_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct InflationLinkedBond {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Notional amount (in real terms)
    pub notional: Money,
    /// Real coupon rate (as decimal)
    pub real_coupon: Decimal,
    /// Coupon frequency
    pub frequency: Tenor,
    /// Day count convention
    pub day_count: DayCount,
    /// Issue date
    #[schemars(with = "String")]
    pub issue_date: Date,
    /// Maturity date
    #[schemars(with = "String")]
    pub maturity: Date,
    /// Base CPI/index value at issue
    pub base_index: f64,
    /// Base date for index (may differ from issue date)
    #[schemars(with = "String")]
    pub base_date: Date,
    /// Indexation method
    pub indexation_method: IndexationMethod,
    /// Inflation lag
    pub lag: InflationLag,
    /// Deflation protection
    pub deflation_protection: DeflationProtection,
    /// Business day convention
    #[builder(default = BusinessDayConvention::ModifiedFollowing)]
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub bdc: BusinessDayConvention,
    /// Stub convention
    #[builder(default = StubKind::ShortFront)]
    #[serde(default = "crate::serde_defaults::stub_short_front")]
    pub stub: StubKind,
    /// Holiday calendar identifier
    pub calendar_id: Option<CalendarId>,
    /// Discount curve identifier (real or nominal depending on method)
    pub discount_curve_id: CurveId,
    /// Inflation index identifier
    pub inflation_index_id: CurveId,
    /// Quoted clean price (if available)
    pub quoted_clean: Option<f64>,
    /// Additional attributes
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl InflationLinkedBond {
    /// Create a canonical example US TIPS inflation-linked bond.
    ///
    /// Returns a 10-year TIPS with semi-annual coupons and standard 3-month lag.
    ///
    /// # Market Conventions (US TIPS)
    ///
    /// - **Day Count**: ACT/ACT ICMA (per Treasury market standards)
    /// - **Frequency**: Semi-annual
    /// - **Indexation Lag**: 3 months
    /// - **Interpolation**: Linear (daily)
    /// - **Deflation Protection**: Maturity only (principal floor at par)
    pub fn example() -> Self {
        use time::macros::date;
        Self {
            id: InstrumentId::new("TIPS-10Y"),
            notional: Money::new(1_000_000.0, Currency::USD),
            real_coupon: Decimal::new(25, 3),
            frequency: Tenor::semi_annual(),
            day_count: DayCount::ActActIsma, // US Treasury convention
            issue_date: date!(2024 - 01 - 15),
            maturity: date!(2034 - 01 - 15),
            base_index: 100.0,
            base_date: date!(2024 - 01 - 15),
            indexation_method: IndexationMethod::TIPS,
            lag: IndexationMethod::TIPS.standard_lag(),
            deflation_protection: DeflationProtection::MaturityOnly,
            bdc: BusinessDayConvention::Following,
            stub: StubKind::None,
            calendar_id: None,
            discount_curve_id: CurveId::new("USD-TIPS"),
            inflation_index_id: CurveId::new("US-CPI"),
            quoted_clean: None,
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Attributes::new(),
        }
    }

    /// Create a new US TIPS bond using parameter structs
    pub fn new_tips(
        id: impl Into<InstrumentId>,
        bond_params: &InflationLinkedBondParams,
        discount_curve_id: impl Into<CurveId>,
        inflation_index_id: impl Into<CurveId>,
    ) -> Self {
        Self {
            id: id.into(),
            notional: bond_params.notional,
            real_coupon: bond_params.real_coupon,
            frequency: bond_params.frequency,
            day_count: bond_params.day_count,
            issue_date: bond_params.issue,
            maturity: bond_params.maturity,
            base_index: bond_params.base_index,
            base_date: bond_params.issue,
            indexation_method: IndexationMethod::TIPS,
            lag: IndexationMethod::TIPS.standard_lag(),
            deflation_protection: DeflationProtection::MaturityOnly,
            bdc: BusinessDayConvention::Following,
            stub: StubKind::None,
            calendar_id: None,
            discount_curve_id: discount_curve_id.into(),
            inflation_index_id: inflation_index_id.into(),
            quoted_clean: None,
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Attributes::new(),
        }
    }

    /// Create a **legacy** UK Index-Linked Gilt (pre-September 2005) using parameter structs.
    ///
    /// # ⚠️ Important: Legacy vs Modern UK Gilts
    ///
    /// This constructor creates a linker with the **8-month lag** convention used for
    /// UK Index-Linked Gilts issued **before September 2005**. For gilts issued on or
    /// after September 2005, use [`new_uk_linker_modern`](Self::new_uk_linker_modern).
    ///
    /// # Market Conventions (Legacy UK Index-Linked Gilt)
    ///
    /// - **Day Count**: User-specified (typically ACT/ACT ICMA)
    /// - **Frequency**: User-specified (typically semi-annual)
    /// - **Indexation Lag**: 8 months
    /// - **Interpolation**: Step (monthly, no daily interpolation)
    /// - **Deflation Protection**: None (no floor)
    /// - **Index**: UK RPI (Retail Price Index)
    ///
    /// # Example Legacy Gilts
    ///
    /// - 2.5% IL Treasury Gilt 2020 (ISIN: GB0009081828)
    /// - 4.125% IL Treasury Gilt 2030 (ISIN: GB0031790826)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use finstack_valuations::instruments::fixed_income::inflation_linked_bond::{
    ///     InflationLinkedBond, InflationLinkedBondParams,
    /// };
    /// use finstack_core::currency::Currency;
    /// use finstack_core::dates::{DayCount, Tenor};
    /// use finstack_core::money::Money;
    /// use rust_decimal::Decimal;
    /// use time::macros::date;
    ///
    /// let params = InflationLinkedBondParams {
    ///     notional: Money::new(1_000_000.0, Currency::GBP),
    ///     real_coupon: Decimal::try_from(0.025).unwrap(),
    ///     frequency: Tenor::semi_annual(),
    ///     day_count: DayCount::ActActIsma,
    ///     issue: date!(1999-07-26),  // Pre-2005 issue
    ///     maturity: date!(2020-07-26),
    ///     base_index: 162.9,
    /// };
    ///
    /// let gilt = InflationLinkedBond::new_uk_linker(
    ///     "UKTI-2020",
    ///     &params,
    ///     date!(1999-07-26),
    ///     "GBP-REAL",
    ///     "UK-RPI",
    /// );
    /// ```
    pub fn new_uk_linker(
        id: impl Into<InstrumentId>,
        bond_params: &InflationLinkedBondParams,
        base_date: Date,
        discount_curve_id: impl Into<CurveId>,
        inflation_index_id: impl Into<CurveId>,
    ) -> Self {
        Self {
            id: id.into(),
            notional: bond_params.notional,
            real_coupon: bond_params.real_coupon,
            frequency: bond_params.frequency,
            day_count: bond_params.day_count,
            issue_date: bond_params.issue,
            maturity: bond_params.maturity,
            base_index: bond_params.base_index,
            base_date,
            indexation_method: IndexationMethod::UK,
            lag: IndexationMethod::UK.standard_lag(), // 8-month lag for legacy gilts
            deflation_protection: DeflationProtection::None,
            bdc: BusinessDayConvention::Following,
            stub: StubKind::None,
            calendar_id: None,
            discount_curve_id: discount_curve_id.into(),
            inflation_index_id: inflation_index_id.into(),
            quoted_clean: None,
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Attributes::new(),
        }
    }

    /// Create a **modern** UK Index-Linked Gilt (September 2005 onwards) using parameter structs.
    ///
    /// # Market Conventions (Modern UK Index-Linked Gilt)
    ///
    /// UK Index-Linked Gilts issued on or after **September 2005** adopted a new
    /// indexation methodology aligned with international standards (TIPS, Canadian RRBs):
    ///
    /// - **Day Count**: User-specified (typically ACT/ACT ICMA)
    /// - **Frequency**: User-specified (typically semi-annual)
    /// - **Indexation Lag**: 3 months (changed from 8 months)
    /// - **Interpolation**: Linear (daily interpolation between monthly readings)
    /// - **Deflation Protection**: None (no floor)
    /// - **Index**: UK RPI (Retail Price Index)
    ///
    /// # Why the Change?
    ///
    /// The UK Debt Management Office (DMO) changed the indexation methodology to:
    /// 1. Improve pricing transparency with daily accruals
    /// 2. Align with international inflation-linked bond standards
    /// 3. Reduce basis risk vs TIPS and other linkers
    ///
    /// # Example Modern Gilts
    ///
    /// - 0.125% IL Treasury Gilt 2024 (ISIN: GB00B3LZBF68)
    /// - 0.125% IL Treasury Gilt 2029 (ISIN: GB00B3Y1JG82)
    /// - 0.625% IL Treasury Gilt 2040 (ISIN: GB00B3MYD345)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use finstack_valuations::instruments::fixed_income::inflation_linked_bond::{
    ///     InflationLinkedBond, InflationLinkedBondParams,
    /// };
    /// use finstack_core::currency::Currency;
    /// use finstack_core::dates::{DayCount, Tenor};
    /// use finstack_core::money::Money;
    /// use rust_decimal::Decimal;
    /// use time::macros::date;
    ///
    /// let params = InflationLinkedBondParams {
    ///     notional: Money::new(1_000_000.0, Currency::GBP),
    ///     real_coupon: Decimal::try_from(0.00125).unwrap(),
    ///     frequency: Tenor::semi_annual(),
    ///     day_count: DayCount::ActActIsma,
    ///     issue: date!(2019-11-22),  // Post-2005 issue
    ///     maturity: date!(2029-11-22),
    ///     base_index: 294.318,  // RPI at issue
    /// };
    ///
    /// let gilt = InflationLinkedBond::new_uk_linker_modern(
    ///     "UKTI-2029",
    ///     &params,
    ///     "GBP-REAL",
    ///     "UK-RPI",
    /// );
    /// ```
    ///
    /// # Production Note
    ///
    /// When pricing modern UK gilts, ensure the inflation index provided uses
    /// **linear interpolation** (daily), not step interpolation. The pricer will
    /// validate this at runtime.
    pub fn new_uk_linker_modern(
        id: impl Into<InstrumentId>,
        bond_params: &InflationLinkedBondParams,
        discount_curve_id: impl Into<CurveId>,
        inflation_index_id: impl Into<CurveId>,
    ) -> Self {
        Self {
            id: id.into(),
            notional: bond_params.notional,
            real_coupon: bond_params.real_coupon,
            frequency: bond_params.frequency,
            day_count: bond_params.day_count,
            issue_date: bond_params.issue,
            maturity: bond_params.maturity,
            base_index: bond_params.base_index,
            base_date: bond_params.issue, // Modern gilts use issue date as base
            indexation_method: IndexationMethod::UK,
            lag: IndexationMethod::UK.standard_lag_modern(), // 3-month lag for modern gilts
            deflation_protection: DeflationProtection::None,
            bdc: BusinessDayConvention::Following,
            stub: StubKind::None,
            calendar_id: None,
            discount_curve_id: discount_curve_id.into(),
            inflation_index_id: inflation_index_id.into(),
            quoted_clean: None,
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Attributes::new(),
        }
    }

    /// Create a new Japanese Government Inflation-Linked Bond (JGBi) using parameter structs
    ///
    /// # Market Conventions (JGBi)
    ///
    /// - **Day Count**: ACT/365F (per MOF Japan standards)
    /// - **Frequency**: Semi-annual
    /// - **Indexation Lag**: 3 months
    /// - **Interpolation**: Step (monthly, no daily interpolation)
    /// - **Deflation Protection**: Maturity only (principal floor at par)
    /// - **Index**: Japan CPI (ex-fresh food)
    ///
    /// # Arguments
    ///
    /// * `id` - Unique instrument identifier
    /// * `bond_params` - Bond parameters (notional, coupon, dates, base_index)
    /// * `discount_curve_id` - Real rate discount curve identifier
    /// * `inflation_index_id` - Japan CPI index identifier
    ///
    /// # Example
    ///
    /// ```ignore
    /// use finstack_valuations::instruments::fixed_income::inflation_linked_bond::{
    ///     InflationLinkedBond, InflationLinkedBondParams,
    /// };
    /// use finstack_core::currency::Currency;
    /// use finstack_core::dates::{DayCount, Tenor};
    /// use finstack_core::money::Money;
    /// use rust_decimal::Decimal;
    /// use time::macros::date;
    ///
    /// let params = InflationLinkedBondParams {
    ///     notional: Money::new(100_000_000.0, Currency::JPY),
    ///     real_coupon: Decimal::try_from(0.001).unwrap(),
    ///     frequency: Tenor::semi_annual(),
    ///     day_count: DayCount::Act365F,
    ///     issue: date!(2024-03-10),
    ///     maturity: date!(2034-03-10),
    ///     base_index: 105.0,
    /// };
    ///
    /// let jgbi = InflationLinkedBond::new_jgbi("JGBi-10Y", &params, "JPY-REAL", "JP-CPI");
    /// ```
    pub fn new_jgbi(
        id: impl Into<InstrumentId>,
        bond_params: &InflationLinkedBondParams,
        discount_curve_id: impl Into<CurveId>,
        inflation_index_id: impl Into<CurveId>,
    ) -> Self {
        Self {
            id: id.into(),
            notional: bond_params.notional,
            real_coupon: bond_params.real_coupon,
            frequency: bond_params.frequency,
            day_count: DayCount::Act365F, // JGB standard day count
            issue_date: bond_params.issue,
            maturity: bond_params.maturity,
            base_index: bond_params.base_index,
            base_date: bond_params.issue,
            indexation_method: IndexationMethod::Japanese,
            lag: IndexationMethod::Japanese.standard_lag(),
            deflation_protection: DeflationProtection::MaturityOnly,
            bdc: BusinessDayConvention::Following,
            stub: StubKind::None,
            calendar_id: Some("jpto".into()),
            discount_curve_id: discount_curve_id.into(),
            inflation_index_id: inflation_index_id.into(),
            quoted_clean: None,
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Attributes::new(),
        }
    }

    fn inflation_source(&self, curves: &MarketContext) -> Result<InflationSource> {
        InflationSource::from_market(curves, &self.inflation_index_id)
    }

    /// Calculate the raw index ratio for a given date (CPI(date) / CPI(base)).
    ///
    /// Returns the **unfloored** ratio. Deflation protection is applied when
    /// building the canonical cashflow schedule so that the
    /// TIPS-style principal-only floor does not leak into coupon indexation.
    ///
    /// | Method | Lag | Interpolation |
    /// |--------|-----|---------------|
    /// | TIPS/Canadian | 3 months | Linear (daily) |
    /// | UK Gilt (legacy) | 8 months | Step (monthly) |
    /// | UK Gilt (modern) | 3 months | Linear (daily) |
    /// | French OAT€i | 3 months | Linear (daily) |
    /// | Japanese JGBi | 3 months | Step (monthly) |
    ///
    /// # Lag Ownership
    ///
    /// The bond owns the lag: it shifts the lookup date before calling
    /// `InflationIndex::value_on`. The provided index **must** have
    /// `lag == InflationLag::None` to avoid double-lagging.
    pub fn index_ratio(&self, date: Date, inflation_index: &InflationIndex) -> Result<f64> {
        if !matches!(inflation_index.lag(), InflationLag::None) {
            return Err(finstack_core::Error::Validation(
                "InflationIndex must have lag=None when used with InflationLinkedBond \
                 (the bond applies its own lag to avoid double-lagging)"
                    .to_string(),
            ));
        }

        let is_modern_uk = matches!(self.indexation_method, IndexationMethod::UK)
            && matches!(self.lag, InflationLag::Months(m) if m <= 3);

        if matches!(self.indexation_method, IndexationMethod::UK) {
            let valid_uk_lag =
                matches!(self.lag, InflationLag::Months(3) | InflationLag::Months(8));
            if !valid_uk_lag {
                return Err(finstack_core::Error::Validation(format!(
                    "Non-standard UK gilt lag {:?}: expected Months(3) for modern or \
                     Months(8) for legacy. Index ratio cannot be reliably computed for \
                     non-standard lags.",
                    self.lag
                )));
            }
        }

        let expected_interp = match self.indexation_method {
            IndexationMethod::TIPS | IndexationMethod::Canadian | IndexationMethod::French => {
                InflationInterpolation::Linear
            }
            IndexationMethod::UK => {
                if is_modern_uk {
                    InflationInterpolation::Linear
                } else {
                    InflationInterpolation::Step
                }
            }
            IndexationMethod::Japanese => InflationInterpolation::Step,
        };
        if inflation_index.interpolation() != expected_interp {
            return Err(finstack_core::Error::Validation(format!(
                "Inflation index interpolation mismatch for {:?}: expected {:?}, got {:?}",
                self.indexation_method,
                expected_interp,
                inflation_index.interpolation()
            )));
        }

        let current_index = match self.lag {
            // Months-lag with daily interpolation follows the official RefCPI
            // formula: anchor on first-of-month CPI(m−L)/CPI(m−L+1) and weight
            // by (day−1)/D(settlement month). A generic calendar shift +
            // interpolation mis-weights month-end settlements (day clamping).
            InflationLag::Months(m) if expected_interp == InflationInterpolation::Linear => {
                inflation_index.ref_cpi_months_lag(date, m.into())?
            }
            InflationLag::Months(m) => inflation_index.value_on(date.add_months(-(m as i32)))?,
            InflationLag::Days(d) => inflation_index.value_on(date - Duration::days(d as i64))?,
            _ => inflation_index.value_on(date)?,
        };

        if self.base_index <= 0.0 {
            return Err(finstack_core::InputError::NonPositiveValue.into());
        }
        Ok(current_index / self.base_index)
    }

    /// Calculate the raw index ratio using an inflation term structure.
    ///
    /// Uses the curve's own base date and day count for time conversion
    /// (via [`InflationCurve::cpi_on_date`]), avoiding day-count basis
    /// mismatches between the bond and the curve.
    ///
    /// Returns the **unfloored** ratio. Deflation protection is applied when
    /// building the canonical cashflow schedule.
    pub fn index_ratio_from_curve(
        &self,
        date: Date,
        inflation_curve: &InflationCurve,
    ) -> Result<f64> {
        let current_index = match self.lag {
            // Same official RefCPI weighting as `index_ratio`: anchor on
            // first-of-month CPI(m−L)/CPI(m−L+1), weight by (day−1)/D(m).
            InflationLag::Months(m) if self.daily_interpolated_indexation() => {
                let (anchor0, anchor1, weight) = Self::ref_cpi_anchors(date, m.into())?;
                let cpi0 = inflation_curve.cpi_on_date(anchor0)?;
                let cpi1 = inflation_curve.cpi_on_date(anchor1)?;
                cpi0 + weight * (cpi1 - cpi0)
            }
            InflationLag::Months(m) => inflation_curve.cpi_on_date(date.add_months(-(m as i32)))?,
            InflationLag::Days(d) => {
                inflation_curve.cpi_on_date(date - Duration::days(d as i64))?
            }
            _ => inflation_curve.cpi_on_date(date)?,
        };

        if self.base_index <= 0.0 {
            return Err(finstack_core::InputError::NonPositiveValue.into());
        }
        Ok(current_index / self.base_index)
    }

    /// Whether the indexation method uses daily-interpolated reference CPI
    /// (the official months-lag RefCPI formula) rather than a step lookup.
    fn daily_interpolated_indexation(&self) -> bool {
        match self.indexation_method {
            IndexationMethod::TIPS | IndexationMethod::Canadian | IndexationMethod::French => true,
            IndexationMethod::UK => matches!(self.lag, InflationLag::Months(m) if m <= 3),
            IndexationMethod::Japanese => false,
        }
    }

    /// First-of-month anchor dates and interpolation weight for the official
    /// RefCPI formula: `RefCPI(d) = CPI(m−L) + (day−1)/D(m) × [CPI(m−L+1) − CPI(m−L)]`.
    fn ref_cpi_anchors(date: Date, lag_months: u32) -> Result<(Date, Date, f64)> {
        let first_of_month =
            Date::from_calendar_date(date.year(), date.month(), 1).map_err(|_| {
                finstack_core::Error::Input(finstack_core::InputError::InvalidDateRange)
            })?;
        let anchor0 = first_of_month.add_months(-(lag_months as i32));
        let anchor1 = anchor0.add_months(1);
        let weight = (f64::from(date.day()) - 1.0) / f64::from(date.month().length(date.year()));
        Ok((anchor0, anchor1, weight))
    }

    /// Calculate index ratio sourcing inflation data from the market context
    pub fn index_ratio_from_market(&self, date: Date, curves: &MarketContext) -> Result<f64> {
        let source = self.inflation_source(curves)?;
        source.ratio(self, date)
    }

    /// Calculate real accrued interest at the given date
    fn accrued_real_interest(&self, as_of: Date) -> Result<f64> {
        if self.issue_date >= self.maturity {
            return Ok(0.0);
        }
        // Reconstruct the date schedule
        let sched = crate::cashflow::builder::build_dates(
            self.issue_date,
            self.maturity,
            self.frequency,
            self.stub,
            self.bdc,
            false,
            0,
            self.calendar_id
                .as_deref()
                .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID),
        )?;
        let periods = &sched.periods;
        if periods.is_empty() {
            return Ok(0.0);
        }

        // Find the active period
        for period in periods {
            let start = period.accrual_start;
            let end = period.accrual_end;
            if start <= as_of && as_of < end {
                // Found the active period
                // ACT/ACT (ISMA) — the default real day count for TIPS/linkers —
                // requires the coupon frequency and reference period; otherwise
                // year_fraction errors with MissingFrequencyForActActIsma.
                let isma_ctx = DayCountContext {
                    frequency: Some(self.frequency),
                    coupon_period: Some((start, end)),
                    ..Default::default()
                };
                let total_yf = self.day_count.year_fraction(start, end, isma_ctx)?;
                let elapsed_yf = self.day_count.year_fraction(start, as_of, isma_ctx)?;

                if total_yf <= 0.0 {
                    return Ok(0.0);
                }

                // Real coupon amount for the full period
                let coupon_rate = self
                    .real_coupon
                    .to_f64()
                    .ok_or(finstack_core::InputError::ConversionOverflow)?;
                let full_coupon = self.notional.amount() * coupon_rate * total_yf;

                // Linear accrual: Coupon * (elapsed / total)
                // Note: This matches standard bond accrual for fixed coupons.
                // If we need exact day-based fraction (e.g. Act/Act), year_fraction handles it roughly,
                // but strictly generic accrual uses Coupon * (AccrualDays / PeriodDays).
                // For Act/Act, year_fraction(start, as_of) / year_fraction(start, end) is the standard ratio.
                return Ok(full_coupon * (elapsed_yf / total_yf));
            }
        }

        // If we are past maturity or before issue
        Ok(0.0)
    }

    /// Build unadjusted real cashflow schedule (no inflation indexation).
    ///
    /// Cashflows with `payment_date < as_of` are excluded (already settled).
    /// The principal payment date is business-day adjusted via the bond's BDC.
    pub(crate) fn build_real_schedule(&self, as_of: Date) -> Result<DatedFlows> {
        if self.issue_date >= self.maturity {
            return Ok(vec![]);
        }
        let cal_id = self
            .calendar_id
            .as_deref()
            .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID);

        let sched = crate::cashflow::builder::build_dates(
            self.issue_date,
            self.maturity,
            self.frequency,
            self.stub,
            self.bdc,
            false,
            0,
            cal_id,
        )?;
        let periods = &sched.periods;
        if periods.is_empty() {
            return Ok(vec![]);
        }

        let mut flows = Vec::with_capacity(periods.len() + 1);
        for period in periods {
            if period.payment_date < as_of {
                continue;
            }
            let year_frac = self
                .day_count
                .year_fraction(
                    period.accrual_start,
                    period.accrual_end,
                    DayCountContext {
                        frequency: Some(self.frequency),
                        coupon_period: Some((period.accrual_start, period.accrual_end)),
                        ..Default::default()
                    },
                )?
                .max(0.0);
            let coupon_rate = self
                .real_coupon
                .to_f64()
                .ok_or(finstack_core::InputError::ConversionOverflow)?;
            let base_amount = self.notional.amount() * coupon_rate * year_frac;
            flows.push((
                period.payment_date,
                Money::new(base_amount, self.notional.currency()),
            ));
        }

        let principal_date =
            crate::cashflow::builder::calendar::adjust_date(self.maturity, self.bdc, cal_id)?;
        if principal_date >= as_of {
            flows.push((principal_date, self.notional));
        }

        Ok(flows)
    }

    /// Calculate real yield (yield in real terms, before inflation)
    ///
    /// Computes the internal rate of return of the **unadjusted (real) cashflows**
    /// against the **real price** (clean price + real accrued interest).
    ///
    /// This is the standard "Real Yield" quoted for TIPS and other linkers.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The clean price is non-positive or non-finite
    /// - There are no cashflows remaining
    /// - The YTM solver fails to converge
    pub fn real_yield(
        &self,
        clean_price: f64,
        _curves: &MarketContext,
        as_of: Date,
    ) -> Result<f64> {
        use crate::instruments::fixed_income::bond::pricing::quote_conversions::YieldCompounding;
        use crate::instruments::fixed_income::bond::pricing::ytm_solver::{
            solve_ytm, YtmPricingSpec,
        };

        if !clean_price.is_finite() || clean_price <= 0.0 {
            return Err(finstack_core::InputError::Invalid.into());
        }

        // 1. Build real cashflows (unadjusted for inflation)
        let flows = self.build_real_schedule(as_of)?;
        if flows.is_empty() {
            return Err(finstack_core::InputError::TooFewPoints.into());
        }

        // 2. Calculate Real Accrued Interest
        // Needed to convert Clean Real Price -> Dirty Real Price
        let real_accrued = self.accrued_real_interest(as_of)?;

        // 3. Calculate Target Dirty Real Price
        // Price is per 100 notional.
        let target_dirty_price_val = (clean_price / 100.0 * self.notional.amount()) + real_accrued;
        let target_price = Money::new(target_dirty_price_val, self.notional.currency());

        let spec = YtmPricingSpec {
            day_count: self.day_count,
            notional: self.notional,
            coupon_rate: self
                .real_coupon
                .to_f64()
                .ok_or(finstack_core::InputError::ConversionOverflow)?,
            compounding: YieldCompounding::Street,
            frequency: self.frequency,
        };

        // 4. Solve yield that matches the target real price to PV of real flows
        // The solver handles convergence internally; we propagate any solver errors
        // rather than clamping, so callers can detect and handle extreme cases.
        solve_ytm(&flows, as_of, target_price, spec)
    }

    /// Calculate breakeven inflation rate
    ///
    /// Uses the exact Fisher equation:
    /// `(1 + nominal) = (1 + real) × (1 + inflation)`
    ///
    /// Solving for inflation:
    /// `breakeven = (1 + nominal_eff) / (1 + real_eff) - 1`
    ///
    /// where both yields are expressed in **effective-annual compounding** before
    /// the Fisher identity is applied.  The Fisher identity is only exact when
    /// both legs share the same compounding basis; mixing conventions (e.g.
    /// a continuous nominal with a semi-annual real yield) introduces a
    /// systematic bias of several basis points.
    ///
    /// # Arguments
    ///
    /// * `nominal_bond_yield` — yield of the comparable nominal bond, quoted in
    ///   **effective-annual (annually-compounded)** convention.
    ///   When calling from `BreakevenInflationCalculator`, this comes from
    ///   `disc_curve.zero_annual(t)`.
    ///
    /// This is more accurate than the simplified approximation (`nominal - real`)
    /// at higher inflation levels where the cross-term becomes significant.
    pub fn breakeven_inflation(
        &self,
        nominal_bond_yield: f64,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<f64> {
        use crate::instruments::fixed_income::bond::pricing::quote_conversions::periods_per_year;
        use finstack_core::math::Compounding;
        use std::num::NonZeroU32;

        let clean_price = self.quoted_clean.ok_or_else(|| {
            finstack_core::Error::Validation(
                "Breakeven inflation requires a quoted clean price. \
                 Set quoted_clean on the bond or pass price explicitly."
                    .to_string(),
            )
        })?;
        let real_yield_street = self.real_yield(clean_price, curves, as_of)?;

        // Convert the Street-compounded (periodic, aligned with coupon frequency)
        // real yield to effective-annual compounding so both legs of the Fisher
        // identity use the same basis.
        //
        // Street convention: DF = (1 + y/f)^(-f*t), so
        //   r_annual = (1 + y/f)^f − 1  (for t = 1, or equivalently via DF round-trip).
        //
        // We use Compounding::convert_rate which goes through the DF:
        //   df = (1 + y/f)^(-f*t)  then  r_ann = df^(-1/t) - 1
        //
        // For annual frequency, Periodic(1) == Annual, so no correction is applied.
        let t = self
            .day_count
            .year_fraction(
                as_of,
                self.maturity,
                DayCountContext {
                    frequency: Some(self.frequency),
                    ..Default::default()
                },
            )
            .unwrap_or(1.0)
            .max(1e-6); // guard against zero / expired bond

        let n_f = periods_per_year(self.frequency).unwrap_or(1.0).max(1.0);
        // ILB coupon frequencies are always an integer number of periods per
        // year (annual/semi-annual/quarterly/monthly); a non-integer here would
        // mean an unsupported tenor whose compounding cannot be expressed as
        // `Compounding::Periodic(n)`.
        debug_assert!(
            (n_f.fract()).abs() < 1e-9,
            "non-integer coupon frequency {n_f} per year is unsupported for breakeven compounding"
        );
        let n_u32 = n_f.round() as u32;
        let real_compounding = NonZeroU32::new(n_u32)
            .map(Compounding::Periodic)
            .unwrap_or(Compounding::Annual);
        let real_yield_annual =
            real_compounding.convert_rate(real_yield_street, t, &Compounding::Annual);

        // Both yields are now effective-annual. Apply the exact Fisher identity.
        // Guard against division by zero for extreme negative real yields.
        let denominator = 1.0 + real_yield_annual;
        if denominator <= 0.0 {
            return Err(finstack_core::InputError::NonPositiveValue.into());
        }
        Ok((1.0 + nominal_bond_yield) / denominator - 1.0)
    }

    /// Calculate inflation-adjusted duration (Real Duration)
    ///
    /// Computes the modified duration of the bond based on its real (unadjusted)
    /// cashflows. This measures sensitivity to changes in real yield.
    pub fn real_duration(&self, curves: &MarketContext, as_of: Date) -> Result<f64> {
        use crate::instruments::fixed_income::bond::pricing::quote_conversions::{
            price_from_ytm_compounded_params, YieldCompounding,
        };

        // Determine a base clean price to center the bump around
        let base_clean = self.quoted_clean.unwrap_or(100.0);
        // Compute base yield
        let y0 = self.real_yield(base_clean, curves, as_of)?;
        // Bump yield by 1bp in decimal terms
        let bp = 1e-4;

        // Use real schedule to calculate sensitivity to real yield (Real Duration)
        // This assumes the "Duration" metric refers to the duration of the real bond component.
        let flows = self.build_real_schedule(as_of)?;

        // Helper to compute price from yield, propagating errors
        let price_from_yield = |y: f64| -> Result<f64> {
            let price = price_from_ytm_compounded_params(
                self.day_count,
                self.frequency,
                &flows,
                as_of,
                y,
                YieldCompounding::Street,
            )?;
            Ok(price / self.notional.amount() * 100.0)
        };

        let p_up = price_from_yield(y0 + bp)?;
        let p_dn = price_from_yield(y0 - bp)?;
        let dp_dy = (p_up - p_dn) / (2.0 * bp);

        // Modified duration in years per 1 delta in yield: D = - (1/P_dirty) * dP_dirty/dy
        let p0 = price_from_yield(y0)?.max(1e-6);
        Ok(-(dp_dy / p0))
    }
}

// Explicit Instrument trait implementation (replaces macro for better IDE visibility)
impl crate::instruments::common_impl::traits::Instrument for InflationLinkedBond {
    impl_instrument_base!(crate::pricer::InstrumentType::InflationLinkedBond);

    fn market_dependencies(&self) -> finstack_core::Result<MarketDependencies> {
        MarketDependencies::from_curve_dependencies(self)
    }

    fn base_value(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<finstack_core::money::Money> {
        if as_of > self.maturity {
            return Ok(finstack_core::money::Money::new(
                0.0,
                self.notional.currency(),
            ));
        }
        crate::instruments::common_impl::helpers::schedule_pv_using_curve_dc(
            self,
            curves,
            as_of,
            &self.discount_curve_id,
        )
    }

    fn expiry(&self) -> Option<finstack_core::dates::Date> {
        Some(self.maturity)
    }

    fn effective_start_date(&self) -> Option<finstack_core::dates::Date> {
        Some(self.issue_date)
    }

    fn pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&mut self.pricing_overrides)
    }

    fn pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&self.pricing_overrides)
    }
}

impl CashflowProvider for InflationLinkedBond {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn cashflow_schedule(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<crate::cashflow::builder::CashFlowSchedule> {
        if self.issue_date >= self.maturity {
            return Ok(crate::cashflow::traits::schedule_from_classified_flows(
                Vec::new(),
                self.day_count,
                crate::cashflow::traits::ScheduleBuildOpts {
                    notional_hint: Some(self.notional),
                    ..Default::default()
                },
            ));
        }
        let inflation_source = self.inflation_source(curves)?;
        let sched = crate::cashflow::builder::build_dates(
            self.issue_date,
            self.maturity,
            self.frequency,
            self.stub,
            self.bdc,
            false,
            0,
            self.calendar_id
                .as_deref()
                .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID),
        )?;
        let periods = &sched.periods;

        let mut detailed_flows = Vec::with_capacity(periods.len() + 1);
        let mut coupon_rows = Vec::with_capacity(periods.len());
        for period in periods {
            let accrual_factor = self
                .day_count
                .year_fraction(
                    period.accrual_start,
                    period.accrual_end,
                    DayCountContext {
                        frequency: Some(self.frequency),
                        coupon_period: Some((period.accrual_start, period.accrual_end)),
                        ..Default::default()
                    },
                )?
                .max(0.0);
            let coupon_rate = self
                .real_coupon
                .to_f64()
                .ok_or(finstack_core::InputError::ConversionOverflow)?;
            let base_amount = self.notional.amount() * coupon_rate * accrual_factor;
            let raw_ratio = inflation_source.ratio(self, period.payment_date)?;
            let ratio = match self.deflation_protection {
                DeflationProtection::AllPayments => raw_ratio.max(1.0),
                DeflationProtection::None | DeflationProtection::MaturityOnly => raw_ratio,
            };
            coupon_rows.push((
                period.payment_date,
                base_amount * ratio,
                accrual_factor,
                coupon_rate,
            ));
        }
        crate::cashflow::builder::emission::emit_inflation_coupons(
            self.notional.currency(),
            &coupon_rows,
            &mut detailed_flows,
        );

        let raw_principal_ratio = inflation_source.ratio(self, self.maturity)?;
        let principal_ratio = match self.deflation_protection {
            DeflationProtection::None => raw_principal_ratio,
            DeflationProtection::MaturityOnly | DeflationProtection::AllPayments => {
                raw_principal_ratio.max(1.0)
            }
        };
        detailed_flows.push(crate::cashflow::primitives::CashFlow {
            date: self.maturity,
            reset_date: None,
            amount: self.notional * principal_ratio,
            kind: crate::cashflow::primitives::CFKind::Notional,
            accrual_factor: 0.0,
            rate: None,
        });

        detailed_flows.sort_by(|a, b| match a.date.cmp(&b.date) {
            core::cmp::Ordering::Equal => crate::cashflow::builder::schedule::kind_rank(a.kind)
                .cmp(&crate::cashflow::builder::schedule::kind_rank(b.kind)),
            other => other,
        });

        let schedule = crate::cashflow::builder::CashFlowSchedule {
            flows: detailed_flows,
            notional: crate::cashflow::builder::Notional::par(
                self.notional.amount(),
                self.notional.currency(),
            ),
            day_count: self.day_count,
            meta: Default::default(),
        };
        Ok(schedule.normalize_public(
            as_of,
            crate::cashflow::builder::CashflowRepresentation::Projected,
        ))
    }
}

impl crate::instruments::common_impl::traits::CurveDependencies for InflationLinkedBond {
    fn curve_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::traits::InstrumentCurves> {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::traits::CashflowProvider;
    use finstack_core::cashflow::CFKind;
    use finstack_core::currency::Currency;
    use finstack_core::market_data::term_structures::{DiscountCurve, InflationCurve};
    use time::Month;

    // ── C8 regression: breakeven inflation compounding convention ──────────────

    /// Regression test for C8: the Fisher identity is only exact when both the
    /// nominal yield and the real yield are expressed in the **same** compounding
    /// convention.  Prior to this fix, `breakeven_inflation` applied the Fisher
    /// formula directly to a Street-compounded (semi-annual) real yield and an
    /// annually-compounded nominal yield, mixing conventions.
    ///
    /// Setup: a 10-year semi-annual ILB with a 4% real coupon priced at par.
    /// At par the real yield equals the coupon rate: 4 % (Street / semi-annual).
    /// The annual equivalent is (1 + 0.04/2)^2 − 1 = 4.04 %.
    ///
    /// With an 8% annually-compounded nominal yield:
    ///   correct  breakeven = (1.08) / (1.0404) − 1 ≈ 3.8062 %
    ///   (wrong)  breakeven = (1.08) / (1.04)   − 1 ≈ 3.8462 %     [~4 bp error]
    ///
    /// The test therefore:
    ///   • verifies the result is within 0.5 bp of the analytically-correct value
    ///   • verifies the result is NOT within 0.5 bp of the convention-mismatched value
    #[test]
    fn breakeven_inflation_uses_consistent_annual_compounding_c8() {
        let as_of = d(2024, Month::January, 15);
        let maturity = d(2034, Month::January, 15); // 10-year bond

        // Flat discount curve (not used in the calculation itself, just needed
        // for the MarketContext that real_yield passes through).
        let discount = DiscountCurve::builder("USD-NOM")
            .base_date(as_of)
            .knots([(0.0, 1.0), (10.0, 1.0)])
            .build()
            .expect("discount curve");
        let inflation = InflationCurve::builder("US-CPI")
            .base_date(as_of)
            .base_cpi(100.0)
            .knots([(0.0, 100.0), (10.0, 100.0)])
            .build()
            .expect("inflation curve");
        let market = MarketContext::new().insert(discount).insert(inflation);

        // Semi-annual ILB with a 4% real coupon, priced at par (100).
        // At par the real YTM equals the coupon: 4 % Street (semi-annual).
        let mut bond = InflationLinkedBond {
            id: InstrumentId::new("ILB-C8"),
            notional: Money::new(1_000_000.0, Currency::USD),
            real_coupon: Decimal::try_from(0.04).expect("valid coupon"),
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Thirty360,
            issue_date: as_of,
            maturity,
            base_index: 100.0,
            base_date: as_of,
            indexation_method: IndexationMethod::TIPS,
            lag: InflationLag::None,
            deflation_protection: DeflationProtection::MaturityOnly,
            bdc: BusinessDayConvention::Following,
            stub: StubKind::None,
            calendar_id: None,
            discount_curve_id: CurveId::new("USD-NOM"),
            inflation_index_id: CurveId::new("US-CPI"),
            quoted_clean: Some(100.0), // par → real yield == coupon (4 % semi-annual)
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Attributes::new(),
        };

        // The nominal yield is expressed in annual compounding (effective annual).
        let nominal_annual = 0.08_f64;

        // Analytically correct breakeven using annual Fisher identity:
        //   real_annual  = (1 + 0.04/2)^2 − 1 = 1.02^2 − 1
        //   breakeven    = (1 + nominal_annual) / (1 + real_annual) − 1
        let real_semi_annual = 0.04_f64;
        let real_annual = (1.0 + real_semi_annual / 2.0).powi(2) - 1.0; // 4.04 %
        let expected_breakeven = (1.0 + nominal_annual) / (1.0 + real_annual) - 1.0;

        // The convention-mismatched (wrong) result for comparison.
        let wrong_breakeven = (1.0 + nominal_annual) / (1.0 + real_semi_annual) - 1.0;

        // The difference between the correct and wrong answer must be at least 3 bp —
        // confirming this is a genuine discriminator, not a trivial check.
        let discrimination = (wrong_breakeven - expected_breakeven).abs();
        assert!(
            discrimination > 3e-4,
            "test is not discriminating: wrong={wrong_breakeven:.6}, correct={expected_breakeven:.6}, diff={discrimination:.6}"
        );

        let result = bond
            .breakeven_inflation(nominal_annual, &market, as_of)
            .expect("breakeven_inflation should succeed");

        // Must be within 0.5 bp of the analytically-correct answer.
        let tol = 5e-5; // 0.5 bp
        assert!(
            (result - expected_breakeven).abs() < tol,
            "breakeven mismatch: got {result:.6} ({:.4}%), expected {expected_breakeven:.6} ({:.4}%), diff = {:.2} bp",
            result * 100.0,
            expected_breakeven * 100.0,
            (result - expected_breakeven).abs() * 10_000.0,
        );

        // Must NOT be within 0.5 bp of the convention-mismatched value.
        assert!(
            (result - wrong_breakeven).abs() > tol,
            "result {result:.6} is suspiciously close to the convention-mismatched value {wrong_breakeven:.6}"
        );

        // Mutation: verify annual frequency bond uses a different correction factor.
        // Annual Street == Annual compounding, so no convention gap → both old and
        // new code agree, confirming the fix is frequency-aware.
        bond.frequency = Tenor::annual();
        bond.real_coupon = Decimal::try_from(0.04).expect("valid");
        let result_annual_freq = bond
            .breakeven_inflation(nominal_annual, &market, as_of)
            .expect("annual-freq breakeven");
        // For annual frequency Street ≡ Annual, so the correction is zero;
        // result should be within 0.5 bp of the "wrong" annual calculation.
        assert!(
            (result_annual_freq - wrong_breakeven).abs() < tol,
            "annual-freq bond should not be corrected: got {result_annual_freq:.6}, wrong={wrong_breakeven:.6}"
        );
    }

    /// Regression: the canonical TIPS/linker constructors default to ACT/ACT
    /// (ISMA), which requires a coupon frequency in the `DayCountContext`.
    /// Previously every real year-fraction call passed `DayCountContext::default()`
    /// (frequency = `None`) and errored with `MissingFrequencyForActActIsma`, so
    /// a default `example()` bond could not be scheduled or accrued at all —
    /// tests only passed by overriding `day_count` to `Thirty360`.
    #[test]
    fn act_act_isma_real_schedule_and_accrual_need_no_day_count_override() {
        let bond = InflationLinkedBond::example();
        assert_eq!(bond.day_count, DayCount::ActActIsma);

        let as_of = d(2026, Month::April, 10); // mid-life, between coupons

        let schedule = bond
            .build_real_schedule(as_of)
            .expect("ACT/ACT (ISMA) real schedule must not require a day-count override");
        assert!(!schedule.is_empty(), "schedule should contain future flows");

        let accrued = bond
            .accrued_real_interest(as_of)
            .expect("ACT/ACT (ISMA) accrued interest must not require a day-count override");
        assert!(accrued.is_finite() && accrued >= 0.0, "accrued = {accrued}");
    }

    #[test]
    fn real_duration_uses_dirty_model_price_denominator() {
        use crate::instruments::fixed_income::bond::pricing::quote_conversions::{
            price_from_ytm_compounded_params, YieldCompounding,
        };

        let issue = d(2024, Month::January, 15);
        let as_of = d(2024, Month::April, 15);
        let maturity = d(2034, Month::January, 15);
        let bond = InflationLinkedBond {
            id: InstrumentId::new("ILB-DURATION-DIRTY"),
            notional: Money::new(1_000_000.0, Currency::USD),
            real_coupon: Decimal::try_from(0.04).expect("valid coupon"),
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Thirty360,
            issue_date: issue,
            maturity,
            base_index: 100.0,
            base_date: issue,
            indexation_method: IndexationMethod::TIPS,
            lag: InflationLag::None,
            deflation_protection: DeflationProtection::MaturityOnly,
            bdc: BusinessDayConvention::Following,
            stub: StubKind::None,
            calendar_id: None,
            discount_curve_id: CurveId::new("USD-REAL"),
            inflation_index_id: CurveId::new("US-CPI"),
            quoted_clean: Some(100.0),
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Attributes::new(),
        };
        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-REAL")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (10.0, 1.0)])
                    .build()
                    .expect("discount curve"),
            )
            .insert(
                InflationCurve::builder("US-CPI")
                    .base_date(as_of)
                    .base_cpi(100.0)
                    .knots([(0.0, 100.0), (10.0, 100.0)])
                    .build()
                    .expect("inflation curve"),
            );

        let duration = bond.real_duration(&market, as_of).expect("real duration");
        let y0 = bond.real_yield(100.0, &market, as_of).expect("real yield");
        let flows = bond.build_real_schedule(as_of).expect("real schedule");
        let price_from_yield = |y: f64| -> Result<f64> {
            let price = price_from_ytm_compounded_params(
                bond.day_count,
                bond.frequency,
                &flows,
                as_of,
                y,
                YieldCompounding::Street,
            )?;
            Ok(price / bond.notional.amount() * 100.0)
        };
        let bp = 1e-4;
        let p_up = price_from_yield(y0 + bp).expect("price up");
        let p_dn = price_from_yield(y0 - bp).expect("price down");
        let p_dirty = price_from_yield(y0).expect("dirty model price");
        let dp_dy = (p_up - p_dn) / (2.0 * bp);
        let expected_dirty_duration = -(dp_dy / p_dirty);
        let clean_denominator_duration = -(dp_dy / 100.0);

        assert!(
            (p_dirty - 100.0).abs() > 0.05,
            "test must be mid-period with non-trivial accrued dirty/clean gap, dirty={p_dirty}"
        );
        assert!(
            (duration - expected_dirty_duration).abs() < 1e-10,
            "real duration must use dirty model price denominator: got {duration}, expected {expected_dirty_duration}"
        );
        assert!(
            (duration - clean_denominator_duration).abs() > 1e-4,
            "test must distinguish dirty denominator from clean denominator"
        );
    }

    fn d(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).expect("valid date")
    }

    fn market_with_deflation(as_of: Date) -> MarketContext {
        let discount = DiscountCurve::builder("USD-TIPS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (3.0, 1.0)])
            .build()
            .expect("discount curve should build");
        let inflation = InflationCurve::builder("US-CPI")
            .base_date(as_of)
            .base_cpi(100.0)
            .knots([(0.0, 100.0), (1.0, 95.0), (2.0, 90.0)])
            .build()
            .expect("inflation curve should build");
        MarketContext::new().insert(discount).insert(inflation)
    }

    fn sample_bond(deflation_protection: DeflationProtection) -> InflationLinkedBond {
        InflationLinkedBond {
            id: InstrumentId::new("ILB"),
            notional: Money::new(1_000_000.0, Currency::USD),
            real_coupon: Decimal::try_from(0.02).expect("valid coupon"),
            frequency: Tenor::annual(),
            day_count: DayCount::Thirty360,
            issue_date: d(2024, Month::January, 15),
            maturity: d(2026, Month::January, 15),
            base_index: 100.0,
            base_date: d(2024, Month::January, 15),
            indexation_method: IndexationMethod::TIPS,
            lag: InflationLag::None,
            deflation_protection,
            bdc: BusinessDayConvention::Following,
            stub: StubKind::None,
            calendar_id: None,
            discount_curve_id: CurveId::new("USD-TIPS"),
            inflation_index_id: CurveId::new("US-CPI"),
            quoted_clean: None,
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Attributes::new(),
        }
    }

    #[test]
    fn maturity_only_deflation_floor_applies_only_to_principal() {
        let as_of = d(2024, Month::January, 15);
        let market = market_with_deflation(as_of);
        let bond = sample_bond(DeflationProtection::MaturityOnly);

        let schedule = bond
            .cashflow_schedule(&market, as_of)
            .expect("schedule should build");
        assert!(schedule.flows.len() >= 3);

        let first_coupon = schedule
            .flows
            .iter()
            .find(|flow| flow.kind == CFKind::InflationCoupon)
            .expect("coupon flow")
            .amount
            .amount();
        let principal = schedule
            .flows
            .iter()
            .find(|flow| flow.date == bond.maturity && flow.kind == CFKind::Notional)
            .expect("principal flow")
            .amount
            .amount();

        assert!(
            first_coupon < 20_000.0,
            "coupon should still deflate under maturity-only floor"
        );
        assert_eq!(principal, 1_000_000.0);
    }

    #[test]
    fn all_payments_deflation_floor_applies_to_coupons_and_principal() {
        let as_of = d(2024, Month::January, 15);
        let market = market_with_deflation(as_of);
        let bond = sample_bond(DeflationProtection::AllPayments);

        let schedule = bond
            .cashflow_schedule(&market, as_of)
            .expect("schedule should build");
        assert!(schedule.flows.len() >= 3);

        let first_coupon = schedule
            .flows
            .iter()
            .find(|flow| flow.kind == CFKind::InflationCoupon)
            .expect("coupon flow")
            .amount
            .amount();
        let principal = schedule
            .flows
            .iter()
            .find(|flow| flow.date == bond.maturity && flow.kind == CFKind::Notional)
            .expect("principal flow")
            .amount
            .amount();

        assert_eq!(first_coupon, 20_000.0);
        assert_eq!(principal, 1_000_000.0);
    }

    #[test]
    fn cashflow_schedule_marks_inflation_coupons_explicitly() {
        let as_of = d(2024, Month::January, 15);
        let market = market_with_deflation(as_of);
        let bond = sample_bond(DeflationProtection::AllPayments);

        let schedule = bond
            .cashflow_schedule(&market, as_of)
            .expect("full schedule should build");

        assert!(
            schedule
                .flows
                .iter()
                .any(|flow| flow.kind == CFKind::InflationCoupon),
            "expected inflation-linked coupons in full schedule"
        );
        assert!(
            schedule
                .flows
                .iter()
                .any(|flow| flow.date == bond.maturity && flow.kind == CFKind::Notional),
            "expected principal notional flow at maturity"
        );
    }

    /// Regression test for the UK gilt lag validation that was previously
    /// `debug_assert!`-only (a release-build silent miscalc). A non-standard
    /// lag (anything other than 3 or 8 months) must now fail explicitly when
    /// `index_ratio` is called.
    #[test]
    fn uk_gilt_non_standard_lag_returns_err() {
        let as_of = d(2024, Month::January, 15);
        let mut bond = sample_bond(DeflationProtection::AllPayments);
        bond.indexation_method = IndexationMethod::UK;
        bond.lag = InflationLag::Months(5); // non-standard

        let index = InflationIndex::new(
            "UK-RPI",
            vec![
                (d(2023, Month::January, 1), 100.0),
                (d(2024, Month::January, 1), 102.0),
            ],
            Currency::USD,
        )
        .expect("index builds");

        let result = bond.index_ratio(as_of, &index);
        let err = result.expect_err("non-standard UK lag must fail at runtime");
        assert!(
            err.to_string().contains("Non-standard UK gilt lag"),
            "error message should identify the issue: {err}"
        );
    }

    #[test]
    fn uk_gilt_standard_lags_pass_validation() {
        let as_of = d(2024, Month::January, 15);
        let mut bond = sample_bond(DeflationProtection::AllPayments);
        bond.indexation_method = IndexationMethod::UK;
        bond.lag = InflationLag::Months(3); // modern UK standard

        // Use a Linear-interpolated index to satisfy the modern UK invariant.
        let index = InflationIndex::new(
            "UK-RPI",
            vec![
                (d(2023, Month::January, 1), 100.0),
                (d(2024, Month::January, 1), 102.0),
            ],
            Currency::USD,
        )
        .expect("index builds")
        .with_interpolation(InflationInterpolation::Linear);

        // Modern (3-month) lag must succeed; we don't care about the exact ratio,
        // just that the validation didn't reject the bond.
        bond.index_ratio(as_of, &index)
            .expect("3-month UK gilt lag is valid");
    }
}
