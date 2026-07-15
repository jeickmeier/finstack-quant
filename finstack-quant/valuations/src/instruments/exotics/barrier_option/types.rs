//! Barrier option instrument definition.

use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::OptionType;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};

/// Backward-compatible instrument path for the canonical barrier type.
pub use finstack_quant_core::types::BarrierType;

/// Default for use_gobet_miri field.
///
/// Returns `true` to enable discrete barrier monitoring correction by default.
/// This matches the recommended production setting.
fn default_gobet_miri() -> bool {
    true
}

/// Barrier option instrument.
///
/// Barrier options are options with a barrier level that can knock in or out.
///
/// # Barrier Monitoring
///
/// Real-world barriers are typically monitored discretely (e.g., daily closes), not continuously.
/// Continuous barrier formulas underestimate discrete barrier option values. The `use_gobet_miri`
/// flag enables the Gobet-Miri discrete monitoring correction (β ≈ 0.5826), which adjusts the
/// effective barrier level: `H_adj = H × exp(±0.5826 × σ × √Δt)`.
///
/// **Recommendation**: Set `use_gobet_miri = true` (the default) for real-world pricing.
/// Only disable for continuous monitoring benchmarks or academic comparisons.
///
/// # References
///
/// - Broadie, Glasserman & Kou (1997), "A Continuity Correction for Discrete Barrier Options"
/// - Gobet (2000), "Weak Approximation of Killed Diffusion Using Euler Schemes"
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    finstack_quant_valuations_macros::FocusedPricingOverrides,
)]
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
    /// Optional rebate amount (paid if the barrier condition is met; see
    /// `rebate_timing` for when a knock-out rebate pays)
    pub rebate: Option<Money>,
    /// Timing of the knock-out rebate payment.
    ///
    /// `at_hit` (default, market standard) pays the rebate the moment a
    /// knock-out barrier is breached; `at_expiry` defers payment to expiry.
    /// Knock-in rebates always pay at expiry (a no-hit is only known then),
    /// so this setting does not affect them. The analytical pricer values
    /// at-hit rebates via the discounted first-passage closed form; the Monte
    /// Carlo pricers currently approximate at-hit rebates as at-expiry.
    #[builder(default)]
    #[serde(default)]
    pub rebate_timing: crate::models::closed_form::barrier::RebateTiming,
    /// Option type (call or put)
    pub option_type: OptionType,
    /// Barrier type (up/down, in/out)
    pub barrier_type: BarrierType,
    /// Option expiry date
    #[schemars(with = "String")]
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
    /// Whether to use Gobet-Miri discrete barrier adjustment for Monte Carlo pricing.
    ///
    /// When `true` (recommended), applies the Broadie-Glasserman-Kou / Gobet-Miri correction
    /// to account for discrete barrier monitoring. This adjusts the effective barrier by
    /// `exp(±0.5826 × σ × √Δt)` where Δt is the time step.
    ///
    /// # Default Value
    ///
    /// **Defaults to `true`** for both builder and serde deserialization, as this
    /// reflects real-world discrete monitoring (daily closes). Set to `false` only
    /// for continuous monitoring benchmarks or academic comparisons.
    ///
    /// # Production Recommendation
    ///
    /// Always use `true` for production pricing of barrier options. Continuous
    /// barrier formulas systematically underestimate discrete barrier option values.
    #[builder(default = default_gobet_miri())]
    #[serde(default = "default_gobet_miri")]
    pub use_gobet_miri: bool,
    /// Monitoring frequency for discrete barrier adjustment (years between observations).
    ///
    /// When set, the analytical pricer applies the Broadie-Glasserman correction
    /// to adjust the barrier level for discrete monitoring. Common values:
    /// - `1.0/252.0` — daily monitoring
    /// - `1.0/52.0` — weekly monitoring
    /// - `1.0/12.0` — monthly monitoring
    ///
    /// When `None`, the analytical pricer uses continuous monitoring formulas.
    /// Note: The MC pricer (`use_gobet_miri = true`) handles discrete monitoring
    /// independently via per-step corrections.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monitoring_frequency: Option<f64>,
    /// Discount curve ID for present value calculations
    pub discount_curve_id: CurveId,
    /// Spot price identifier
    pub spot_id: PriceId,
    /// Volatility surface ID
    pub vol_surface_id: CurveId,
    /// Optional dividend yield curve ID
    pub div_yield_id: Option<CurveId>,
    /// Pricing overrides (manual price, yield, spread)
    #[serde(default)]
    #[builder(default)]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-only pricing controls.
    #[builder(default)]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only valuation adjustments.
    #[builder(default)]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and grouping
    pub attributes: Attributes,
}

impl BarrierOption {
    /// Create a canonical example barrier option (up-and-out call).
    ///
    /// Note: Uses `use_gobet_miri = true` by default for realistic discrete monitoring.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::dates::DayCount;
        use time::macros::date;
        BarrierOption::builder()
            .id(InstrumentId::new("BAR-SPX-UO-CALL"))
            .underlying_ticker("SPX".to_string())
            .strike(4500.0)
            .barrier(Money::new(5000.0, Currency::USD))
            .rebate(Money::new(50.0, Currency::USD))
            .option_type(crate::instruments::OptionType::Call)
            .barrier_type(BarrierType::UpAndOut)
            .expiry(date!(2024 - 12 - 20))
            .expiry_fixing_opt(None)
            .observed_barrier_breached_opt(None)
            .notional(Money::new(100_000.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .use_gobet_miri(true) // Enable discrete monitoring correction (recommended)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .attributes(Attributes::new())
            .build()
    }

    /// Calculate the net present value using Monte Carlo.
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
        if let Some(fixing) = self.expiry_fixing {
            if fixing.currency() != self.notional.currency() {
                return Err(finstack_quant_core::Error::CurrencyMismatch {
                    expected: self.notional.currency(),
                    actual: fixing.currency(),
                });
            }
            if fixing.amount() <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(
                    "BarrierOption expiry_fixing must be positive".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        if self.use_gobet_miri {
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
        deps.add_spot_id(self.spot_id.as_str());
        deps.add_volatility_dependency(
            crate::instruments::common_impl::dependencies::VolatilityDependency::new(
                self.vol_surface_id.clone(),
                Some(self.spot_id.clone()),
                Some(self.strike),
            ),
        );
        if let Some(dividend_yield) = &self.div_yield_id {
            deps.add_series_id(dividend_yield.as_str());
        }
        Ok(deps)
    }

    /// Compute the present value with explicit monitoring semantics.
    ///
    /// Dispatch rules:
    /// - `use_gobet_miri = false` -> analytical continuous-monitoring pricer
    /// - `use_gobet_miri = true` -> MC discrete-monitoring-corrected pricer
    ///
    /// If `use_gobet_miri = true` but no compatible Monte Carlo pricer is registered,
    /// this returns an error instead of silently falling back to continuous pricing.
    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        if self.use_gobet_miri {
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
        option.use_gobet_miri = false;
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
                MarketScalar::Price(Money::new(5100.0, Currency::USD)),
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
        fn assert_barrier_type(label: &str, expected: super::BarrierType) {
            assert!(matches!(super::BarrierType::from_str(label), Ok(value) if value == expected));
        }

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
        // Test aliases
        assert_barrier_type("upandin", super::BarrierType::UpAndIn);
        assert_barrier_type("downandout", super::BarrierType::DownAndOut);
        assert!(super::BarrierType::from_str("invalid").is_err());
    }
}
