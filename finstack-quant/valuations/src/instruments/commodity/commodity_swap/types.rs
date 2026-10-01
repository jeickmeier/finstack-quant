//! Commodity swap types and implementations.
//!
//! Defines the `CommoditySwap` instrument for fixed-for-floating commodity
//! price exchange contracts. One party pays a fixed price per unit while
//! the other pays a floating price based on an index.

use crate::cashflow::builder::CashFlowSchedule;
use crate::cashflow::primitives::{CFKind, CashFlow};
use crate::impl_instrument_base;
use crate::instruments::common_impl::parameters::legs::PayReceive;
use crate::instruments::common_impl::parameters::CommodityUnderlyingParams;
use crate::instruments::common_impl::traits::Attributes;
use finstack_quant_core::cashflow::CashFlowAccrual;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{calendar_by_id, BusinessDayConvention, Date, DateExt, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CalendarId, CurveId, InstrumentId};
use finstack_quant_core::Result;

/// Commodity swap (fixed-for-floating commodity price exchange).
///
/// One party pays a fixed price per unit, the other pays a floating price
/// determined by an index or average of spot prices over the period.
///
/// # Pricing
///
/// Fixed leg: ∑ Q × P_fixed × DF(t_i)
/// Floating leg: ∑ Q × E[P_float(t_i)] × DF(t_i)
///
/// For a payer of fixed:
/// NPV = Floating leg PV - Fixed leg PV
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::commodity::commodity_swap::CommoditySwap;
/// use finstack_quant_valuations::instruments::CommodityUnderlyingParams;
/// use finstack_quant_valuations::instruments::PayReceive;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::{Date, BusinessDayConvention, Tenor, TenorUnit};
/// use finstack_quant_core::types::{CurveId, InstrumentId};
/// use time::Month;
///
/// let swap = CommoditySwap::builder()
///     .id(InstrumentId::new("NG-SWAP-2025"))
///     .underlying(CommodityUnderlyingParams::new("Energy", "NG", "MMBTU", Currency::USD))
///     .quantity(10000.0)
///     .fixed_price(3.50)
///     .forward_curve_id(CurveId::new("NG-SPOT-AVG"))
///     .side(PayReceive::Pay)
///     .start_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
///     .maturity(Date::from_calendar_date(2025, Month::December, 31).unwrap())
///     .frequency(Tenor::new(1, TenorUnit::Months).expect("valid tenor fixture"))
///     .discount_curve_id(CurveId::new("USD-OIS"))
///     .build()
///     .expect("Valid swap");
/// ```
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "json-schema", schemars(deny_unknown_fields))]
pub struct CommoditySwap {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Underlying commodity parameters (type, ticker, unit, currency).
    #[serde(flatten)]
    pub underlying: CommodityUnderlyingParams,
    /// Notional quantity per period.
    #[serde(
        serialize_with = "finstack_quant_core::wire::serialize_positive_f64",
        deserialize_with = "finstack_quant_core::wire::deserialize_positive_f64"
    )]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::PositiveF64Wire")
    )]
    pub quantity: f64,
    /// Fixed price per commodity unit, in the notional currency (finite).
    pub fixed_price: f64,
    /// Commodity forward `PriceCurve` that projects the floating-leg price observations.
    pub forward_curve_id: CurveId,
    /// Direction of the swap: Pay means paying the fixed price leg,
    /// Receive means receiving the fixed price leg.
    #[serde(default = "default_pay_receive")]
    pub side: PayReceive,
    /// Start date of the swap.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start_date: Date,
    /// End date of the swap.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub maturity: Date,
    /// Payment frequency as a Tenor.
    pub frequency: Tenor,
    /// Optional calendar ID for date adjustments.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar_id: Option<CalendarId>,
    /// Business day convention for payment-schedule date adjustments.
    ///
    /// Only applied when `calendar_id` is set and resolves to a registered
    /// holiday calendar; without a calendar the schedule dates are left
    /// unadjusted.
    #[builder(default = BusinessDayConvention::ModifiedFollowing)]
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub business_day_convention: BusinessDayConvention,
    /// Discount curve ID.
    pub discount_curve_id: CurveId,
    /// Optional index lag in **calendar days**: the floating-leg averaging
    /// window is shifted back by exactly this many calendar days (no
    /// business-day adjustment of the shifted window endpoints).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_lag_days: Option<i32>,
    /// Realized floating-index fixings as `(date, price)` pairs.
    ///
    /// Floating-leg observations with date strictly before the valuation date
    /// read from this store; a missing past fixing is an error — no silent
    /// substitution of today's spot . Observations on or
    /// after the valuation date project from the price curve.
    #[builder(default)]
    #[serde(default)]
    #[serde(with = "finstack_quant_core::wire::dated_f64_values")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<(finstack_quant_core::wire::DateWire, f64)>")
    )]
    pub past_fixings: Vec<(Date, f64)>,
    /// Instrument-owned pricing inputs.
    #[builder(default)]
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
    #[serde(default)]
    #[builder(default)]
    pub attributes: Attributes,
    /// Rejects unknown JSON fields despite the flattened underlying.
    #[serde(flatten)]
    #[cfg_attr(feature = "json-schema", schemars(skip))]
    #[builder(default)]
    pub(crate) unknown_fields: finstack_quant_core::serde_guard::UnknownFieldGuard,
}

