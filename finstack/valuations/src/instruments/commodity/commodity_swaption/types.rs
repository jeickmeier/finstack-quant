//! Commodity swaption instrument definition and pricing logic.
//!
//! A commodity swaption is an option to enter into a commodity swap at a
//! predetermined fixed price. The holder has the right, but not the obligation,
//! to enter a fixed-for-floating commodity swap at expiry.
//!
//! # Pricing
//!
//! Uses the Black-76 model applied to the forward swap rate:
//! ```text
//! C = DF * annuity * [F * N(d1) - K * N(d2)]
//! P = DF * annuity * [K * N(-d2) - F * N(-d1)]
//! ```
//! where:
//! - F = forward swap rate (annuity-weighted average of forward prices over swap periods)
//! - K = fixed price (strike)
//! - annuity = sum of discount factors x period lengths
//! - d1 = [ln(F/K) + 0.5*sigma^2*T] / (sigma*sqrt(T))
//! - d2 = d1 - sigma*sqrt(T)

use crate::impl_instrument_base;
use crate::instruments::common_impl::parameters::CommodityUnderlyingParams;
use crate::instruments::common_impl::traits::{Attributes, CurveDependencies, InstrumentCurves};
use crate::instruments::OptionType;
use finstack_core::currency::Currency;
use finstack_core::dates::{
    BusinessDayConvention, CalendarRegistry, Date, DayCount, DayCountContext, ScheduleBuilder,
    Tenor,
};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::types::{CalendarId, CurveId, InstrumentId};
use finstack_core::Result;

/// Commodity swaption (option on a fixed-for-floating commodity swap).
///
/// The holder has the right to enter a commodity swap at expiry, paying
/// (or receiving) a fixed price in exchange for floating commodity prices.
///
/// # Pricing
///
/// Black-76 model on the forward swap rate:
/// - Forward swap rate is the weighted average of forward commodity prices
///   over the swap period
/// - Annuity factor captures the present value of a unit payment stream
///
/// # Examples
///
/// ```rust
/// use finstack_valuations::instruments::commodity::commodity_swaption::CommoditySwaption;
/// use finstack_valuations::instruments::CommodityUnderlyingParams;
/// use finstack_valuations::instruments::OptionType;
/// use finstack_core::currency::Currency;
/// use finstack_core::dates::{Date, Tenor, TenorUnit};
/// use finstack_core::types::{CurveId, InstrumentId};
/// use time::Month;
///
/// let swaption = CommoditySwaption::builder()
///     .id(InstrumentId::new("NG-SWAPTION-2025"))
///     .underlying(CommodityUnderlyingParams::new("Energy", "NG", "MMBTU", Currency::USD))
///     .option_type(OptionType::Call)
///     .expiry(Date::from_calendar_date(2025, Month::June, 15).unwrap())
///     .swap_start(Date::from_calendar_date(2025, Month::July, 1).unwrap())
///     .swap_end(Date::from_calendar_date(2026, Month::June, 30).unwrap())
///     .swap_frequency(Tenor::new(1, TenorUnit::Months))
///     .fixed_price(3.50)
///     .notional(10000.0)
///     .forward_curve_id(CurveId::new("NG-FORWARD"))
///     .discount_curve_id(CurveId::new("USD-OIS"))
///     .vol_surface_id(CurveId::new("NG-VOL"))
///     .build()
///     .expect("Valid swaption");
/// ```
#[derive(
    Clone,
    Debug,
    finstack_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[builder(validate = CommoditySwaption::validate)]
pub struct CommoditySwaption {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Commodity underlying parameters (commodity_type, ticker, unit, currency).
    #[serde(flatten)]
    pub underlying: CommodityUnderlyingParams,
    /// Option type (call = right to enter pay-fixed swap, put = right to enter receive-fixed swap).
    pub option_type: OptionType,
    /// Option expiry date.
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Underlying swap start date.
    #[schemars(with = "String")]
    pub swap_start: Date,
    /// Underlying swap end date.
    #[schemars(with = "String")]
    pub swap_end: Date,
    /// Underlying swap payment frequency.
    pub swap_frequency: Tenor,
    /// Fixed price (strike) of the underlying swap.
    pub fixed_price: f64,
    /// Notional quantity per period.
    pub notional: f64,
    /// Forward/futures curve ID for commodity price interpolation.
    pub forward_curve_id: CurveId,
    /// Discount curve ID for present value.
    pub discount_curve_id: CurveId,
    /// Volatility surface ID for implied vol.
    pub vol_surface_id: CurveId,
    /// Optional calendar ID for date adjustments.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar_id: Option<CalendarId>,
    /// Business day convention for date adjustments.
    #[builder(default = BusinessDayConvention::ModifiedFollowing)]
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub bdc: BusinessDayConvention,
    /// Day count convention for time to expiry.
    #[serde(default = "crate::serde_defaults::day_count_act365f")]
    #[builder(default = DayCount::Act365F)]
    pub day_count: DayCount,
    /// Pricing overrides (implied vol, etc.).
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging.
    #[builder(default)]
    #[serde(default)]
    pub attributes: Attributes,
    /// Rejects unknown JSON fields (restores `deny_unknown_fields` despite the
    /// `#[serde(flatten)]` on `underlying`). See [`UnknownFieldGuard`].
    #[serde(flatten)]
    #[schemars(skip)]
    #[builder(default)]
    pub(crate) unknown_fields: crate::instruments::common_impl::serde_guard::UnknownFieldGuard,
}

