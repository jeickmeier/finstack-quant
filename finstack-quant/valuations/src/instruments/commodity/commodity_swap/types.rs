//! Commodity swap types and implementations.
//!
//! Defines the `CommoditySwap` instrument for fixed-for-floating commodity
//! price exchange contracts. One party pays a fixed price per unit while
//! the other pays a floating price based on an index.

use crate::cashflow::builder::CashFlowSchedule;
use crate::cashflow::primitives::CFKind;
use crate::impl_instrument_base;
use crate::instruments::common_impl::parameters::legs::PayReceive;
use crate::instruments::common_impl::parameters::CommodityUnderlyingParams;
use crate::instruments::common_impl::traits::Attributes;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, CalendarRegistry, Date, ScheduleBuilder, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CalendarId, CurveId, InstrumentId};
use finstack_quant_core::Result;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

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
///     .fixed_price(rust_decimal::Decimal::try_from(3.50).expect("valid decimal"))
///     .floating_index_id(CurveId::new("NG-SPOT-AVG"))
///     .side(PayReceive::Pay)
///     .start_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
///     .maturity(Date::from_calendar_date(2025, Month::December, 31).unwrap())
///     .frequency(Tenor::new(1, TenorUnit::Months))
///     .discount_curve_id(CurveId::new("USD-OIS"))
///     .build()
///     .expect("Valid swap");
/// ```
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    schemars::JsonSchema,
)]
pub struct CommoditySwap {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Underlying commodity parameters (type, ticker, unit, currency).
    #[serde(flatten)]
    pub underlying: CommodityUnderlyingParams,
    /// Notional quantity per period.
    pub quantity: f64,
    /// Fixed price per unit.
    pub fixed_price: Decimal,
    /// Floating index ID for price lookups.
    pub floating_index_id: CurveId,
    /// Direction of the swap: Pay means paying the fixed price leg,
    /// Receive means receiving the fixed price leg.
    pub side: PayReceive,
    /// Start date of the swap.
    #[schemars(with = "String")]
    pub start_date: Date,
    /// End date of the swap.
    #[schemars(with = "String")]
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
    pub bdc: BusinessDayConvention,
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
    #[schemars(with = "Vec<(String, f64)>")]
    pub realized_fixings: Vec<(Date, f64)>,
    /// Attributes for tagging and selection.
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    #[serde(default)]
    #[builder(default)]
    pub attributes: Attributes,
}

/// Custom deserializer for CommoditySwap that reads the `side` field
/// (PayReceive enum), defaulting to `Pay` when omitted.
impl<'de> serde::Deserialize<'de> for CommoditySwap {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Helper {
            id: InstrumentId,
            #[serde(flatten)]
            underlying: CommodityUnderlyingParams,
            quantity: f64,
            fixed_price: Decimal,
            floating_index_id: CurveId,
            #[serde(default)]
            side: Option<PayReceive>,
            start_date: Date,
            maturity: Date,
            frequency: Tenor,
            #[serde(default)]
            calendar_id: Option<CalendarId>,
            #[serde(default = "crate::serde_defaults::bdc_modified_following")]
            bdc: BusinessDayConvention,
            discount_curve_id: CurveId,
            #[serde(default)]
            index_lag_days: Option<i32>,
            #[serde(default)]
            realized_fixings: Vec<(Date, f64)>,
            #[serde(default)]
            pricing_overrides: crate::instruments::PricingOverrides,
            attributes: Attributes,
            /// Rejects unknown JSON fields (restores `deny_unknown_fields`
            /// despite the `#[serde(flatten)]` on `underlying`).
            #[serde(flatten)]
            #[allow(dead_code)]
            unknown_fields: crate::instruments::common_impl::serde_guard::UnknownFieldGuard,
        }

        let helper = Helper::deserialize(deserializer)?;

        let side = helper.side.unwrap_or(PayReceive::Pay);

        Ok(CommoditySwap {
            id: helper.id,
            underlying: helper.underlying,
            quantity: helper.quantity,
            fixed_price: helper.fixed_price,
            floating_index_id: helper.floating_index_id,
            side,
            start_date: helper.start_date,
            maturity: helper.maturity,
            frequency: helper.frequency,
            calendar_id: helper.calendar_id,
            bdc: helper.bdc,
            discount_curve_id: helper.discount_curve_id,
            index_lag_days: helper.index_lag_days,
            realized_fixings: helper.realized_fixings,
            pricing_overrides: helper.pricing_overrides,
            attributes: helper.attributes,
        })
    }
}

