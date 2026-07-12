//! Zero-coupon Inflation Swap types and pricing implementation.

use crate::cashflow::builder::{CashFlowSchedule, Notional};
use crate::cashflow::primitives::CFKind;
use crate::cashflow::CashflowProvider;
use crate::impl_instrument_base;
use crate::instruments::common_impl::numeric::decimal_to_f64;
use crate::instruments::common_impl::parameters::legs::PayReceive;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::common_impl::validation;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, DayCountContext, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::InflationLag;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CalendarId, CurveId, InstrumentId, Rate};
use rust_decimal::Decimal;

/// Zero-coupon Inflation Swap instrument.
///
/// Represents a zero-coupon inflation swap where one party pays a fixed real rate
/// and the other receives the cumulative inflation over the swap's life. At maturity:
///
/// ```text
/// Inflation leg = Notional × [CPI(T_mat - Lag) / CPI(T_start - Lag) - 1]
/// Fixed leg     = Notional × [(1 + fixed_rate)^τ - 1]
/// ```
///
/// # Market Conventions
///
/// - **Lag**: Standard 3-month lag for US CPI, EUR HICP; 8-month for UK RPI
/// - **Day Count**: Typically ACT/ACT for accrual, curve-specific for discounting
/// - **Business Day**: Payment dates adjusted per calendar; index observation typically unadjusted
///
/// # Validation
///
/// Call [`validate()`](Self::validate) to check structural invariants before pricing.
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct InflationSwap {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Notional in quote currency
    pub notional: Money,
    /// Start date of indexation
    #[schemars(with = "String")]
    pub start_date: Date,
    /// Maturity date
    #[schemars(with = "String")]
    pub maturity: Date,
    /// Fixed real rate (as decimal)
    pub fixed_rate: Decimal,
    /// Inflation index identifier (e.g., US-CPI-U)
    pub inflation_index_id: CurveId,
    /// Discount curve identifier (quote currency)
    pub discount_curve_id: CurveId,
    /// Day count for accrual calculation (fixed leg compounding)
    pub day_count: DayCount,
    /// Trade side
    pub side: PayReceive,
    /// Optional contract-level lag override (if set, overrides index lag)
    #[builder(optional)]
    pub lag_override: Option<InflationLag>,
    /// Explicit Base CPI (reference index level at start with lag applied).
    /// If not provided, it will be looked up/calculated from start date.
    #[builder(optional)]
    pub base_cpi: Option<f64>,
    /// Business day convention for payment date adjustment.
    /// Defaults to `Following` if not specified.
    #[builder(default = BusinessDayConvention::ModifiedFollowing)]
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub bdc: BusinessDayConvention,
    /// Holiday calendar identifier for payment date adjustment.
    /// If not specified, payment dates are used unadjusted.
    #[builder(optional)]
    pub calendar_id: Option<CalendarId>,
    /// Attributes for scenario selection and tagging
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl InflationSwap {
    /// Create a canonical example zero-coupon inflation swap (US CPI, 5Y).
    ///
    /// Returns a 5-year USD inflation swap with standard 3-month CPI lag.
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        use finstack_quant_core::currency::Currency;
        use time::Month;
        InflationSwap::builder()
            .id(InstrumentId::new("INFLSWAP-USD-5Y"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(
                Date::from_calendar_date(2024, Month::January, 15).expect("Valid example date"),
            )
            .maturity(
                Date::from_calendar_date(2029, Month::January, 15).expect("Valid example date"),
            )
            .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
            .inflation_index_id(CurveId::new("US-CPI"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Act365F)
            .side(PayReceive::Pay)
            .lag_override(InflationLag::Months(3))
            .bdc(BusinessDayConvention::Following)
            .attributes(Attributes::new())
            .build()
            .expect("Example InflationSwap construction should not fail")
    }

    /// Validate structural invariants of the inflation swap.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `start >= maturity` (invalid date ordering)
    /// - `notional` is non-positive
    /// - `base_cpi` is provided but non-positive
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        validation::require_or(
            self.start_date < self.maturity,
            finstack_quant_core::InputError::InvalidDateRange,
        )?;
        validation::require_or(
            self.notional.amount() > 0.0,
            finstack_quant_core::InputError::NonPositiveValue,
        )?;
        if let Some(base) = self.base_cpi {
            validation::require_or(
                base > 0.0,
                finstack_quant_core::InputError::NonPositiveValue,
            )?;
        }
        let fixed_rate = decimal_to_f64(self.fixed_rate, "InflationSwap fixed_rate")?;
        validation::require_with(fixed_rate > -1.0, || {
            format!(
                "InflationSwap fixed_rate ({fixed_rate}) must be greater than -1.0 for geometric compounding"
            )
        })?;
        Ok(())
    }

    /// Apply lag to a date according to the instrument's lag policy.
    ///
    /// Uses `lag_override` if set, otherwise falls back to `default_lag`.
    ///
    /// # Supported Lag Types
    ///
    /// - `InflationLag::None`: No lag applied
    /// - `InflationLag::Months(n)`: Subtract n months from the date
    /// - `InflationLag::Days(n)`: Subtract n days from the date
    ///
    /// # Note
    ///
    /// The `InflationLag` enum is `#[non_exhaustive]`, so unknown variants
    /// fall back to no lag with a debug assertion. This ensures forward
    /// compatibility while catching unexpected variants in development.
    pub(crate) fn apply_lag(&self, date: Date, default_lag: InflationLag) -> Date {
        let lag_policy = self.lag_override.unwrap_or(default_lag);
        crate::instruments::common_impl::helpers::apply_inflation_lag(date, lag_policy)
    }

    fn effective_lag(&self, curves: &MarketContext) -> InflationLag {
        crate::instruments::common_impl::helpers::resolve_inflation_lag(
            self.lag_override,
            self.inflation_index_id.as_str(),
            curves,
        )
    }

    fn cpi_value_at_lagged_date(
        &self,
        curves: &MarketContext,
        discount_base: Date,
        unlagged_date: Date,
        lagged_date: Date,
    ) -> finstack_quant_core::Result<f64> {
        // Once the lagged fixing date is on or before the valuation date, prefer the
        // realized index history. If a history is supplied, lookup failures (most
        // importantly dates before the first observation) are data errors and must
        // not be hidden by projecting a historical value from the curve.
        if lagged_date <= discount_base {
            if let Ok(index) = curves.get_inflation_index(self.inflation_index_id.as_str()) {
                return crate::instruments::common_impl::helpers::realized_inflation_index_value(
                    index.as_ref(),
                    unlagged_date,
                    lagged_date,
                    self.effective_lag(curves),
                );
            }
        }

        let inflation_curve = curves.get_inflation_curve(self.inflation_index_id.as_str())?;
        Self::curve_cpi_value(inflation_curve.as_ref(), discount_base, lagged_date)
    }

    /// Calculate the projected index ratio I(T_mat - Lag) / I(T_start - Lag).
    ///
    /// When using an InflationIndex, the index applies its own lag internally via `value_on()`,
    /// so we pass unlagged dates. When falling back to the inflation curve, we must apply
    /// the lag ourselves since the curve works in year fractions from its base date.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Inflation curve is not found
    /// - Base CPI (start index) is non-positive
    pub(crate) fn projected_index_ratio(
        &self,
        curves: &MarketContext,
        discount_base: Date,
    ) -> finstack_quant_core::Result<f64> {
        let default_lag = self.effective_lag(curves);
        let lagged_start = self.apply_lag(self.start_date, default_lag);
        let lagged_maturity = self.apply_lag(self.maturity, default_lag);

        let i_start = if let Some(base) = self.base_cpi {
            base
        } else {
            self.cpi_value_at_lagged_date(curves, discount_base, self.start_date, lagged_start)?
        };

        if i_start <= 0.0 {
            return Err(finstack_quant_core::InputError::NonPositiveValue.into());
        }

        let i_maturity_projected =
            self.cpi_value_at_lagged_date(curves, discount_base, self.maturity, lagged_maturity)?;

        Ok(i_maturity_projected / i_start)
    }

    pub(crate) fn curve_cpi_value(
        curve: &finstack_quant_core::market_data::term_structures::InflationCurve,
        fallback_base: Date,
        lookup_date: Date,
    ) -> finstack_quant_core::Result<f64> {
        let default_anchor =
            Date::from_calendar_date(1970, time::Month::January, 1).unwrap_or(time::Date::MIN);
        if curve.base_date() == default_anchor {
            // Anchor curve: measure time from the supplied fallback base. A
            // failed day-count calculation must be PROPAGATED, not silently
            // collapsed to `t = 0`: `unwrap_or(0.0)` masked a genuine error
            // (e.g. an inverted date) by quietly returning the base CPI, which
            // mis-prices the swap with no diagnostic.
            let t = DayCount::Act365F.signed_year_fraction(
                fallback_base,
                lookup_date,
                DayCountContext::default(),
            )?;
            Ok(curve.cpi(t))
        } else if lookup_date < curve.base_date() {
            // Lagged observation dates can fall before the curve base (e.g. a
            // 3-month lag at trade inception with no fixing history). Read the
            // curve at a signed (negative) time so it extrapolates from its
            // base CPI instead of rejecting the inverted date range.
            let t = curve.day_count().signed_year_fraction(
                curve.base_date(),
                lookup_date,
                DayCountContext::default(),
            )?;
            Ok(curve.cpi(t))
        } else {
            curve.cpi_on_date(lookup_date)
        }
    }

    /// Get the adjusted payment date based on business day convention and calendar.
    ///
    /// If no calendar is specified, returns the unadjusted date.
    fn adjusted_payment_date(&self, date: Date) -> finstack_quant_core::Result<Date> {
        let bdc = self.bdc;
        if let Some(ref cal_id) = self.calendar_id {
            use finstack_quant_core::dates::CalendarRegistry;
            let cal = CalendarRegistry::global()
                .resolve_str(cal_id)
                .ok_or_else(|| {
                    finstack_quant_core::Error::Validation(format!(
                        "InflationSwap '{}' calendar '{}' is not registered",
                        self.id, cal_id
                    ))
                })?;
            return finstack_quant_core::dates::adjust(date, bdc, cal);
        }
        // No calendar specified - return unadjusted (common for inflation swaps)
        Ok(date)
    }

    /// Calculate PV of the fixed leg (real rate leg).
    ///
    /// The fixed leg pays `Notional × [(1 + fixed_rate)^τ - 1]` at maturity,
    /// where τ is the accrual year fraction using the instrument's day count.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Discount curve is not found
    /// - Year fraction calculation fails
    pub fn pv_fixed_leg(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        let disc = curves.get_discount(self.discount_curve_id.as_str())?;

        // Use instrument day count for accrual period
        let tau_accrual = self.day_count.year_fraction(
            self.start_date,
            self.maturity,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;

        let fixed_rate = decimal_to_f64(self.fixed_rate, "InflationSwap fixed_rate")?;
        let fixed_payment = self.notional * ((1.0 + fixed_rate).powf(tau_accrual) - 1.0);

        // Date-based DF from as_of to payment: correct when the curve base
        // date differs from as_of.
        let payment_date = self.adjusted_payment_date(self.maturity)?;
        let df = crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
            disc.as_ref(),
            as_of,
            payment_date,
        )?;

        Ok(fixed_payment * df)
    }

    fn fixed_leg_amount(&self) -> finstack_quant_core::Result<Money> {
        let tau_accrual = self.day_count.year_fraction(
            self.start_date,
            self.maturity,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        let fixed_rate = decimal_to_f64(self.fixed_rate, "InflationSwap fixed_rate")?;
        Ok(self.notional * ((1.0 + fixed_rate).powf(tau_accrual) - 1.0))
    }

    /// Calculate PV of the inflation leg.
    ///
    /// The inflation leg pays `Notional × [I(T_mat - Lag) / I(T_start - Lag) - 1]`
    /// at maturity, where I(t) is the inflation index value.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Discount curve is not found
    /// - Inflation curve is not found
    /// - Index ratio calculation fails
    /// - Year fraction calculation fails
    pub fn pv_inflation_leg(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        let disc = curves.get_discount(self.discount_curve_id.as_str())?;
        let index_ratio = self.projected_index_ratio(curves, as_of)?;
        let inflation_payment = self.notional * (index_ratio - 1.0);

        // Date-based DF from as_of to payment: correct when the curve base
        // date differs from as_of.
        let payment_date = self.adjusted_payment_date(self.maturity)?;
        let df = crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
            disc.as_ref(),
            as_of,
            payment_date,
        )?;

        Ok(inflation_payment * df)
    }

    fn inflation_leg_amount(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        let index_ratio = self.projected_index_ratio(curves, as_of)?;
        Ok(self.notional * (index_ratio - 1.0))
    }

    /// Fixed rate that sets the swap's present value to zero (par real rate / breakeven).
    ///
    /// For a zero-coupon inflation swap, the par rate K satisfies:
    /// ```text
    /// (1 + K)^τ = I(T_mat - Lag) / I(T_start - Lag)
    /// K = [I(T_mat - Lag) / I(T_start - Lag)]^(1/τ) - 1
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Index ratio is non-positive
    /// - Year fraction calculation fails
    pub fn par_rate(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        let index_ratio = self.projected_index_ratio(curves, as_of)?;

        if index_ratio <= 0.0 {
            return Err(finstack_quant_core::InputError::NonPositiveValue.into());
        }

        let tau = self.day_count.year_fraction(
            self.start_date,
            self.maturity,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if tau <= 0.0 {
            return Ok(0.0);
        }

        Ok(index_ratio.powf(1.0 / tau) - 1.0)
    }

    /// Raw (unrounded `f64`) present value of the swap from the holder's
    /// perspective, used by finite-difference metrics (convexity, gamma)
    /// where Money quantization noise would be amplified by tiny bump sizes.
    pub fn npv_raw(&self, curves: &MarketContext, as_of: Date) -> finstack_quant_core::Result<f64> {
        let payment_date = self.adjusted_payment_date(self.maturity)?;
        if as_of >= payment_date {
            return Ok(0.0);
        }

        let disc = curves.get_discount(self.discount_curve_id.as_str())?;
        let df = crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
            disc.as_ref(),
            as_of,
            payment_date,
        )?;

        let tau_accrual = self.day_count.year_fraction(
            self.start_date,
            self.maturity,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        let fixed_rate = decimal_to_f64(self.fixed_rate, "InflationSwap fixed_rate")?;
        let fixed_pv = self.notional.amount() * ((1.0 + fixed_rate).powf(tau_accrual) - 1.0) * df;

        let index_ratio = self.projected_index_ratio(curves, as_of)?;
        let inflation_pv = self.notional.amount() * (index_ratio - 1.0) * df;

        Ok(match self.side {
            PayReceive::Receive => fixed_pv - inflation_pv,
            PayReceive::Pay => inflation_pv - fixed_pv,
        })
    }
}

impl InflationSwapBuilder {
    /// Set the fixed rate using a typed rate.
    pub fn fixed_rate_rate(mut self, rate: Rate) -> Self {
        self.fixed_rate = Decimal::try_from(rate.as_decimal()).ok();
        self
    }
}

impl crate::instruments::common_impl::traits::Instrument for InflationSwap {
    impl_instrument_base!(crate::pricer::InstrumentType::InflationSwap);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        InflationSwap::validate(self)
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        let payment_date = self.adjusted_payment_date(self.maturity)?;
        if as_of >= payment_date {
            return Ok(finstack_quant_core::money::Money::new(
                0.0,
                self.notional.currency(),
            ));
        }

        let pv_fixed = self.pv_fixed_leg(curves, as_of)?;
        let pv_inflation = self.pv_inflation_leg(curves, as_of)?;
        match self.side {
            PayReceive::Receive => pv_fixed.checked_sub(pv_inflation),
            PayReceive::Pay => pv_inflation.checked_sub(pv_fixed),
        }
    }

    fn value_raw(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        self.npv_raw(curves, as_of)
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.maturity)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.start_date)
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

impl CashflowProvider for InflationSwap {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn cashflow_schedule(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        let payment_date = self.adjusted_payment_date(self.maturity)?;
        let anchor = if as_of < payment_date {
            as_of
        } else {
            payment_date - time::Duration::days(1)
        };
        let fixed_amount = self.fixed_leg_amount()?;
        let inflation_amount = self.inflation_leg_amount(curves, as_of)?;
        let (fixed_signed, inflation_signed) = match self.side {
            PayReceive::Pay => (-fixed_amount.amount(), inflation_amount.amount()),
            PayReceive::Receive => (fixed_amount.amount(), -inflation_amount.amount()),
        };
        let ccy = self.notional.currency();
        let mut builder = CashFlowSchedule::builder();
        let _ = builder.principal(Money::new(0.0, ccy), anchor, payment_date);
        let _ = builder.add_principal_event(
            payment_date,
            Money::new(0.0, ccy),
            Some(Money::new(-fixed_signed, ccy)),
            CFKind::Notional,
        );
        let _ = builder.add_principal_event(
            payment_date,
            Money::new(0.0, ccy),
            Some(Money::new(-inflation_signed, ccy)),
            CFKind::Notional,
        );
        let mut schedule = builder.build_with_curves(None)?;
        schedule.notional = Notional::par(self.notional.amount(), ccy);
        schedule.day_count = self.day_count;
        Ok(schedule.normalize_public(
            as_of,
            crate::cashflow::builder::CashflowRepresentation::Projected,
        ))
    }
}

// Implement CurveDependencies for DV01 calculator
impl crate::instruments::common_impl::traits::CurveDependencies for InflationSwap {
    fn curve_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::InstrumentCurves>
    {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .inflation(self.inflation_index_id.clone())
            .build()
    }
}

/// Year-on-year (YoY) Inflation Swap instrument.
///
/// Pays periodic inflation rates (CPI ratios over each period) versus a fixed rate.
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct YoYInflationSwap {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Notional in quote currency
    pub notional: Money,
    /// Start date of the first accrual period
    #[schemars(with = "String")]
    pub start_date: Date,
    /// Maturity date
    #[schemars(with = "String")]
    pub maturity: Date,
    /// Fixed rate (decimal)
    pub fixed_rate: Decimal,
    /// Payment frequency
    pub frequency: Tenor,
    /// Inflation index identifier (e.g., US-CPI-U)
    pub inflation_index_id: CurveId,
    /// Discount curve identifier (quote currency)
    pub discount_curve_id: CurveId,
    /// Day count for fixed leg accrual calculation
    pub day_count: DayCount,
    /// Trade side
    pub side: PayReceive,
    /// Optional contract-level lag override (if set, overrides index lag)
    #[builder(optional)]
    pub lag_override: Option<InflationLag>,
    /// Business day convention for payment date adjustment.
    #[builder(default = BusinessDayConvention::ModifiedFollowing)]
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub bdc: BusinessDayConvention,
    /// Holiday calendar identifier for payment date adjustment.
    #[builder(optional)]
    pub calendar_id: Option<CalendarId>,
    /// Attributes for scenario selection and tagging
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl YoYInflationSwap {
    /// Create a canonical example USD 5Y YoY inflation swap (US-CPI, annual payments).
    ///
    /// Returns a 5-year pay-fixed YoY inflation swap with 2.5% fixed rate,
    /// $1M notional, annual frequency, and standard 3-month CPI lag.
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        use finstack_quant_core::currency::Currency;
        use time::Month;

        YoYInflationSwap::builder()
            .id(InstrumentId::new("YOYSWAP-USD-5Y"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(
                Date::from_calendar_date(2024, Month::January, 15).expect("Valid example date"),
            )
            .maturity(
                Date::from_calendar_date(2029, Month::January, 15).expect("Valid example date"),
            )
            .fixed_rate(Decimal::try_from(0.025).expect("valid decimal"))
            .frequency(Tenor::annual())
            .inflation_index_id(CurveId::new("US-CPI"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Act365F)
            .side(PayReceive::Pay)
            .lag_override(InflationLag::Months(3))
            .bdc(BusinessDayConvention::ModifiedFollowing)
            .attributes(Attributes::new())
            .build()
            .expect("Example YoYInflationSwap construction should not fail")
    }

    /// Validate structural invariants of the YoY inflation swap.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        validation::require_or(
            self.start_date < self.maturity,
            finstack_quant_core::InputError::InvalidDateRange,
        )?;
        validation::require_or(
            self.notional.amount() > 0.0,
            finstack_quant_core::InputError::NonPositiveValue,
        )?;
        validation::require_or(
            self.frequency.count() != 0,
            finstack_quant_core::InputError::Invalid,
        )?;
        Ok(())
    }

    fn effective_lag(&self, curves: &MarketContext) -> InflationLag {
        crate::instruments::common_impl::helpers::resolve_inflation_lag(
            self.lag_override,
            self.inflation_index_id.as_str(),
            curves,
        )
    }

    fn apply_lag(date: Date, lag: InflationLag) -> Date {
        crate::instruments::common_impl::helpers::apply_inflation_lag(date, lag)
    }

    fn cpi_value(
        &self,
        curves: &MarketContext,
        as_of: Date,
        date: Date,
    ) -> finstack_quant_core::Result<f64> {
        let lag = self.effective_lag(curves);
        let lagged_date = Self::apply_lag(date, lag);

        // Only consult realized fixings for observations whose (lagged) fixing
        // date is on or before the valuation date. Reading later entries from a
        // fixing series that extends past as_of would introduce look-ahead bias.
        if lagged_date <= as_of {
            if let Ok(index) = curves.get_inflation_index(self.inflation_index_id.as_str()) {
                return crate::instruments::common_impl::helpers::realized_inflation_index_value(
                    index.as_ref(),
                    date,
                    lagged_date,
                    lag,
                );
            }
        }

        let curve = curves.get_inflation_curve(self.inflation_index_id.as_str())?;
        InflationSwap::curve_cpi_value(curve.as_ref(), as_of, lagged_date)
    }

    fn schedule(&self) -> finstack_quant_core::Result<Vec<(Date, Date, Date)>> {
        let bdc = self.bdc;
        let periods = crate::cashflow::builder::periods::build_periods(
            crate::cashflow::builder::periods::BuildPeriodsParams {
                start: self.start_date,
                end: self.maturity,
                frequency: self.frequency,
                stub: StubKind::None,
                bdc,
                calendar_id: self
                    .calendar_id
                    .as_deref()
                    .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID),
                end_of_month: false,
                day_count: self.day_count,
                payment_lag_days: 0,
                reset_lag_days: None,
                adjust_accrual_dates: false,
            },
        )?;

        if periods.is_empty() {
            return Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::Invalid,
            ));
        }

        Ok(periods
            .into_iter()
            .map(|period| {
                (
                    period.accrual_start,
                    period.accrual_end,
                    period.payment_date,
                )
            })
            .collect())
    }

    /// Calculates the raw present value (f64) of the YoY inflation swap.
    ///
    /// # Model limitation — no YoY convexity / timing adjustment
    ///
    /// The year-on-year inflation leg is valued with the deterministic ratio
    /// `F(CPI_end) / F(CPI_start) − 1`. Under the payment measure the true
    /// expectation `E^{T_pay}[CPI(tᵢ)/CPI(tᵢ₋₁)]` differs from
    /// `F(tᵢ)/F(tᵢ₋₁)` by a convexity/timing correction that depends on
    /// inflation volatility and the rate–inflation correlation (Brigo-Mercurio
    /// Ch. 16; Mercurio 2005). This implementation omits that correction, so
    /// for long-dated YoY swaps it carries a few-bp-to-material bias. Use a
    /// stochastic-inflation model where that bias is unacceptable.
    pub fn npv_raw(&self, curves: &MarketContext, as_of: Date) -> finstack_quant_core::Result<f64> {
        let disc = curves.get_discount(self.discount_curve_id.as_str())?;
        let mut pv = 0.0_f64;

        for (start, end, pay) in self.schedule()? {
            if pay <= as_of {
                continue;
            }
            let accrual = self
                .day_count
                .year_fraction(start, end, DayCountContext::default())?;

            let cpi_start = self.cpi_value(curves, as_of, start)?;
            if cpi_start <= 0.0 {
                return Err(finstack_quant_core::InputError::NonPositiveValue.into());
            }
            let cpi_end = self.cpi_value(curves, as_of, end)?;

            let inflation_leg = self.notional.amount() * (cpi_end / cpi_start - 1.0);
            let fixed_rate = decimal_to_f64(self.fixed_rate, "YoYInflationSwap fixed_rate")?;
            let fixed_leg = self.notional.amount() * fixed_rate * accrual;

            let net = match self.side {
                PayReceive::Pay => inflation_leg - fixed_leg,
                PayReceive::Receive => fixed_leg - inflation_leg,
            };

            // Date-based DF from as_of to payment: correct when the curve base
            // date differs from as_of.
            let df = crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
                disc.as_ref(),
                as_of,
                pay,
            )?;
            pv += net * df;
        }

        Ok(pv)
    }

    /// Fixed rate that sets the swap's present value to zero (par rate / breakeven).
    ///
    /// For a YoY inflation swap, the par rate K satisfies:
    /// ```text
    /// Sum_i [ DF_i × (CPI_i / CPI_{i-1} - 1) ] = Sum_i [ DF_i × K × accrual_i ]
    ///
    /// K = Sum_i [ DF_i × (CPI_i / CPI_{i-1} - 1) ] / Sum_i [ DF_i × accrual_i ]
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Discount curve is not found
    /// - Inflation curve/index is not found
    /// - Any CPI value is non-positive
    /// - The annuity (sum of discounted accruals) is zero
    pub fn par_rate(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        let disc = curves.get_discount(self.discount_curve_id.as_str())?;

        let mut sum_infl_pv = 0.0_f64;
        let mut sum_annuity = 0.0_f64;

        for (start, end, pay) in self.schedule()? {
            if pay <= as_of {
                continue;
            }
            let accrual = self
                .day_count
                .year_fraction(start, end, DayCountContext::default())?;

            let cpi_start = self.cpi_value(curves, as_of, start)?;
            if cpi_start <= 0.0 {
                return Err(finstack_quant_core::InputError::NonPositiveValue.into());
            }
            let cpi_end = self.cpi_value(curves, as_of, end)?;

            let df = crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
                disc.as_ref(),
                as_of,
                pay,
            )?;

            // Inflation leg contribution: DF × (CPI_end / CPI_start - 1)
            sum_infl_pv += df * (cpi_end / cpi_start - 1.0);

            // Annuity contribution: DF × accrual
            sum_annuity += df * accrual;
        }

        if sum_annuity.abs() < 1e-15 {
            // Degenerate case: no accrual periods
            return Ok(0.0);
        }

        Ok(sum_infl_pv / sum_annuity)
    }

    fn signed_period_flows(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Vec<(Date, Money)>> {
        let mut flows = Vec::new();
        let fixed_rate = decimal_to_f64(self.fixed_rate, "YoYInflationSwap fixed_rate")?;

        for (start, end, pay) in self.schedule()? {
            if pay <= as_of {
                continue;
            }
            let accrual = self
                .day_count
                .year_fraction(start, end, DayCountContext::default())?;
            let cpi_start = self.cpi_value(curves, as_of, start)?;
            if cpi_start <= 0.0 {
                return Err(finstack_quant_core::InputError::NonPositiveValue.into());
            }
            let cpi_end = self.cpi_value(curves, as_of, end)?;

            let fixed_leg = self.notional.amount() * fixed_rate * accrual;
            let inflation_leg = self.notional.amount() * (cpi_end / cpi_start - 1.0);
            let (fixed_signed, inflation_signed) = match self.side {
                PayReceive::Pay => (-fixed_leg, inflation_leg),
                PayReceive::Receive => (fixed_leg, -inflation_leg),
            };
            flows.push((pay, Money::new(fixed_signed, self.notional.currency())));
            flows.push((pay, Money::new(inflation_signed, self.notional.currency())));
        }

        Ok(flows)
    }
}