fn default_pay_receive() -> PayReceive {
    PayReceive::Pay
}

impl CommoditySwap {
    fn validate(&self) -> Result<()> {
        use crate::instruments::common_impl::validation;

        self.underlying.validate("CommoditySwap")?;
        validation::validate_f64_positive(self.quantity, "CommoditySwap quantity")?;
        validation::validate_f64_finite(self.fixed_price, "CommoditySwap fixed_price")?;
        validation::validate_date_range_strict(self.start_date, self.maturity, "commodity swap")?;
        if self.frequency.count() == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "CommoditySwap frequency must contain at least one tenor unit".to_string(),
            ));
        }
        if let Some(calendar_id) = self.calendar_id.as_deref() {
            calendar_by_id(calendar_id).ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "CommoditySwap '{}' references unknown calendar_id '{calendar_id}'",
                    self.id
                ))
            })?;
        }
        let mut seen = std::collections::BTreeSet::new();
        for (date, value) in &self.past_fixings {
            if !seen.insert(*date) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CommoditySwap '{}' has duplicate past fixing for date {date}",
                    self.id
                )));
            }
            validation::validate_f64_finite(*value, "CommoditySwap realized-fixing value")?;
        }
        Ok(())
    }

    /// Create a canonical example commodity swap for testing and documentation.
    ///
    /// Returns a natural gas swap with monthly settlements.
    pub fn example() -> finstack_quant_core::Result<Self> {
        Self::builder()
            .id(InstrumentId::new("NG-SWAP-2025"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "NG",
                "MMBTU",
                Currency::USD,
            ))
            .quantity(10000.0)
            .fixed_price(3.50)
            .forward_curve_id(CurveId::new("NG-SPOT-AVG"))
            .side(PayReceive::Pay)
            .start_date(time::macros::date!(2025 - 01 - 01))
            .maturity(time::macros::date!(2025 - 12 - 31))
            .frequency(Tenor::monthly())
            .business_day_convention(BusinessDayConvention::ModifiedFollowing)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(
                Attributes::new()
                    .with_tag("energy")
                    .with_meta("sector", "natural-gas"),
            )
            .build()
    }

    /// Calculate the fixed-price leg's present value before applying the pay/receive side.
    ///
    /// # Arguments
    /// * `market` - Market snapshot containing the quote-currency discount curve.
    /// * `as_of` - Valuation date; payments on this date remain outstanding.
    pub fn fixed_leg_pv(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
        let disc = market.get_discount(self.discount_curve_id.as_str())?;
        let mut pv = 0.0;
        for period in self
            .periods()?
            .into_iter()
            .filter(|p| p.payment_date >= as_of)
        {
            pv += self.quantity
                * self.fixed_price
                * disc.df_between_dates(as_of, period.payment_date)?;
        }
        Ok(pv)
    }

    /// Calculate the floating-price leg's present value before applying the pay/receive side.
    ///
    /// Contractual observation windows remain unadjusted when payment dates roll.
    /// Observations strictly before valuation require instrument-owned past fixings;
    /// later observations project from the commodity price curve.
    ///
    /// # Arguments
    /// * `market` - Market snapshot containing the commodity price and discount curves.
    /// * `as_of` - Valuation date; payments on this date remain outstanding.
    pub fn floating_leg_pv(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
        let disc = market.get_discount(self.discount_curve_id.as_str())?;
        let periods = self
            .periods()?
            .into_iter()
            .filter(|p| p.payment_date >= as_of);
        let mut pv = 0.0;
        for (period, price) in self.projected_period_prices(market, as_of, periods)? {
            pv += self.quantity * price * disc.df_between_dates(as_of, period.payment_date)?;
        }
        Ok(pv)
    }

    fn periods(&self) -> Result<Vec<crate::cashflow::builder::periods::SchedulePeriod>> {
        self.validate()?;
        super::super::averaging::commodity_periods(
            self.start_date,
            self.maturity,
            self.frequency,
            self.calendar_id.as_deref(),
            self.business_day_convention,
        )
    }

    /// Generate adjusted payment dates, retaining contractual averaging dates internally.
    pub fn payment_schedule(&self) -> Result<Vec<Date>> {
        Ok(self
            .periods()?
            .into_iter()
            .map(|period| period.payment_date)
            .collect())
    }

    // Prepare curve, calendar and historical fixings once for the requested periods.
    fn projected_period_prices(
        &self,
        market: &MarketContext,
        as_of: Date,
        periods: impl IntoIterator<Item = crate::cashflow::builder::periods::SchedulePeriod>,
    ) -> Result<Vec<(crate::cashflow::builder::periods::SchedulePeriod, f64)>> {
        let price_curve = market.get_price_curve(self.forward_curve_id.as_str())?;
        let calendar = self
            .calendar_id
            .as_deref()
            .map(|id| {
                calendar_by_id(id).ok_or_else(|| {
                    finstack_quant_core::Error::Validation(format!(
                        "CommoditySwap '{}' references unknown calendar_id '{id}'",
                        self.id
                    ))
                })
            })
            .transpose()?;
        let fixings: std::collections::BTreeMap<Date, f64> =
            self.past_fixings.iter().copied().collect();
        let get_price = |date: Date| -> Result<f64> {
            if date < as_of {
                fixings.get(&date).copied().ok_or_else(|| {
                    finstack_quant_core::Error::Validation(format!(
                        "CommoditySwap '{}' is missing a past fixing for past observation date {date} (as_of {as_of}); past floating-leg observations must be supplied via past_fixings",
                        self.id
                    ))
                })
            } else {
                price_curve.price_on_date(date)
            }
        };
        let is_business_day = |date: Date| -> bool {
            !date.is_weekend() && calendar.is_none_or(|cal| cal.is_business_day(date))
        };
        let lag = time::Duration::days(i64::from(self.index_lag_days.unwrap_or(0)));
        periods
            .into_iter()
            .map(|period| {
                let price = super::super::averaging::business_day_average_price(
                    get_price,
                    is_business_day,
                    period.accrual_start - lag,
                    period.accrual_end - lag,
                    period.accrual_end == self.maturity,
                )?;
                Ok((period, price))
            })
            .collect()
    }

    fn classified_flow(
        &self,
        period: &crate::cashflow::builder::periods::SchedulePeriod,
        amount: f64,
        kind: CFKind,
    ) -> Result<CashFlow> {
        Ok(CashFlow::new(
            period.payment_date,
            (kind == CFKind::FloatReset).then_some(period.accrual_start),
            Money::new(amount, self.underlying.currency)?,
            kind,
            period.accrual_year_fraction,
            None,
        )
        .with_accrual(CashFlowAccrual {
            coupon_period: None,
            end_is_termination_date: period.accrual_end == self.maturity,
            calendar_id: self.calendar_id.as_deref().map(str::to_owned),
            start: period.accrual_start,
            end: period.accrual_end,
            day_count: finstack_quant_core::dates::DayCount::Act365F,
            projected_index_rate: None,
        }))
    }
}

