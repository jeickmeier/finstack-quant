//! Year-on-year (YoY) Inflation Swap instrument and pricing implementation.

use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_core::cashflow::CashFlowAccrual;
use finstack_quant_core::cashflow::{CFKind, CashFlow, CashFlowParams};
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, DayCountContext, StubKind, Tenor,
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

/// Year-on-year (YoY) Inflation Swap instrument.
///
/// Pays periodic inflation rates (CPI ratios over each period) versus a fixed rate.
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
pub struct YoYInflationSwap {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Notional in quote currency
    pub notional: Money,
    /// Start date of the first accrual period
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
    /// Fixed rate (decimal)
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
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
    /// Explicit contractual CPI observation lag; missing terms fail reference resolution.
    pub lag: Option<InflationLag>,
    /// Explicit contractual monthly reference-index interpolation; no source default is used.
    pub interpolation: Option<SeriesInterpolation>,
    /// Observed contractual reference CPI at start, with lag and interpolation
    /// already applied. If absent, start CPI must resolve from market observations.
    pub base_cpi: Option<f64>,
    /// Business day convention for payment date adjustment.
    #[builder(default = BusinessDayConvention::ModifiedFollowing)]
    #[serde(default = "finstack_quant_valuations_core::serde_defaults::bdc_modified_following")]
    pub business_day_convention: BusinessDayConvention,
    /// Holiday calendar identifier for payment date adjustment.
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

impl YoYInflationSwap {
    /// Create a canonical example USD 5Y YoY inflation swap (US-CPI, annual payments).
    ///
    /// Returns a 5-year pay-fixed YoY inflation swap with 2.5% fixed rate,
    /// $1M notional, annual frequency, and standard 3-month CPI lag.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::currency::Currency;

        YoYInflationSwap::builder()
            .id(InstrumentId::new("YOYSWAP-USD-5Y"))
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .start_date(time::macros::date!(2024 - 01 - 15))
            .maturity(time::macros::date!(2029 - 01 - 15))
            .fixed_rate(Decimal::try_from(0.025).map_err(|e: rust_decimal::Error| {
                finstack_quant_core::Error::Validation(e.to_string())
            })?)
            .frequency(Tenor::annual())
            .inflation_index_id(CurveId::new("US-CPI"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Act365F)
            .side(PayReceive::Pay)
            .lag(InflationLag::months(3))
            .interpolation(SeriesInterpolation::Linear)
            .business_day_convention(BusinessDayConvention::ModifiedFollowing)
            .attributes(Attributes::new())
            .build()
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
            finstack_quant_core::Error::Validation(
                "frequency count must be greater than zero".to_string(),
            ),
        )?;
        if self
            .base_cpi
            .is_some_and(|cpi| !cpi.is_finite() || cpi <= 0.0)
        {
            return Err(finstack_quant_core::InputError::NonPositiveValue.into());
        }
        Ok(())
    }

