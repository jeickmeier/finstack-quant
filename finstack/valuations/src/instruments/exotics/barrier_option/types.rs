//! Barrier option instrument definition.

use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::OptionType;
use crate::instruments::PricingOverrides;
use finstack_core::dates::Date;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId, PriceId};

/// Barrier type for barrier options.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BarrierType {
    /// Up-and-out: option knocked out if S >= B
    UpAndOut,
    /// Up-and-in: option activated if S >= B
    UpAndIn,
    /// Down-and-out: option knocked out if S <= B
    DownAndOut,
    /// Down-and-in: option activated if S <= B
    DownAndIn,
}

impl std::fmt::Display for BarrierType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UpAndOut => write!(f, "up_and_out"),
            Self::UpAndIn => write!(f, "up_and_in"),
            Self::DownAndOut => write!(f, "down_and_out"),
            Self::DownAndIn => write!(f, "down_and_in"),
        }
    }
}

/// One-line conversion from the valuations-side `BarrierType` to the
/// `finstack_monte_carlo` payoff enum. Replaces three identical
/// `convert_barrier_type` helpers that previously lived in
/// `exotics/barrier_option/{pricer,heston_mc_pricer}.rs` and
/// `fx/fx_barrier_option/pricer.rs`.
impl From<BarrierType> for finstack_monte_carlo::payoff::barrier::BarrierType {
    fn from(bt: BarrierType) -> Self {
        match bt {
            BarrierType::UpAndOut => Self::UpAndOut,
            BarrierType::UpAndIn => Self::UpAndIn,
            BarrierType::DownAndOut => Self::DownAndOut,
            BarrierType::DownAndIn => Self::DownAndIn,
        }
    }
}

impl std::str::FromStr for BarrierType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace(['-', '/', ' '], "_");
        match normalized.as_str() {
            "up_and_out" | "upandout" => Ok(Self::UpAndOut),
            "up_and_in" | "upandin" => Ok(Self::UpAndIn),
            "down_and_out" | "downandout" => Ok(Self::DownAndOut),
            "down_and_in" | "downandin" => Ok(Self::DownAndIn),
            other => Err(format!(
                "Unknown barrier type: '{}'. Valid: up_and_in, up_and_out, down_and_in, down_and_out",
                other
            )),
        }
    }
}

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
    finstack_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
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
    pub day_count: finstack_core::dates::DayCount,
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
    pub pricing_overrides: PricingOverrides,
    /// Attributes for scenario selection and grouping
    pub attributes: Attributes,
}

impl BarrierOption {
    /// Create a canonical example barrier option (up-and-out call).
    ///
    /// Note: Uses `use_gobet_miri = true` by default for realistic discrete monitoring.
    pub fn example() -> finstack_core::Result<Self> {
        use finstack_core::currency::Currency;
        use finstack_core::dates::DayCount;
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
            .observed_barrier_breached_opt(None)
            .notional(Money::new(100_000.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .use_gobet_miri(true) // Enable discrete monitoring correction (recommended)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
    }

    /// Calculate the net present value using Monte Carlo.
    pub fn npv_mc(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<finstack_core::money::Money> {
        use crate::instruments::exotics::barrier_option::pricer;
        pricer::compute_pv(self, curves, as_of)
    }
}

impl crate::instruments::common_impl::traits::Instrument for BarrierOption {
    impl_instrument_base!(crate::pricer::InstrumentType::BarrierOption);

    fn default_model(&self) -> crate::pricer::ModelKey {
        if self.use_gobet_miri {
            crate::pricer::ModelKey::MonteCarloGBM
        } else {
            crate::pricer::ModelKey::BarrierBSContinuous
        }
    }

    fn market_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::dependencies::MarketDependencies>
    {
        crate::instruments::common_impl::dependencies::MarketDependencies::from_curves_and_equity(
            self,
        )
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
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<finstack_core::money::Money> {
        if self.use_gobet_miri {
            return self.npv_mc(market, as_of);
        }

        use crate::instruments::exotics::barrier_option::pricer::BarrierOptionAnalyticalPricer;
        use crate::pricer::Pricer;

        let pricer = BarrierOptionAnalyticalPricer::new();
        let result = pricer
            .price_dyn(self, market, as_of)
            .map_err(|e| finstack_core::Error::Validation(e.to_string()))?;
        Ok(result.value)
    }

    fn effective_start_date(&self) -> Option<finstack_core::dates::Date> {
        None
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

crate::impl_empty_cashflow_provider!(
    BarrierOption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use finstack_core::currency::Currency;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::scalars::MarketScalar;
    use finstack_core::market_data::surfaces::VolSurface;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::money::Money;

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
