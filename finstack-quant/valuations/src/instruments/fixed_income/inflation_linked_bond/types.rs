//! Inflation-Linked Bond (ILB) types and implementation.

use crate::cashflow::traits::DatedFlows;
use crate::instruments::common_impl::dependencies::MarketDependencies;
use crate::instruments::common_impl::traits::Attributes;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, DayCountContext, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::InflationLag;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CalendarId;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_core::Result;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use super::parameters::InflationLinkedBondParams;
use crate::impl_instrument_base;

/// Indexation method for inflation adjustment
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum IndexationMethod {
    /// Canadian model (real yield, indexed principal and coupons)
    Canadian,
    /// US TIPS model (real yield, indexed principal and coupons)
    Tips,
    /// UK model (nominal yield; indexed principal and coupons, no deflation floor)
    Uk,
    /// French OATi/OAT€i model
    French,
    /// Japanese JGBi model
    Japanese,
}

impl std::fmt::Display for IndexationMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexationMethod::Canadian => write!(f, "canadian"),
            IndexationMethod::Tips => write!(f, "tips"),
            IndexationMethod::Uk => write!(f, "uk"),
            IndexationMethod::French => write!(f, "french"),
            IndexationMethod::Japanese => write!(f, "japanese"),
        }
    }
}

impl std::str::FromStr for IndexationMethod {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "canadian" => Ok(IndexationMethod::Canadian),
            "tips" => Ok(IndexationMethod::Tips),
            "uk" => Ok(IndexationMethod::Uk),
            "french" => Ok(IndexationMethod::French),
            "japanese" => Ok(IndexationMethod::Japanese),
            _ => Err(format!("Unknown indexation method: {s}")),
        }
    }
}

impl IndexationMethod {
    /// Get the standard lag for this indexation method.
    ///
    /// # UK Gilt Convention Notes
    ///
    /// For `IndexationMethod::Uk`, this returns the **legacy 8-month lag** which applies to
    /// UK Index-Linked Gilts issued **before September 2005**. Modern Gilts issued
    /// **on or after September 2005** use a 3-month lag with daily interpolation,
    /// consistent with international standards; set `lag` explicitly for those.
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
    /// - Build modern (3-month lag) gilts with [`InflationLinkedBond::builder`]
    pub fn standard_lag(&self) -> InflationLag {
        match self {
            IndexationMethod::Uk => InflationLag::Months(8), // Legacy UK Gilts (pre-Sep 2005)
            IndexationMethod::Canadian
            | IndexationMethod::Tips
            | IndexationMethod::French
            | IndexationMethod::Japanese => InflationLag::Months(3),
        }
    }
}

/// Deflation protection type
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
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
        match s {
            "none" => Ok(DeflationProtection::None),
            "maturity_only" => Ok(DeflationProtection::MaturityOnly),
            "all_payments" => Ok(DeflationProtection::AllPayments),
            _ => Err(format!("Unknown deflation protection: {s}")),
        }
    }
}

