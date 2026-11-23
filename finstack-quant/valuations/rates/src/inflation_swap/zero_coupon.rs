//! Zero-coupon Inflation Swap instrument and pricing implementation.

use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_core::cashflow::CashFlowAccrual;
use finstack_quant_core::cashflow::{CFKind, CashFlowParams};
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DateExt, DayCount, DayCountContext, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{InflationLag, SeriesInterpolation};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::{CalendarId, CurveId, InstrumentId};
use finstack_quant_core::validation;
use finstack_quant_valuations_core::impl_instrument_base;
use finstack_quant_valuations_core::instruments::PayReceive;
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
/// - **Lag**: Standard 3-month lag for US CPI/EUR HICP; 2-month for UK RPI
/// - **Day Count**: Standard fixed legs compound annually with 1/1 accrual per period
/// - **Business Day**: Payment dates adjusted per calendar; index observation typically unadjusted
///
/// # Validation
///
/// Call [`validate()`](Self::validate) to check structural invariants before pricing.
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
pub struct InflationSwap {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Notional in quote currency
    pub notional: Money,
    /// Start date of indexation
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start_date: Date,
    /// Maturity date
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub maturity: Date,
    /// Fixed real rate (as decimal)
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub fixed_rate: Decimal,
    /// Inflation index identifier (e.g., US-CPI-U)
    pub inflation_index_id: CurveId,
    /// Discount curve identifier (quote currency)
    pub discount_curve_id: CurveId,
    /// Day count for accrual calculation (fixed leg compounding)
    pub day_count: DayCount,
    /// Trade side
    pub side: PayReceive,
    /// Explicit contractual CPI observation lag; missing terms fail reference resolution.
    pub lag: Option<InflationLag>,
    /// Explicit contractual monthly reference-index interpolation; no source default is used.
    pub interpolation: Option<SeriesInterpolation>,
    /// Explicit Base CPI (reference index level at start with lag applied).
    /// If not provided, it will be looked up/calculated from start date.
    pub base_cpi: Option<f64>,
    /// Business day convention for payment date adjustment.
    /// Defaults to `Following` if not specified.
    #[builder(default = BusinessDayConvention::ModifiedFollowing)]
    #[serde(default = "finstack_quant_valuations_core::serde_defaults::bdc_modified_following")]
    pub business_day_convention: BusinessDayConvention,
    /// Holiday calendar identifier for payment date adjustment.
    /// If not specified, payment dates are used unadjusted.
    pub calendar_id: Option<CalendarId>,
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

impl InflationSwap {
    /// Create a canonical example zero-coupon inflation swap (US CPI, 5Y).
    ///
    /// Returns a 5-year USD inflation swap with standard 3-month CPI lag.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::currency::Currency;
        InflationSwap::builder()
            .id(InstrumentId::new("INFLSWAP-USD-5Y"))
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .start_date(time::macros::date!(2024 - 01 - 15))
            .maturity(time::macros::date!(2029 - 01 - 15))
            .fixed_rate(Decimal::try_from(0.02).map_err(|e: rust_decimal::Error| {
                finstack_quant_core::Error::Validation(e.to_string())
            })?)
            .inflation_index_id(CurveId::new("US-CPI"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Act365F)
            .side(PayReceive::Pay)
            .lag(InflationLag::months(3))
            .interpolation(SeriesInterpolation::Linear)
            .business_day_convention(BusinessDayConvention::Following)
            .attributes(Attributes::new())
            .build()
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
        let fixed_rate = self.fixed_rate.as_f64();
        validation::require_with(fixed_rate > -1.0, || {
            format!(
                "InflationSwap fixed_rate ({fixed_rate}) must be greater than -1.0 for geometric compounding"
            )
        })?;
        Ok(())
    }