impl CommoditySwaption {
    /// Validate commodity swaption input invariants.
    pub fn validate(&self) -> finstack_core::Result<()> {
        // expiry <= swap_start < swap_end
        crate::instruments::common_impl::validation::validate_date_range_non_strict(
            self.expiry,
            self.swap_start,
            "CommoditySwaption expiry/swap_start",
        )?;
        crate::instruments::common_impl::validation::validate_date_range_strict(
            self.swap_start,
            self.swap_end,
            "CommoditySwaption swap_start/swap_end",
        )?;
        // notional > 0
        crate::instruments::common_impl::validation::validate_f64_positive(
            self.notional,
            "CommoditySwaption notional",
        )?;
        // fixed_price must be finite (negative strikes can be legitimate for spread commodities)
        crate::instruments::common_impl::validation::validate_f64_finite(
            self.fixed_price,
            "CommoditySwaption fixed_price",
        )?;
        // swap_frequency count must be > 0 (count is u32, so only zero is invalid)
        if self.swap_frequency.count == 0 {
            return Err(finstack_core::Error::Validation(
                "CommoditySwaption swap_frequency count must be positive (got 0)".to_string(),
            ));
        }
        Ok(())
    }

    /// Create a canonical example commodity swaption for testing and documentation.
    ///
    /// Returns a natural gas European call swaption.
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        Self::builder()
            .id(InstrumentId::new("NG-SWAPTION-2025"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "NG",
                "MMBTU",
                Currency::USD,
            ))
            .option_type(OptionType::Call)
            .expiry(
                Date::from_calendar_date(2025, time::Month::June, 15).expect("valid example date"),
            )
            .swap_start(
                Date::from_calendar_date(2025, time::Month::July, 1).expect("valid example date"),
            )
            .swap_end(
                Date::from_calendar_date(2026, time::Month::June, 30).expect("valid example date"),
            )
            .swap_frequency(Tenor::new(1, finstack_core::dates::TenorUnit::Months))
            .fixed_price(3.50)
            .notional(10000.0)
            .forward_curve_id(CurveId::new("NG-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("NG-VOL"))
            .day_count(DayCount::Act365F)
            .pricing_overrides(crate::instruments::PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("Example commodity swaption construction should not fail")
    }

    /// Generate the underlying swap payment schedule.
    pub fn swap_payment_schedule(&self) -> Result<Vec<Date>> {
        let mut builder = ScheduleBuilder::new(self.swap_start, self.swap_end)?
            .frequency(self.swap_frequency)
            .stub_rule(finstack_core::dates::StubKind::ShortBack);

        if let Some(ref cal_id) = self.calendar_id {
            if let Some(cal) = CalendarRegistry::global().resolve_str(cal_id) {
                builder = builder.adjust_with(self.bdc, cal);
            }
        }

        let schedule = builder.build()?;

        let dates: Vec<Date> = schedule
            .into_iter()
            .filter(|&d| d > self.swap_start && d <= self.swap_end)
            .collect();

        Ok(dates)
    }

    /// Compute the forward swap rate from the commodity forward curve.
    ///
    /// The forward swap rate is the **annuity-weighted** average of the
    /// **period-average** forward prices over each settlement period:
    /// ```text
    /// F_swap = Σ (F̄_i · DF_i) / Σ DF_i
    /// ```
    /// where `F̄_i` is the business-day average of the forward curve over the
    /// half-open period `[T_{i-1}, T_i)` (the final period also observes the
    /// swap end date) — exactly the quantity the underlying
    /// [`super::super::commodity_swap::CommoditySwap`] floating leg settles on
    /// — and `DF_i = DF(as_of, payment_date_i)`.
    ///
    /// Sampling `F(payment_date_i)` instead of the period average moves the
    /// swaption by ~half a period of carry per period on a sloped
    /// (contango/backwardation) curve, because the underlying floats on the
    /// business-day average, not the end-of-period print. On a flat curve the
    /// two coincide.
    ///
    /// This is the fair fixed price consistent with the `annuity · Black76`
    /// pricing identity: the swaption is priced as `annuity · Black76(F_swap, K)`
    /// where `annuity = Σ DF_i`, so the fair swap rate must be averaged with
    /// the same `DF_i` weights. The underlying swap pays `quantity × price`
    /// per period with **no** year-fraction accrual, so the weights carry no
    /// `τ_i` factor . The rate reduces to the
    /// equal-weighted mean when `DF_i` is constant across periods. If the
    /// annuity denominator is zero (degenerate schedule), the equal-weighted
    /// mean is returned.
    ///
    /// A forward-curve coverage failure on any observation date is propagated
    /// as a hard error — never silently substituted with spot (W-11 policy,
    /// matching the underlying swap).
    pub fn forward_swap_rate(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
        let price_curve = market.get_price_curve(self.forward_curve_id.as_str())?;
        let disc = market.get_discount(self.discount_curve_id.as_str())?;
        let schedule = self.swap_payment_schedule()?;

        if schedule.is_empty() {
            return Err(finstack_core::Error::Validation(
                "CommoditySwaption: underlying swap has no payment dates".to_string(),
            ));
        }

        // Business-day filter consistent with the underlying swap's
        // averaging: weekends out, plus exchange holidays when a calendar
        // is configured.
        let calendar = self
            .calendar_id
            .as_deref()
            .and_then(|id| CalendarRegistry::global().resolve_str(id));
        let is_business_day = |date: Date| -> bool {
            let wd = date.weekday();
            if wd == time::Weekday::Saturday || wd == time::Weekday::Sunday {
                return false;
            }
            if let Some(cal) = &calendar {
                return cal.is_business_day(date);
            }
            true
        };
        // The swap underlying a swaption is forward-starting (swap_start ≥
        // expiry ≥ as_of), so every observation projects from the curve;
        // coverage failures propagate (W-11).
        let get_price = |date: Date| -> Result<f64> { price_curve.price_on_date(date) };

        let last_payment = schedule.last().copied();
        let mut prev_period_end = self.swap_start;
        let mut sum_fwd = 0.0;
        let mut weighted_fwd = 0.0;
        let mut weight_total = 0.0;
        for &payment_date in &schedule {
            // Period-average forward over the half-open settlement window —
            // the same average the underlying floating leg settles on.
            let include_end = Some(payment_date) == last_payment;
            let fwd = super::super::averaging::business_day_average_price(
                get_price,
                is_business_day,
                prev_period_end,
                payment_date,
                include_end,
            )?;

            // Annuity weight DF_i — identical to the per-period term
            // accumulated in `annuity()`.
            let weight = disc.df_between_dates(as_of, payment_date)?;

            sum_fwd += fwd;
            weighted_fwd += fwd * weight;
            weight_total += weight;
            prev_period_end = payment_date;
        }

        // Guard against a zero (or negative) annuity denominator: fall back to
        // the equal-weighted mean.
        if weight_total <= 0.0 {
            return Ok(sum_fwd / schedule.len() as f64);
        }

        Ok(weighted_fwd / weight_total)
    }

    /// Compute the annuity factor for the underlying swap.
    ///
    /// The annuity is the sum of discount factors to each payment date — the
    /// PV of receiving 1 unit per period. The underlying `CommoditySwap` pays
    /// `quantity × price` per period with no year-fraction accrual, and
    /// `notional` is a per-period quantity, so the annuity must not carry a
    /// `τ_i` factor (the IR-swaption `Σ DF·τ` convention
    /// understated a monthly-settling swaption ~12×).
    pub fn annuity(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
        let disc = market.get_discount(self.discount_curve_id.as_str())?;
        let schedule = self.swap_payment_schedule()?;

        let mut annuity = 0.0;
        for &payment_date in &schedule {
            annuity += disc.df_between_dates(as_of, payment_date)?;
        }

        Ok(annuity)
    }

    fn time_to_expiry(&self, as_of: Date) -> Result<f64> {
        self.day_count
            .year_fraction(as_of, self.expiry, DayCountContext::default())
            .map(|t| t.max(0.0))
    }
}

impl CurveDependencies for CommoditySwaption {
    fn curve_dependencies(&self) -> finstack_core::Result<InstrumentCurves> {
        InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .forward(self.forward_curve_id.clone())
            .build()
    }
}

impl crate::instruments::common_impl::traits::Instrument for CommoditySwaption {
    impl_instrument_base!(crate::pricer::InstrumentType::CommoditySwaption);

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::Black76
    }

    fn market_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::dependencies::MarketDependencies>
    {
        let mut deps =
            crate::instruments::common_impl::dependencies::MarketDependencies::from_curve_dependencies(
                self,
            )?;
        deps.add_vol_surface_id(self.vol_surface_id.as_str());
        Ok(deps)
    }

    fn base_value(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        // Post-expiry: option is fully settled, value is 0
        if as_of > self.expiry {
            return Ok(Money::new(0.0, self.underlying.currency));
        }

        let t = self.time_to_expiry(as_of)?;
        let forward = self.forward_swap_rate(market, as_of)?;
        let annuity = self.annuity(market, as_of)?;

        // At or past expiry: return intrinsic value
        if t <= 0.0 {
            let intrinsic = match self.option_type {
                OptionType::Call => (forward - self.fixed_price).max(0.0),
                OptionType::Put => (self.fixed_price - forward).max(0.0),
            };
            return Ok(Money::new(
                intrinsic * annuity * self.notional,
                self.underlying.currency,
            ));
        }

        let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
            &self.pricing_overrides.market_quotes,
            market,
            self.vol_surface_id.as_str(),
            t,
            self.fixed_price,
        )?;

        // Black-76 on forward swap rate
        let unit_price = black76_swaption_price(
            forward,
            self.fixed_price,
            sigma,
            t,
            annuity,
            self.option_type,
        );

        Ok(Money::new(
            unit_price * self.notional,
            self.underlying.currency,
        ))
    }

