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

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};
use finstack_quant_models::types::OptionType;
use finstack_quant_valuations_core::impl_instrument_base;
use finstack_quant_valuations_core::instruments::Monitoring;

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
#[serde(deny_unknown_fields, remote = "Self")]
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
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::ScenarioPricingOverrides,
    /// Observed minimum underlying level since inception, in the same quote
    /// units as `strike` (required for Floating Call / Fixed Put once seasoned).
    pub observed_min: Option<f64>,
    /// Observed maximum underlying level since inception, in the same quote
    /// units as `strike` (required for Floating Put / Fixed Call once seasoned).
    pub observed_max: Option<f64>,
    /// Attributes for scenario selection and grouping
    pub attributes: Attributes,
}

finstack_quant_valuations_core::validate_on_deserialize!(LookbackOption);

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
            .option_type(finstack_quant_models::types::OptionType::Call)
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
}

impl finstack_quant_valuations_core::instruments::Instrument for LookbackOption {
    fn includes_valuation_date_cashflows(&self) -> bool {
        true
    }
    impl_instrument_base!(
        finstack_quant_valuations_core::pricer::InstrumentType::LookbackOption,
        crate::EXOTICS
    );

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        use finstack_quant_core::validation;

        if self.underlying_ticker.trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "LookbackOption requires a non-empty underlying_ticker".to_string(),
            ));
        }
        validation::validate_f64_positive(self.quantity, "LookbackOption quantity")?;
        match self.lookback_type {
            LookbackType::FixedStrike => match self.strike {
                Some(strike) => validation::validate_f64_positive(strike, "LookbackOption strike")?,
                None => {
                    return Err(finstack_quant_core::Error::Validation(
                        "LookbackOption FixedStrike requires a positive strike".to_string(),
                    ));
                }
            },
            LookbackType::FloatingStrike => {
                if self.strike.is_some() {
                    return Err(finstack_quant_core::Error::Validation(
                        "LookbackOption FloatingStrike cannot specify a strike".to_string(),
                    ));
                }
            }
        }
        if let Some(observed_min) = self.observed_min {
            validation::validate_f64_positive(observed_min, "LookbackOption observed_min")?;
        }
        if let Some(observed_max) = self.observed_max {
            validation::validate_f64_positive(observed_max, "LookbackOption observed_max")?;
        }
        if let (Some(observed_min), Some(observed_max)) = (self.observed_min, self.observed_max)
            && observed_min > observed_max
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "LookbackOption observed_min ({observed_min}) must not exceed observed_max ({observed_max})"
            )));
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

    fn default_model(&self) -> finstack_quant_valuations_core::pricer::ModelKey {
        if matches!(self.monitoring, Monitoring::Discrete { .. }) {
            finstack_quant_valuations_core::pricer::ModelKey::MonteCarloGBM
        } else {
            finstack_quant_valuations_core::pricer::ModelKey::LookbackBSContinuous
        }
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations_core::instruments::MarketDependencies>
    {
        let mut deps = finstack_quant_valuations_core::instruments::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        deps.add_market_scalar_id(self.spot_id.as_str());
        deps.add_volatility_dependency(
            finstack_quant_valuations_core::instruments::VolatilityDependency::new(
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
            use crate::exotics::lookback_option::pricer::LookbackOptionMcPricer;
            return LookbackOptionMcPricer::new().price_internal(self, market, as_of);
        }

        use crate::exotics::lookback_option::pricer::LookbackOptionAnalyticalPricer;
        use finstack_quant_valuations_core::pricer::Pricer;

        let pricer = LookbackOptionAnalyticalPricer;
        let result = pricer.price_dyn(self, market, as_of)?;
        Ok(result.value)
    }

    fn expiry(&self) -> Option<Date> {
        Some(self.expiry)
    }

    /// Record the held `as_of` spot as the running extremum when theta rolls
    /// across expiry or a discrete observation date without an existing extremum.
    ///
    /// # Arguments
    /// * `market` - Market at `as_of` supplying the `spot_id` level held over the roll.
    /// * `as_of` - Valuation date the roll starts from.
    /// * `rolled_date` - Date theta reprices at, capped at expiry.
    fn theta_observed_state(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
        rolled_date: Date,
    ) -> finstack_quant_core::Result<
        Option<Box<dyn finstack_quant_valuations_core::instruments::Instrument>>,
    > {
        if as_of >= self.expiry {
            return Ok(None);
        }
        let needs_roll_extremum = rolled_date >= self.expiry
            || matches!(
                &self.monitoring,
                Monitoring::Discrete { observation_dates }
                    if observation_dates.iter().any(|date| *date < rolled_date)
            );
        if !needs_roll_extremum {
            return Ok(None);
        }
        let spot = crate::exotics::bs_gbm::resolve_positive_spot(
            market,
            &self.spot_id,
            self.currency,
            "LookbackOption",
        )?;
        let mut observed = self.clone();
        if super::pricer::tracks_max(self) {
            let next = self.observed_max.map_or(spot, |prev| prev.max(spot));
            if self.observed_max == Some(next) {
                return Ok(None);
            }
            observed.observed_max = Some(next);
        } else {
            let next = self.observed_min.map_or(spot, |prev| prev.min(spot));
            if self.observed_min == Some(next) {
                return Ok(None);
            }
            observed.observed_min = Some(next);
        }
        Ok(Some(Box::new(observed)))
    }

    finstack_quant_valuations_core::impl_focused_pricing_overrides!();
}

finstack_quant_valuations_core::impl_empty_cashflow_provider!(
    LookbackOption,
    finstack_quant_cashflows::builder::CashflowRepresentation::Placeholder
);
