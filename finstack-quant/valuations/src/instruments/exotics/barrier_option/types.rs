//! Barrier option instrument definition.

use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::{Monitoring, OptionType};
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};

use finstack_quant_core::types::BarrierType;

/// Barrier option with a total trade rebate and explicit contractual monitoring.
/// Continuous monitoring uses closed-form pricing by default. Discrete monitoring
/// observes only the supplied dates and uses Monte Carlo by default.
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
pub struct BarrierOption {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Underlying asset ticker symbol
    pub underlying_ticker: String,
    /// Strike price
    pub strike: f64,
    /// Barrier level (price that triggers knock-in/out)
    pub barrier: Money,
    /// Total contractual trade rebate in the payoff currency, independent of
    /// notional. Knock-outs pay on a hit according to `rebate_timing`;
    /// knock-ins pay at expiry only if no hit occurred.
    pub rebate: Option<Money>,
    /// Timing of the knock-out rebate payment.
    ///
    /// `at_hit` (default, market standard) pays the rebate the moment a
    /// knock-out barrier is breached; `at_expiry` defers payment to expiry.
    /// Knock-in rebates always pay at expiry (a no-hit is only known then),
    /// so this setting does not affect them. The analytical pricer values
    /// at-hit rebates via the discounted first-passage closed form. Monte
    /// Carlo applies at-hit when `rebate_timing == AtHit` via
    /// `with_rebate_at_hit`; the crate primitive defaults to at-expiry.
    #[builder(default)]
    #[serde(default)]
    pub rebate_timing: finstack_quant_models::closed_form::barrier::RebateTiming,
    /// Option type (call or put)
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
    /// Terminal underlying fixing observed at expiry.
    ///
    /// Required when valuing after expiry so the realized intrinsic value is
    /// invariant to later market spot updates. At expiry, the current market
    /// spot is used when this field is absent.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiry_fixing: Option<Money>,
    /// Observed barrier state for expired options.
    ///
    /// Historical barrier monitoring must be supplied explicitly for expired
    /// options because terminal spot alone does not reveal whether the barrier
    /// was breached intralife and then reversed.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_barrier_breached: Option<bool>,
    /// Notional amount
    pub notional: Money,
    /// Day count convention
    pub day_count: finstack_quant_core::dates::DayCount,
    /// Contractual monitoring: continuous or an explicit, strictly increasing
    /// set of observation dates no later than expiry. Discrete pricing does not
    /// interpolate hits between those dates.
    #[builder(default)]
    #[serde(default)]
    pub monitoring: Monitoring,
    /// Discount curve ID for present value calculations
    pub discount_curve_id: CurveId,
    /// Spot price identifier
    pub spot_id: PriceId,
    /// Volatility surface ID
    pub vol_surface_id: CurveId,
    /// Optional dividend-yield scalar ID
    pub div_yield_id: Option<PriceId>,
    /// Pricing overrides (manual price, yield, spread)
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-only pricing controls.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only valuation adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and grouping
    pub attributes: Attributes,
}

