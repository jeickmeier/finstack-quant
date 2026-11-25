//! Inflation-Linked Bond (ILB) types and implementation.

use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DateExt, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::InflationLag;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::CalendarId;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations_core::instruments::MarketDependencies;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use finstack_quant_valuations_core::impl_instrument_base;

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
    /// - Build legacy gilts with `lag = InflationLag::months(8)` and modern
    ///   gilts with `lag = InflationLag::months(3)` via
    ///   [`InflationLinkedBond::builder`]
    pub fn standard_lag(&self) -> InflationLag {
        match self {
            IndexationMethod::Uk => InflationLag::months(8), // Legacy UK Gilts (pre-Sep 2005)
            IndexationMethod::Canadian
            | IndexationMethod::Tips
            | IndexationMethod::French
            | IndexationMethod::Japanese => InflationLag::months(3),
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
#[serde(deny_unknown_fields, remote = "Self")]
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
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub issue_date: Date,
    /// Maturity date
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub maturity: Date,
    /// Base CPI/index value at issue
    pub base_cpi: f64,
    /// Base date for index (may differ from issue date)
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
    /// Black lognormal volatility of the contractual reference-CPI/base-CPI ratio.
    /// The surface uses expiry in Act/365F years to the last required publication
    /// and a Strike axis in ratio units (the par floor strikes at 1.0). The
    /// supplied forward ratio and volatility are under the payment-date measure.
    /// Required for a protected payment whose reference CPI is not yet known;
    /// an explicitly supplied zero-volatility surface selects deterministic pricing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor_vol_surface_id: Option<CurveId>,
    /// Business day convention
    #[builder(default = BusinessDayConvention::ModifiedFollowing)]
    #[serde(default = "finstack_quant_valuations_core::serde_defaults::bdc_modified_following")]
    pub business_day_convention: BusinessDayConvention,
    /// Stub convention
    #[builder(default = StubKind::ShortFront)]
    #[serde(default = "finstack_quant_valuations_core::serde_defaults::stub_short_front")]
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
    /// Instrument-owned pricing inputs.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::ScenarioPricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

finstack_quant_valuations_core::validate_on_deserialize!(InflationLinkedBond);

impl InflationLinkedBond {
    /// Quoted clean price per 100 of real face from
    /// `instrument_pricing_overrides.market_quotes.quoted_clean_price_pct`.
    ///
    /// # Arguments
    ///
    /// * `purpose` - Name of the calculation needing the quote, used in the error.
    pub(crate) fn quoted_clean_price_pct(&self, purpose: &str) -> Result<f64> {
        self.instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price_pct
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "{purpose} requires the clean price per 100 of real face in \
                     instrument_pricing_overrides.market_quotes.quoted_clean_price_pct"
                ))
            })
    }

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
        if !self.base_cpi.is_finite() || self.base_cpi <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} base_cpi must be positive and finite"
            )));
        }
        if self
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price_pct
            .is_some_and(|price| !price.is_finite() || price <= 0.0)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} instrument_pricing_overrides.market_quotes.quoted_clean_price_pct \
                 must be positive and finite"
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
            && !matches!(self.lag, InflationLag::Months(months) if matches!(months.get(), 3 | 8))
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} UK linkers require a 3-month modern or 8-month legacy lag"
            )));
        }
        finstack_quant_core::dates::calendar_by_id_strict(self.schedule_calendar_id())?;
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
    pub fn example() -> finstack_quant_core::Result<Self> {
        use time::macros::date;
        Ok(Self {
            id: InstrumentId::new("TIPS-10Y"),
            notional: Money::from((1_000_000_i64, Currency::USD)),
            real_coupon: Decimal::new(25, 3),
            frequency: Tenor::semi_annual(),
            day_count: DayCount::ActActIsma, // US Treasury convention
            issue_date: date!(2024 - 01 - 15),
            maturity: date!(2034 - 01 - 15),
            base_cpi: 100.0,
            base_date: date!(2024 - 01 - 15),
            indexation_method: IndexationMethod::Tips,
            lag: IndexationMethod::Tips.standard_lag(),
            deflation_protection: DeflationProtection::MaturityOnly,
            floor_vol_surface_id: None,
            business_day_convention: BusinessDayConvention::Unadjusted,
            stub: StubKind::None,
            calendar_id: None,
            discount_curve_id: CurveId::new("USD-OIS"),
            inflation_index_id: CurveId::new("US-CPI"),
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        })
    }

    /// Holiday calendar for schedule adjustment; weekends-only when unset.
    fn schedule_calendar_id(&self) -> &str {
        self.calendar_id
            .as_deref()
            .unwrap_or(finstack_quant_core::dates::WEEKENDS_ONLY_ID)
    }

    /// Coupon periods from issue to maturity under the bond's schedule
    /// conventions (unadjusted accrual dates, business-day adjusted payment
    /// dates, no payment lag).
    ///
    /// Shared by validation, the real schedule and the projected schedule so
    /// every view sees the same periods.
    fn periods(&self) -> Result<Vec<finstack_quant_cashflows::builder::SchedulePeriod>> {
        finstack_quant_cashflows::builder::build_periods(
            finstack_quant_cashflows::builder::BuildPeriodsParams {
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
                roll_rule: finstack_quant_cashflows::builder::RollRule::None,
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
        let cal = finstack_quant_core::dates::calendar_by_id_strict(self.schedule_calendar_id())?;
        finstack_quant_core::dates::adjust(self.maturity, self.business_day_convention, cal)
    }

    /// Real (unindexed) cashflow schedule: one coupon per period carrying its
    /// contractual accrual boundaries, plus principal at the adjusted maturity.
    ///
    /// The coupon's ACT/ACT ICMA reference period is its own accrual period.
    pub(super) fn real_cashflow_schedule(
        &self,
    ) -> Result<finstack_quant_cashflows::builder::CashFlowSchedule> {
        use finstack_quant_core::cashflow::{CFKind, CashFlowAccrual, CashFlowParams};

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
            let coupon_period = if self.day_count == DayCount::ActActIsma {
                if matches!(self.stub, StubKind::ShortFront | StubKind::LongFront) {
                    finstack_quant_cashflows::builder::icma_reference_period(
                        self.day_count,
                        self.frequency,
                        [period.unadjusted_end],
                        period.unadjusted_start,
                    )
                } else {
                    self.frequency
                        .months()
                        .and_then(|m| i32::try_from(m).ok())
                        .and_then(|m| period.unadjusted_start.add_months(m).ok())
                        .map(|nom_end| (period.unadjusted_start, nom_end))
                }
            } else {
                Some((period.accrual_start, period.accrual_end))
            };
            flows.push(
                CashFlowParams::new(
                    period.payment_date,
                    None,
                    Money::from_model_value(
                        self.notional.amount() * coupon_rate * accrual_factor,
                        currency,
                    )?,
                    CFKind::Fixed,
                    accrual_factor,
                    Some(coupon_rate),
                )
                .with_accrual(CashFlowAccrual::new(
                    period.accrual_start,
                    period.accrual_end,
                    self.day_count,
                    None,
                    finstack_quant_core::dates::DayCountContextState::try_new(
                        None,
                        (self.day_count == DayCount::ActActIsma).then_some(self.frequency),
                        None,
                        coupon_period,
                        Some(period.accrual_end == self.maturity),
                    )?,
                )?)
                .build()?,
            );
        }
        if !periods.is_empty() {
            flows.push({
                // The par change equals the cash (record CORE-CONVENTIONS-015 #4).
                let cash = self.notional;
                CashFlowParams::new(
                    self.principal_payment_date()?,
                    None,
                    cash,
                    CFKind::Notional,
                    0.0,
                    None,
                )
                .with_principal_delta(cash.checked_neg())
                .with_principal_date(self.principal_payment_date()?)
                .build()?
            });
        }
        Ok(finstack_quant_cashflows::schedule_from_classified_flows(
            flows,
            self.day_count,
            finstack_quant_cashflows::ScheduleBuildOpts {
                notional_hint: Some(self.notional),
                meta: finstack_quant_cashflows::builder::CashFlowMeta {
                    issue_date: Some(self.issue_date),
                    ..Default::default()
                },
            },
        ))
    }

    /// Real accrued interest at `as_of` on the real schedule, from the shared
    /// schedule accrual engine (linear accrual; ACT/ACT ICMA uses the bond's
    /// coupon frequency).
    pub(super) fn real_accrued_from(
        &self,
        schedule: &finstack_quant_cashflows::builder::CashFlowSchedule,
        as_of: Date,
    ) -> Result<f64> {
        Ok(finstack_quant_cashflows::accrual::AccrualIndex::build(
            schedule,
            &finstack_quant_cashflows::accrual::AccrualConfig {
                frequency: Some(self.frequency),
                ..Default::default()
            },
        )?
        .accrued_at(as_of)?
        .amount())
    }

    /// Real flows paid on or after `as_of` (earlier flows are settled).
    fn real_flows_from(
        schedule: &finstack_quant_cashflows::builder::CashFlowSchedule,
        as_of: Date,
    ) -> Vec<(Date, Money)> {
        schedule
            .get_flows()
            .iter()
            .filter(|cf| cf.get_date() >= as_of)
            .map(|cf| (cf.get_date(), cf.get_amount()))
            .collect()
    }

    /// Contractual yield times retaining the real schedule's coupon metadata.
    pub(super) fn real_flow_times(
        &self,
        schedule: &finstack_quant_cashflows::builder::CashFlowSchedule,
        as_of: Date,
    ) -> Result<Vec<(Date, f64, Money)>> {
        use crate::bond::pricing::quote_conversions::{act365l_flow_times, flow_times};
        let flows = Self::real_flows_from(schedule, as_of);
        if self.day_count != DayCount::Act365L {
            return flow_times(self.day_count, self.frequency, &flows, as_of);
        }
        let coupons = schedule
            .get_flows()
            .iter()
            .filter(|flow| flow.get_kind().is_coupon())
            .map(|flow| {
                let (start, end) = flow
                    .get_accrual()
                    .as_ref()
                    .and_then(|accrual| accrual.get_context().get_coupon_period())
                    .ok_or_else(|| {
                        finstack_quant_core::Error::Validation(
                            "ACT/365L real yield requires full contractual coupon metadata".into(),
                        )
                    })?;
                Ok((flow.get_date(), start, end))
            })
            .collect::<Result<Vec<_>>>()?;
        act365l_flow_times(&coupons, &flows, as_of)
    }

    /// Calculate real yield (yield in real terms, before inflation)
    ///
    /// Computes the internal rate of return of the **unadjusted (real) cashflows**
    /// against the **real price** (clean price + real accrued interest).
    ///
    /// This is the standard "Real Yield" quoted for TIPS and other linkers.
    /// ACT/365L timing retains the real schedule's full coupon denominators;
    /// payment adjustment does not extend the contractual quoted-yield clock.
    ///
    /// # Arguments
    ///
    /// * `clean_price` - Unindexed real clean price as a percentage of par (100 = par),
    ///   finite and strictly positive; real accrued interest is added for the yield solve.
    /// * `as_of` - Settlement date for real accrued interest and remaining contractual yield times.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The clean price is non-positive or non-finite
    /// - There are no cashflows remaining
    /// - The YTM solver fails to converge
    /// - ACT/365L settlement is after the contractual end of an unpaid coupon
    pub fn real_yield(&self, clean_price: f64, as_of: Date) -> Result<f64> {
        use crate::bond::pricing::quote_conversions::YieldCompounding;
        use crate::bond::pricing::ytm_solver::{YtmPricingSpec, solve_ytm_timed};

        if !clean_price.is_finite() || clean_price <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "clean price must be finite and positive, got {clean_price}"
            )));
        }

        // 1. Build real cashflows (unadjusted for inflation)
        let schedule = self.real_cashflow_schedule()?;
        let flows = self.real_flow_times(&schedule, as_of)?;
        if flows.is_empty() {
            return Err(finstack_quant_core::InputError::TooFewPoints.into());
        }

        // 2. Calculate Real Accrued Interest
        // Needed to convert Clean Real Price -> Dirty Real Price
        let real_accrued = self.real_accrued_from(&schedule, as_of)?;

        // 3. Calculate Target Dirty Real Price
        // Price is per 100 notional.
        let target_dirty_price_val = (clean_price / 100.0 * self.notional.amount()) + real_accrued;
        let target_price =
            Money::from_model_value(target_dirty_price_val, self.notional.currency())?;

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
        solve_ytm_timed(&flows, target_price, spec)
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
    ///   `disc_curve.zero_rate(t, Compounding::Periodic(std::num::NonZeroU32::MIN))`.
    ///
    /// This is more accurate than the simplified approximation (`nominal - real`)
    /// at higher inflation levels where the cross-term becomes significant.
    pub fn breakeven_inflation(
        &self,
        nominal_bond_yield: f64,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        use finstack_quant_core::math::Compounding;
        use std::num::NonZeroU32;

        let clean_price = self.quoted_clean_price_pct("Breakeven inflation")?;
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
        let ref_period = finstack_quant_cashflows::builder::icma_reference_period(
            self.day_count,
            self.frequency,
            [self.maturity],
            as_of,
        );
        let t = finstack_quant_cashflows::builder::icma_year_fraction(
            self.day_count,
            Some(self.frequency),
            ref_period,
            as_of,
            self.maturity,
        )
        .unwrap_or(1.0)
        .max(1e-6); // guard against zero / expired bond

        let n_f = self.frequency.payments_per_year()?.max(1.0);
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
            .unwrap_or(Compounding::Periodic(std::num::NonZeroU32::MIN));
        let real_yield_annual = real_compounding.convert_rate(
            real_yield_street,
            t,
            &Compounding::Periodic(std::num::NonZeroU32::MIN),
        )?;

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
    /// cashflows by a central difference in the real yield. This measures
    /// sensitivity to changes in real yield.
    ///
    /// # Arguments
    ///
    /// * `as_of` - Valuation date; the real yield is solved from
    ///   `instrument_pricing_overrides.market_quotes.quoted_clean_price_pct` at
    ///   this date.
    /// * `ytm_bump_bp` - Real-yield shock in basis points (1.0 = 1bp) used for
    ///   the central difference; must be finite and positive. The
    ///   `RealDuration` metric passes
    ///   `metric_pricing_overrides.bump_config.ytm_bump_bp`, defaulting to 1bp.
    pub fn real_duration(&self, as_of: Date, ytm_bump_bp: f64) -> Result<f64> {
        use crate::bond::pricing::quote_conversions::{YieldCompounding, price_from_ytm_timed};

        // Determine a base clean price to center the bump around
        let base_clean = self.quoted_clean_price_pct("Real duration")?;
        if !ytm_bump_bp.is_finite() || ytm_bump_bp <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "real duration ytm_bump_bp must be finite and positive, got {ytm_bump_bp}"
            )));
        }
        let y0 = self.real_yield(base_clean, as_of)?;
        // Yield shock in decimal terms.
        let bp = ytm_bump_bp / 10_000.0;

        // Use real schedule to calculate sensitivity to real yield (Real Duration)
        // This assumes the "Duration" metric refers to the duration of the real bond component.
        let flows = self.real_flow_times(&self.real_cashflow_schedule()?, as_of)?;

        let price_from_yield = |y: f64| -> Result<f64> {
            let price = price_from_ytm_timed(&flows, self.frequency, y, YieldCompounding::Street)?;
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
impl finstack_quant_valuations_core::instruments::Instrument for InflationLinkedBond {
    fn cashflow_export_terms(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<
        finstack_quant_valuations_core::instruments::cashflow_export::CashflowExportTerms,
    > {
        let mut terms = finstack_quant_valuations_core::instruments::cashflow_export::CashflowExportTerms::from_schedule(
            finstack_quant_cashflows::CashflowProvider::cashflow_schedule(self, market, as_of)?,
        );
        let mut contractual_by_payment: finstack_quant_core::FxHashMap<Date, Date> = self
            .periods()?
            .into_iter()
            .map(|period| (period.payment_date, period.unadjusted_end))
            .collect();
        contractual_by_payment.insert(self.principal_payment_date()?, self.maturity);

        // A ratio the market cannot supply exports as an empty cell.
        let ratios = terms
            .schedule
            .get_flows()
            .iter()
            .filter_map(|flow| {
                let reference_date = contractual_by_payment
                    .get(&flow.get_date())
                    .copied()
                    .unwrap_or_else(|| flow.get_date());
                match self.index_ratio_from_market(reference_date, market, as_of) {
                    Ok(ratio) => Some((flow.get_date(), ratio)),
                    Err(err) => {
                        tracing::warn!(
                            instrument_id = %self.id.as_str(),
                            date = %flow.get_date(),
                            %err,
                            "cashflow export: inflation index ratio lookup failed; \
                             exporting an empty index_ratio cell"
                        );
                        None
                    }
                }
            })
            .collect();
        terms.index_ratios = Some(ratios);
        Ok(terms)
    }

    impl_instrument_base!(
        finstack_quant_valuations_core::pricer::InstrumentType::InflationLinkedBond,
        crate::FIXED_INCOME
    );

    fn validate_invariants(&self) -> Result<()> {
        self.validate()
    }

    fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
        let mut deps = MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        deps.add_inflation_curve(self.inflation_index_id.clone());
        if let Some(id) = &self.floor_vol_surface_id {
            deps.add_volatility_dependency(
                finstack_quant_valuations_core::instruments::VolatilityDependency::new(
                    id.clone(),
                    None,
                    Some(1.0),
                ),
            );
        }
        Ok(deps)
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        if as_of > self.principal_payment_date()? {
            return Ok(finstack_quant_core::money::Money::from((
                0_i64,
                self.notional.currency(),
            )));
        }
        // Holder view: a coupon settling on `as_of` is no longer owed.
        finstack_quant_valuations_core::instruments::pricing::discounting::schedule_pv(
            self,
            curves,
            as_of,
            &self.discount_curve_id,
            false,
        )
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.maturity)
    }

    fn last_payment_date(&self, _curves: &MarketContext, _as_of: Date) -> Result<Option<Date>> {
        self.principal_payment_date().map(Some)
    }

    fn includes_valuation_date_cashflows(&self) -> bool {
        false
    }

    finstack_quant_valuations_core::impl_focused_pricing_overrides!();
}