impl YoYInflationSwapBuilder {
    /// Set the fixed rate using a typed rate.
    pub fn fixed_rate_rate(mut self, rate: Rate) -> Self {
        self.fixed_rate = Decimal::try_from(rate.as_decimal()).ok();
        self
    }
}

impl crate::instruments::common_impl::traits::Instrument for YoYInflationSwap {
    impl_instrument_base!(crate::pricer::InstrumentType::YoYInflationSwap);

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        let pv = self.npv_raw(curves, as_of)?;
        Ok(finstack_quant_core::money::Money::new(
            pv,
            self.notional.currency(),
        ))
    }

    fn value_raw(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        self.npv_raw(curves, as_of)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.start_date)
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

impl CashflowProvider for YoYInflationSwap {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn cashflow_schedule(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        let anchor = if as_of < self.maturity {
            as_of
        } else {
            self.maturity - time::Duration::days(1)
        };
        let mut builder = CashFlowSchedule::builder();
        let ccy = self.notional.currency();
        let _ = builder.principal(Money::new(0.0, ccy), anchor, self.maturity);
        for (pay, amount) in self.signed_period_flows(curves, as_of)? {
            let _ = builder.add_principal_event(
                pay,
                Money::new(0.0, ccy),
                Some(Money::new(-amount.amount(), ccy)),
                CFKind::Notional,
            );
        }
        let mut schedule = builder.build_with_curves(None)?;
        schedule.notional = Notional::par(self.notional.amount(), ccy);
        schedule.day_count = self.day_count;
        Ok(schedule.normalize_public(
            as_of,
            crate::cashflow::builder::CashflowRepresentation::Projected,
        ))
    }
}