impl crate::instruments::common_impl::traits::Instrument for CommoditySwap {
    impl_instrument_base!(crate::pricer::InstrumentType::CommoditySwap);

    fn validate_invariants(&self) -> Result<()> {
        self.validate()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        deps.add_forward_curve(self.forward_curve_id.clone());
        Ok(deps)
    }

    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        let disc = market.get_discount(self.discount_curve_id.as_str())?;
        let periods = self
            .periods()?
            .into_iter()
            .filter(|p| p.payment_date >= as_of);
        let mut fixed_pv = 0.0;
        let mut floating_pv = 0.0;
        for (period, floating_price) in self.projected_period_prices(market, as_of, periods)? {
            let df = disc.df_between_dates(as_of, period.payment_date)?;
            fixed_pv += self.quantity * self.fixed_price * df;
            floating_pv += self.quantity * floating_price * df;
        }
        let npv = match self.side {
            PayReceive::Pay => floating_pv - fixed_pv,
            PayReceive::Receive => fixed_pv - floating_pv,
        };

        finstack_quant_core::money::Money::new(npv, self.underlying.currency)
    }

    fn effective_start_date(&self) -> Option<Date> {
        Some(self.start_date)
    }

    fn last_payment_date(&self, _curves: &MarketContext, _as_of: Date) -> Result<Option<Date>> {
        Ok(self.periods()?.last().map(|period| period.payment_date))
    }

    fn expiry(&self) -> Option<Date> {
        Some(self.maturity)
    }

    crate::impl_focused_pricing_overrides!();
}