/// Inflation-Linked Bond instrument
#[derive(
    PartialEq,
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct InflationLinkedBond {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Notional amount (in real terms)
    pub notional: Money,
    /// Real coupon rate (as decimal)
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub real_coupon: Decimal,
    /// Coupon frequency
    pub frequency: Tenor,
    /// Day count convention
    pub day_count: DayCount,
    /// Issue date
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub issue_date: Date,
    /// Maturity date
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub maturity: Date,
    /// Base CPI/index value at issue
    pub base_index: f64,
    /// Base date for index (may differ from issue date)
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
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
    pub business_day_convention: BusinessDayConvention,
    /// Stub convention
    #[builder(default = StubKind::ShortFront)]
    #[serde(default = "crate::serde_defaults::stub_short_front")]
    pub stub: StubKind,
    /// Holiday calendar identifier
    pub calendar_id: Option<CalendarId>,
    /// Discount curve identifier. This **must** be a NOMINAL curve (e.g.
    /// "USD-OIS"): the cashflow schedule contains inflation-projected nominal
    /// amounts (real amount × projected index ratio), so discounting on a real
    /// curve would double-count inflation. The real curve is never used for PV;
    /// it enters only through real-yield style metrics computed from real flows.
    pub discount_curve_id: CurveId,
    /// Inflation index identifier
    pub inflation_index_id: CurveId,
    /// Quoted clean price (if available)
    pub quoted_clean: Option<f64>,
    /// Additional attributes
    #[builder(default)]
    /// Instrument-owned pricing inputs.
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl InflationLinkedBond {
    /// Validate the linker contract, schedule, and market-data identifiers.
    pub fn validate(&self) -> Result<()> {
        let context = format!("Inflation-linked bond '{}'", self.id.as_str());
        if !self.notional.amount().is_finite() || self.notional.amount() <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} notional must be positive and finite"
            )));
        }
        self.real_coupon
            .to_f64()
            .filter(|coupon| coupon.is_finite())
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "{context} real_coupon must be finite"
                ))
            })?;
        if self.issue_date >= self.maturity {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} issue_date must precede maturity"
            )));
        }
        if self.base_date > self.maturity {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} base_date cannot follow maturity"
            )));
        }
        if !self.base_index.is_finite() || self.base_index <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} base_index must be positive and finite"
            )));
        }
        if self
            .quoted_clean
            .is_some_and(|price| !price.is_finite() || price <= 0.0)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} quoted_clean must be positive and finite"
            )));
        }
        if self.discount_curve_id.as_str().trim().is_empty()
            || self.inflation_index_id.as_str().trim().is_empty()
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} curve and inflation-index identifiers cannot be empty"
            )));
        }
        if matches!(self.indexation_method, IndexationMethod::Uk)
            && !matches!(self.lag, InflationLag::Months(3) | InflationLag::Months(8))
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} UK linkers require a 3-month modern or 8-month legacy lag"
            )));
        }
        crate::cashflow::builder::calendar::resolve_calendar_strict(self.schedule_calendar_id())?;
        let periods = self.periods()?;
        if periods.is_empty() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} must generate at least one coupon period"
            )));
        }
        Ok(())
    }

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
    /// - **Discounting**: Nominal curve ("USD-OIS"); cashflows are
    ///   inflation-projected nominal amounts
    pub fn example() -> Self {
        use time::macros::date;
        Self {
            id: InstrumentId::new("TIPS-10Y"),
            notional: Money::from((1_000_000_i64, Currency::USD)),
            real_coupon: Decimal::new(25, 3),
            frequency: Tenor::semi_annual(),
            day_count: DayCount::ActActIsma, // US Treasury convention
            issue_date: date!(2024 - 01 - 15),
            maturity: date!(2034 - 01 - 15),
            base_index: 100.0,
            base_date: date!(2024 - 01 - 15),
            indexation_method: IndexationMethod::Tips,
            lag: IndexationMethod::Tips.standard_lag(),
            deflation_protection: DeflationProtection::MaturityOnly,
            business_day_convention: BusinessDayConvention::Unadjusted,
            stub: StubKind::None,
            calendar_id: None,
            discount_curve_id: CurveId::new("USD-OIS"),
            inflation_index_id: CurveId::new("US-CPI"),
            quoted_clean: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
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
            indexation_method: IndexationMethod::Tips,
            lag: IndexationMethod::Tips.standard_lag(),
            deflation_protection: DeflationProtection::MaturityOnly,
            business_day_convention: BusinessDayConvention::Following,
            stub: StubKind::None,
            calendar_id: None,
            discount_curve_id: discount_curve_id.into(),
            inflation_index_id: inflation_index_id.into(),
            quoted_clean: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        }
    }

    /// Create a **legacy** UK Index-Linked Gilt (pre-September 2005) using parameter structs.
    ///
    /// # ⚠️ Important: Legacy vs Modern UK Gilts
    ///
    /// This constructor creates a linker with the **8-month lag** convention used for
    /// UK Index-Linked Gilts issued **before September 2005**. For gilts issued on or
    /// after September 2005, build with a 3-month lag via [`Self::builder`].
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
    /// ```
    /// use finstack_quant_valuations::instruments::fixed_income::inflation_linked_bond::{
    ///     InflationLinkedBond, InflationLinkedBondParams,
    /// };
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::{DayCount, Tenor};
    /// use finstack_quant_core::money::Money;
    /// use rust_decimal::Decimal;
    /// use time::macros::date;
    ///
    /// let params = InflationLinkedBondParams {
    ///     notional: Money::from((1_000_000_i64, Currency::GBP)),
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
    ///     "GBP-NOMINAL",
    ///     "UK-RPI",
    /// );
    /// ```
    ///
    /// # Arguments
    ///
    /// * `id` - Trade identifier stored on the gilt and used in results and serialization.
    /// * `bond_params` - UK linker economics: notional, real coupon, frequency, day count,
    ///   issue, maturity, and base RPI.
    /// * `base_date` - Curve or model anchor date from which times are measured.
    /// * `discount_curve_id` - Identifier of the discount curve used for present-value calculations.
    /// * `inflation_index_id` - Identifier of the inflation index used for fixing lookup.
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
            indexation_method: IndexationMethod::Uk,
            lag: IndexationMethod::Uk.standard_lag(), // 8-month lag for legacy gilts
            deflation_protection: DeflationProtection::None,
            business_day_convention: BusinessDayConvention::Following,
            stub: StubKind::None,
            calendar_id: None,
            discount_curve_id: discount_curve_id.into(),
            inflation_index_id: inflation_index_id.into(),
            quoted_clean: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        }
    }

    /// Holiday calendar for schedule adjustment; weekends-only when unset.
    fn schedule_calendar_id(&self) -> &str {
        self.calendar_id
            .as_deref()
            .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID)
    }

    /// Coupon periods from issue to maturity under the bond's schedule
    /// conventions (unadjusted accrual dates, business-day adjusted payment
    /// dates, no payment lag).
    ///
    /// Shared by validation, the real schedule and the projected schedule so
    /// every view sees the same periods.
    fn periods(&self) -> Result<Vec<crate::cashflow::builder::periods::SchedulePeriod>> {
        crate::cashflow::builder::periods::build_periods(
            crate::cashflow::builder::periods::BuildPeriodsParams {
                start: self.issue_date,
                end: self.maturity,
                frequency: self.frequency,
                stub: self.stub,
                business_day_convention: self.business_day_convention,
                calendar_id: self.schedule_calendar_id(),
                end_of_month: false,
                day_count: self.day_count,
                payment_lag_days: 0,
                reset_lag_days: None,
                adjust_accrual_dates: false,
                roll_rule: crate::cashflow::builder::specs::RollRule::None,
            },
        )
    }

    /// Real coupon rate as a decimal (0.02 = 2%).
    fn real_coupon_rate(&self) -> Result<f64> {
        Ok(self
            .real_coupon
            .to_f64()
            .ok_or(finstack_quant_core::InputError::ConversionOverflow)?)
    }

    /// Business-day-adjusted maturity on which principal is paid.
    ///
    /// Shared by the projected (PV) schedule and the real-yield schedule so
    /// both pay principal on the same date as the final coupon.
    fn principal_payment_date(&self) -> Result<Date> {
        crate::cashflow::builder::calendar::adjust_date(
            self.maturity,
            self.business_day_convention,
            self.schedule_calendar_id(),
        )
    }

    /// Real (unindexed) cashflow schedule: one coupon per period carrying its
    /// contractual accrual boundaries, plus principal at the adjusted maturity.
    ///
    /// The coupon's ACT/ACT ICMA reference period is its own accrual period.
    fn real_cashflow_schedule(&self) -> Result<crate::cashflow::builder::CashFlowSchedule> {
        use crate::cashflow::primitives::{CFKind, CashFlow, CashFlowAccrual};

        let coupon_rate = self.real_coupon_rate()?;
        let currency = self.notional.currency();
        let periods = if self.issue_date < self.maturity {
            self.periods()?
        } else {
            Vec::new()
        };
        let mut flows = Vec::with_capacity(periods.len() + 1);
        for period in &periods {
            let accrual_factor = period.accrual_year_fraction.max(0.0);
            flows.push(
                CashFlow::new(
                    period.payment_date,
                    None,
                    Money::new(
                        self.notional.amount() * coupon_rate * accrual_factor,
                        currency,
                    )?,
                    CFKind::Fixed,
                    accrual_factor,
                    Some(coupon_rate),
                )
                .with_accrual(CashFlowAccrual {
                    start: period.accrual_start,
                    end: period.accrual_end,
                    day_count: self.day_count,
                    projected_index_rate: None,
                    calendar_id: None,
                    coupon_period: Some((period.accrual_start, period.accrual_end)),
                    end_is_termination_date: false,
                }),
            );
        }
        if !periods.is_empty() {
            flows.push(CashFlow::new(
                self.principal_payment_date()?,
                None,
                self.notional,
                CFKind::Notional,
                0.0,
                None,
            ));
        }
        Ok(crate::cashflow::traits::schedule_from_classified_flows(
            flows,
            self.day_count,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: Some(self.notional),
                meta: crate::cashflow::builder::CashFlowMeta {
                    issue_date: Some(self.issue_date),
                    ..Default::default()
                },
            },
        ))
    }

    /// Real accrued interest at `as_of` on the real schedule, from the shared
    /// schedule accrual engine (linear accrual; ACT/ACT ICMA uses the bond's
    /// coupon frequency).
    fn real_accrued_from(
        &self,
        schedule: &crate::cashflow::builder::CashFlowSchedule,
        as_of: Date,
    ) -> Result<f64> {
        crate::cashflow::accrual::AccrualIndex::build(
            schedule,
            &crate::cashflow::accrual::AccrualConfig {
                frequency: Some(self.frequency),
                ..Default::default()
            },
        )?
        .accrued_at(as_of)
    }

    /// Real flows paid on or after `as_of` (earlier flows are settled).
    fn real_flows_from(
        schedule: &crate::cashflow::builder::CashFlowSchedule,
        as_of: Date,
    ) -> DatedFlows {
        schedule
            .get_flows()
            .iter()
            .filter(|cf| cf.date >= as_of)
            .map(|cf| (cf.date, cf.amount))
            .collect()
    }

    /// Build unadjusted real cashflow schedule (no inflation indexation).
    ///
    /// Cashflows with `payment_date < as_of` are excluded (already settled).
    /// The principal payment date is business-day adjusted via the bond's BDC.
    pub(crate) fn build_real_schedule(&self, as_of: Date) -> Result<DatedFlows> {
        Ok(Self::real_flows_from(
            &self.real_cashflow_schedule()?,
            as_of,
        ))
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
    pub fn real_yield(&self, clean_price: f64, as_of: Date) -> Result<f64> {
        use crate::instruments::fixed_income::bond::pricing::quote_conversions::YieldCompounding;
        use crate::instruments::fixed_income::bond::pricing::ytm_solver::{
            solve_ytm, YtmPricingSpec,
        };

        if !clean_price.is_finite() || clean_price <= 0.0 {
            return Err(finstack_quant_core::InputError::Invalid.into());
        }

        // 1. Build real cashflows (unadjusted for inflation)
        let schedule = self.real_cashflow_schedule()?;
        let flows = Self::real_flows_from(&schedule, as_of);
        if flows.is_empty() {
            return Err(finstack_quant_core::InputError::TooFewPoints.into());
        }

        // 2. Calculate Real Accrued Interest
        // Needed to convert Clean Real Price -> Dirty Real Price
        let real_accrued = self.real_accrued_from(&schedule, as_of)?;

        // 3. Calculate Target Dirty Real Price
        // Price is per 100 notional.
        let target_dirty_price_val = (clean_price / 100.0 * self.notional.amount()) + real_accrued;
        let target_price = Money::new(target_dirty_price_val, self.notional.currency())?;

        let spec = YtmPricingSpec {
            day_count: self.day_count,
            notional: self.notional,
            coupon_rate: self.real_coupon_rate()?,
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
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        use crate::instruments::fixed_income::bond::pricing::quote_conversions::periods_per_year;
        use finstack_quant_core::math::Compounding;
        use std::num::NonZeroU32;

        let clean_price = self.quoted_clean.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "Breakeven inflation requires a quoted clean price. \
                 Set quoted_clean on the bond or pass price explicitly."
                    .to_string(),
            )
        })?;
        let real_yield_street = self.real_yield(clean_price, as_of)?;

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
            return Err(finstack_quant_core::InputError::NonPositiveValue.into());
        }
        Ok((1.0 + nominal_bond_yield) / denominator - 1.0)
    }

    /// Calculate inflation-adjusted duration (Real Duration)
    ///
    /// Computes the modified duration of the bond based on its real (unadjusted)
    /// cashflows. This measures sensitivity to changes in real yield.
    pub fn real_duration(&self, as_of: Date) -> Result<f64> {
        use crate::instruments::fixed_income::bond::pricing::quote_conversions::{
            price_from_ytm_compounded_params, YieldCompounding,
        };

        // Determine a base clean price to center the bump around
        let base_clean = self.quoted_clean.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "Real duration requires quoted_clean, the same clean price per 100 used for real yield".into(),
            )
        })?;
        let y0 = self.real_yield(base_clean, as_of)?;
        // Bump yield by 1bp in decimal terms
        let bp = 1e-4;

        // Use real schedule to calculate sensitivity to real yield (Real Duration)
        // This assumes the "Duration" metric refers to the duration of the real bond component.
        let flows = self.build_real_schedule(as_of)?;

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

    fn validate_invariants(&self) -> Result<()> {
        self.validate()
    }

    fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
        let mut deps = MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        deps.add_inflation_curve(self.inflation_index_id.clone());
        Ok(deps)
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        if as_of > self.maturity {
            return Ok(finstack_quant_core::money::Money::from((
                0_i64,
                self.notional.currency(),
            )));
        }
        crate::instruments::common_impl::helpers::schedule_pv(
            self,
            curves,
            as_of,
            &self.discount_curve_id,
        )
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.maturity)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.issue_date)
    }

    crate::impl_focused_pricing_overrides!();
}

