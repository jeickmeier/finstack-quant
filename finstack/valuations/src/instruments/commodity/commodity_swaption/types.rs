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
}

impl CommoditySwaption {
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
    /// The forward swap rate is the **annuity-weighted** average of forward
    /// commodity prices at each swap payment period midpoint:
    /// ```text
    /// F_swap = Σ (F_i · DF_i · τ_i) / Σ (DF_i · τ_i)
    /// ```
    /// This is the fair fixed price consistent with the `annuity · Black76`
    /// pricing identity: the swaption is priced as `annuity · Black76(F_swap, K)`
    /// where `annuity = Σ DF_i · τ_i`, so the fair swap rate must be averaged
    /// with the same `DF_i · τ_i` weights. It reduces to the equal-weighted mean
    /// when `DF_i · τ_i` is constant across periods. If the annuity denominator
    /// is zero (degenerate schedule), the equal-weighted mean is returned.
    pub fn forward_swap_rate(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
        let price_curve = market.get_price_curve(self.forward_curve_id.as_str())?;
        let disc = market.get_discount(self.discount_curve_id.as_str())?;
        let schedule = self.swap_payment_schedule()?;

        if schedule.is_empty() {
            return Err(finstack_core::Error::Validation(
                "CommoditySwaption: underlying swap has no payment dates".to_string(),
            ));
        }

        let mut sum_fwd = 0.0;
        let mut weighted_fwd = 0.0;
        let mut weight_total = 0.0;
        let mut prev = self.swap_start;
        for &payment_date in &schedule {
            // Use the midpoint of each period for forward price lookup
            let mid = prev + (payment_date - prev) / 2;
            let fwd = price_curve
                .price_on_date(mid)
                .unwrap_or_else(|_| price_curve.spot_price());

            // Annuity weight DF_i * tau_i — identical to the per-period term
            // accumulated in `annuity()`.
            let df = disc.df_between_dates(as_of, payment_date)?;
            let period_frac =
                self.day_count
                    .year_fraction(prev, payment_date, DayCountContext::default())?;
            let weight = df * period_frac;

            sum_fwd += fwd;
            weighted_fwd += fwd * weight;
            weight_total += weight;
            prev = payment_date;
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
    /// The annuity is the sum of (discount factor * period year fraction) for
    /// each payment period, representing the PV of receiving 1 unit per period.
    pub fn annuity(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
        let disc = market.get_discount(self.discount_curve_id.as_str())?;
        let schedule = self.swap_payment_schedule()?;

        let mut annuity = 0.0;
        let mut prev = self.swap_start;
        for &payment_date in &schedule {
            let df = disc.df_between_dates(as_of, payment_date)?;
            let period_frac =
                self.day_count
                    .year_fraction(prev, payment_date, DayCountContext::default())?;
            annuity += df * period_frac;
            prev = payment_date;
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

impl crate::instruments::common_impl::traits::OptionDeltaProvider for CommoditySwaption {
    fn option_delta(&self, market: &MarketContext, as_of: Date) -> finstack_core::Result<f64> {
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
            return Ok(intrinsic * annuity * self.notional);
        }

        let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
            &self.pricing_overrides.market_quotes,
            market,
            self.vol_surface_id.as_str(),
            t,
            self.fixed_price,
        )?;
        if sigma <= 0.0 {
            return Ok(0.0);
        }

        let d1 = crate::instruments::common_impl::models::d1_black76(
            forward,
            self.fixed_price,
            sigma,
            t,
        );
        let nd1 = norm_cdf(d1);

        let delta_unit = match self.option_type {
            OptionType::Call => annuity * nd1,
            OptionType::Put => annuity * (nd1 - 1.0),
        };
        Ok(delta_unit * self.notional)
    }
}

impl crate::instruments::common_impl::traits::OptionGammaProvider for CommoditySwaption {
    fn option_gamma(&self, market: &MarketContext, as_of: Date) -> finstack_core::Result<f64> {
        use crate::instruments::common_impl::traits::Instrument;
        use finstack_core::market_data::bumps::{
            BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump,
        };

        let bump_pct = crate::metrics::bump_sizes::SPOT;
        let forward_price = self.forward_swap_rate(market, as_of)?;
        let bump_size = forward_price * bump_pct;
        if bump_size <= 0.0 {
            return Ok(0.0);
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

        Ok((pv_up - 2.0 * pv_base + pv_down) / (bump_size * bump_size))
    }
}

impl crate::instruments::common_impl::traits::OptionGreeksProvider for CommoditySwaption {
    fn option_greeks(
        &self,
        market: &MarketContext,
        as_of: Date,
        request: &crate::instruments::common_impl::traits::OptionGreeksRequest,
    ) -> finstack_core::Result<crate::instruments::common_impl::traits::OptionGreeks> {
        use crate::instruments::common_impl::traits::{
            OptionDeltaProvider, OptionGammaProvider, OptionGreekKind, OptionGreeks,
            OptionVegaProvider,
        };

        match request.greek {
            OptionGreekKind::Delta => Ok(OptionGreeks {
                delta: Some(OptionDeltaProvider::option_delta(self, market, as_of)?),
                ..OptionGreeks::default()
            }),
            OptionGreekKind::Gamma => Ok(OptionGreeks {
                gamma: Some(OptionGammaProvider::option_gamma(self, market, as_of)?),
                ..OptionGreeks::default()
            }),
            OptionGreekKind::Vega => Ok(OptionGreeks {
                vega: Some(OptionVegaProvider::option_vega(self, market, as_of)?),
                ..OptionGreeks::default()
            }),
            _ => Ok(OptionGreeks::default()),
        }
    }
}

impl crate::instruments::common_impl::traits::OptionVegaProvider for CommoditySwaption {
    fn option_vega(&self, market: &MarketContext, as_of: Date) -> finstack_core::Result<f64> {
        use finstack_core::math::special_functions::norm_pdf;

        let t = self
            .day_count
            .year_fraction(as_of, self.expiry, DayCountContext::default())?
            .max(0.0);
        if t <= 0.0 {
            return Ok(0.0);
        }

        let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
            &self.pricing_overrides.market_quotes,
            market,
            self.vol_surface_id.as_str(),
            t,
            self.fixed_price,
        )?;
        if sigma <= 0.0 {
            return Ok(0.0);
        }

        let forward = self.forward_swap_rate(market, as_of)?;
        let annuity = self.annuity(market, as_of)?;
        let d1 = crate::instruments::common_impl::models::d1_black76(
            forward,
            self.fixed_price,
            sigma,
            t,
        );
        // Vega = annuity * F * N'(d1) * sqrt(T) * 0.01 (per vol point)
        let vega_abs = annuity * forward * norm_pdf(d1) * t.sqrt();
        Ok(vega_abs * 0.01 * self.notional)
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

    let d1 = crate::instruments::common_impl::models::d1_black76(forward, strike, sigma, t);
    let d2 = crate::instruments::common_impl::models::d2_black76(forward, strike, sigma, t);

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
}
