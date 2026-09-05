//! FX barrier option instrument definition.
//!
//! Strike and barrier are plain `f64` exchange rates (quote-per-base), consistent
//! with all other FX option modules (`fx_option`, `fx_digital_option`,
//! `fx_touch_option`).

use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::OptionType;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::BarrierType;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};
/// Contractual barrier-monitoring convention.
#[derive(PartialEq, Eq, Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Monitoring {
    /// Monitor continuously from `monitoring_start_date` through expiry.
    #[default]
    Continuous,
    /// Monitor only at the stated contractual observation dates.
    Discrete {
        /// Strictly increasing dates on which the barrier level is observed.
        #[serde(with = "finstack_quant_core::wire::dates")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
        )]
        observation_dates: Vec<Date>,
    },
}

/// FX barrier option instrument.
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
pub struct FxBarrierOption {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Strike exchange rate (quote per base, dimensionless)
    pub strike: f64,
    /// Barrier level (exchange rate that triggers knock-in/out, dimensionless)
    pub barrier: f64,
    /// Optional rebate amount (paid if the barrier condition is met, dimensionless;
    /// see `rebate_timing` for when a knock-out rebate pays)
    pub rebate: Option<f64>,
    /// Timing of the knock-out rebate payment.
    ///
    /// `at_hit` (default, market standard) pays the rebate the moment a
    /// knock-out barrier is breached; `at_expiry` defers payment to expiry.
    /// Knock-in rebates always pay at expiry, so this setting does not affect
    /// them. The analytical pricer values at-hit rebates via the discounted
    /// first-passage closed form. Monte Carlo applies at-hit when
    /// `rebate_timing == AtHit` via `with_rebate_at_hit`; the crate primitive
    /// defaults to at-expiry.
    #[builder(default)]
    #[serde(default)]
    pub rebate_timing: finstack_quant_models::closed_form::barrier::RebateTiming,
    /// Option type (call or put on foreign currency)
    pub option_type: OptionType,
    /// Barrier type (up/down, in/out)
    pub barrier_type: BarrierType,
    /// Option expiry date
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub expiry: Date,
    /// First date on which barrier monitoring is active. When set, a live
    /// valuation after this date requires `observed_barrier_breached`.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "finstack_quant_core::wire::optional_date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub monitoring_start_date: Option<Date>,
    /// Observed barrier state for expired options.
    ///
    /// Historical monitoring must be supplied explicitly for expired contracts.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_barrier_breached: Option<bool>,
    /// Notional amount in foreign currency
    pub notional: Money,
    /// Base currency (the currency being priced, formerly foreign_currency)
    pub base_currency: Currency,
    /// Quote currency (the pricing/settlement currency, formerly domestic_currency)
    pub quote_currency: Currency,
    /// Day count convention (defaults to ACT/365F, consistent with FxOption)
    #[serde(default = "crate::serde_defaults::day_count_act365f")]
    #[builder(default = finstack_quant_core::dates::DayCount::Act365F)]
    pub day_count: finstack_quant_core::dates::DayCount,
    /// Contractual barrier-monitoring convention.
    ///
    /// Continuous monitoring uses the analytical Reiner-Rubinstein pricer by
    /// default. Discrete monitoring requires explicit observation dates and
    /// uses Monte Carlo without interpolating barrier hits between those dates.
    #[serde(default)]
    #[builder(default)]
    pub monitoring: Monitoring,
    /// Domestic discount curve ID
    pub domestic_discount_curve_id: CurveId,
    /// Foreign discount curve ID
    pub foreign_discount_curve_id: CurveId,
    /// Optional FX spot scalar identifier.
    ///
    /// If omitted, pricing falls back to `FxMatrix(base_currency, quote_currency)`.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fx_spot_id: Option<PriceId>,
    /// FX volatility surface ID
    pub vol_surface_id: CurveId,
    /// Pricing overrides (manual price, yield, spread)
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
    /// Attributes for scenario selection and grouping
    pub attributes: Attributes,
}