    fn cpi_value(
        &self,
        curves: &MarketContext,
        _as_of: Date,
        date: Date,
    ) -> finstack_quant_core::Result<f64> {
        let lag = self.lag.ok_or_else(|| {
            finstack_quant_core::Error::Validation("InflationSwap requires an explicit lag".into())
        })?;
        let interpolation = self.interpolation.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "InflationSwap requires explicit reference interpolation".into(),
            )
        })?;
        let index = curves.get_inflation_index(self.inflation_index_id.as_str())?;
        Ok(
            finstack_quant_valuations_core::instruments::pricing::inflation::resolve_reference_inflation(
                curves,
                self.inflation_index_id.as_str(),
                date,
                index.knowledge_date(),
                lag,
                interpolation,
            )?
            .value,
        )
    }

    /// Calculate the projected index ratio I(T_mat - Lag) / I(T_start - Lag).
    ///
    /// The shared resolver applies contractual lag once and uses the explicitly
    /// selected observation snapshot, independently of the valuation date.
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
        let i_start = if let Some(base) = self.base_cpi {
            base
        } else {
            self.cpi_value(curves, discount_base, self.start_date)?
        };

        if !i_start.is_finite() || i_start <= 0.0 {
            return Err(finstack_quant_core::InputError::NonPositiveValue.into());
        }

        let i_maturity_projected = self.cpi_value(curves, discount_base, self.maturity)?;

        Ok(i_maturity_projected / i_start)
    }

    /// Get the adjusted payment date based on business day convention and calendar.
    ///
    /// If no calendar is specified, returns the unadjusted date.
    fn adjusted_payment_date(&self, date: Date) -> finstack_quant_core::Result<Date> {
        let business_day_convention = self.business_day_convention;
        if let Some(ref cal_id) = self.calendar_id {
            use finstack_quant_core::dates::calendar_by_id_strict;
            let cal = calendar_by_id_strict(cal_id)?;
            return finstack_quant_core::dates::adjust(date, business_day_convention, cal);
        }
        // No calendar specified - return unadjusted (common for inflation swaps)
        Ok(date)
    }

    /// Calculate PV of the fixed leg (real rate leg).
    ///
    /// The fixed leg pays `Notional × [(1 + fixed_rate)^τ - 1]` at maturity,
    /// where τ counts the annual contractual periods under 1/1: one for each
    /// full year, plus a short initial stub as the fraction of the year it
    /// covers. Other explicitly selected day counts use their full-term year
    /// fraction.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Discount curve is not found
    /// - Year fraction calculation fails
    pub fn pv_fixed_leg(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        let payment_date = self.adjusted_payment_date(self.maturity)?;
        if as_of >= payment_date {
            return Ok(Money::from((0_i64, self.notional.currency())));
        }
        let disc = market.get_discount(self.discount_curve_id.as_str())?;

        // Use instrument day count for accrual period
        let tau_accrual = self.fixed_accrual()?;

        let fixed_rate = self.fixed_rate.as_f64();
        let fixed_payment = self
            .notional
            .checked_mul_f64((1.0 + fixed_rate).powf(tau_accrual) - 1.0)?;

        // Date-based DF from as_of to payment: correct when the curve base
        // date differs from as_of.
        let df = disc.as_ref().df_between_dates(as_of, payment_date)?;

        fixed_payment.checked_mul_f64(df)
    }

    /// Compounding term τ of the fixed leg in years.
    ///
    /// Under 1/1 every full annual period, rolled back from maturity, counts
    /// exactly one. A swap whose term is not a whole number of years has a
    /// short first period; it counts as its share of the annual period that
    /// ends on the first roll date, so τ is continuous in the maturity date
    /// (a 5-year-and-one-day swap compounds for 5.003 years, not 6).
    fn fixed_accrual(&self) -> finstack_quant_core::Result<f64> {
        if self.day_count != DayCount::OneOne {
            return self.day_count.year_fraction(
                self.start_date,
                self.maturity,
                DayCountContext::default(),
            );
        }
        let periods = finstack_quant_cashflows::builder::build_periods(
            finstack_quant_cashflows::builder::BuildPeriodsParams {
                start: self.start_date,
                end: self.maturity,
                frequency: Tenor::annual(),
                stub: StubKind::ShortFront,
                business_day_convention: BusinessDayConvention::Unadjusted,
                calendar_id: finstack_quant_core::dates::WEEKENDS_ONLY_ID,
                end_of_month: false,
                day_count: DayCount::OneOne,
                payment_lag_days: 0,
                reset_lag_days: None,
                adjust_accrual_dates: false,
                roll_rule: finstack_quant_cashflows::builder::RollRule::None,
            },
        )?;
        let Some(first) = periods.first() else {
            return Ok(0.0);
        };
        let reference_start = first.accrual_end.add_months(-12)?;
        let first_fraction = if first.accrual_start <= reference_start {
            1.0
        } else {
            (first.accrual_end - first.accrual_start).whole_days() as f64
                / (first.accrual_end - reference_start).whole_days() as f64
        };
        Ok((periods.len() - 1) as f64 + first_fraction)
    }

    fn fixed_leg_amount(&self) -> finstack_quant_core::Result<Money> {
        let tau_accrual = self.fixed_accrual()?;
        let fixed_rate = self.fixed_rate.as_f64();
        self.notional
            .checked_mul_f64((1.0 + fixed_rate).powf(tau_accrual) - 1.0)
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
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        let payment_date = self.adjusted_payment_date(self.maturity)?;
        if as_of >= payment_date {
            return Ok(Money::from((0_i64, self.notional.currency())));
        }
        let disc = market.get_discount(self.discount_curve_id.as_str())?;
        let index_ratio = self.projected_index_ratio(market, as_of)?;
        let inflation_payment = self.notional.checked_mul_f64(index_ratio - 1.0)?;

        // Date-based DF from as_of to payment: correct when the curve base
        // date differs from as_of.
        let df = disc.as_ref().df_between_dates(as_of, payment_date)?;

        inflation_payment.checked_mul_f64(df)
    }

    fn inflation_leg_amount(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        let index_ratio = self.projected_index_ratio(curves, as_of)?;
        self.notional.checked_mul_f64(index_ratio - 1.0)
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
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        let index_ratio = self.projected_index_ratio(market, as_of)?;

        if index_ratio <= 0.0 {
            return Err(finstack_quant_core::InputError::NonPositiveValue.into());
        }

        let tau = self.fixed_accrual()?;
        if tau <= 0.0 {
            return Ok(0.0);
        }

        Ok(index_ratio.powf(1.0 / tau) - 1.0)
    }

    /// Raw (unrounded `f64`) present value of the swap from the holder's
    /// perspective, used by finite-difference metrics (convexity, gamma)
    /// where Money quantization noise would be amplified by tiny bump sizes.
    pub fn npv_raw(&self, market: &MarketContext, as_of: Date) -> finstack_quant_core::Result<f64> {
        let payment_date = self.adjusted_payment_date(self.maturity)?;
        if as_of >= payment_date {
            return Ok(0.0);
        }

        let disc = market.get_discount(self.discount_curve_id.as_str())?;
        let df = disc.as_ref().df_between_dates(as_of, payment_date)?;

        let tau_accrual = self.fixed_accrual()?;
        let fixed_rate = self.fixed_rate.as_f64();
        let fixed_pv = self.notional.amount() * ((1.0 + fixed_rate).powf(tau_accrual) - 1.0) * df;

        let index_ratio = self.projected_index_ratio(market, as_of)?;
        let inflation_pv = self.notional.amount() * (index_ratio - 1.0) * df;

        Ok(match self.side {
            PayReceive::Receive => fixed_pv - inflation_pv,
            PayReceive::Pay => inflation_pv - fixed_pv,
        })
    }
}