impl finstack_quant_cashflows::CashflowScheduleSource for InflationLinkedBond {
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        Ok(Some(self.notional))
    }

    fn raw_cashflow_schedule(
        &self,
        curves: &MarketContext,
        _as_of: Date,
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
        let periods = self.periods()?;
        let coupon_rate = self.real_coupon_rate()?;

        let mut detailed_flows = Vec::with_capacity(periods.len() + 1);
        let mut coupon_rows = Vec::with_capacity(periods.len());
        for period in &periods {
            let accrual_factor = period.accrual_year_fraction.max(0.0);
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
        )?;

        let principal_date = self.principal_payment_date()?;
        let raw_principal_ratio = inflation_source.ratio(self, principal_date)?;
        let principal_ratio = match self.deflation_protection {
            DeflationProtection::None => raw_principal_ratio,
            DeflationProtection::MaturityOnly | DeflationProtection::AllPayments => {
                raw_principal_ratio.max(1.0)
            }
        };
        detailed_flows.push(crate::cashflow::primitives::CashFlow::new(
            principal_date,
            None,
            self.notional * principal_ratio,
            crate::cashflow::primitives::CFKind::Notional,
            0.0,
            None,
        ));

        let schedule = crate::cashflow::traits::schedule_from_classified_flows(
            detailed_flows,
            self.day_count,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: Some(self.notional),
                ..Default::default()
            },
        );
        Ok(schedule
            .with_representation(crate::cashflow::builder::CashflowRepresentation::Projected))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::traits::CashflowProvider;
    use finstack_quant_core::cashflow::CFKind;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::scalars::{InflationIndex, InflationInterpolation};
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, InflationCurve};
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

        // Semi-annual ILB with a 4% real coupon, priced at par (100).
        // At par the real YTM equals the coupon: 4 % Street (semi-annual).
        let mut bond = InflationLinkedBond {
            id: InstrumentId::new("ILB-C8"),
            notional: Money::from((1_000_000_i64, Currency::USD)),
            real_coupon: Decimal::try_from(0.04).expect("valid coupon"),
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Thirty360,
            issue_date: as_of,
            maturity,
            base_index: 100.0,
            base_date: as_of,
            indexation_method: IndexationMethod::Tips,
            lag: InflationLag::None,
            deflation_protection: DeflationProtection::MaturityOnly,
            business_day_convention: BusinessDayConvention::Following,
            stub: StubKind::None,
            calendar_id: None,
            discount_curve_id: CurveId::new("USD-NOM"),
            inflation_index_id: CurveId::new("US-CPI"),
            quoted_clean: Some(100.0), // par → real yield == coupon (4 % semi-annual)
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
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
            .breakeven_inflation(nominal_annual, as_of)
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
        let result_annual_frequency = bond
            .breakeven_inflation(nominal_annual, as_of)
            .expect("annual-frequency breakeven");
        // For annual frequency Street ≡ Annual, so the correction is zero;
        // result should be within 0.5 bp of the "wrong" annual calculation.
        assert!(
            (result_annual_frequency - wrong_breakeven).abs() < tol,
            "annual-frequency bond should not be corrected: got {result_annual_frequency:.6}, wrong={wrong_breakeven:.6}"
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

        let accrued = real_accrued(&bond, as_of)
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
            notional: Money::from((1_000_000_i64, Currency::USD)),
            real_coupon: Decimal::try_from(0.04).expect("valid coupon"),
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Thirty360,
            issue_date: issue,
            maturity,
            base_index: 100.0,
            base_date: issue,
            indexation_method: IndexationMethod::Tips,
            lag: InflationLag::None,
            deflation_protection: DeflationProtection::MaturityOnly,
            business_day_convention: BusinessDayConvention::Following,
            stub: StubKind::None,
            calendar_id: None,
            discount_curve_id: CurveId::new("USD-OIS"),
            inflation_index_id: CurveId::new("US-CPI"),
            quoted_clean: Some(100.0),
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        };
        let duration = bond.real_duration(as_of).expect("real duration");
        let y0 = bond.real_yield(100.0, as_of).expect("real yield");
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
        let discount = DiscountCurve::builder("USD-OIS")
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
            notional: Money::from((1_000_000_i64, Currency::USD)),
            real_coupon: Decimal::try_from(0.02).expect("valid coupon"),
            frequency: Tenor::annual(),
            day_count: DayCount::Thirty360,
            issue_date: d(2024, Month::January, 15),
            maturity: d(2026, Month::January, 15),
            base_index: 100.0,
            base_date: d(2024, Month::January, 15),
            indexation_method: IndexationMethod::Tips,
            lag: InflationLag::None,
            deflation_protection,
            business_day_convention: BusinessDayConvention::Following,
            stub: StubKind::None,
            calendar_id: None,
            discount_curve_id: CurveId::new("USD-OIS"),
            inflation_index_id: CurveId::new("US-CPI"),
            quoted_clean: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        }
    }

    fn real_accrued(bond: &InflationLinkedBond, as_of: Date) -> Result<f64> {
        bond.real_accrued_from(&bond.real_cashflow_schedule()?, as_of)
    }

    #[test]
    fn accrued_real_interest_matches_hand_calculation() {
        // 30/360 annual 2% on 1,000,000: coupon 20,000; 2024-01-15 → 2024-07-15
        // is 180/360 of the period, so accrued = 10,000.
        let bond = sample_bond(DeflationProtection::None);
        let accrued = real_accrued(&bond, d(2024, Month::July, 15)).expect("accrued");
        assert!(
            (accrued - 10_000.0).abs() < 1e-6,
            "30/360 accrued {accrued}"
        );

        // ACT/ACT ICMA semi-annual 2%: coupon 10,000 over 2024-01-15 → 07-15
        // (182 days); 2024-03-01 is 46 days in, so 10,000 × 46/182.
        let mut icma = sample_bond(DeflationProtection::None);
        icma.frequency = Tenor::semi_annual();
        icma.day_count = DayCount::ActActIsma;
        icma.maturity = d(2029, Month::January, 15);
        let accrued = real_accrued(&icma, d(2024, Month::March, 1)).expect("accrued");
        let expected = 10_000.0 * 46.0 / 182.0;
        assert!(
            (accrued - expected).abs() < 1e-6,
            "ICMA accrued {accrued} vs {expected}"
        );

        // Before issue and on/after maturity nothing accrues.
        assert_eq!(
            real_accrued(&icma, d(2023, Month::December, 1)).expect("accrued"),
            0.0
        );
        assert_eq!(
            real_accrued(&icma, d(2029, Month::January, 15)).expect("accrued"),
            0.0
        );
    }

    #[test]
    fn hybrid_ref_cpi_resolves_published_and_projected_anchors_independently() {
        use finstack_quant_core::market_data::scalars::{InflationIndex, InflationInterpolation};

        let mut bond = sample_bond(DeflationProtection::None);
        bond.lag = InflationLag::Months(3);
        let index = InflationIndex::new(
            "US-CPI",
            vec![
                (d(2025, Month::January, 1), 100.0),
                (d(2025, Month::February, 1), 105.0),
                (d(2025, Month::March, 1), 110.0),
            ],
            Currency::USD,
        )
        .expect("published CPI")
        .with_interpolation(InflationInterpolation::Linear);
        let curve = InflationCurve::builder("US-CPI")
            .base_date(d(2025, Month::March, 1))
            .base_cpi(110.0)
            .knots([(0.0, 110.0), (31.0 / 365.0, 120.0)])
            .build()
            .expect("projected CPI");
        let market = MarketContext::new()
            .insert(curve)
            .insert_inflation_index("US-CPI", index);

        let ratio = bond
            .index_ratio_from_market(d(2025, Month::June, 15), &market)
            .expect("hybrid RefCPI");
        let expected = (110.0 + (14.0 / 30.0) * (120.0 - 110.0)) / bond.base_index;
        assert!((ratio - expected).abs() < 1e-12);
    }

    /// Principal on a weekend maturity (Sunday 2034-01-15) is paid on the
    /// Following business day, 2034-01-16,
    /// in both the PV schedule and the real-yield schedule, together with the
    /// final coupon.
    #[test]
    fn principal_paid_on_adjusted_maturity_in_pv_and_real_schedules() {
        let as_of = d(2024, Month::January, 15);
        let mut bond = sample_bond(DeflationProtection::None);
        bond.issue_date = d(2024, Month::January, 15);
        bond.maturity = d(2034, Month::January, 15); // Sunday
        let inflation = InflationCurve::builder("US-CPI")
            .base_date(as_of)
            .base_cpi(100.0)
            .knots([(0.0, 100.0), (12.0, 100.0)])
            .build()
            .expect("inflation curve");
        let market = MarketContext::new().insert(inflation);

        let adjusted = d(2034, Month::January, 16);
        let schedule = bond.cashflow_schedule(&market, as_of).expect("pv schedule");
        let principal_dates: Vec<Date> = schedule
            .get_flows()
            .iter()
            .filter(|cf| cf.kind == CFKind::Notional)
            .map(|cf| cf.date)
            .collect();
        assert_eq!(principal_dates, vec![adjusted]);
        assert_ne!(principal_dates[0], bond.maturity);

        let real = bond.build_real_schedule(as_of).expect("real schedule");
        assert_eq!(real.last().map(|(date, _)| *date), Some(adjusted));
    }

    #[test]
    fn maturity_only_deflation_floor_applies_only_to_principal() {
        let as_of = d(2024, Month::January, 15);
        let market = market_with_deflation(as_of);
        let bond = sample_bond(DeflationProtection::MaturityOnly);

        let schedule = bond
            .cashflow_schedule(&market, as_of)
            .expect("schedule should build");
        assert!(schedule.get_flows().len() >= 3);

        let first_coupon = schedule
            .get_flows()
            .iter()
            .find(|flow| flow.kind == CFKind::InflationCoupon)
            .expect("coupon flow")
            .amount
            .amount();
        let principal = schedule
            .get_flows()
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
        assert!(schedule.get_flows().len() >= 3);

        let first_coupon = schedule
            .get_flows()
            .iter()
            .find(|flow| flow.kind == CFKind::InflationCoupon)
            .expect("coupon flow")
            .amount
            .amount();
        let principal = schedule
            .get_flows()
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
                .get_flows()
                .iter()
                .any(|flow| flow.kind == CFKind::InflationCoupon),
            "expected inflation-linked coupons in full schedule"
        );
        assert!(
            schedule
                .get_flows()
                .iter()
                .any(|flow| flow.date == bond.maturity && flow.kind == CFKind::Notional),
            "expected principal notional flow at maturity"
        );
    }

    /// UK legacy 8-month step lag reads the whole-month print: for a date of
    /// 2025-06-15 the reference month is October 2024, i.e. the curve at
    /// 2024-10-01 = 100 (a knot), giving ratio 100/100 = 1.0. Reading the
    /// curve on 2024-10-15 would interpolate toward the November value 110.
    #[test]
    fn uk_step_lag_reads_first_of_month_from_curve_and_hybrid() {
        use finstack_quant_core::market_data::scalars::InflationIndex;

        let mut bond = sample_bond(DeflationProtection::None);
        bond.indexation_method = IndexationMethod::Uk;
        bond.lag = InflationLag::Months(8);
        let curve = InflationCurve::builder("US-CPI")
            .base_date(d(2024, Month::October, 1))
            .base_cpi(100.0)
            .knots([(0.0, 100.0), (31.0 / 365.0, 110.0)])
            .build()
            .expect("curve");
        let date = d(2025, Month::June, 15);

        let ratio = bond
            .index_ratio_from_curve(date, &curve)
            .expect("curve ratio");
        assert!((ratio - 1.0).abs() < 1e-12, "curve ratio {ratio}");
        let mid_month = curve
            .cpi_on_date(d(2024, Month::October, 15))
            .expect("mid-month cpi");
        assert!((mid_month - 100.0).abs() > 1.0, "fixture must discriminate");

        // Hybrid: prints published through September 2024, so October comes
        // from the projection curve on its first-of-month anchor.
        let index = InflationIndex::new(
            "US-CPI",
            vec![
                (d(2024, Month::August, 1), 98.0),
                (d(2024, Month::September, 1), 99.0),
            ],
            Currency::USD,
        )
        .expect("published CPI");
        let market = MarketContext::new()
            .insert(curve)
            .insert_inflation_index("US-CPI", index);
        let hybrid = bond
            .index_ratio_from_market(date, &market)
            .expect("hybrid ratio");
        assert!((hybrid - 1.0).abs() < 1e-12, "hybrid ratio {hybrid}");
    }

    /// Regression test for the UK gilt lag validation that was previously
    /// `debug_assert!`-only (a release-build silent miscalc). A non-standard
    /// lag (anything other than 3 or 8 months) must now fail explicitly when
    /// `index_ratio` is called.
    #[test]
    fn uk_gilt_non_standard_lag_returns_err() {
        let as_of = d(2024, Month::January, 15);
        let mut bond = sample_bond(DeflationProtection::AllPayments);
        bond.indexation_method = IndexationMethod::Uk;
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
        bond.indexation_method = IndexationMethod::Uk;
        bond.lag = InflationLag::Months(3); // modern UK standard

        // Use a Linear-interpolated index to satisfy the modern UK invariant.
        let index = InflationIndex::new(
            "UK-RPI",
            vec![
                (d(2023, Month::October, 1), 100.0),
                (d(2023, Month::November, 1), 102.0),
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
