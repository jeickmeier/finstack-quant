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
//! Discretely-monitored lookbacks set `monitoring` to
//! `Monitoring::Discrete { observation_dates }`. They are priced by Monte
//! Carlo, which observes the extremum only on those contractual dates, rather
//! than by the continuous analytical formulas.
//!
//! # References
//!
//! - Goldman, M. B., Sosin, H. B., & Gatto, M. A. (1979). "Path Dependent Options."
//! - Broadie, M., Glasserman, P., & Kou, S. G. (1997). "A Continuity Correction
//!   for Discrete Barrier Options." `docs/REFERENCES.md#glasserman-2004-monte-carlo` `docs/REFERENCES.md#broadie-glasserman-kou-1997`

use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::{Monitoring, OptionType};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};

/// Lookback option type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
        match s {
            "fixed_strike" => Ok(Self::FixedStrike),
            "floating_strike" => Ok(Self::FloatingStrike),
            _ => Err(format!(
                "Unknown lookback type: '{}'. Valid: fixed_strike, floating_strike",
                s
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
    PartialEq,
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub expiry: Date,
    /// Terminal underlying fixing observed at expiry, in the same quote units
    /// as `strike`.
    ///
    /// Required when valuing after expiry so the realized payoff cannot move
    /// with a later market spot snapshot. At expiry itself, the current market
    /// spot is treated as the terminal fixing when this field is absent.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiry_fixing: Option<f64>,
    /// Number of underlying units the option is written on; PV and Greeks scale linearly with it.
    pub quantity: f64,
    /// Currency of the strike, premium and present value.
    pub currency: Currency,
    /// Day count convention
    pub day_count: finstack_quant_core::dates::DayCount,
    /// Discount curve ID for present value calculations
    pub discount_curve_id: CurveId,
    /// Spot price identifier
    pub spot_id: PriceId,
    /// Volatility surface ID
    pub vol_surface_id: CurveId,
    /// Optional dividend-yield scalar ID
    pub div_yield_id: Option<PriceId>,
    /// Contractual monitoring of the path extremum.
    ///
    /// `continuous` (default) prices with the Goldman-Sosin-Gatto closed form.
    /// `discrete` observes the extremum only on the strictly increasing
    /// `observation_dates` (no later than expiry) and prices by Monte Carlo.
    #[builder(default)]
    #[serde(default)]
    pub monitoring: Monitoring,
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
    /// Observed minimum underlying level since inception, in the same quote
    /// units as `strike` (required for Floating Call / Fixed Put once seasoned).
    pub observed_min: Option<f64>,
    /// Observed maximum underlying level since inception, in the same quote
    /// units as `strike` (required for Floating Put / Fixed Call once seasoned).
    pub observed_max: Option<f64>,
    /// Attributes for scenario selection and grouping
    pub attributes: Attributes,
}

// Declare canonical market dependencies for the DV01 calculator.
impl LookbackOption {
    /// Create a canonical example lookback option (fixed strike call).
    pub fn example() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::dates::DayCount;
        use time::macros::date;
        LookbackOption::builder()
            .id(InstrumentId::new("LOOKBACK-SPX-FIXED-CALL"))
            .underlying_ticker("SPX".to_string())
            .strike_opt(Some(4500.0))
            .option_type(crate::instruments::OptionType::Call)
            .lookback_type(LookbackType::FixedStrike)
            .expiry(date!(2024 - 12 - 20))
            .expiry_fixing_opt(None)
            .quantity(100_000.0)
            .currency(Currency::USD)
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(PriceId::new("SPX-DIV")))
            .observed_min_opt(None)
            .observed_max_opt(None)
            .attributes(Attributes::new())
            .build()
    }
    /// Calculate the net present value using Monte Carlo.
    pub fn npv_mc(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        use crate::instruments::exotics::lookback_option::pricer;
        pricer::compute_pv(self, curves, as_of)
    }
}

impl crate::instruments::common_impl::traits::Instrument for LookbackOption {
    impl_instrument_base!(crate::pricer::InstrumentType::LookbackOption);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        use crate::instruments::common_impl::validation;
        // Fixed-strike lookbacks feed `strike` into ln-based closed forms that
        // require a strictly positive strike; rejecting here surfaces a
        // `Validation` error instead of a NaN sentinel reaching pricing.
        if let Some(strike) = self.strike {
            validation::validate_f64_positive(strike, "LookbackOption strike")?;
        }
        if let Some(observed_min) = self.observed_min {
            validation::validate_f64_positive(observed_min, "LookbackOption observed_min")?;
        }
        if let Some(observed_max) = self.observed_max {
            validation::validate_f64_positive(observed_max, "LookbackOption observed_max")?;
        }
        if let Monitoring::Discrete { observation_dates } = &self.monitoring {
            validation::require_with(!observation_dates.is_empty(), || {
                "LookbackOption monitoring.observation_dates must not be empty".to_string()
            })?;
            validation::validate_sorted_strict(
                observation_dates,
                "LookbackOption monitoring.observation_dates",
            )?;
            validation::require_with(
                observation_dates.iter().all(|date| *date <= self.expiry),
                || {
                    "LookbackOption monitoring.observation_dates must not be after expiry"
                        .to_string()
                },
            )?;
        }
        if let Some(fixing) = self.expiry_fixing {
            validation::validate_f64_positive(fixing, "LookbackOption expiry_fixing")?;
        }
        Ok(())
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        if matches!(self.monitoring, Monitoring::Discrete { .. }) {
            crate::pricer::ModelKey::MonteCarloGBM
        } else {
            crate::pricer::ModelKey::LookbackBSContinuous
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
                self.strike,
            ),
        );
        if let Some(dividend_yield) = &self.div_yield_id {
            deps.add_market_scalar_id(dividend_yield.as_str());
        }
        Ok(deps)
    }

    /// Compute the present value with explicit monitoring semantics.
    ///
    /// Dispatch rules:
    /// - `monitoring = continuous` -> analytical continuous-monitoring pricer
    /// - `monitoring = discrete` -> MC pricer observing only the contractual dates
    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        if matches!(self.monitoring, Monitoring::Discrete { .. }) {
            return self.npv_mc(market, as_of);
        }

        use crate::instruments::exotics::lookback_option::pricer::LookbackOptionAnalyticalPricer;
        use crate::pricer::Pricer;

        let pricer = LookbackOptionAnalyticalPricer::new();
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
    LookbackOption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn lookback_type_fromstr_display_roundtrip() {
        let variants = [LookbackType::FixedStrike, LookbackType::FloatingStrike];
        for v in variants {
            let s = v.to_string();
            let parsed = LookbackType::from_str(&s).expect("roundtrip parse should succeed");
            assert_eq!(v, parsed, "roundtrip failed for {s}");
        }
        for retired in ["fixedstrike", "floatingstrike", "fixed"] {
            assert!(LookbackType::from_str(retired).is_err());
        }
        assert!(LookbackType::from_str("invalid").is_err());
    }
}
