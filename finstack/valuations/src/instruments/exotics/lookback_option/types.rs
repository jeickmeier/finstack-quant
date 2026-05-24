//! Lookback option instrument definition.
//!
//! # Monitoring Convention
//!
//! **Important**: This implementation uses **continuous monitoring** formulas
//! (Goldman-Sosin-Gatto closed-form solutions). Real-world lookback options are
//! typically monitored discretely (e.g., daily closes), which affects pricing.
//!
//! ## Continuous vs Discrete Monitoring
//!
//! | Aspect | Continuous | Discrete |
//! |--------|------------|----------|
//! | Monitoring | Every instant | At specific times (e.g., daily) |
//! | Pricing | Analytical formulas | Monte Carlo or numerical |
//! | Value | Higher (more observations) | Lower (fewer opportunities) |
//! | Greeks | Closed-form | Numerical |
//!
//! ## Discrete Monitoring Adjustment
//!
//! For daily-monitored lookbacks, the continuous price can be adjusted using
//! the Broadie-Glasserman-Kou correction factor:
//!
//! ```text
//! M_discrete ≈ M_continuous × exp(0.5826 × σ × √Δt)  [for max]
//! m_discrete ≈ m_continuous × exp(-0.5826 × σ × √Δt) [for min]
//! ```
//!
//! where:
//! - `σ` = volatility
//! - `Δt` = monitoring interval (e.g., 1/252 for daily)
//! - `0.5826 = -ζ(1/2)/√(2π)` (Riemann zeta constant)
//!
//! ## Production Recommendation
//!
//! For production pricing of discretely-monitored lookbacks, use Monte Carlo
//! simulation (`npv_mc`) with the actual monitoring dates rather than the
//! continuous analytical formulas.
//!
//! # References
//!
//! - Goldman, M. B., Sosin, H. B., & Gatto, M. A. (1979). "Path Dependent Options."
//! - Broadie, M., Glasserman, P., & Kou, S. G. (1997). "A Continuity Correction
//!   for Discrete Barrier Options."

use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::OptionType;
use crate::instruments::PricingOverrides;
use finstack_core::dates::Date;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId, PriceId};

/// Lookback option type.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum LookbackType {
    /// Fixed strike lookback: payoff depends on max/min relative to fixed strike
    FixedStrike,
    /// Floating strike lookback: strike is determined by path extremum
    FloatingStrike,
}

impl std::fmt::Display for LookbackType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FixedStrike => write!(f, "fixed_strike"),
            Self::FloatingStrike => write!(f, "floating_strike"),
        }
    }
}

impl std::str::FromStr for LookbackType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace(['-', '/', ' '], "_");
        match normalized.as_str() {
            "fixed_strike" | "fixedstrike" | "fixed" => Ok(Self::FixedStrike),
            "floating_strike" | "floatingstrike" | "floating" => Ok(Self::FloatingStrike),
            other => Err(format!(
                "Unknown lookback type: '{}'. Valid: fixed_strike, floating_strike",
                other
            )),
        }
    }
}