impl CommoditySwap {
    /// Create a canonical example commodity swap for testing and documentation.
    ///
    /// Returns a natural gas swap with monthly settlements.
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        Self::builder()
            .id(InstrumentId::new("NG-SWAP-2025"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "NG",
                "MMBTU",
                Currency::USD,
            ))
            .quantity(10000.0)
            .fixed_price(Decimal::try_from(3.50).expect("valid decimal"))
            .floating_index_id(CurveId::new("NG-SPOT-AVG"))
            .side(PayReceive::Pay)
            .start_date(
                Date::from_calendar_date(2025, time::Month::January, 1)
                    .expect("Valid example date"),
            )
            .maturity(
                Date::from_calendar_date(2025, time::Month::December, 31)
                    .expect("Valid example date"),
            )
            .frequency(Tenor::new(1, finstack_quant_core::dates::TenorUnit::Months))
            .bdc(BusinessDayConvention::ModifiedFollowing)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(
                Attributes::new()
                    .with_tag("energy")
                    .with_meta("sector", "natural-gas"),
            )
            .build()
            .expect("Example commodity swap construction should not fail")
    }

    /// Calculate the present value of the fixed leg.
    pub fn fixed_leg_pv(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
        let disc = market.get_discount(self.discount_curve_id.as_str())?;
        let schedule = self.payment_schedule(as_of)?;
        let fixed_price = self
            .fixed_price
            .to_f64()
            .ok_or(finstack_quant_core::InputError::ConversionOverflow)?;

        let mut pv = 0.0;
        for payment_date in schedule {
            if payment_date < as_of {
                continue; // Skip past payments
            }
            let df = disc.df_between_dates(as_of, payment_date)?;
            let period_value = self.quantity * fixed_price;
            pv += period_value * df;
        }

        Ok(pv)
    }

    /// Calculate the present value of the floating leg.
    ///
    /// Projects floating prices from the `PriceCurve` referenced by `floating_index_id`,
    /// with optional index lag and period averaging.
    pub fn floating_leg_pv(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
        let disc = market.get_discount(self.discount_curve_id.as_str())?;
        let schedule = self.payment_schedule(as_of)?;

        // Try to get PriceCurve for floating index
        let price_curve = market.get_price_curve(self.floating_index_id.as_str())?;

        let mut pv = 0.0;
        let mut prev_period_end = self.start_date;
        let last_payment = schedule.last().copied();

        for payment_date in schedule {
            if payment_date < as_of {
                prev_period_end = payment_date;
                continue; // Skip past payments
            }

            // Period start is previous period end (or swap start for first period)
            let period_start = prev_period_end;
            let period_end = payment_date;

            // Get expected average price for this period. Windows are
            // half-open; the final period observes the maturity date too.
            let include_end = Some(payment_date) == last_payment;
            let forward_price = self.expected_period_price(
                &price_curve,
                as_of,
                period_start,
                period_end,
                include_end,
            )?;

            let df = disc.df_between_dates(as_of, payment_date)?;
            let period_value = self.quantity * forward_price;
            pv += period_value * df;

            prev_period_end = payment_date;
        }

        Ok(pv)
    }

    /// Calculate expected average price for a period.
    ///
    /// Uses business day weighted averaging for the observation period, which is
    /// the market standard for commodity swaps. Weekends are excluded from the
    /// average (no calendar applied yet - just weekday filtering).
    ///
    /// # Arguments
    /// * `price_curve` - The commodity price curve
    /// * `as_of` - Valuation date
    /// * `period_start` - Start of the averaging period
    /// * `period_end` - End of the averaging period
    ///
    /// # Averaging Method
    ///
    /// Uses daily business day sampling for all periods (market standard for
    /// commodity swaps). When a `calendar_id` is provided and resolves to a
    /// valid holiday calendar, exchange holidays are also excluded from the
    /// average. Otherwise, only weekends are filtered.
    ///
    /// # Past vs future observations
    ///
    /// Observation dates strictly before `as_of` read from
    /// [`realized_fixings`](Self::realized_fixings); a missing past fixing is
    /// an `Error::Validation` naming the date — silently substituting today's
    /// spot would mis-mark every seasoned averaging period. Observation dates
    /// on or after `as_of` project from the curve via `price_on_date(date)`
    /// (respecting the curve's day count convention); a curve-lookup failure
    /// inside the window is propagated as an error (W-11).
    ///
    /// # Window convention
    ///
    /// Windows are half-open `[period_start, period_end)` so a payment date
    /// is never observed by two adjacent periods; the final period passes
    /// `include_end = true` so the swap maturity is observed exactly once.
    pub(crate) fn expected_period_price(
        &self,
        price_curve: &finstack_quant_core::market_data::term_structures::PriceCurve,
        as_of: Date,
        period_start: Date,
        period_end: Date,
        include_end: bool,
    ) -> Result<f64> {
        // Apply index lag if specified (shift observation window backwards)
        let lag_days = self.index_lag_days.unwrap_or(0);
        let obs_start = period_start - time::Duration::days(lag_days as i64);
        let obs_end = period_end - time::Duration::days(lag_days as i64);

        // Resolve holiday calendar if available (Item 8: integrate holiday calendars)
        let calendar = match self.calendar_id.as_deref() {
            Some(id) => Some(CalendarRegistry::global().resolve_str(id).ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "CommoditySwap '{}' references unknown calendar_id '{id}'",
                    self.id
                ))
            })?),
            None => None,
        };

        // Business day filter: exclude weekends and exchange holidays
        let is_business_day = |date: Date| -> bool {
            let wd = date.weekday();
            if wd == time::Weekday::Saturday || wd == time::Weekday::Sunday {
                return false;
            }
            // If we have a holiday calendar, check it
            if let Some(cal) = &calendar {
                return cal.is_business_day(date);
            }
            true
        };

        // Realized-fixing lookup for past observation dates, with duplicate
        // rejection (a duplicate would silently skew the average).
        let mut fixings: std::collections::BTreeMap<Date, f64> = std::collections::BTreeMap::new();
        for (d, v) in &self.realized_fixings {
            if fixings.insert(*d, *v).is_some() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CommoditySwap '{}' has duplicate realized fixing for date {d}",
                    self.id.as_str()
                )));
            }
        }

        let get_price = |date: Date| -> Result<f64> {
            if date < as_of {
                fixings.get(&date).copied().ok_or_else(|| {
                    finstack_quant_core::Error::Validation(format!(
                        "CommoditySwap '{}' is missing a realized fixing for past \
                         observation date {date} (as_of {as_of}); past floating-leg \
                         observations must be supplied via realized_fixings",
                        self.id.as_str()
                    ))
                })
            } else {
                price_curve.price_on_date(date)
            }
        };

        // Market standard: daily business day sampling for all periods, over
        // half-open windows shared with the commodity swaption.
        crate::instruments::commodity::averaging::business_day_average_price(
            get_price,
            is_business_day,
            obs_start,
            obs_end,
            include_end,
        )
    }

    /// Generate the payment schedule for this swap.
    pub fn payment_schedule(&self, _as_of: Date) -> Result<Vec<Date>> {
        // Market standard: Modified Following for commodity swaps (matches QuantLib/Bloomberg)
        let bdc = self.bdc;

        let mut builder = ScheduleBuilder::new(self.start_date, self.maturity)?
            .frequency(self.frequency)
            .stub_rule(finstack_quant_core::dates::StubKind::ShortBack);

        // Apply calendar adjustment if calendar_id is specified
        if let Some(ref cal_id) = self.calendar_id {
            let cal = CalendarRegistry::global()
                .resolve_str(cal_id)
                .ok_or_else(|| {
                    finstack_quant_core::Error::Validation(format!(
                        "CommoditySwap '{}' references unknown calendar_id '{cal_id}'",
                        self.id
                    ))
                })?;
            builder = builder.adjust_with(bdc, cal);
        }

        let schedule = builder.build()?;

        // ScheduleBuilder always emits the adjusted start anchor first.
        // Preserve every subsequent adjusted anchor positionally: comparing
        // adjusted dates with raw start/maturity can admit a rolled start or
        // discard a rolled final payment.
        let dates: Vec<Date> = schedule.into_iter().skip(1).collect();

        Ok(dates)
    }

    fn leg_schedule_from_amounts(&self, flows: &[(Date, Money)]) -> Result<Vec<(Date, Money)>> {
        let ccy = self.underlying.currency;
        Ok(flows
            .iter()
            .map(|(date, amount)| (*date, Money::new(amount.amount(), ccy)))
            .collect())
    }

    fn fixed_leg_flows(&self) -> Result<Vec<(Date, Money)>> {
        let fixed_price = self
            .fixed_price
            .to_f64()
            .ok_or(finstack_quant_core::InputError::ConversionOverflow)?;
        let signed_amount = match self.side {
            PayReceive::Pay => -self.quantity * fixed_price,
            PayReceive::Receive => self.quantity * fixed_price,
        };
        Ok(self
            .payment_schedule(self.start_date)?
            .into_iter()
            .map(|payment_date| {
                (
                    payment_date,
                    Money::new(signed_amount, self.underlying.currency),
                )
            })
            .collect())
    }

    fn floating_leg_flows(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Vec<(Date, Money)>> {
        let price_curve = market.get_price_curve(self.floating_index_id.as_str())?;
        let mut prev_period_end = self.start_date;
        let mut flows = Vec::new();
        let schedule = self.payment_schedule(as_of)?;
        let last_payment = schedule.last().copied();
        for payment_date in schedule {
            let period_start = prev_period_end;
            let period_end = payment_date;
            let include_end = Some(payment_date) == last_payment;
            let forward_price = self.expected_period_price(
                &price_curve,
                as_of,
                period_start,
                period_end,
                include_end,
            )?;
            let signed_amount = match self.side {
                PayReceive::Pay => self.quantity * forward_price,
                PayReceive::Receive => -self.quantity * forward_price,
            };
            flows.push((
                payment_date,
                Money::new(signed_amount, self.underlying.currency),
            ));
            prev_period_end = payment_date;
        }
        Ok(flows)
    }
}