impl InflationSwapBuilder {}

impl finstack_quant_valuations_core::instruments::Instrument for InflationSwap {
    impl_instrument_base!(
        finstack_quant_valuations_core::pricer::InstrumentType::InflationSwap,
        crate::RATES
    );

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        InflationSwap::validate(self)
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations_core::instruments::MarketDependencies>
    {
        let mut deps = finstack_quant_valuations_core::instruments::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        deps.add_inflation_curve(self.inflation_index_id.clone());
        Ok(deps)
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        let payment_date = self.adjusted_payment_date(self.maturity)?;
        if as_of >= payment_date {
            return Ok(finstack_quant_core::money::Money::from((
                0_i64,
                self.notional.currency(),
            )));
        }

        let pv_fixed = self.pv_fixed_leg(curves, as_of)?;
        let pv_inflation = self.pv_inflation_leg(curves, as_of)?;
        match self.side {
            PayReceive::Receive => pv_fixed.checked_sub(pv_inflation),
            PayReceive::Pay => pv_inflation.checked_sub(pv_fixed),
        }
    }

    fn base_value_raw(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        self.npv_raw(curves, as_of)
    }

    fn base_value_raw_with_currency(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<(f64, finstack_quant_core::currency::Currency)> {
        Ok((self.npv_raw(curves, as_of)?, self.notional.currency()))
    }

    fn includes_valuation_date_cashflows(&self) -> bool {
        false
    }

    fn last_payment_date(
        &self,
        _curves: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<Option<Date>> {
        Ok(Some(self.adjusted_payment_date(self.maturity)?))
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.maturity)
    }

    finstack_quant_valuations_core::impl_focused_pricing_overrides!();
}

impl finstack_quant_cashflows::CashflowScheduleSource for InflationSwap {
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        Ok(Some(self.notional))
    }