impl finstack_quant_cashflows::CashflowScheduleSource for InflationLinkedBond {
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        Ok(Some(self.notional))
    }

    fn raw_cashflow_schedule(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<finstack_quant_cashflows::builder::CashFlowSchedule> {
        if self.issue_date >= self.maturity {
            return Ok(finstack_quant_cashflows::schedule_from_classified_flows(
                Vec::new(),
                self.day_count,
                finstack_quant_cashflows::ScheduleBuildOpts {
                    notional_hint: Some(self.notional),
                    ..Default::default()
                },
            ));
        }
        let periods = self.periods()?;
        let coupon_rate = self.real_coupon_rate()?;

        let mut detailed_flows = Vec::with_capacity(periods.len() + 1);
        for period in periods.iter().filter(|period| period.payment_date > as_of) {
            let accrual_factor = period.accrual_year_fraction.max(0.0);
            let base_amount = self.notional.amount() * coupon_rate * accrual_factor;
            let ratio = self.payment_index_ratio(
                period.unadjusted_end,
                curves,
                as_of,
                self.deflation_protection == DeflationProtection::AllPayments,
            )?;
            detailed_flows.push(
                finstack_quant_core::cashflow::CashFlowParams::new(
                    period.payment_date,
                    None,
                    Money::from_model_value(base_amount * ratio, self.notional.currency())?,
                    finstack_quant_core::cashflow::CFKind::InflationCoupon,
                    accrual_factor,
                    Some(coupon_rate),
                )
                .build()?,
            );
        }

        let principal_date = self.principal_payment_date()?;
        if principal_date > as_of {
            let principal_ratio = self.payment_index_ratio(
                self.maturity,
                curves,
                as_of,
                self.deflation_protection != DeflationProtection::None,
            )?;
            detailed_flows.push({
                // The par change equals the cash (record CORE-CONVENTIONS-015 #4).
                let cash = self.notional.checked_mul_f64(principal_ratio)?;
                finstack_quant_core::cashflow::CashFlowParams::new(
                    principal_date,
                    None,
                    cash,
                    finstack_quant_core::cashflow::CFKind::Notional,
                    0.0,
                    None,
                )
                .with_principal_delta(cash.checked_neg())
                .with_principal_date(principal_date)
                .build()?
            });
        }

        let schedule = finstack_quant_cashflows::schedule_from_classified_flows(
            detailed_flows,
            self.day_count,
            finstack_quant_cashflows::ScheduleBuildOpts {
                notional_hint: Some(self.notional),
                ..Default::default()
            },
        );
        Ok(schedule.with_representation(
            finstack_quant_cashflows::builder::CashflowRepresentation::Projected,
        ))
    }
}