impl finstack_quant_cashflows::CashflowScheduleSource for CommoditySwap {
    fn raw_cashflow_schedule(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        let periods = self.periods()?;
        let mut flows = Vec::with_capacity(periods.len() * 2);
        let fixed_sign = match self.side {
            PayReceive::Pay => -1.0,
            PayReceive::Receive => 1.0,
        };
        for (period, floating_price) in self.projected_period_prices(market, as_of, periods)? {
            flows.push(self.classified_flow(
                &period,
                fixed_sign * self.quantity * self.fixed_price,
                CFKind::Fixed,
            )?);
            flows.push(self.classified_flow(
                &period,
                -fixed_sign * self.quantity * floating_price,
                CFKind::FloatReset,
            )?);
        }
        let schedule = crate::cashflow::traits::schedule_from_classified_flows(
            flows,
            finstack_quant_core::dates::DayCount::Act365F,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: Some(Money::from((0_i64, self.underlying.currency))),
                meta: crate::cashflow::builder::CashFlowMeta {
                    representation: crate::cashflow::builder::CashflowRepresentation::Projected,
                    ..Default::default()
                },
            },
        );
        Ok(schedule
            .with_representation(crate::cashflow::builder::CashflowRepresentation::Projected))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::CashflowProvider;
    use crate::instruments::common_impl::parameters::CommodityUnderlyingParams;
    use crate::instruments::common_impl::traits::Instrument;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, PriceCurve};
    use time::Month;

    fn test_market(as_of: Date) -> MarketContext {
        // Create discount curve
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (0.5, 0.975), (1.0, 0.95), (2.0, 0.90)])
            .build()
            .expect("Valid discount curve");

        // Create price curve for NG forward prices (slight contango)
        let price_curve = PriceCurve::builder("NG-SPOT-AVG")
            .base_date(as_of)
            .spot_price(3.50)
            .knots([
                (0.0, 3.50),
                (0.25, 3.55),
                (0.5, 3.60),
                (0.75, 3.65),
                (1.0, 3.70),
            ])
            .build()
            .expect("Valid price curve");

        MarketContext::new().insert(disc).insert(price_curve)
    }

    #[test]
    fn payment_adjustment_keeps_contractual_commodity_observations() {
        use crate::instruments::commodity::commodity_swaption::CommoditySwaption;
        use time::macros::date;
        let as_of = date!(2025 - 01 - 31);
        let maturity = date!(2025 - 05 - 31);
        let payment = date!(2025 - 06 - 02);
        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (1.0, 1.0)])
                    .build()
                    .expect("discount curve"),
            )
            .insert(
                PriceCurve::builder("NG-SPOT-AVG")
                    .base_date(as_of)
                    .knots([
                        (0.0, 0.0),
                        (121.0 / 365.0, 0.0),
                        (122.0 / 365.0, 1000.0),
                        (1.0, 1000.0),
                    ])
                    .build()
                    .expect("price curve"),
            );
        let mut swap = CommoditySwap::example().expect("example");
        swap.start_date = as_of;
        swap.maturity = maturity;
        swap.quantity = 1.0;
        swap.fixed_price = 0.0;
        swap.calendar_id = Some("weekends_only".into());
        swap.business_day_convention = BusinessDayConvention::Unadjusted;
        assert!(swap.value_raw(&market, as_of).expect("unadjusted PV").abs() < 1e-12);
        swap.business_day_convention = BusinessDayConvention::Following;
        assert!(swap.value_raw(&market, as_of).expect("adjusted PV").abs() < 1e-12);
        assert_eq!(
            swap.last_payment_date(&market, as_of)
                .expect("last payment"),
            Some(payment)
        );
        let schedule = swap.cashflow_schedule(&market, as_of).expect("cashflows");
        let final_flow = schedule.get_flows().last().expect("last flow");
        assert_eq!(final_flow.date, payment);
        assert_eq!(final_flow.accrual.as_ref().expect("accrual").end, maturity);
        assert!(final_flow.amount.amount().abs() < 1e-12);

        let mut swaption = CommoditySwaption::example().expect("swaption");
        swaption.expiry = as_of;
        swaption.underlying_start_date = as_of;
        swaption.underlying_maturity = maturity;
        swaption.forward_curve_id = swap.forward_curve_id;
        swaption.calendar_id = swap.calendar_id;
        swaption.business_day_convention = BusinessDayConvention::Following;
        assert_eq!(
            swaption
                .swap_payment_schedule()
                .expect("swaption payments")
                .last(),
            Some(&payment)
        );
        assert!(
            swaption
                .forward_swap_rate(&market, as_of)
                .expect("swap forward")
                .abs()
                < 1e-12
        );
    }

    #[test]
    fn test_commodity_swap_creation() {
        let swap = CommoditySwap::builder()
            .id(InstrumentId::new("TEST-SWAP"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "CL",
                "BBL",
                Currency::USD,
            ))
            .quantity(1000.0)
            .fixed_price(70.0)
            .forward_curve_id(CurveId::new("CL-AVG"))
            .side(PayReceive::Pay)
            .start_date(Date::from_calendar_date(2025, Month::January, 1).expect("valid date"))
            .maturity(Date::from_calendar_date(2025, Month::June, 30).expect("valid date"))
            .frequency(
                Tenor::new(1, finstack_quant_core::dates::TenorUnit::Months)
                    .expect("valid tenor fixture"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(Attributes::new())
            .build()
            .expect("should build");

        assert_eq!(swap.id.as_str(), "TEST-SWAP");
        assert_eq!(swap.underlying.underlying_ticker, "CL");
        assert_eq!(swap.quantity, 1000.0);
        assert_eq!(swap.fixed_price, 70.0);
        assert_eq!(swap.side, PayReceive::Pay);
    }

    #[test]
    fn validation_rejects_invalid_schedule_and_fixing_inputs() {
        let mut swap = CommoditySwap::example().expect("example");
        swap.quantity = 0.0;
        assert!(swap.validate_for_pricing().is_err());

        swap.quantity = 1.0;
        swap.start_date = swap.maturity;
        assert!(swap.validate_for_pricing().is_err());

        swap.start_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        swap.past_fixings = vec![
            (
                Date::from_calendar_date(2025, Month::February, 3).expect("date"),
                3.5,
            ),
            (
                Date::from_calendar_date(2025, Month::February, 3).expect("date"),
                3.6,
            ),
        ];
        assert!(swap.validate_for_pricing().is_err());
    }

    #[test]
    fn test_commodity_swap_example() {
        let swap = CommoditySwap::example().expect("example");
        assert_eq!(swap.id.as_str(), "NG-SWAP-2025");
        assert_eq!(swap.underlying.commodity_type, "Energy");
        assert_eq!(swap.underlying.underlying_ticker, "NG");
        assert!(swap.attributes.has_tag("energy"));
    }

    #[test]
    fn test_commodity_swap_npv_at_market() {
        // When fixed price equals expected floating average, NPV should be ~0
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let market = test_market(as_of);

        let swap = CommoditySwap::builder()
            .id(InstrumentId::new("AT-MARKET-SWAP"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "NG",
                "MMBTU",
                Currency::USD,
            ))
            .quantity(10000.0)
            .fixed_price(3.50) // Same as spot
            .forward_curve_id(CurveId::new("NG-SPOT-AVG"))
            .side(PayReceive::Pay)
            .start_date(as_of)
            .maturity(Date::from_calendar_date(2025, Month::June, 30).expect("valid date"))
            .frequency(
                Tenor::new(1, finstack_quant_core::dates::TenorUnit::Months)
                    .expect("valid tenor fixture"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("should build");

        let npv = swap.value(&market, as_of).expect("should price");

        // In contango (forward > spot), pay-fixed should receive more on floating leg
        // So NPV should be slightly positive
        assert!(
            npv.amount() > 0.0,
            "Pay-fixed swap in contango should have positive NPV, got {}",
            npv.amount()
        );
    }

    #[test]
    fn test_commodity_swap_pay_receive_symmetry() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let market = test_market(as_of);

        let pay_fixed = CommoditySwap::builder()
            .id(InstrumentId::new("PAY-FIXED"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "NG",
                "MMBTU",
                Currency::USD,
            ))
            .quantity(10000.0)
            .fixed_price(3.55)
            .forward_curve_id(CurveId::new("NG-SPOT-AVG"))
            .side(PayReceive::Pay)
            .start_date(as_of)
            .maturity(Date::from_calendar_date(2025, Month::June, 30).expect("valid date"))
            .frequency(
                Tenor::new(1, finstack_quant_core::dates::TenorUnit::Months)
                    .expect("valid tenor fixture"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("should build");

        let receive_fixed = CommoditySwap::builder()
            .id(InstrumentId::new("RECEIVE-FIXED"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "NG",
                "MMBTU",
                Currency::USD,
            ))
            .quantity(10000.0)
            .fixed_price(3.55)
            .forward_curve_id(CurveId::new("NG-SPOT-AVG"))
            .side(PayReceive::Receive) // Receiving fixed
            .start_date(as_of)
            .maturity(Date::from_calendar_date(2025, Month::June, 30).expect("valid date"))
            .frequency(
                Tenor::new(1, finstack_quant_core::dates::TenorUnit::Months)
                    .expect("valid tenor fixture"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("should build");

        let pay_npv = pay_fixed.value(&market, as_of).expect("should price");
        let recv_npv = receive_fixed.value(&market, as_of).expect("should price");

        // Offsetting swaps should net to zero
        let net = pay_npv.amount() + recv_npv.amount();
        assert!(
            net.abs() < 1e-10,
            "Pay + Receive NPV should sum to 0, got {}",
            net
        );
    }

    #[test]
    fn test_commodity_swap_cashflows() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let market = test_market(as_of);

        let swap = CommoditySwap::builder()
            .id(InstrumentId::new("CASHFLOW-TEST"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "NG",
                "MMBTU",
                Currency::USD,
            ))
            .quantity(10000.0)
            .fixed_price(3.50)
            .forward_curve_id(CurveId::new("NG-SPOT-AVG"))
            .side(PayReceive::Pay)
            .start_date(as_of)
            .maturity(Date::from_calendar_date(2025, Month::March, 31).expect("valid date"))
            .frequency(
                Tenor::new(1, finstack_quant_core::dates::TenorUnit::Months)
                    .expect("valid tenor fixture"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("should build");

        let flows = swap
            .dated_cashflows(&market, as_of)
            .expect("should get flows");
        let schedule = swap
            .cashflow_schedule(&market, as_of)
            .expect("classified schedule");
        assert_eq!(
            schedule
                .get_flows()
                .iter()
                .filter(|flow| flow.kind == CFKind::Fixed)
                .count(),
            3
        );
        assert_eq!(
            schedule
                .get_flows()
                .iter()
                .filter(|flow| flow.kind == CFKind::FloatReset)
                .count(),
            3
        );
        assert!(schedule
            .get_flows()
            .iter()
            .all(|flow| flow.accrual.is_some()));

        // The canonical contractual schedule emits both fixed and floating legs.
        assert_eq!(
            flows.len(),
            6,
            "Expected fixed and floating rows for 3 payments"
        );

        let mut net_by_date = std::collections::BTreeMap::new();
        for (date, cf) in &flows {
            *net_by_date.entry(*date).or_insert(0.0) += cf.amount();
        }
        assert_eq!(net_by_date.len(), 3, "Expected 3 monthly payment dates");

        // Net cashflows should still be positive in contango (floating > fixed).
        for (date, net) in net_by_date {
            assert!(
                net > 0.0,
                "Net cashflow on {} should be positive in contango, got {}",
                date,
                net
            );
        }
    }

    #[test]
    fn test_commodity_swap_instrument_trait() {
        use crate::instruments::common_impl::traits::Instrument;

        let swap = CommoditySwap::example().expect("example");

        assert_eq!(swap.id(), "NG-SWAP-2025");
        assert_eq!(swap.key(), crate::pricer::InstrumentType::CommoditySwap);
    }

    #[test]
    fn test_commodity_swap_market_dependencies() {
        let swap = CommoditySwap::example().expect("example");
        let deps = swap
            .market_dependencies()
            .expect("market_dependencies")
            .curves;

        assert_eq!(deps.discount_curves.len(), 1);
        assert_eq!(deps.forward_curves.len(), 1);
    }

    #[test]
    fn test_commodity_swap_serde_roundtrip() {
        let swap = CommoditySwap::example().expect("example");
        let json = serde_json::to_string(&swap).expect("serialize");
        let deserialized: CommoditySwap = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(swap.id.as_str(), deserialized.id.as_str());
        assert_eq!(
            swap.underlying.underlying_ticker,
            deserialized.underlying.underlying_ticker
        );
        assert_eq!(swap.fixed_price, deserialized.fixed_price);
    }

    /// W-11: a floating-leg observation window that starts before the curve
    /// base date must propagate the curve-lookup error rather than silently
    /// substituting the curve spot for the pre-base days. Silently mixing spot
    /// into the daily average would hide a misconfigured (too-short) curve.
    #[test]
    fn w11_floating_leg_propagates_curve_lookup_error_for_straddling_period() {
        // Curve base is well after the swap's first observation period, so the
        // first period's averaging window straddles the curve base.
        let curve_base = Date::from_calendar_date(2025, Month::March, 15).expect("date");
        let price_curve = PriceCurve::builder("NG-SPOT-AVG")
            .base_date(curve_base)
            .spot_price(3.50)
            .knots([(0.0, 3.50), (1.0, 3.70)])
            .build()
            .expect("price curve");
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(Date::from_calendar_date(2025, Month::January, 1).expect("date"))
            .knots([(0.0, 1.0), (2.0, 0.90)])
            .build()
            .expect("discount curve");
        let market = MarketContext::new().insert(disc).insert(price_curve);

        // Swap starts 2025-01-01; the first monthly period (Jan-Feb) is fully
        // before the curve base — but a period straddling the base exercises
        // the pre-base lookup inside the averaging loop.
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let swap = CommoditySwap::builder()
            .id(InstrumentId::new("W11-SWAP"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "NG",
                "MMBTU",
                Currency::USD,
            ))
            .quantity(10000.0)
            .fixed_price(3.50)
            .forward_curve_id(CurveId::new("NG-SPOT-AVG"))
            .side(PayReceive::Pay)
            .start_date(as_of)
            .maturity(Date::from_calendar_date(2025, Month::June, 30).expect("date"))
            .frequency(
                Tenor::new(1, finstack_quant_core::dates::TenorUnit::Months)
                    .expect("valid tenor fixture"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("should build");

        // The period containing the curve base (March) straddles it: some
        // observation days precede the base. The lookup error must propagate.
        let result = swap.floating_leg_pv(&market, as_of);
        assert!(
            result.is_err(),
            "a floating-leg observation window straddling the curve base must \
             propagate the curve-lookup error, not silently use spot; got {result:?}"
        );
    }

    /// W-11: the daily-averaged floating price must be computed with
    /// compensated summation and remain correct over a long observation
    /// window. With a flat curve the average equals the flat price exactly.
    #[test]
    fn w11_floating_leg_average_is_accurate_on_flat_curve() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        // Flat price curve: every business day observes the same price, so the
        // compensated average must equal that price to full precision.
        let price_curve = PriceCurve::builder("NG-SPOT-AVG")
            .base_date(as_of)
            .spot_price(3.50)
            .knots([(0.0, 3.50), (2.0, 3.50)])
            .build()
            .expect("price curve");
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (2.0, 0.90)])
            .build()
            .expect("discount curve");
        let market = MarketContext::new().insert(disc).insert(price_curve);

        let swap = CommoditySwap::builder()
            .id(InstrumentId::new("W11-FLAT-SWAP"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "NG",
                "MMBTU",
                Currency::USD,
            ))
            .quantity(10000.0)
            .fixed_price(3.50)
            .forward_curve_id(CurveId::new("NG-SPOT-AVG"))
            .side(PayReceive::Pay)
            .start_date(as_of)
            .maturity(Date::from_calendar_date(2025, Month::December, 31).expect("date"))
            .frequency(
                Tenor::new(1, finstack_quant_core::dates::TenorUnit::Months)
                    .expect("valid tenor fixture"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("should build");

        // Flat curve ⇒ fixed price == floating average ⇒ NPV is exactly zero.
        let npv = swap.value(&market, as_of).expect("should price").amount();
        assert!(
            npv.abs() < 1e-6,
            "on a flat curve with fixed == flat price the swap NPV must be ~0, \
             got {npv}; a non-zero value indicates summation error in the \
             daily floating-leg average"
        );
    }

    /// Helper: every business day (weekend-filtered) in `[start, end]`.
    fn business_days(start: Date, end: Date) -> Vec<Date> {
        let mut days = Vec::new();
        let mut current = start;
        while current <= end {
            let wd = current.weekday();
            if wd != time::Weekday::Saturday && wd != time::Weekday::Sunday {
                days.push(current);
            }
            current += time::Duration::days(1);
        }
        days
    }

    fn seasoned_swap(past_fixings: Vec<(Date, f64)>) -> finstack_quant_core::Result<CommoditySwap> {
        CommoditySwap::builder()
            .id(InstrumentId::new("SEASONED-SWAP"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "NG",
                "MMBTU",
                Currency::USD,
            ))
            .quantity(10000.0)
            .fixed_price(3.50)
            .forward_curve_id(CurveId::new("NG-SPOT-AVG"))
            .side(PayReceive::Pay)
            .start_date(Date::from_calendar_date(2025, Month::January, 1).expect("date"))
            .maturity(Date::from_calendar_date(2025, Month::June, 30).expect("date"))
            .frequency(
                Tenor::new(1, finstack_quant_core::dates::TenorUnit::Months)
                    .expect("valid tenor fixture"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            .past_fixings(past_fixings)
            .build()
    }

    /// past floating-leg observations read from the
    /// realized-fixings store. With flat fixings equal to a flat curve and a
    /// matching fixed price, the seasoned swap marks to ~0.
    #[test]
    fn m15_seasoned_swap_uses_realized_fixings() {
        // Mid-February valuation: the Jan 31 payment has settled; the live
        // Feb period straddles as_of and needs realized fixings.
        let as_of = Date::from_calendar_date(2025, Month::February, 14).expect("date");
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("date");

        let fixings: Vec<(Date, f64)> = business_days(start, as_of)
            .into_iter()
            .map(|d| (d, 3.50))
            .collect();
        let swap = seasoned_swap(fixings).expect("valid seasoned swap");

        let price_curve = PriceCurve::builder("NG-SPOT-AVG")
            .base_date(as_of)
            .spot_price(3.50)
            .knots([(0.0, 3.50), (2.0, 3.50)])
            .build()
            .expect("price curve");
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (2.0, 0.90)])
            .build()
            .expect("discount curve");
        let market = MarketContext::new().insert(disc).insert(price_curve);

        let npv = swap.value(&market, as_of).expect("should price").amount();
        assert!(
            npv.abs() < 1e-6,
            "flat fixings + flat curve at the fixed price must mark to ~0, got {npv}"
        );
    }

    /// a missing past fixing is an error naming the
    /// missing date — never silently substituted with spot.
    #[test]
    fn m15_missing_past_fixing_errors() {
        let as_of = Date::from_calendar_date(2025, Month::February, 14).expect("date");
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("date");

        // Drop one mid-window business day from the fixings.
        let missing = Date::from_calendar_date(2025, Month::February, 5).expect("date");
        let fixings: Vec<(Date, f64)> = business_days(start, as_of)
            .into_iter()
            .filter(|d| *d != missing)
            .map(|d| (d, 3.50))
            .collect();
        let swap = seasoned_swap(fixings).expect("valid seasoned swap");

        let price_curve = PriceCurve::builder("NG-SPOT-AVG")
            .base_date(as_of)
            .spot_price(3.50)
            .knots([(0.0, 3.50), (2.0, 3.50)])
            .build()
            .expect("price curve");
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (2.0, 0.90)])
            .build()
            .expect("discount curve");
        let market = MarketContext::new().insert(disc).insert(price_curve);

        let err = swap
            .value(&market, as_of)
            .expect_err("missing past fixing must error");
        assert!(
            err.to_string().contains("2025-02-05"),
            "error should name the missing date, got: {err}"
        );
    }

    /// duplicate past fixing dates are rejected.
    #[test]
    fn m15_duplicate_fixing_errors() {
        let as_of = Date::from_calendar_date(2025, Month::February, 14).expect("date");
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("date");

        let mut fixings: Vec<(Date, f64)> = business_days(start, as_of)
            .into_iter()
            .map(|d| (d, 3.50))
            .collect();
        fixings.push((
            Date::from_calendar_date(2025, Month::February, 5).expect("date"),
            3.60,
        ));
        let err = seasoned_swap(fixings).expect_err("duplicate fixing must fail construction");
        assert!(
            err.to_string().contains("duplicate past fixing"),
            "error should mention the duplicate, got: {err}"
        );
    }

    #[test]
    fn test_commodity_swap_cashflow_provider_emits_both_legs() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let market = test_market(as_of);
        let swap = CommoditySwap::builder()
            .id(InstrumentId::new("PROVIDER-TEST"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "NG",
                "MMBTU",
                Currency::USD,
            ))
            .quantity(10000.0)
            .fixed_price(3.50)
            .forward_curve_id(CurveId::new("NG-SPOT-AVG"))
            .side(PayReceive::Pay)
            .start_date(as_of)
            .maturity(Date::from_calendar_date(2025, Month::March, 31).expect("valid date"))
            .frequency(
                Tenor::new(1, finstack_quant_core::dates::TenorUnit::Months)
                    .expect("valid tenor fixture"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("should build");

        let flows = swap
            .dated_cashflows(&market, as_of)
            .expect("commodity swap contractual schedule should build");

        assert_eq!(
            flows.len(),
            6,
            "three payments should emit fixed and floating rows"
        );
        assert_eq!(
            flows
                .iter()
                .filter(|(_, money)| money.amount() < 0.0)
                .count(),
            3
        );
        assert_eq!(
            flows
                .iter()
                .filter(|(_, money)| money.amount() > 0.0)
                .count(),
            3
        );
    }

    #[test]
    fn canonical_dependencies_include_discount_and_price_projection_curves() {
        let swap = CommoditySwap::example().expect("example");
        let deps =
            crate::instruments::Instrument::market_dependencies(&swap).expect("dependencies");

        assert_eq!(
            deps.curves.discount_curves.as_slice(),
            &[swap.discount_curve_id]
        );
        assert_eq!(
            deps.curves.forward_curves.as_slice(),
            &[swap.forward_curve_id]
        );
    }
}