    fn raw_cashflow_schedule(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        let payment_date = self.adjusted_payment_date(self.maturity)?;
        let fixed_amount = self.fixed_leg_amount()?;
        let inflation_amount = self.inflation_leg_amount(curves, as_of)?;
        let (fixed_signed, inflation_signed) = match self.side {
            PayReceive::Pay => (-fixed_amount.amount(), inflation_amount.amount()),
            PayReceive::Receive => (fixed_amount.amount(), -inflation_amount.amount()),
        };
        let ccy = self.notional.currency();
        let accrual_factor = self.fixed_accrual()?;
        let fixed_rate = self.fixed_rate.as_f64();
        let inflation_rate = (accrual_factor > 0.0)
            .then_some(inflation_amount.amount() / self.notional.amount() / accrual_factor);
        let fixed_flow = CashFlowParams::new(
            payment_date,
            None,
            Money::from_model_value(fixed_signed, ccy)?,
            CFKind::Fixed,
            accrual_factor,
            Some(fixed_rate),
        )
        .with_accrual(CashFlowAccrual::new(
            self.start_date,
            self.maturity,
            self.day_count,
            None,
            finstack_quant_core::dates::DayCountContextState::try_new(
                None,
                None,
                None,
                None,
                Some(false),
            )?,
        )?)
        .build()?;
        let inflation_flow = CashFlowParams::new(
            payment_date,
            Some(self.start_date),
            Money::from_model_value(inflation_signed, ccy)?,
            CFKind::InflationCoupon,
            accrual_factor,
            inflation_rate,
        )
        .with_accrual(CashFlowAccrual::new(
            self.start_date,
            self.maturity,
            self.day_count,
            inflation_rate,
            finstack_quant_core::dates::DayCountContextState::try_new(
                None,
                None,
                None,
                None,
                Some(false),
            )?,
        )?)
        .build()?;
        let schedule = finstack_quant_cashflows::schedule_from_classified_flows(
            vec![fixed_flow, inflation_flow],
            self.day_count,
            finstack_quant_cashflows::ScheduleBuildOpts {
                notional_hint: Some(self.notional),
                meta: finstack_quant_cashflows::builder::CashFlowMeta {
                    representation:
                        finstack_quant_cashflows::builder::CashflowRepresentation::Projected,
                    ..Default::default()
                },
            },
        );
        Ok(schedule.with_representation(
            finstack_quant_cashflows::builder::CashflowRepresentation::Projected,
        ))
    }
}