impl crate::instruments::common_impl::traits::CurveDependencies for YoYInflationSwap {
    fn curve_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::InstrumentCurves>
    {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .inflation(self.inflation_index_id.clone())
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::CashflowProvider;
    use crate::instruments::common_impl::traits::Instrument;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::scalars::InflationIndex;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, InflationCurve};
    use time::Month;

    fn d(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).expect("valid date")
    }

    fn sample_inflation_curve(base_date: Date) -> InflationCurve {
        InflationCurve::builder("US-CPI")
            .base_date(base_date)
            .base_cpi(100.0)
            .knots([(0.0, 100.0), (1.0, 110.0), (2.0, 121.0)])
            .build()
            .expect("inflation curve should build")
    }

    fn sample_discount_curve(base_date: Date) -> DiscountCurve {
        DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (2.0, 1.0)])
            .build()
            .expect("discount curve should build")
    }

    fn flat_historical_index() -> InflationIndex {
        InflationIndex::new(
            "US-CPI",
            vec![
                (d(2020, Month::January, 1), 100.0),
                (d(2030, Month::January, 1), 100.0),
            ],
            Currency::USD,
        )
        .expect("inflation index should build")
    }

    fn sample_swap(start_date: Date, maturity: Date) -> InflationSwap {
        InflationSwap::builder()
            .id(InstrumentId::new("INFL-SWAP"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(start_date)
            .maturity(maturity)
            .fixed_rate(Decimal::ZERO)
            .inflation_index_id(CurveId::new("US-CPI"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Act365F)
            .side(PayReceive::Pay)
            .lag_override(InflationLag::None)
            .attributes(Attributes::new())
            .build()
            .expect("swap should build")
    }

    #[test]
    fn projected_index_ratio_uses_inflation_curve_anchor_date() {
        let curve_base = d(2024, Month::January, 1);
        let discount_base = d(2024, Month::June, 1);
        let start = d(2024, Month::March, 1);
        let maturity = d(2025, Month::March, 1);
        let swap = sample_swap(start, maturity);
        let inflation_curve = sample_inflation_curve(curve_base);
        let market = MarketContext::new()
            .insert(sample_discount_curve(discount_base))
            .insert(inflation_curve.clone())
            .insert_inflation_index("US-CPI", flat_historical_index());

        let ratio = swap
            .projected_index_ratio(&market, discount_base)
            .expect("ratio should compute");
        let expected = inflation_curve.cpi_on_date(maturity).expect("maturity CPI") / 100.0;

        assert!(
            (ratio - expected).abs() < 1e-10,
            "expected ratio {expected}, got {ratio}"
        );
    }

    #[test]
    fn yoy_swap_prices_with_curve_only_market() {
        let as_of = d(2025, Month::January, 1);
        let market = MarketContext::new()
            .insert(sample_discount_curve(as_of))
            .insert(sample_inflation_curve(as_of));
        let swap = YoYInflationSwap::builder()
            .id(InstrumentId::new("YOY-CURVE-ONLY"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(d(2027, Month::January, 1))
            .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
            .frequency(Tenor::annual())
            .inflation_index_id(CurveId::new("US-CPI"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Act365F)
            .side(PayReceive::Pay)
            .lag_override(InflationLag::None)
            .attributes(Attributes::new())
            .build()
            .expect("yoy swap should build");

        let pv = swap
            .npv_raw(&market, as_of)
            .expect("curve-only market should price");

        assert!(pv.is_finite());
    }

    #[test]
    fn matured_but_unpaid_swap_retains_value_until_adjusted_payment_date() {
        let start = d(2024, Month::January, 15);
        let maturity = d(2025, Month::January, 18);
        let as_of = d(2025, Month::January, 19);
        let mut swap = sample_swap(start, maturity);
        swap.bdc = BusinessDayConvention::Following;
        swap.calendar_id = Some("nyse".into());

        let market = MarketContext::new()
            .insert(sample_discount_curve(as_of))
            .insert(sample_inflation_curve(d(2024, Month::January, 1)))
            .insert_inflation_index(
                "US-CPI",
                InflationIndex::new(
                    "US-CPI",
                    vec![(start, 100.0), (maturity, 110.0)],
                    Currency::USD,
                )
                .expect("inflation index should build"),
            );

        let pv = swap.value(&market, as_of).expect("value should compute");
        assert!(
            pv.amount().abs() > 0.0,
            "swap should retain value between contractual maturity and adjusted payment date"
        );
    }

    #[test]
    fn zero_coupon_swap_cashflow_provider_emits_two_maturity_flows() {
        let as_of = d(2025, Month::January, 1);
        let maturity = d(2027, Month::January, 1);
        let market = MarketContext::new()
            .insert(sample_discount_curve(as_of))
            .insert(sample_inflation_curve(as_of))
            .insert_inflation_index("US-CPI", flat_historical_index());
        let swap = InflationSwap::builder()
            .id(InstrumentId::new("INFL-CF"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(maturity)
            .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
            .inflation_index_id(CurveId::new("US-CPI"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Act365F)
            .side(PayReceive::Pay)
            .lag_override(InflationLag::None)
            .attributes(Attributes::new())
            .build()
            .expect("swap should build");

        let flows = swap
            .dated_cashflows(&market, as_of)
            .expect("contractual schedule should build");

        assert_eq!(flows.len(), 2, "zc inflation swap should emit both legs");
        assert!(flows.iter().all(|(date, _)| *date == maturity));
        assert!(
            flows[0].1.amount() < 0.0,
            "pay-fixed swap should pay fixed leg"
        );
        assert!(
            flows[1].1.amount() > 0.0,
            "pay-fixed swap should receive inflation leg"
        );
    }

    /// YoY `npv_raw` must discount with DF(as_of → pay) = DF(pay)/DF(as_of)
    /// (curve-base relative), not DF(yf(as_of, pay)) which silently assumes
    /// the curve base equals as_of.
    #[test]
    fn yoy_npv_raw_discounts_relative_to_as_of_on_rebased_curve() {
        let curve_base = d(2024, Month::January, 1);
        let as_of = d(2025, Month::January, 1);
        let maturity = d(2026, Month::January, 1);

        // Non-trivial DF shape so DF(pay)/DF(as_of) != DF(yf(as_of, pay)).
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(curve_base)
            .knots([(0.0, 1.0), (1.0, 0.95), (2.0, 0.88), (3.0, 0.80)])
            .build()
            .expect("discount curve should build");
        // Flat CPI so the inflation leg is exactly zero.
        let infl = InflationCurve::builder("US-CPI")
            .base_date(curve_base)
            .base_cpi(100.0)
            .knots([(0.0, 100.0), (3.0, 100.0)])
            .build()
            .expect("inflation curve should build");
        let market = MarketContext::new()
            .insert(disc.clone())
            .insert(infl)
            .insert_inflation_index("US-CPI", flat_historical_index());

        let swap = YoYInflationSwap::builder()
            .id(InstrumentId::new("YOY-REBASE"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(maturity)
            .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
            .frequency(Tenor::annual())
            .inflation_index_id(CurveId::new("US-CPI"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Act365F)
            .side(PayReceive::Receive)
            .lag_override(InflationLag::None)
            .attributes(Attributes::new())
            .build()
            .expect("yoy swap should build");

        let pv = swap.npv_raw(&market, as_of).expect("npv should compute");

        // Inflation leg is 0 (flat CPI); fixed leg = N * r * accrual * DF.
        let accrual = DayCount::Act365F
            .year_fraction(as_of, maturity, DayCountContext::default())
            .expect("accrual");
        let df_expected = disc.df_between_dates(as_of, maturity).expect("relative df");
        let expected = 1_000_000.0 * 0.02 * accrual * df_expected;
        assert!(
            (pv - expected).abs() < 1e-6,
            "expected relative-DF PV {expected}, got {pv}"
        );

        // Sanity: the buggy absolute-time DF would differ materially.
        let t_wrong = DayCount::Act365F
            .year_fraction(as_of, maturity, DayCountContext::default())
            .expect("yf");
        let df_wrong = disc.df(t_wrong);
        assert!(
            (df_expected - df_wrong).abs() > 1e-3,
            "test setup must distinguish the two discounting conventions"
        );
    }

    /// Realized CPI fixings dated after `as_of` must not be consulted
    /// (look-ahead bias): pricing must match the curve-projection PV.
    #[test]
    fn yoy_cpi_value_ignores_fixings_after_as_of() {
        let as_of = d(2025, Month::January, 1);
        let maturity = d(2026, Month::January, 1);

        let disc = sample_discount_curve(as_of);
        let infl = InflationCurve::builder("US-CPI")
            .base_date(as_of)
            .base_cpi(100.0)
            .knots([(0.0, 100.0), (2.0, 110.0)])
            .build()
            .expect("inflation curve should build");

        let market_no_index = MarketContext::new()
            .insert(disc.clone())
            .insert(infl.clone())
            .insert_inflation_index("US-CPI", flat_historical_index());

        // Index with a *future* fixing wildly different from the curve.
        let index = InflationIndex::new(
            "US-CPI",
            vec![(d(2024, Month::December, 1), 100.0), (maturity, 500.0)],
            Currency::USD,
        )
        .expect("index should build");
        let market_with_future_fixing = MarketContext::new()
            .insert(disc)
            .insert(infl)
            .insert_inflation_index("US-CPI", index);

        let swap = YoYInflationSwap::builder()
            .id(InstrumentId::new("YOY-LOOKAHEAD"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(maturity)
            .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
            .frequency(Tenor::annual())
            .inflation_index_id(CurveId::new("US-CPI"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Act365F)
            .side(PayReceive::Pay)
            .lag_override(InflationLag::None)
            .attributes(Attributes::new())
            .build()
            .expect("yoy swap should build");

        let pv_clean = swap
            .npv_raw(&market_no_index, as_of)
            .expect("clean pv should compute");
        let pv_with_future = swap
            .npv_raw(&market_with_future_fixing, as_of)
            .expect("pv with future fixing should compute");

        assert!(
            (pv_clean - pv_with_future).abs() < 1e-6,
            "future-dated fixings must not affect PV (look-ahead bias): \
             clean={pv_clean}, with_future_fixing={pv_with_future}"
        );
    }

    #[test]
    fn yoy_swap_cashflow_provider_emits_two_flows_per_period() {
        let as_of = d(2025, Month::January, 1);
        let market = MarketContext::new()
            .insert(sample_discount_curve(as_of))
            .insert(sample_inflation_curve(as_of))
            .insert_inflation_index("US-CPI", flat_historical_index());
        let swap = YoYInflationSwap::builder()
            .id(InstrumentId::new("YOY-CF"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(d(2027, Month::January, 1))
            .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
            .frequency(Tenor::annual())
            .inflation_index_id(CurveId::new("US-CPI"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Act365F)
            .side(PayReceive::Pay)
            .lag_override(InflationLag::None)
            .attributes(Attributes::new())
            .build()
            .expect("yoy swap should build");

        let flows = swap
            .dated_cashflows(&market, as_of)
            .expect("yoy contractual schedule should build");

        assert_eq!(
            flows.len(),
            4,
            "two annual periods should emit fixed and inflation rows"
        );
        assert_eq!(
            flows
                .iter()
                .filter(|(_, money)| money.amount() < 0.0)
                .count(),
            2
        );
        assert_eq!(
            flows
                .iter()
                .filter(|(_, money)| money.amount() > 0.0)
                .count(),
            2
        );
    }
}