    fn effective_start_date(&self) -> Option<Date> {
        Some(self.swap_start)
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

impl crate::instruments::common_impl::traits::OptionGreeksProvider for CommoditySwaption {
    fn option_delta(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<Option<f64>> {
        use finstack_core::math::special_functions::norm_cdf;

        let t = self
            .day_count
            .year_fraction(as_of, self.expiry, DayCountContext::default())?
            .max(0.0);

        let forward = self.forward_swap_rate(market, as_of)?;
        let annuity = self.annuity(market, as_of)?;

        if t <= 0.0 {
            let intrinsic = match self.option_type {
                OptionType::Call => {
                    if forward > self.fixed_price {
                        1.0
                    } else {
                        0.0
                    }
                }
                OptionType::Put => {
                    if forward < self.fixed_price {
                        -1.0
                    } else {
                        0.0
                    }
                }
            };
            return Ok(Some(intrinsic * annuity * self.notional));
        }

        let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
            &self.pricing_overrides.market_quotes,
            market,
            self.vol_surface_id.as_str(),
            t,
            self.fixed_price,
        )?;
        if sigma <= 0.0 {
            return Ok(Some(0.0));
        }

        let d1 = crate::models::d1_black76(forward, self.fixed_price, sigma, t);
        let nd1 = norm_cdf(d1);

        let delta_unit = match self.option_type {
            OptionType::Call => annuity * nd1,
            OptionType::Put => annuity * (nd1 - 1.0),
        };
        Ok(Some(delta_unit * self.notional))
    }

    fn option_gamma(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<Option<f64>> {
        use crate::instruments::common_impl::traits::Instrument;
        use finstack_core::market_data::bumps::{
            BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump,
        };

        let bump_pct = crate::metrics::bump_sizes::SPOT;
        let forward_price = self.forward_swap_rate(market, as_of)?;
        let bump_size = forward_price * bump_pct;
        if bump_size <= 0.0 {
            return Ok(Some(0.0));
        }

        let pv_base = self.value(market, as_of)?.amount();

        let curve_id = CurveId::new(self.forward_curve_id.as_str());
        let up = market.bump([MarketBump::Curve {
            id: curve_id.clone(),
            spec: BumpSpec {
                bump_type: BumpType::Parallel,
                mode: BumpMode::Additive,
                units: BumpUnits::Percent,
                value: bump_pct * 100.0,
            },
        }])?;
        let pv_up = self.value(&up, as_of)?.amount();

        let down = market.bump([MarketBump::Curve {
            id: curve_id,
            spec: BumpSpec {
                bump_type: BumpType::Parallel,
                mode: BumpMode::Additive,
                units: BumpUnits::Percent,
                value: -bump_pct * 100.0,
            },
        }])?;
        let pv_down = self.value(&down, as_of)?.amount();

        Ok(Some(
            (pv_up - 2.0 * pv_base + pv_down) / (bump_size * bump_size),
        ))
    }

    fn option_vega(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<Option<f64>> {
        use finstack_core::math::special_functions::norm_pdf;

        let t = self
            .day_count
            .year_fraction(as_of, self.expiry, DayCountContext::default())?
            .max(0.0);
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
            &self.pricing_overrides.market_quotes,
            market,
            self.vol_surface_id.as_str(),
            t,
            self.fixed_price,
        )?;
        if sigma <= 0.0 {
            return Ok(Some(0.0));
        }

        let forward = self.forward_swap_rate(market, as_of)?;
        let annuity = self.annuity(market, as_of)?;
        let d1 = crate::models::d1_black76(forward, self.fixed_price, sigma, t);
        // Vega = annuity * F * N'(d1) * sqrt(T) * 0.01 (per vol point)
        let vega_abs = annuity * forward * norm_pdf(d1) * t.sqrt();
        Ok(Some(vega_abs * 0.01 * self.notional))
    }
}

/// Black-76 swaption price.
///
/// C = annuity * [F * N(d1) - K * N(d2)]
/// P = annuity * [K * N(-d2) - F * N(-d1)]
///
/// The discount factor is already embedded in the annuity factor.
fn black76_swaption_price(
    forward: f64,
    strike: f64,
    sigma: f64,
    t: f64,
    annuity: f64,
    option_type: OptionType,
) -> f64 {
    if t <= 0.0 || sigma <= 0.0 {
        let intrinsic = match option_type {
            OptionType::Call => (forward - strike).max(0.0),
            OptionType::Put => (strike - forward).max(0.0),
        };
        return intrinsic * annuity;
    }

    let d1 = crate::models::d1_black76(forward, strike, sigma, t);
    let d2 = crate::models::d2_black76(forward, strike, sigma, t);

    let price = match option_type {
        OptionType::Call => {
            forward * finstack_core::math::norm_cdf(d1) - strike * finstack_core::math::norm_cdf(d2)
        }
        OptionType::Put => {
            strike * finstack_core::math::norm_cdf(-d2)
                - forward * finstack_core::math::norm_cdf(-d1)
        }
    };

    price * annuity
}

crate::impl_empty_cashflow_provider!(
    CommoditySwaption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;

    // -----------------------------------------------------------------------
    // Validation tests
    // -----------------------------------------------------------------------

    fn base_swaption_builder() -> CommoditySwaptionBuilder {
        use finstack_core::dates::TenorUnit;
        CommoditySwaption::builder()
            .id(InstrumentId::new("NG-SWAPTION-VALID"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "NG",
                "MMBTU",
                Currency::USD,
            ))
            .option_type(OptionType::Call)
            .expiry(Date::from_calendar_date(2025, time::Month::June, 15).expect("valid date"))
            .swap_start(Date::from_calendar_date(2025, time::Month::July, 1).expect("valid date"))
            .swap_end(Date::from_calendar_date(2026, time::Month::June, 30).expect("valid date"))
            .swap_frequency(Tenor::new(1, TenorUnit::Months))
            .fixed_price(3.50)
            .notional(10_000.0)
            .forward_curve_id(CurveId::new("NG-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("NG-VOL"))
    }

    #[test]
    fn validation_valid_swaption_builds_ok() {
        assert!(base_swaption_builder().build().is_ok());
    }

    #[test]
    fn validation_rejects_swap_start_after_swap_end() {
        // swap_start == swap_end: invalid
        let result = base_swaption_builder()
            .swap_start(Date::from_calendar_date(2026, time::Month::June, 30).expect("valid date"))
            .swap_end(Date::from_calendar_date(2025, time::Month::July, 1).expect("valid date"))
            .build();
        assert!(
            result.is_err(),
            "CommoditySwaption must reject swap_start > swap_end"
        );
    }

    #[test]
    fn validation_rejects_swap_start_equal_swap_end() {
        let same_date = Date::from_calendar_date(2025, time::Month::July, 1).expect("valid date");
        let result = base_swaption_builder()
            .swap_start(same_date)
            .swap_end(same_date)
            .build();
        assert!(
            result.is_err(),
            "CommoditySwaption must reject swap_start == swap_end"
        );
    }

    #[test]
    fn validation_rejects_expiry_after_swap_start() {
        // expiry > swap_start: invalid
        let result = base_swaption_builder()
            .expiry(Date::from_calendar_date(2025, time::Month::August, 1).expect("valid date"))
            .swap_start(Date::from_calendar_date(2025, time::Month::July, 1).expect("valid date"))
            .build();
        assert!(
            result.is_err(),
            "CommoditySwaption must reject expiry > swap_start"
        );
    }

    #[test]
    fn validation_accepts_expiry_equal_swap_start() {
        // expiry == swap_start is allowed (option expires exactly when swap starts)
        let same_date = Date::from_calendar_date(2025, time::Month::July, 1).expect("valid date");
        let result = base_swaption_builder()
            .expiry(same_date)
            .swap_start(same_date)
            .build();
        assert!(
            result.is_ok(),
            "CommoditySwaption must allow expiry == swap_start"
        );
    }

    #[test]
    fn validation_rejects_zero_notional() {
        let result = base_swaption_builder().notional(0.0).build();
        assert!(
            result.is_err(),
            "CommoditySwaption must reject zero notional"
        );
    }

    #[test]
    fn validation_rejects_negative_notional() {
        let result = base_swaption_builder().notional(-1000.0).build();
        assert!(
            result.is_err(),
            "CommoditySwaption must reject negative notional"
        );
    }

    #[test]
    fn validation_rejects_nan_fixed_price() {
        let result = base_swaption_builder().fixed_price(f64::NAN).build();
        assert!(
            result.is_err(),
            "CommoditySwaption must reject NaN fixed_price"
        );
    }

    #[test]
    fn validation_rejects_inf_fixed_price() {
        let result = base_swaption_builder().fixed_price(f64::INFINITY).build();
        assert!(
            result.is_err(),
            "CommoditySwaption must reject infinite fixed_price"
        );
    }

    #[test]
    fn validation_accepts_negative_fixed_price() {
        // Negative fixed price is legitimate for certain commodity spreads
        let result = base_swaption_builder().fixed_price(-1.0).build();
        assert!(
            result.is_ok(),
            "CommoditySwaption must allow negative fixed_price"
        );
    }

    #[test]
    fn validation_rejects_zero_frequency_count() {
        let result = base_swaption_builder()
            .swap_frequency(Tenor::new(0, finstack_core::dates::TenorUnit::Months))
            .build();
        assert!(
            result.is_err(),
            "CommoditySwaption must reject swap_frequency with count = 0"
        );
    }

    #[test]
    fn test_commodity_swaption_example() {
        let swaption = CommoditySwaption::example();
        assert_eq!(swaption.id.as_str(), "NG-SWAPTION-2025");
        assert_eq!(swaption.underlying.ticker, "NG");
    }

    #[test]
    fn test_commodity_swaption_instrument_trait() {
        let swaption = CommoditySwaption::example();
        assert_eq!(swaption.id(), "NG-SWAPTION-2025");
        assert_eq!(
            swaption.key(),
            crate::pricer::InstrumentType::CommoditySwaption
        );
    }

    #[test]
    fn test_commodity_swaption_curve_dependencies() {
        let swaption = CommoditySwaption::example();
        let deps = swaption.curve_dependencies().expect("curve_dependencies");
        assert_eq!(deps.discount_curves.len(), 1);
        assert_eq!(deps.forward_curves.len(), 1);
    }

    #[test]
    fn test_commodity_swaption_serde_roundtrip() {
        let swaption = CommoditySwaption::example();
        let json = serde_json::to_string(&swaption).expect("serialize");
        let deserialized: CommoditySwaption = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(swaption.id.as_str(), deserialized.id.as_str());
        assert_eq!(swaption.underlying.ticker, deserialized.underlying.ticker);
        assert_eq!(swaption.fixed_price, deserialized.fixed_price);
    }

    /// W-02 / M-commodity-averaging: `forward_swap_rate` must use the
    /// **period-average** forward over each half-open settlement window —
    /// the same business-day average the underlying floating leg settles on
    /// — not a single end-of-period (payment-date) print. On a sloped
    /// (contango) forward curve the payment-date forward exceeds the period
    /// average by ~half a period of carry, so sampling the payment date
    /// biases the swap rate upward.
    ///
    /// This test builds a steep linear contango curve and asserts the
    /// computed forward swap rate equals an independently reconstructed
    /// annuity-weighted average of business-day period averages — and that
    /// it differs measurably from the payment-date-sampled average the
    /// pre-fix code produced.
    #[test]
    fn w02_forward_swap_rate_uses_period_average_not_payment_date() {
        use finstack_core::dates::TenorUnit;
        use finstack_core::market_data::term_structures::{DiscountCurve, PriceCurve};
        use finstack_core::types::CurveId;
        use time::Month;

        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        // Steep linear contango: price rises ~1.0/yr. Midpoint vs payment-date
        // forwards then differ by ~half a period of carry.
        let price_curve = PriceCurve::builder("NG-FORWARD")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .spot_price(3.00)
            .knots([(0.0, 3.00), (0.5, 3.50), (1.0, 4.00), (2.0, 5.00)])
            .interp(finstack_core::math::interp::InterpStyle::Linear)
            .build()
            .expect("price curve");

        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (2.0, 0.94)])
            .build()
            .expect("discount curve");

        let market = MarketContext::new().insert(disc).insert(price_curve);

        let swaption = CommoditySwaption::builder()
            .id(InstrumentId::new("NG-SWAPTION-W02"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "NG",
                "MMBTU",
                Currency::USD,
            ))
            .option_type(OptionType::Call)
            .expiry(Date::from_calendar_date(2025, Month::June, 1).expect("valid date"))
            .swap_start(Date::from_calendar_date(2025, Month::July, 1).expect("valid date"))
            .swap_end(Date::from_calendar_date(2026, Month::July, 1).expect("valid date"))
            .swap_frequency(Tenor::new(3, TenorUnit::Months))
            .fixed_price(4.0)
            .notional(10000.0)
            .forward_curve_id(CurveId::new("NG-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("NG-VOL"))
            .build()
            .expect("swaption");

        let actual = swaption
            .forward_swap_rate(&market, as_of)
            .expect("forward swap rate");

        // Re-derive the expected payment-date-weighted average and the buggy
        // midpoint-weighted average independently.
        let schedule = swaption.swap_payment_schedule().expect("schedule");
        let pc = market.get_price_curve("NG-FORWARD").expect("price curve");
        let dc = market.get_discount("USD-OIS").expect("discount");

        let is_weekday = |d: Date| -> bool {
            let wd = d.weekday();
            wd != time::Weekday::Saturday && wd != time::Weekday::Sunday
        };

        let last_pay = *schedule.last().expect("non-empty schedule");
        let mut prev = swaption.swap_start;
        let mut avg_weighted = 0.0;
        let mut pay_weighted = 0.0;
        let mut weight_total = 0.0;
        for &pay in &schedule {
            // Annuity weight is DF only (B3): the underlying swap pays
            // quantity × price per period with no year-fraction accrual.
            let weight = dc.df_between_dates(as_of, pay).expect("df");
            let fwd_pay = pc.price_on_date(pay).expect("fwd at payment date");

            // Independent business-day average over the half-open window
            // [prev, pay), with the final period also observing `pay`.
            let mut sum = 0.0;
            let mut count = 0u64;
            let mut cur = prev;
            while cur < pay {
                if is_weekday(cur) {
                    sum += pc.price_on_date(cur).expect("fwd inside window");
                    count += 1;
                }
                cur += time::Duration::days(1);
            }
            if pay == last_pay && is_weekday(pay) {
                sum += fwd_pay;
                count += 1;
            }
            let fwd_avg = sum / count as f64;

            avg_weighted += fwd_avg * weight;
            pay_weighted += fwd_pay * weight;
            weight_total += weight;
            prev = pay;
        }
        let expected_average = avg_weighted / weight_total;
        let buggy_payment_date = pay_weighted / weight_total;

        assert!(
            (actual - expected_average).abs() < 1e-10,
            "forward_swap_rate {actual} must equal the period-average-weighted \
             rate {expected_average}"
        );
        // Sanity: contango means the payment-date print exceeds the period
        // average, so the fix changes the result measurably.
        assert!(
            (expected_average - buggy_payment_date).abs() > 1e-3,
            "test setup is degenerate: period-average ({expected_average}) and \
             payment-date ({buggy_payment_date}) rates must differ on a sloped curve"
        );
        assert!(
            (actual - buggy_payment_date).abs() > 1e-3,
            "forward_swap_rate {actual} must NOT equal the payment-date-\
             sampled average {buggy_payment_date}"
        );
    }

    /// Par consistency with the underlying swap: at zero vol, an ITM call
    /// swaption (right to pay fixed) on a sloped curve must price to the PV
    /// of the underlying pay-fixed swap, because both now settle on the
    /// same half-open business-day period averages. Mismatched averaging
    /// conventions (payment-date sampling vs period averages, or closed vs
    /// half-open windows) break this identity on a sloped curve.
    #[test]
    fn zero_vol_itm_swaption_matches_underlying_swap_pv_on_sloped_curve() {
        use finstack_core::dates::TenorUnit;
        use finstack_core::market_data::term_structures::{DiscountCurve, PriceCurve};
        use time::Month;

        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        // Sloped contango curve so averaging conventions matter.
        let price_curve = PriceCurve::builder("NG-FORWARD")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .spot_price(3.00)
            .knots([(0.0, 3.00), (1.0, 4.00), (2.0, 5.00)])
            .interp(finstack_core::math::interp::InterpStyle::Linear)
            .build()
            .expect("price curve");
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (2.0, 0.94)])
            .build()
            .expect("discount curve");
        let market = MarketContext::new().insert(disc).insert(price_curve);

        let swap_start = Date::from_calendar_date(2025, Month::July, 1).expect("valid date");
        let swap_end = Date::from_calendar_date(2026, Month::July, 1).expect("valid date");
        let strike = 2.0; // deep ITM for a call (pay-fixed) in contango

        let mut swaption = CommoditySwaption::builder()
            .id(InstrumentId::new("NG-SWAPTION-PAR"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "NG",
                "MMBTU",
                Currency::USD,
            ))
            .option_type(OptionType::Call)
            .expiry(Date::from_calendar_date(2025, Month::June, 1).expect("valid date"))
            .swap_start(swap_start)
            .swap_end(swap_end)
            .swap_frequency(Tenor::new(1, TenorUnit::Months))
            .fixed_price(strike)
            .notional(10_000.0)
            .forward_curve_id(CurveId::new("NG-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("NG-VOL"))
            .build()
            .expect("swaption");
        swaption.pricing_overrides.market_quotes.implied_volatility = Some(0.0);

        let swaption_pv = swaption
            .value(&market, as_of)
            .expect("swaption pricing")
            .amount();

        // Underlying pay-fixed swap with the same schedule parameters.
        let swap = super::super::super::commodity_swap::CommoditySwap::builder()
            .id(InstrumentId::new("NG-SWAP-PAR"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "NG",
                "MMBTU",
                Currency::USD,
            ))
            .quantity(10_000.0)
            .fixed_price(rust_decimal::Decimal::try_from(strike).expect("decimal"))
            .floating_index_id(CurveId::new("NG-FORWARD"))
            .side(crate::instruments::PayReceive::Pay)
            .start_date(swap_start)
            .maturity(swap_end)
            .frequency(Tenor::new(1, TenorUnit::Months))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("swap");
        let swap_pv = swap.value(&market, as_of).expect("swap pricing").amount();

        let rel = (swaption_pv - swap_pv).abs() / swap_pv.abs();
        assert!(
            rel < 1e-6,
            "zero-vol ITM swaption PV ({swaption_pv}) must match the \
             underlying swap PV ({swap_pv}); rel err {rel}"
        );
    }

    // -----------------------------------------------------------------------
    // Black-76 pricing behavior tests
    // -----------------------------------------------------------------------

    fn date(year: i32, month: u8, day: u8) -> Date {
        use time::Month;
        Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
            .expect("valid date")
    }

    fn flat_vol_surface(id: &str, vol: f64) -> finstack_core::market_data::surfaces::VolSurface {
        use finstack_core::market_data::surfaces::VolSurface;
        let expiries = [0.25, 0.5, 1.0, 2.0];
        let strikes = [2.0, 3.0, 3.5, 4.0, 5.0];
        let mut builder = VolSurface::builder(id)
            .expiries(&expiries)
            .strikes(&strikes);
        for _ in &expiries {
            builder = builder.row(&vec![vol; strikes.len()]);
        }
        builder.build().expect("vol surface should build in tests")
    }

    fn build_market(as_of: Date, flat_fwd: f64, vol: f64, rate: f64) -> MarketContext {
        use finstack_core::market_data::term_structures::{DiscountCurve, PriceCurve};

        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, (-rate * 5.0).exp())])
            .build()
            .expect("discount curve");

        let price_curve = PriceCurve::builder("NG-FORWARD")
            .base_date(as_of)
            .spot_price(flat_fwd)
            .knots([(0.0, flat_fwd), (2.0, flat_fwd)])
            .build()
            .expect("price curve");

        MarketContext::new()
            .insert(disc)
            .insert(price_curve)
            .insert_surface(flat_vol_surface("NG-VOL", vol))
    }

    fn base_swaption(option_type: OptionType, fixed_price: f64) -> CommoditySwaption {
        use finstack_core::dates::TenorUnit;

        CommoditySwaption::builder()
            .id(InstrumentId::new("TEST-SWAPTION"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "NG",
                "MMBTU",
                Currency::USD,
            ))
            .option_type(option_type)
            .expiry(date(2025, 6, 15))
            .swap_start(date(2025, 7, 1))
            .swap_end(date(2026, 6, 30))
            .swap_frequency(Tenor::new(1, TenorUnit::Months))
            .fixed_price(fixed_price)
            .notional(10000.0)
            .forward_curve_id(CurveId::new("NG-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("NG-VOL"))
            .day_count(DayCount::Act365F)
            .build()
            .expect("should build")
    }

    #[test]
    fn test_atm_swaption_price_positive() {
        let as_of = date(2025, 1, 2);
        let fwd = 3.50;
        let market = build_market(as_of, fwd, 0.30, 0.05);

        // ATM: fixed_price = forward
        let swaption = base_swaption(OptionType::Call, fwd);
        let pv = swaption
            .base_value(&market, as_of)
            .expect("pricing should succeed");

        assert!(
            pv.amount() > 0.0,
            "ATM swaption should have positive value, got {}",
            pv.amount()
        );
    }

    #[test]
    fn test_deep_itm_call_approaches_intrinsic() {
        let as_of = date(2025, 1, 2);
        let fwd = 5.00;
        let market = build_market(as_of, fwd, 0.30, 0.05);

        // Deep ITM call: strike << forward
        let swaption = base_swaption(OptionType::Call, 2.00);

        let pv = swaption
            .value(&market, as_of)
            .expect("pricing should succeed");

        // Compute intrinsic ~ annuity * (F - K) * notional
        let annuity = swaption.annuity(&market, as_of).expect("annuity");
        let intrinsic = (fwd - 2.00) * annuity * swaption.notional;

        assert!(
            pv.amount() >= intrinsic * 0.95,
            "Deep ITM call PV ({}) should be near intrinsic ({})",
            pv.amount(),
            intrinsic
        );
    }

    #[test]
    fn test_put_call_parity() {
        // Put-call parity: C - P = annuity * (F - K) * notional
        let as_of = date(2025, 1, 2);
        let fwd = 3.50;
        let strike = 3.30;
        let market = build_market(as_of, fwd, 0.30, 0.05);

        let call = base_swaption(OptionType::Call, strike);
        let put = base_swaption(OptionType::Put, strike);

        let call_pv = call
            .value(&market, as_of)
            .expect("call pricing should succeed")
            .amount();
        let put_pv = put
            .value(&market, as_of)
            .expect("put pricing should succeed")
            .amount();

        let annuity = call.annuity(&market, as_of).expect("annuity");
        let forward = call.forward_swap_rate(&market, as_of).expect("forward");
        let parity_rhs = annuity * (forward - strike) * call.notional;

        let diff = (call_pv - put_pv) - parity_rhs;
        assert!(
            diff.abs() < 1.0,
            "Put-call parity violated: C-P={}, annuity*(F-K)*N={}, diff={}",
            call_pv - put_pv,
            parity_rhs,
            diff
        );
    }

    #[test]
    fn test_zero_vol_gives_intrinsic() {
        let as_of = date(2025, 1, 2);
        let fwd = 4.00;
        let strike = 3.50;

        // Use pricing override for zero vol to bypass vol surface
        let mut swaption = base_swaption(OptionType::Call, strike);
        swaption.pricing_overrides.market_quotes.implied_volatility = Some(0.0);

        let market = build_market(as_of, fwd, 0.30, 0.05);

        let pv = swaption
            .value(&market, as_of)
            .expect("pricing should succeed");

        let annuity = swaption.annuity(&market, as_of).expect("annuity");
        let forward = swaption.forward_swap_rate(&market, as_of).expect("forward");
        let expected_intrinsic = (forward - strike).max(0.0) * annuity * swaption.notional;

        assert!(
            (pv.amount() - expected_intrinsic).abs() < 0.01,
            "Zero vol call should equal intrinsic: got {}, expected {}",
            pv.amount(),
            expected_intrinsic
        );
    }

    #[test]
    fn test_zero_vol_otm_gives_zero() {
        let as_of = date(2025, 1, 2);
        let fwd = 3.00;
        let strike = 4.00;

        let mut swaption = base_swaption(OptionType::Call, strike);
        swaption.pricing_overrides.market_quotes.implied_volatility = Some(0.0);

        let market = build_market(as_of, fwd, 0.30, 0.05);

        let pv = swaption
            .value(&market, as_of)
            .expect("pricing should succeed");

        assert!(
            pv.amount().abs() < 0.01,
            "OTM call with zero vol should be ~0, got {}",
            pv.amount()
        );
    }

    /// The swaption annuity must be consistent with its own underlying.
    /// A `CommoditySwap` pays `quantity × price` per period with no
    /// year-fraction accrual, so a zero-vol ITM call swaption on a flat
    /// forward curve must equal `notional × (F − K) × Σ DF_i` — computed here
    /// independently from the discount curve. The pre-fix `Σ DF·τ` annuity
    /// understated a monthly-settling swaption ~12×.
    #[test]
    fn b3_zero_vol_itm_swaption_matches_underlying_swap_pv() {
        let as_of = date(2025, 1, 2);
        let fwd = 4.00;
        let strike = 3.50;

        // Monthly settlement (τ ≈ 1/12) makes the old mis-scaling ~12×.
        let mut swaption = base_swaption(OptionType::Call, strike);
        swaption.pricing_overrides.market_quotes.implied_volatility = Some(0.0);

        let market = build_market(as_of, fwd, 0.30, 0.05);
        let pv = swaption
            .value(&market, as_of)
            .expect("pricing should succeed")
            .amount();

        // Independent reference: per-period payoff quantity × (F − K),
        // discounted to each payment date — exactly what the underlying
        // CommoditySwap would pay if exercised.
        let dc = market.get_discount("USD-OIS").expect("discount");
        let schedule = swaption.swap_payment_schedule().expect("schedule");
        let sum_df: f64 = schedule
            .iter()
            .map(|&d| dc.df_between_dates(as_of, d).expect("df"))
            .sum();
        let expected = swaption.notional * (fwd - strike) * sum_df;

        let rel = (pv - expected).abs() / expected;
        assert!(
            rel < 1e-10,
            "zero-vol ITM swaption PV ({pv}) must equal the underlying swap \
             payoff PV ({expected}); rel err {rel}"
        );
        // Guard against the old Σ DF·τ annuity (~12× smaller for monthly).
        assert!(
            pv > expected * 0.5,
            "swaption PV ({pv}) shows the Σ DF·τ mis-scaling vs ({expected})"
        );
    }
}