impl crate::instruments::common_impl::traits::CurveDependencies for CommoditySwap {
    fn curve_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::InstrumentCurves>
    {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .forward(self.floating_index_id.clone())
            .build()
    }
}

impl crate::instruments::common_impl::traits::Instrument for CommoditySwap {
    impl_instrument_base!(crate::pricer::InstrumentType::CommoditySwap);

    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        let fixed_leg_pv = self.fixed_leg_pv(market, as_of)?;
        let floating_leg_pv = self.floating_leg_pv(market, as_of)?;

        let npv = match self.side {
            PayReceive::Pay => {
                // Pay fixed, receive floating
                floating_leg_pv - fixed_leg_pv
            }
            PayReceive::Receive => {
                // Receive fixed, pay floating
                fixed_leg_pv - floating_leg_pv
            }
        };

        Ok(finstack_quant_core::money::Money::new(
            npv,
            self.underlying.currency,
        ))
    }

    fn effective_start_date(&self) -> Option<Date> {
        Some(self.start_date)
    }

    fn expiry(&self) -> Option<Date> {
        Some(self.maturity)
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

impl finstack_quant_cashflows::CashflowScheduleSource for CommoditySwap {
    fn raw_cashflow_schedule(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        let flows = self
            .leg_schedule_from_amounts(&self.fixed_leg_flows()?)?
            .into_iter()
            .chain(self.leg_schedule_from_amounts(&self.floating_leg_flows(market, as_of)?)?)
            .collect();
        let schedule = crate::cashflow::traits::schedule_from_dated_flows(
            flows,
            finstack_quant_core::dates::DayCount::Act365F,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: Some(Money::new(0.0, self.underlying.currency)),
                kind: Some(CFKind::Notional),
                representation: crate::cashflow::builder::CashflowRepresentation::Projected,
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
            .fixed_price(rust_decimal::Decimal::try_from(70.0).expect("valid decimal"))
            .floating_index_id(CurveId::new("CL-AVG"))
            .side(PayReceive::Pay)
            .start_date(Date::from_calendar_date(2025, Month::January, 1).expect("valid date"))
            .maturity(Date::from_calendar_date(2025, Month::June, 30).expect("valid date"))
            .frequency(Tenor::new(1, finstack_quant_core::dates::TenorUnit::Months))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(Attributes::new())
            .build()
            .expect("should build");

        assert_eq!(swap.id.as_str(), "TEST-SWAP");
        assert_eq!(swap.underlying.ticker, "CL");
        assert_eq!(swap.quantity, 1000.0);
        assert_eq!(swap.fixed_price.to_f64().expect("decimal to f64"), 70.0);
        assert_eq!(swap.side, PayReceive::Pay);
    }

    #[test]
    fn test_commodity_swap_example() {
        let swap = CommoditySwap::example();
        assert_eq!(swap.id.as_str(), "NG-SWAP-2025");
        assert_eq!(swap.underlying.commodity_type, "Energy");
        assert_eq!(swap.underlying.ticker, "NG");
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
            .fixed_price(rust_decimal::Decimal::try_from(3.50).expect("valid decimal")) // Same as spot
            .floating_index_id(CurveId::new("NG-SPOT-AVG"))
            .side(PayReceive::Pay)
            .start_date(as_of)
            .maturity(Date::from_calendar_date(2025, Month::June, 30).expect("valid date"))
            .frequency(Tenor::new(1, finstack_quant_core::dates::TenorUnit::Months))
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
            .fixed_price(rust_decimal::Decimal::try_from(3.55).expect("valid decimal"))
            .floating_index_id(CurveId::new("NG-SPOT-AVG"))
            .side(PayReceive::Pay)
            .start_date(as_of)
            .maturity(Date::from_calendar_date(2025, Month::June, 30).expect("valid date"))
            .frequency(Tenor::new(1, finstack_quant_core::dates::TenorUnit::Months))
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
            .fixed_price(rust_decimal::Decimal::try_from(3.55).expect("valid decimal"))
            .floating_index_id(CurveId::new("NG-SPOT-AVG"))
            .side(PayReceive::Receive) // Receiving fixed
            .start_date(as_of)
            .maturity(Date::from_calendar_date(2025, Month::June, 30).expect("valid date"))
            .frequency(Tenor::new(1, finstack_quant_core::dates::TenorUnit::Months))
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
            .fixed_price(rust_decimal::Decimal::try_from(3.50).expect("valid decimal"))
            .floating_index_id(CurveId::new("NG-SPOT-AVG"))
            .side(PayReceive::Pay)
            .start_date(as_of)
            .maturity(Date::from_calendar_date(2025, Month::March, 31).expect("valid date"))
            .frequency(Tenor::new(1, finstack_quant_core::dates::TenorUnit::Months))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("should build");

        let flows = swap
            .dated_cashflows(&market, as_of)
            .expect("should get flows");

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

        let swap = CommoditySwap::example();

        assert_eq!(swap.id(), "NG-SWAP-2025");
        assert_eq!(swap.key(), crate::pricer::InstrumentType::CommoditySwap);
    }

    #[test]
    fn test_commodity_swap_curve_dependencies() {
        use crate::instruments::common_impl::traits::CurveDependencies;

        let swap = CommoditySwap::example();
        let deps = swap.curve_dependencies().expect("curve_dependencies");

        assert_eq!(deps.discount_curves.len(), 1);
        assert_eq!(deps.forward_curves.len(), 1);
    }

    #[test]
    fn test_commodity_swap_serde_roundtrip() {
        let swap = CommoditySwap::example();
        let json = serde_json::to_string(&swap).expect("serialize");
        let deserialized: CommoditySwap = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(swap.id.as_str(), deserialized.id.as_str());
        assert_eq!(swap.underlying.ticker, deserialized.underlying.ticker);
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
            .fixed_price(rust_decimal::Decimal::try_from(3.50).expect("decimal"))
            .floating_index_id(CurveId::new("NG-SPOT-AVG"))
            .side(PayReceive::Pay)
            .start_date(as_of)
            .maturity(Date::from_calendar_date(2025, Month::June, 30).expect("date"))
            .frequency(Tenor::new(1, finstack_quant_core::dates::TenorUnit::Months))
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
            .fixed_price(rust_decimal::Decimal::try_from(3.50).expect("decimal"))
            .floating_index_id(CurveId::new("NG-SPOT-AVG"))
            .side(PayReceive::Pay)
            .start_date(as_of)
            .maturity(Date::from_calendar_date(2025, Month::December, 31).expect("date"))
            .frequency(Tenor::new(1, finstack_quant_core::dates::TenorUnit::Months))
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

    fn seasoned_swap(realized_fixings: Vec<(Date, f64)>) -> CommoditySwap {
        CommoditySwap::builder()
            .id(InstrumentId::new("SEASONED-SWAP"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "NG",
                "MMBTU",
                Currency::USD,
            ))
            .quantity(10000.0)
            .fixed_price(rust_decimal::Decimal::try_from(3.50).expect("decimal"))
            .floating_index_id(CurveId::new("NG-SPOT-AVG"))
            .side(PayReceive::Pay)
            .start_date(Date::from_calendar_date(2025, Month::January, 1).expect("date"))
            .maturity(Date::from_calendar_date(2025, Month::June, 30).expect("date"))
            .frequency(Tenor::new(1, finstack_quant_core::dates::TenorUnit::Months))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .realized_fixings(realized_fixings)
            .build()
            .expect("should build")
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
        let swap = seasoned_swap(fixings);

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
        let swap = seasoned_swap(fixings);

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

    /// duplicate realized fixing dates are rejected.
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
        let swap = seasoned_swap(fixings);

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
            .expect_err("duplicate fixing must error");
        assert!(
            err.to_string().contains("duplicate realized fixing"),
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
            .fixed_price(rust_decimal::Decimal::try_from(3.50).expect("valid decimal"))
            .floating_index_id(CurveId::new("NG-SPOT-AVG"))
            .side(PayReceive::Pay)
            .start_date(as_of)
            .maturity(Date::from_calendar_date(2025, Month::March, 31).expect("valid date"))
            .frequency(Tenor::new(1, finstack_quant_core::dates::TenorUnit::Months))
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
}