/// Lookback option instrument.
///
/// # Monitoring Convention
///
/// This instrument uses **continuous monitoring** for analytical pricing. Real-world
/// lookback options are typically monitored discretely (daily closes). The continuous
/// formulas provide an upper bound; for accurate discrete pricing, use Monte Carlo.
///
/// See module-level documentation for details on discrete monitoring adjustments.
///
/// # Observed Extrema
///
/// For seasoned options (where some monitoring has already occurred), provide:
/// - `observed_min`: Minimum spot observed so far (for floating calls / fixed puts)
/// - `observed_max`: Maximum spot observed so far (for floating puts / fixed calls)
///
/// If not provided, the current spot is used as the starting extremum.
#[derive(
    Clone,
    Debug,
    finstack_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct LookbackOption {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Underlying asset ticker symbol
    pub underlying_ticker: String,
    /// Strike price (None for floating strike lookbacks)
    pub strike: Option<f64>, // None for floating strike
    /// Option type (call or put)
    pub option_type: OptionType,
    /// Lookback type (fixed or floating strike)
    pub lookback_type: LookbackType,
    /// Option expiry date
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Notional amount
    pub notional: Money,
    /// Day count convention
    pub day_count: finstack_core::dates::DayCount,
    /// Discount curve ID for present value calculations
    pub discount_curve_id: CurveId,
    /// Spot price identifier
    pub spot_id: PriceId,
    /// Volatility surface ID
    pub vol_surface_id: CurveId,
    /// Optional dividend yield curve ID
    pub div_yield_id: Option<CurveId>,
    /// Whether to use Monte Carlo with Gobet-Miri correction for discrete monitoring.
    ///
    /// When `true`, `value()` dispatches to `npv_mc()` for discrete-monitoring-corrected
    /// pricing using Monte Carlo simulation.
    ///
    /// **Defaults to `false`** (analytical continuous pricing).
    /// Set to `true` for production pricing of discretely-monitored lookbacks.
    #[builder(default)]
    #[serde(default)]
    pub use_gobet_miri: bool,
    /// Pricing overrides (manual price, yield, spread)
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: PricingOverrides,
    /// Observed minimum spot price since inception (required for Floating Call / Fixed Put)
    pub observed_min: Option<Money>,
    /// Observed maximum spot price since inception (required for Floating Put / Fixed Call)
    pub observed_max: Option<Money>,
    /// Attributes for scenario selection and grouping
    pub attributes: Attributes,
}

// Implement CurveDependencies for DV01 calculator
impl crate::instruments::common_impl::traits::CurveDependencies for LookbackOption {
    fn curve_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::traits::InstrumentCurves> {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .build()
    }
}

impl LookbackOption {
    /// Create a canonical example lookback option (fixed strike call).
    pub fn example() -> finstack_core::Result<Self> {
        use finstack_core::currency::Currency;
        use finstack_core::dates::DayCount;
        use time::macros::date;
        LookbackOption::builder()
            .id(InstrumentId::new("LOOKBACK-SPX-FIXED-CALL"))
            .underlying_ticker("SPX".to_string())
            .strike_opt(Some(4500.0))
            .option_type(crate::instruments::OptionType::Call)
            .lookback_type(LookbackType::FixedStrike)
            .expiry(date!(2024 - 12 - 20))
            .notional(Money::new(100_000.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .pricing_overrides(PricingOverrides::default())
            .observed_min_opt(None)
            .observed_max_opt(None)
            .attributes(Attributes::new())
            .build()
    }
    /// Calculate the net present value using Monte Carlo.
    pub fn npv_mc(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<finstack_core::money::Money> {
        use crate::instruments::exotics::lookback_option::pricer;
        pricer::compute_pv(self, curves, as_of)
    }
}

impl crate::instruments::common_impl::traits::Instrument for LookbackOption {
    impl_instrument_base!(crate::pricer::InstrumentType::LookbackOption);

    fn default_model(&self) -> crate::pricer::ModelKey {
        if self.use_gobet_miri {
            crate::pricer::ModelKey::MonteCarloGBM
        } else {
            crate::pricer::ModelKey::LookbackBSContinuous
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
    /// - `use_gobet_miri = true` -> MC discrete-monitoring pricer
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

        use crate::instruments::exotics::lookback_option::pricer::LookbackOptionAnalyticalPricer;
        use crate::pricer::Pricer;

        let pricer = LookbackOptionAnalyticalPricer::new();
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
    LookbackOption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn lookback_type_fromstr_display_roundtrip() {
        fn assert_lookback_type(label: &str, expected: LookbackType) {
            assert!(matches!(LookbackType::from_str(label), Ok(value) if value == expected));
        }

        let variants = [LookbackType::FixedStrike, LookbackType::FloatingStrike];
        for v in variants {
            let s = v.to_string();
            let parsed = LookbackType::from_str(&s).expect("roundtrip parse should succeed");
            assert_eq!(v, parsed, "roundtrip failed for {s}");
        }
        // Test aliases
        assert_lookback_type("fixedstrike", LookbackType::FixedStrike);
        assert_lookback_type("floatingstrike", LookbackType::FloatingStrike);
        assert_lookback_type("fixed", LookbackType::FixedStrike);
        assert!(LookbackType::from_str("invalid").is_err());
    }
}