    fn cpi_value(
        &self,
        curves: &MarketContext,
        _as_of: Date,
        date: Date,
    ) -> finstack_quant_core::Result<f64> {
        if date == self.start_date
            && let Some(base) = self.base_cpi
        {
            if !base.is_finite() || base <= 0.0 {
                return Err(finstack_quant_core::InputError::NonPositiveValue.into());
            }
            return Ok(base);
        }
        let lag = self.lag.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "YoYInflationSwap requires an explicit lag".into(),
            )
        })?;
        let interpolation = self.interpolation.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "YoYInflationSwap requires explicit reference interpolation".into(),
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

    fn schedule(&self) -> finstack_quant_core::Result<Vec<(Date, Date, Date)>> {
        let business_day_convention = self.business_day_convention;
        let periods = finstack_quant_cashflows::builder::build_periods(
            finstack_quant_cashflows::builder::BuildPeriodsParams {
                start: self.start_date,
                end: self.maturity,
                frequency: self.frequency,
                stub: StubKind::None,
                business_day_convention,
                calendar_id: self
                    .calendar_id
                    .as_deref()
                    .unwrap_or(finstack_quant_core::dates::WEEKENDS_ONLY_ID),
                end_of_month: false,
                day_count: self.day_count,
                payment_lag_days: 0,
                reset_lag_days: None,
                adjust_accrual_dates: false,
                roll_rule: finstack_quant_cashflows::builder::RollRule::None,
            },
        )?;

        if periods.is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "inflation swap schedule produced no periods".to_string(),
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
    pub fn npv_raw(&self, market: &MarketContext, as_of: Date) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        let disc = market.get_discount(self.discount_curve_id.as_str())?;
        let mut pv = 0.0_f64;

        for (start, end, pay) in self.schedule()? {
            if pay <= as_of {
                continue;
            }
            let accrual = self
                .day_count
                .year_fraction(start, end, DayCountContext::default())?;

            let cpi_start = self.cpi_value(market, as_of, start)?;
            if cpi_start <= 0.0 {
                return Err(finstack_quant_core::InputError::NonPositiveValue.into());
            }
            let cpi_end = self.cpi_value(market, as_of, end)?;

            let inflation_leg = self.notional.amount() * (cpi_end / cpi_start - 1.0);
            let fixed_rate = self.fixed_rate.as_f64();
            let fixed_leg = self.notional.amount() * fixed_rate * accrual;

            let net = match self.side {
                PayReceive::Pay => inflation_leg - fixed_leg,
                PayReceive::Receive => fixed_leg - inflation_leg,
            };

            // Date-based DF from as_of to payment: correct when the curve base
            // date differs from as_of.
            let df = disc.as_ref().df_between_dates(as_of, pay)?;
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
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        let disc = market.get_discount(self.discount_curve_id.as_str())?;

        let mut sum_infl_pv = 0.0_f64;
        let mut sum_annuity = 0.0_f64;

        for (start, end, pay) in self.schedule()? {
            if pay <= as_of {
                continue;
            }
            let accrual = self
                .day_count
                .year_fraction(start, end, DayCountContext::default())?;

            let cpi_start = self.cpi_value(market, as_of, start)?;
            if cpi_start <= 0.0 {
                return Err(finstack_quant_core::InputError::NonPositiveValue.into());
            }
            let cpi_end = self.cpi_value(market, as_of, end)?;

            let df = disc.as_ref().df_between_dates(as_of, pay)?;

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
    ) -> finstack_quant_core::Result<Vec<CashFlow>> {
        self.validate()?;
        let mut flows = Vec::new();
        let fixed_rate = self.fixed_rate.as_f64();

        for (start, end, pay) in self.schedule()? {
            if pay < as_of {
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
            let inflation_rate = (cpi_end / cpi_start - 1.0) / accrual;
            let (fixed_signed, inflation_signed) = match self.side {
                PayReceive::Pay => (-fixed_leg, inflation_leg),
                PayReceive::Receive => (fixed_leg, -inflation_leg),
            };
            let fixed_flow = CashFlowParams::new(
                pay,
                None,
                Money::from_model_value(fixed_signed, self.notional.currency())?,
                CFKind::Fixed,
                accrual,
                Some(fixed_rate),
            )
            .with_accrual(CashFlowAccrual::new(
                start,
                end,
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
                pay,
                Some(start),
                Money::from_model_value(inflation_signed, self.notional.currency())?,
                CFKind::InflationCoupon,
                accrual,
                Some(inflation_rate),
            )
            .with_accrual(CashFlowAccrual::new(
                start,
                end,
                self.day_count,
                Some(inflation_rate),
                finstack_quant_core::dates::DayCountContextState::try_new(
                    None,
                    None,
                    None,
                    None,
                    Some(false),
                )?,
            )?)
            .build()?;
            flows.push(fixed_flow);
            flows.push(inflation_flow);
        }

        Ok(flows)
    }
}

impl YoYInflationSwapBuilder {}

impl finstack_quant_valuations_core::instruments::Instrument for YoYInflationSwap {
    impl_instrument_base!(
        finstack_quant_valuations_core::pricer::InstrumentType::YoYInflationSwap,
        crate::RATES
    );

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
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
        let pv = self.npv_raw(curves, as_of)?;
        finstack_quant_core::money::Money::from_model_value(pv, self.notional.currency())
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
        Ok(self.schedule()?.last().map(|(_, _, payment)| *payment))
    }

    fn expiry(&self) -> Option<Date> {
        Some(self.maturity)
    }

    finstack_quant_valuations_core::impl_focused_pricing_overrides!();
}

impl finstack_quant_cashflows::CashflowScheduleSource for YoYInflationSwap {
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        Ok(Some(self.notional))
    }

    fn raw_cashflow_schedule(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        let flows = self.signed_period_flows(curves, as_of)?;
        let schedule = finstack_quant_cashflows::schedule_from_classified_flows(
            flows,
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