// Declare canonical market dependencies for the DV01 calculator.
// FxBarrierOption uses both domestic and foreign curves for FX carry calculation
impl FxBarrierOption {
    /// Validate FX barrier option currency invariants.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        crate::instruments::common_impl::validation::validate_distinct_currencies(
            self.base_currency,
            self.quote_currency,
            "FxBarrierOption",
        )?;
        crate::instruments::common_impl::validation::validate_f64_positive(
            self.strike,
            "FxBarrierOption strike",
        )?;
        crate::instruments::common_impl::validation::validate_f64_positive(
            self.barrier,
            "FxBarrierOption barrier",
        )?;
        crate::instruments::common_impl::validation::validate_money_finite(
            self.notional,
            "FxBarrierOption notional",
        )?;
        if self.notional.amount() <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "FxBarrierOption notional must be positive".to_string(),
            ));
        }
        if self.notional.currency() != self.base_currency {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: self.base_currency,
                actual: self.notional.currency(),
            });
        }
        let start = self.monitoring_start_date.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "FxBarrierOption requires monitoring_start_date".to_string(),
            )
        })?;
        if start > self.expiry {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FxBarrierOption monitoring_start_date ({start}) must not be after expiry ({})",
                self.expiry
            )));
        }
        if let Monitoring::Discrete { observation_dates } = &self.monitoring {
            if observation_dates.is_empty() {
                return Err(finstack_quant_core::Error::Validation(
                    "FxBarrierOption discrete monitoring requires observation_dates".to_string(),
                ));
            }
            if observation_dates.first().copied() != Some(start) {
                return Err(finstack_quant_core::Error::Validation(
                    "FxBarrierOption monitoring_start_date must equal the first discrete observation date"
                        .to_string(),
                ));
            }
            if observation_dates.iter().any(|date| *date > self.expiry) {
                return Err(finstack_quant_core::Error::Validation(
                    "FxBarrierOption observation_dates must not be after expiry".to_string(),
                ));
            }
            if observation_dates
                .windows(2)
                .any(|dates| dates[0] >= dates[1])
            {
                return Err(finstack_quant_core::Error::Validation(
                    "FxBarrierOption observation_dates must be strictly increasing".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Create a canonical example FX barrier option (EURUSD up-and-out call).
    ///
    /// # Currency Conventions
    ///
    /// For EUR/USD (foreign=EUR, domestic=USD):
    /// - Strike and barrier are dimensionless exchange rates (USD per EUR)
    /// - Notional is in EUR (the foreign/base currency being bought)
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        use finstack_quant_core::dates::DayCount;
        use time::Month;
        FxBarrierOption::builder()
            .id(InstrumentId::new("FXBAR-EURUSD-UO-CALL"))
            .strike(1.10) // Strike rate (USD per EUR)
            .barrier(1.20) // Barrier rate (USD per EUR)
            .option_type(crate::instruments::OptionType::Call)
            .barrier_type(BarrierType::UpAndOut)
            .expiry(
                Date::from_calendar_date(2024, Month::December, 20).expect("Valid example date"),
            )
            .monitoring_start_date_opt(Some(
                Date::from_calendar_date(2024, Month::January, 1).expect("Valid example date"),
            ))
            .observed_barrier_breached_opt(None)
            .notional(Money::from((1_000_000_i64, Currency::EUR))) // Notional in foreign currency (EUR)
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .day_count(DayCount::Act365F)
            .monitoring(Monitoring::Continuous)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .fx_spot_id_opt(Some("EURUSD-SPOT".into()))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .attributes(Attributes::new())
            .build()
            .expect("Example FxBarrierOption construction should not fail")
    }
}

// Option risk metric providers (metrics adapters)

impl crate::instruments::common_impl::traits::OptionGreeksProvider for FxBarrierOption {
    fn option_delta(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }
        let spot_id = self.fx_spot_id.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "FxBarrierOption delta requires fx_spot_id for finite-difference spot bumps"
                    .to_string(),
            )
        })?;
        Ok(Some(crate::metrics::central_diff_scalar_relative(
            self,
            market,
            as_of,
            spot_id,
            crate::metrics::bump_sizes::SPOT,
        )?))
    }

    fn option_gamma(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        use crate::instruments::common_impl::traits::Instrument;

        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let base_pv = self.value(market, as_of)?.amount();

        let spot_id = self.fx_spot_id.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "FxBarrierOption gamma requires fx_spot_id for finite-difference spot bumps"
                    .to_string(),
            )
        })?;
        let spot_scalar = market.get_price(spot_id)?;
        let current_spot = crate::metrics::scalar_numeric_value(spot_scalar);
        let bump_size = current_spot * crate::metrics::bump_sizes::SPOT;
        if bump_size <= 0.0 {
            return Ok(Some(0.0));
        }

        let up =
            crate::metrics::bump_scalar_price(market, spot_id, crate::metrics::bump_sizes::SPOT)?;
        let pv_up = self.value(&up, as_of)?.amount();
        let down =
            crate::metrics::bump_scalar_price(market, spot_id, -crate::metrics::bump_sizes::SPOT)?;
        let pv_down = self.value(&down, as_of)?.amount();

        Ok(Some(
            (pv_up - 2.0 * base_pv + pv_down) / (bump_size * bump_size),
        ))
    }

    fn option_vega(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        use crate::instruments::common_impl::traits::Instrument;

        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let vol_bump = crate::metrics::bump_sizes::VOLATILITY;
        let up = crate::metrics::bump_surface_vol_absolute(
            market,
            self.vol_surface_id.as_str(),
            vol_bump,
        )?;
        let down = crate::metrics::bump_surface_vol_absolute(
            market,
            self.vol_surface_id.as_str(),
            -vol_bump,
        )?;
        let pv_up = self.value(&up, as_of)?.amount();
        let pv_down = self.value(&down, as_of)?.amount();
        let width = 2.0 * vol_bump * crate::metrics::VOL_POINTS_PER_ABSOLUTE_VOL;
        Ok(Some((pv_up - pv_down) / width))
    }

    fn option_rho_bp(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        use crate::instruments::common_impl::traits::Instrument;

        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let base_pv = self.value(market, as_of)?.amount();
        let bump_bp = self.metric_pricing_overrides.rho_bump_bp();
        let bumped = crate::metrics::bump_discount_curve_parallel(
            market,
            &self.domestic_discount_curve_id,
            bump_bp,
        )?;
        let pv_bumped = self.value(&bumped, as_of)?.amount();
        Ok(Some((pv_bumped - base_pv) / bump_bp))
    }

    fn option_vanna(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        use crate::instruments::common_impl::traits::Instrument;

        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let spot_id = self.fx_spot_id.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "FxBarrierOption vanna requires fx_spot_id for finite-difference spot bumps"
                    .to_string(),
            )
        })?;
        let spot_scalar = market.get_price(spot_id)?;
        let current_spot = crate::metrics::scalar_numeric_value(spot_scalar);

        let spot_bump = current_spot * crate::metrics::bump_sizes::SPOT;
        if spot_bump <= 0.0 {
            return Ok(Some(0.0));
        }
        let vol_bump = crate::metrics::bump_sizes::VOLATILITY;

        // Delta at vol_up (central diff in spot)
        let curves_vol_up = crate::metrics::bump_surface_vol_absolute(
            market,
            self.vol_surface_id.as_str(),
            vol_bump,
        )?;
        let curves_up = crate::metrics::bump_scalar_price(
            &curves_vol_up,
            spot_id,
            crate::metrics::bump_sizes::SPOT,
        )?;
        let curves_dn = crate::metrics::bump_scalar_price(
            &curves_vol_up,
            spot_id,
            -crate::metrics::bump_sizes::SPOT,
        )?;
        let pv_up = self.value(&curves_up, as_of)?.amount();
        let pv_dn = self.value(&curves_dn, as_of)?.amount();
        let delta_vol_up = (pv_up - pv_dn) / (2.0 * spot_bump);

        // Delta at vol_down
        let curves_vol_dn = crate::metrics::bump_surface_vol_absolute(
            market,
            self.vol_surface_id.as_str(),
            -vol_bump,
        )?;
        let curves_up = crate::metrics::bump_scalar_price(
            &curves_vol_dn,
            spot_id,
            crate::metrics::bump_sizes::SPOT,
        )?;
        let curves_dn = crate::metrics::bump_scalar_price(
            &curves_vol_dn,
            spot_id,
            -crate::metrics::bump_sizes::SPOT,
        )?;
        let pv_up = self.value(&curves_up, as_of)?.amount();
        let pv_dn = self.value(&curves_dn, as_of)?.amount();
        let delta_vol_dn = (pv_up - pv_dn) / (2.0 * spot_bump);

        // Report vanna per **vol point** on the σ axis (consistent with vega
        // and `MetricId::Vanna`): normalize by the bump width expressed in
        // vol points.
        let width = 2.0 * vol_bump * crate::metrics::VOL_POINTS_PER_ABSOLUTE_VOL;
        Ok(Some((delta_vol_up - delta_vol_dn) / width))
    }

    fn option_volga(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        base_pv: f64,
    ) -> finstack_quant_core::Result<Option<f64>> {
        use crate::instruments::common_impl::traits::Instrument;

        let t = self.day_count.year_fraction(
            as_of,
            self.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(Some(0.0));
        }

        let vol_bump = crate::metrics::bump_sizes::VOLATILITY;
        let up = crate::metrics::bump_surface_vol_absolute(
            market,
            self.vol_surface_id.as_str(),
            vol_bump,
        )?;
        let dn = crate::metrics::bump_surface_vol_absolute(
            market,
            self.vol_surface_id.as_str(),
            -vol_bump,
        )?;
        let pv_up = self.value(&up, as_of)?.amount();
        let pv_dn = self.value(&dn, as_of)?.amount();
        let width = vol_bump * crate::metrics::VOL_POINTS_PER_ABSOLUTE_VOL;
        Ok(Some((pv_up - 2.0 * base_pv + pv_dn) / (width * width)))
    }
}