impl BarrierOption {
    /// Create a canonical example barrier option (up-and-out call).
    ///
    /// Uses continuous contractual monitoring by default.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::dates::DayCount;
        use time::macros::date;
        BarrierOption::builder()
            .id(InstrumentId::new("BAR-SPX-UO-CALL"))
            .underlying_ticker("SPX".to_string())
            .strike(4500.0)
            .barrier(Money::from((5000_i64, Currency::USD)))
            .rebate(Money::from((50_i64, Currency::USD)))
            .option_type(crate::instruments::OptionType::Call)
            .barrier_type(BarrierType::UpAndOut)
            .expiry(date!(2024 - 12 - 20))
            .expiry_fixing_opt(None)
            .observed_barrier_breached_opt(None)
            .notional(Money::from((100_000_i64, Currency::USD)))
            .day_count(DayCount::Act365F)
            .monitoring(Monitoring::Continuous)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(PriceId::new("SPX-DIV")))
            .attributes(Attributes::new())
            .build()
    }

    pub(crate) fn validate_monitoring_state(&self, as_of: Date) -> finstack_quant_core::Result<()> {
        use crate::instruments::Instrument;
        self.validate_invariants()?;
        if let Monitoring::Discrete { observation_dates } = &self.monitoring {
            if observation_dates.iter().any(|date| *date < as_of)
                && self.observed_barrier_breached.is_none()
            {
                return Err(finstack_quant_core::Error::Validation(
                    "BarrierOption requires observed_barrier_breached for past observation dates"
                        .to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Price the total trade payoff and rebate with the GBM Monte Carlo engine.
    ///
    /// # Arguments
    ///
    /// - `curves`: Market context containing the instrument's spot, volatility,
    ///   discount curve and optional dividend-yield inputs. Volatilities and
    ///   yields are decimal annualized values; spot is in the payoff currency.
    /// - `as_of`: Valuation date defining the start of the remaining simulation.
    ///   Discrete monitoring uses the contract's exact observation dates;
    ///   continuous monitoring uses a Brownian bridge between simulation steps.
    ///
    /// # Returns
    /// Present value in the notional currency. Rebate Money is the total trade
    /// payment and is discounted according to its contractual payment timing.
    ///
    /// # Errors
    /// Returns an error for missing market inputs, invalid parameters or missing
    /// observed barrier state for monitoring dates before `as_of`.
    pub fn npv_mc(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        use crate::instruments::exotics::barrier_option::pricer;
        pricer::compute_pv(self, curves, as_of)
    }
}

impl crate::instruments::common_impl::traits::Instrument for BarrierOption {
    impl_instrument_base!(crate::pricer::InstrumentType::BarrierOption);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        use crate::instruments::common_impl::validation;
        // Strike and barrier must satisfy the closed-form model domain
        // (`validate_barrier_inputs` rejects non-positive values with a NaN
        // sentinel); rejecting them here surfaces a `Validation` error at the
        // instrument boundary instead of panicking inside `Money::new`.
        validation::validate_f64_positive(self.strike, "BarrierOption strike")?;
        validation::validate_money_gt(self.notional, 0.0, "BarrierOption notional")?;
        for money in [Some(self.barrier), self.rebate].into_iter().flatten() {
            if money.currency() != self.notional.currency() {
                return Err(finstack_quant_core::Error::CurrencyMismatch {
                    expected: self.notional.currency(),
                    actual: money.currency(),
                });
            }
        }
        validation::validate_money_gt(self.barrier, 0.0, "BarrierOption barrier")?;
        if let Some(rebate) = self.rebate {
            // A zero rebate is economically identical to `None` and stays
            // accepted; only negative or non-finite amounts are rejected.
            validation::validate_f64_non_negative(rebate.amount(), "BarrierOption rebate")?;
        }
        if let Monitoring::Discrete { observation_dates } = &self.monitoring {
            validation::require_with(!observation_dates.is_empty(), || {
                "BarrierOption discrete monitoring requires observation_dates".to_string()
            })?;
            validation::validate_sorted_strict(
                observation_dates,
                "BarrierOption observation_dates",
            )?;
            validation::require_with(
                observation_dates.iter().all(|date| *date <= self.expiry),
                || "BarrierOption observation_dates must not be after expiry".to_string(),
            )?;
        }
        if let Some(fixing) = self.expiry_fixing {
            if fixing.currency() != self.notional.currency() {
                return Err(finstack_quant_core::Error::CurrencyMismatch {
                    expected: self.notional.currency(),
                    actual: fixing.currency(),
                });
            }
            if !fixing.amount().is_finite() || fixing.amount() <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(
                    "BarrierOption expiry_fixing must be positive".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        if matches!(self.monitoring, Monitoring::Discrete { .. }) {
            crate::pricer::ModelKey::MonteCarloGBM
        } else {
            crate::pricer::ModelKey::BarrierBSContinuous
        }
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        deps.add_market_scalar_id(self.spot_id.as_str());
        deps.add_volatility_dependency(
            crate::instruments::common_impl::dependencies::VolatilityDependency::new(
                self.vol_surface_id.clone(),
                Some(self.spot_id.clone()),
                Some(self.strike),
            ),
        );
        if let Some(dividend_yield) = &self.div_yield_id {
            deps.add_market_scalar_id(dividend_yield.as_str());
        }
        Ok(deps)
    }

    /// Compute the present value with explicit monitoring semantics.
    ///
    /// Continuous monitoring selects analytical pricing; discrete dates select MC.
    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        if matches!(self.monitoring, Monitoring::Discrete { .. }) {
            return self.npv_mc(market, as_of);
        }

        use crate::instruments::exotics::barrier_option::pricer::BarrierOptionAnalyticalPricer;
        use crate::pricer::Pricer;

        let pricer = BarrierOptionAnalyticalPricer::new();
        let result = pricer
            .price_dyn(self, market, as_of)
            .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))?;
        Ok(result.value)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        None
    }

    crate::impl_focused_pricing_overrides!();
}

crate::impl_empty_cashflow_provider!(
    BarrierOption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;

    #[test]
    fn expired_barrier_requires_observed_state() {
        let mut option = super::BarrierOption::example().expect("BarrierOption example is valid");
        option.monitoring = super::Monitoring::Continuous;
        option.observed_barrier_breached = None;
        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(option.expiry)
                    .knots([(0.0, 1.0), (1.0, 1.0)])
                    .build()
                    .expect("discount curve"),
            )
            .insert_surface(
                VolSurface::from_grid(
                    "SPX-VOL",
                    &[0.0, 1.0],
                    &[4000.0, 6000.0],
                    &[0.2, 0.2, 0.2, 0.2],
                )
                .expect("surface"),
            )
            .insert_price("SPX-DIV", MarketScalar::Unitless(0.0))
            .insert_price(
                "SPX-SPOT",
                MarketScalar::Price(Money::from((5100_i64, Currency::USD))),
            );

        let err = crate::instruments::common_impl::traits::Instrument::value(
            &option,
            &market,
            option.expiry,
        )
        .expect_err("expired barrier should require observed barrier state");
        assert!(
            format!("{err}").contains("observed_barrier_breached"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn barrier_type_fromstr_display_roundtrip() {
        use std::str::FromStr;

        let variants = [
            super::BarrierType::UpAndOut,
            super::BarrierType::UpAndIn,
            super::BarrierType::DownAndOut,
            super::BarrierType::DownAndIn,
        ];
        for v in variants {
            let s = v.to_string();
            let parsed = super::BarrierType::from_str(&s).expect("roundtrip parse should succeed");
            assert_eq!(v, parsed, "roundtrip failed for {s}");
        }
        assert!(super::BarrierType::from_str("upandin").is_err());
        assert!(super::BarrierType::from_str("downandout").is_err());
        assert!(super::BarrierType::from_str("invalid").is_err());
    }
}