impl crate::instruments::common_impl::traits::Instrument for FxBarrierOption {
    impl_instrument_base!(crate::pricer::InstrumentType::FxBarrierOption);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        match self.monitoring {
            Monitoring::Continuous => crate::pricer::ModelKey::FxBarrierBSContinuous,
            Monitoring::Discrete { .. } => crate::pricer::ModelKey::MonteCarloGBM,
        }
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.domestic_discount_curve_id.clone());
        deps.add_discount_curve(self.foreign_discount_curve_id.clone());
        if let Some(spot_id) = self.fx_spot_id.as_ref() {
            deps.add_market_scalar_id(spot_id.as_str());
        }
        deps.add_volatility_dependency(
            crate::instruments::common_impl::dependencies::VolatilityDependency::new(
                self.vol_surface_id.clone(),
                self.fx_spot_id.clone(),
                Some(self.strike),
            ),
        );
        deps.add_fx_pair(self.base_currency, self.quote_currency);
        Ok(deps)
    }

    /// Compute present value with explicit contractual monitoring semantics.
    ///
    /// Continuous monitoring uses the analytical Reiner-Rubinstein pricer.
    /// Discrete monitoring uses Monte Carlo and observes the barrier only on
    /// the dates carried by [`Monitoring::Discrete`].
    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        use crate::instruments::fx::fx_barrier_option::pricer::{
            compute_pv, FxBarrierOptionAnalyticalPricer,
        };
        use crate::pricer::Pricer;

        if matches!(self.monitoring, Monitoring::Discrete { .. }) {
            return compute_pv(self, market, as_of);
        }

        let pricer = FxBarrierOptionAnalyticalPricer::new();
        let result = pricer
            .price_dyn(self, market, as_of)
            .map_err(finstack_quant_core::Error::from)?;
        Ok(result.value)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        None
    }

    crate::impl_focused_pricing_overrides!();
}

crate::impl_empty_cashflow_provider!(
    FxBarrierOption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;

    #[test]
    fn test_fx_barrier_option_curve_dependencies_includes_both_curves() {
        let option = FxBarrierOption::example();
        let deps = option
            .market_dependencies()
            .expect("market_dependencies")
            .curves;

        // Should include both domestic and foreign discount curves
        assert_eq!(
            deps.discount_curves.len(),
            2,
            "FxBarrierOption should depend on both domestic and foreign curves"
        );
        assert!(
            deps.discount_curves.iter().any(|c| c.as_str() == "USD-OIS"),
            "Should include domestic curve"
        );
        assert!(
            deps.discount_curves.iter().any(|c| c.as_str() == "EUR-OIS"),
            "Should include foreign curve"
        );
    }

    #[test]
    fn test_fx_barrier_option_example_has_correct_values() {
        let option = FxBarrierOption::example();

        // Strike and barrier are f64 exchange rates
        assert!(
            (option.strike - 1.10).abs() < 1e-12,
            "Strike should be 1.10"
        );
        assert!(
            (option.barrier - 1.20).abs() < 1e-12,
            "Barrier should be 1.20"
        );

        // Notional should be in base currency (EUR)
        assert_eq!(
            option.notional.currency(),
            option.base_currency,
            "Notional should be in base currency"
        );
    }

    #[test]
    fn test_fx_barrier_option_creation_with_f64_strike_barrier() {
        use finstack_quant_core::dates::DayCount;
        use time::Month;

        let option = FxBarrierOption::builder()
            .id(InstrumentId::new("TEST-FXBAR"))
            .strike(1.10)
            .barrier(1.20)
            .option_type(OptionType::Call)
            .barrier_type(BarrierType::UpAndOut)
            .monitoring_start_date(
                Date::from_calendar_date(2024, Month::January, 1).expect("valid date"),
            )
            .expiry(Date::from_calendar_date(2025, Month::June, 15).expect("valid date"))
            .notional(Money::from((1_000_000_i64, Currency::EUR)))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .day_count(DayCount::Act365F)
            .monitoring(Monitoring::Continuous)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .fx_spot_id_opt(Some("EURUSD-SPOT".into()))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .attributes(Attributes::new())
            .build()
            .expect("should build");

        assert!((option.strike - 1.10).abs() < 1e-12);
        assert!((option.barrier - 1.20).abs() < 1e-12);
        assert_eq!(option.notional.currency(), Currency::EUR);
    }

    #[test]
    fn test_fx_barrier_option_serde_defaults_to_continuous_monitoring() {
        let mut value = serde_json::to_value(FxBarrierOption::example()).expect("serialize");
        let obj = value
            .as_object_mut()
            .expect("FxBarrierOption should serialize to an object");
        obj.remove("monitoring");
        let option: FxBarrierOption = serde_json::from_value(value).expect("deserialize");
        assert_eq!(option.monitoring, Monitoring::Continuous);
    }

    #[test]
    fn discrete_monitoring_requires_strict_contractual_dates() {
        let mut option = FxBarrierOption::example();
        let start = option.monitoring_start_date.expect("example start");
        option.monitoring = Monitoring::Discrete {
            observation_dates: vec![start, start],
        };
        let error = option
            .validate()
            .expect_err("duplicate observation dates must fail");
        assert!(error.to_string().contains("strictly increasing"));
    }

    #[test]
    fn test_fx_barrier_option_serde_allows_missing_fx_spot_id() {
        let mut value = serde_json::to_value(FxBarrierOption::example()).expect("serialize");
        let obj = value
            .as_object_mut()
            .expect("FxBarrierOption should serialize to an object");
        obj.remove("fx_spot_id");
        let option: FxBarrierOption = serde_json::from_value(value).expect("deserialize");
        assert!(option.fx_spot_id.is_none());
    }

    #[test]
    fn builder_rejects_same_base_and_quote_currency() {
        use finstack_quant_core::dates::DayCount;
        use time::Month;

        let result = FxBarrierOption::builder()
            .id(InstrumentId::new("FXBAR-USDUSD"))
            .strike(1.0)
            .barrier(1.1)
            .option_type(OptionType::Call)
            .barrier_type(BarrierType::UpAndOut)
            .expiry(Date::from_calendar_date(2025, Month::June, 15).expect("valid date"))
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .base_currency(Currency::USD)
            .quote_currency(Currency::USD)
            .day_count(DayCount::Act365F)
            .monitoring(Monitoring::Continuous)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("USDUSD-VOL"))
            .attributes(Attributes::new())
            .build();

        assert!(
            result.is_err(),
            "FX barrier option builder must reject identical base and quote currencies"
        );
    }
}
