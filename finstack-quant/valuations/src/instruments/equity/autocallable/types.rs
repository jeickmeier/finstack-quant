//! Autocallable structured product instrument definition.
//!
//! # Barrier Monitoring Convention
//!
//! Autocallable barriers are monitored **discretely** at the specified observation dates.
//! This implementation does NOT apply continuous monitoring or the Broadie-Glasserman-Kou
//! adjustment for discrete monitoring of continuous barriers.
//!
//! ## Why No Adjustment
//!
//! The Broadie-Glasserman-Kou adjustment (see reference below) is designed to correct for
//! discretely sampling a barrier that is contractually monitored continuously:
//! ```text
//! H_adj = H × exp(±0.5826 × σ × √Δt)
//! ```
//!
//! However, for autocallables, barriers are typically **contractually discrete** - they
//! are only checked on specific observation dates as defined in the term sheet. Therefore:
//! - The `observation_dates` field specifies the exact barrier monitoring dates
//! - Monte Carlo paths are evaluated exactly at these dates (time grid includes them)
//! - No adjustment is needed because there is no approximation of continuous monitoring
//!
//! ## For Continuously Monitored Barriers
//!
//! If you need to price a product with continuous barrier monitoring (e.g., daily close
//! knock-in/knock-out), you should either:
//! 1. Apply the BGK adjustment externally to the barrier levels
//! 2. Use a finer time grid with many intraday steps
//!
//! # References
//!
//! - Broadie, M., Glasserman, P., & Kou, S. (1997). "A Continuity Correction for
//!   Discrete Barrier Options." *Mathematical Finance*, 7(4), 325-349. `docs/REFERENCES.md#glasserman-2004-monte-carlo` `docs/REFERENCES.md#broadie-glasserman-kou-1997`
//! - Haug, E. G. (2007). *The Complete Guide to Option Pricing Formulas*, Section 4.17. `docs/REFERENCES.md#haug-2007-option-formulas`

use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::equity::EquityPathModel;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};
use time::macros::date;

/// Final payoff type for autocallable products.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FinalPayoffType {
    /// Capital protection: max(floor, participation * min(S_T/S_0, cap))
    CapitalProtection {
        /// Minimum return floor (e.g., 1.0 for 100% protection)
        floor: f64,
    },
    /// Participation: 1 + participation_rate * max(0, S_T/S_0 - 1)
    Participation {
        /// Participation rate in upside (e.g., 1.0 for 100% participation)
        rate: f64,
    },
    /// Knock-in put: Put option if barrier breached, otherwise return principal
    KnockInPut {
        /// Strike price for knock-in put option
        strike: f64,
    },
}

/// Autocallable structured product instrument.
#[derive(
    PartialEq,
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields, try_from = "AutocallableUnchecked")]
pub struct Autocallable {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Underlying asset ticker symbol
    pub underlying_ticker: String,
    /// Observation dates for autocall and coupon checks.
    ///
    /// Barriers are monitored **discretely** at these exact dates only.
    /// The Monte Carlo time grid is constructed to include these dates precisely.
    #[serde(with = "finstack_quant_core::wire::dates")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    pub observation_dates: Vec<Date>,
    /// Contractual payment dates for coupons and early redemption.
    ///
    /// Each entry corresponds to `observation_dates` at the same index.
    #[serde(with = "finstack_quant_core::wire::dates")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    pub payment_dates: Vec<Date>,
    /// Explicit terminal expiry date for the structure.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub expiry: Date,
    /// Autocall barrier levels (as ratios of initial spot, e.g., 1.0 = 100%).
    ///
    /// Each barrier corresponds to the observation date at the same index.
    /// If spot ≥ barrier × initial_spot on the observation date, the product autocalls.
    pub autocall_barriers: Vec<f64>,
    /// Coupon barriers as ratios of initial spot.
    ///
    /// Coupon eligibility is independent of the autocall barrier. Each entry
    /// corresponds to the observation date at the same index.
    pub coupon_barriers: Vec<f64>,
    /// Coupon return amounts for each observation date.
    pub coupons: Vec<f64>,
    /// Memory ("Phoenix") coupon feature.
    ///
    /// When enabled, a coupon whose coupon barrier is missed accrues until a
    /// later observation meets its coupon barrier. The current and all accrued
    /// coupons are then paid on that observation's payment date. Coupon
    /// eligibility remains independent of autocall eligibility.
    #[serde(default)]
    #[builder(default)]
    pub memory_coupons: bool,
    /// Final barrier level for final payoff determination
    pub final_barrier: f64,
    /// Type of final payoff (capital protection, participation, knock-in put)
    pub final_payoff_type: FinalPayoffType,
    /// Participation rate in underlying performance
    pub participation_rate: f64,
    /// Cap level for final payoff (maximum return)
    pub cap_level: f64,
    /// Notional amount
    pub notional: Money,
    /// Day count convention for interest calculations
    pub day_count: finstack_quant_core::dates::DayCount,
    /// Discount curve ID for present value calculations
    pub discount_curve_id: CurveId,
    /// Spot price identifier for underlying asset
    pub spot_id: PriceId,
    /// Volatility surface ID for option pricing
    pub vol_surface_id: CurveId,
    /// Explicit path model selection.
    ///
    /// `AtmTermGbm` is an ATM-term-structure approximation and does not model
    /// equity strike skew; callers must opt into that limitation.
    pub path_model: EquityPathModel,
    /// Optional dividend-yield scalar ID.
    ///
    /// `Some(id)`: lookup MUST succeed (a missing or non-unitless scalar
    /// returns an error). `None`: no implicit default; treated as zero
    /// continuous dividend yield. Set explicitly for index underlyings.
    pub div_yield_id: Option<PriceId>,
    /// Initial (strike-set) underlying level S_0 used as the reference for
    /// barrier and payoff ratios.
    ///
    /// `None` (the default) uses the spot at the valuation date, which is only
    /// correct for a new trade priced on its strike-set date. **Required for
    /// seasoned trades** (any observation date on or before `as_of`): pricing
    /// errors if past observation dates exist without it.
    #[builder(default)]
    #[serde(default)]
    pub initial_level: Option<f64>,
    /// Observed underlying fixings for seasoned trades (date, level pairs).
    ///
    /// For a mid-life autocallable, every observation date on or before the
    /// valuation date must have a matching fixing here; pricing errors
    /// otherwise. Past fixings are evaluated deterministically (autocall,
    /// missed memory coupons, discrete knock-in monitoring) and only the
    /// remaining future observation dates are simulated.
    #[builder(default)]
    #[serde(default)]
    #[serde(with = "finstack_quant_core::wire::dated_f64_values")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<(finstack_quant_core::wire::DateWire, f64)>")
    )]
    pub past_fixings: Vec<(Date, f64)>,
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

/// Mirror of `Autocallable` used by serde to apply `validate()` after
/// deserialization. Not part of the public API.
#[derive(Debug, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct AutocallableUnchecked {
    /// Unique instrument identifier
    id: InstrumentId,
    /// Underlying asset ticker symbol
    underlying_ticker: String,
    /// Observation dates for autocall and coupon checks.
    ///
    /// Barriers are monitored **discretely** at these exact dates only.
    /// The Monte Carlo time grid is constructed to include these dates precisely.
    #[serde(with = "finstack_quant_core::wire::dates")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    observation_dates: Vec<Date>,
    /// Contractual payment dates corresponding to observation dates.
    #[serde(with = "finstack_quant_core::wire::dates")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    payment_dates: Vec<Date>,
    /// Explicit terminal expiry date for the structure.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    expiry: Date,
    /// Autocall barrier levels (as ratios of initial spot, e.g., 1.0 = 100%).
    ///
    /// Each barrier corresponds to the observation date at the same index.
    /// If spot ≥ barrier × initial_spot on the observation date, the product autocalls.
    autocall_barriers: Vec<f64>,
    /// Coupon barriers corresponding to observation dates.
    coupon_barriers: Vec<f64>,
    /// Coupon return amounts for each observation date.
    coupons: Vec<f64>,
    /// Memory ("Phoenix") coupon feature.
    ///
    /// Missed coupons accrue until a later coupon barrier is met.
    #[serde(default)]
    memory_coupons: bool,
    /// Final barrier level for final payoff determination
    final_barrier: f64,
    /// Type of final payoff (capital protection, participation, knock-in put)
    final_payoff_type: FinalPayoffType,
    /// Participation rate in underlying performance
    participation_rate: f64,
    /// Cap level for final payoff (maximum return)
    cap_level: f64,
    /// Notional amount
    notional: Money,
    /// Day count convention for interest calculations
    day_count: finstack_quant_core::dates::DayCount,
    /// Discount curve ID for present value calculations
    discount_curve_id: CurveId,
    /// Spot price identifier for underlying asset
    spot_id: PriceId,
    /// Volatility surface ID for option pricing
    vol_surface_id: CurveId,
    /// Explicit path model selection; required to acknowledge model risk.
    path_model: EquityPathModel,
    /// Optional dividend-yield scalar ID.
    ///
    /// `Some(id)`: lookup MUST succeed (a missing or non-unitless scalar
    /// returns an error). `None`: no implicit default; treated as zero
    /// continuous dividend yield. Set explicitly for index underlyings.
    #[serde(default)]
    div_yield_id: Option<PriceId>,
    /// Initial (strike-set) underlying level S_0 used as the reference for
    /// barrier and payoff ratios.
    ///
    /// `None` (the default) uses the spot at the valuation date, which is only
    /// correct for a new trade priced on its strike-set date. **Required for
    /// seasoned trades** (any observation date on or before `as_of`): pricing
    /// errors if past observation dates exist without it.
    #[serde(default)]
    initial_level: Option<f64>,
    /// Observed underlying fixings for seasoned trades (date, level pairs).
    ///
    /// For a mid-life autocallable, every observation date on or before the
    /// valuation date must have a matching fixing here; pricing errors
    /// otherwise. Past fixings are evaluated deterministically (autocall,
    /// missed memory coupons, discrete knock-in monitoring) and only the
    /// remaining future observation dates are simulated.
    #[serde(default)]
    #[serde(with = "finstack_quant_core::wire::dated_f64_values")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<(finstack_quant_core::wire::DateWire, f64)>")
    )]
    past_fixings: Vec<(Date, f64)>,
    /// Pricing overrides (manual price, yield, spread)
    #[serde(default)]
    instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-only pricing controls.
    #[serde(default)]
    metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only valuation adjustments.
    #[serde(default)]
    scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and grouping
    attributes: Attributes,
}

impl TryFrom<AutocallableUnchecked> for Autocallable {
    type Error = finstack_quant_core::Error;

    fn try_from(value: AutocallableUnchecked) -> std::result::Result<Self, Self::Error> {
        let inst = Self {
            id: value.id,
            underlying_ticker: value.underlying_ticker,
            observation_dates: value.observation_dates,
            payment_dates: value.payment_dates,
            expiry: value.expiry,
            autocall_barriers: value.autocall_barriers,
            coupon_barriers: value.coupon_barriers,
            coupons: value.coupons,
            memory_coupons: value.memory_coupons,
            final_barrier: value.final_barrier,
            final_payoff_type: value.final_payoff_type,
            participation_rate: value.participation_rate,
            cap_level: value.cap_level,
            notional: value.notional,
            day_count: value.day_count,
            discount_curve_id: value.discount_curve_id,
            spot_id: value.spot_id,
            vol_surface_id: value.vol_surface_id,
            path_model: value.path_model,
            div_yield_id: value.div_yield_id,
            initial_level: value.initial_level,
            past_fixings: value.past_fixings,
            instrument_pricing_overrides: value.instrument_pricing_overrides,
            metric_pricing_overrides: value.metric_pricing_overrides,
            scenario_pricing_overrides: value.scenario_pricing_overrides,
            attributes: value.attributes,
        };
        inst.validate()?;
        Ok(inst)
    }
}

impl Autocallable {
    /// Validate structural invariants required by the pricing engine.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `observation_dates` is empty
    /// - `observation_dates` are not strictly increasing
    /// - any observation date is strictly after `expiry`
    /// - payment/coupon/autocall vector lengths differ from `observation_dates`
    /// - a payment date precedes its observation date or follows `expiry`
    /// - any barrier is negative or non-finite
    /// - any coupon is non-finite
    /// - `final_barrier`, `participation_rate`, or `cap_level` are non-finite
    /// - `cap_level <= 0`
    /// - `notional.amount()` is not finite
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        let n = self.observation_dates.len();
        if n == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "Autocallable requires at least one observation date".into(),
            ));
        }
        for window in self.observation_dates.windows(2) {
            if window[0] >= window[1] {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Autocallable observation_dates must be strictly increasing; got {} >= {}",
                    window[0], window[1]
                )));
            }
        }
        // Safe: observation_dates is non-empty (checked above).
        if let Some(&last_obs) = self.observation_dates.last() {
            if last_obs > self.expiry {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Autocallable last observation date {} is after expiry {}",
                    last_obs, self.expiry
                )));
            }
        }
        if self.payment_dates.len() != n {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Autocallable payment_dates.len() ({}) must match observation_dates.len() ({n})",
                self.payment_dates.len()
            )));
        }
        for (index, (&observation, &payment)) in self
            .observation_dates
            .iter()
            .zip(&self.payment_dates)
            .enumerate()
        {
            if payment < observation || payment > self.expiry {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Autocallable payment_dates[{index}]={payment} must be on/after \
                     observation date {observation} and on/before expiry {}",
                    self.expiry
                )));
            }
        }
        if self
            .payment_dates
            .windows(2)
            .any(|window| window[0] > window[1])
        {
            return Err(finstack_quant_core::Error::Validation(
                "Autocallable payment_dates must be nondecreasing".into(),
            ));
        }
        if self.autocall_barriers.len() != n {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Autocallable autocall_barriers.len() ({}) must match observation_dates.len() ({})",
                self.autocall_barriers.len(),
                n
            )));
        }
        if self.coupon_barriers.len() != n {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Autocallable coupon_barriers.len() ({}) must match observation_dates.len() ({n})",
                self.coupon_barriers.len()
            )));
        }
        if self.coupons.len() != n {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Autocallable coupons.len() ({}) must match observation_dates.len() ({})",
                self.coupons.len(),
                n
            )));
        }
        for (i, b) in self.autocall_barriers.iter().enumerate() {
            if !b.is_finite() || *b < 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Autocallable autocall_barriers[{}] = {} must be finite and non-negative",
                    i, b
                )));
            }
        }
        for (i, barrier) in self.coupon_barriers.iter().enumerate() {
            if !barrier.is_finite() || *barrier < 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Autocallable coupon_barriers[{i}]={barrier} must be finite and non-negative"
                )));
            }
        }
        for (i, c) in self.coupons.iter().enumerate() {
            if !c.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Autocallable coupons[{}] = {} must be finite",
                    i, c
                )));
            }
        }
        if !self.final_barrier.is_finite() || self.final_barrier < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Autocallable final_barrier = {} must be finite and non-negative",
                self.final_barrier
            )));
        }
        if !self.participation_rate.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Autocallable participation_rate = {} must be finite",
                self.participation_rate
            )));
        }
        if !self.cap_level.is_finite() || self.cap_level <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Autocallable cap_level = {} must be finite and positive",
                self.cap_level
            )));
        }
        match self.final_payoff_type {
            FinalPayoffType::CapitalProtection { floor } => {
                if !floor.is_finite() || floor < 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Autocallable capital-protection floor = {floor} must be finite and non-negative"
                    )));
                }
            }
            FinalPayoffType::Participation { rate } => {
                if !rate.is_finite() || rate < 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Autocallable final participation rate = {rate} must be finite and non-negative"
                    )));
                }
            }
            FinalPayoffType::KnockInPut { strike } => {
                if !strike.is_finite() || strike <= 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Autocallable knock-in put strike = {strike} must be finite and positive"
                    )));
                }
            }
        }
        if !self.notional.amount().is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "Autocallable notional amount must be finite".into(),
            ));
        }
        if let Some(level) = self.initial_level {
            if !level.is_finite() || level <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Autocallable initial_level = {} must be finite and positive",
                    level
                )));
            }
        }
        for (d, v) in &self.past_fixings {
            if !v.is_finite() || *v <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Autocallable past_fixings[{}] = {} must be finite and positive",
                    d, v
                )));
            }
        }
        Ok(())
    }

    /// Look up the observed fixing for an observation date, if provided.
    pub fn fixing_on(&self, date: Date) -> Option<f64> {
        self.past_fixings
            .iter()
            .find(|(d, _)| *d == date)
            .map(|(_, v)| *v)
    }

    /// Create a canonical example autocallable (quarterly observations, simple barriers/coupons).
    pub fn example() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::dates::DayCount;
        let observation_dates = vec![
            date!(2024 - 03 - 29),
            date!(2024 - 06 - 28),
            date!(2024 - 09 - 30),
            date!(2024 - 12 - 31),
        ];
        let payment_dates = observation_dates.clone();
        let autocall_barriers = vec![1.0, 1.0, 1.0, 1.0];
        let coupon_barriers = vec![0.7, 0.7, 0.7, 0.7];
        let coupons = vec![0.02, 0.02, 0.02, 0.02];
        Autocallable::builder()
            .id(InstrumentId::new("AUTO-SPX-QTR"))
            .underlying_ticker("SPX".to_string())
            .observation_dates(observation_dates)
            .payment_dates(payment_dates)
            .expiry(date!(2024 - 12 - 31))
            .autocall_barriers(autocall_barriers)
            .coupon_barriers(coupon_barriers)
            .coupons(coupons)
            .final_barrier(0.6) // 60% final KI barrier
            .final_payoff_type(FinalPayoffType::Participation { rate: 1.0 })
            .participation_rate(1.0)
            .cap_level(1.5) // 150% cap
            .notional(Money::from((100_000_i64, Currency::USD)))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .path_model(crate::instruments::equity::EquityPathModel::AtmTermGbm)
            .div_yield_id_opt(Some(PriceId::new("SPX-DIV")))
            .attributes(Attributes::new())
            .build()
    }
}

impl crate::instruments::common_impl::traits::Instrument for Autocallable {
    impl_instrument_base!(crate::pricer::InstrumentType::Autocallable);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::MonteCarloGBM
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
                None,
            ),
        );
        if let Some(dividend_yield) = &self.div_yield_id {
            deps.add_market_scalar_id(dividend_yield.as_str());
        }
        Ok(deps)
    }

    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        use crate::instruments::equity::autocallable::pricer;
        self.validate()?;
        pricer::compute_pv(self, market, as_of)
    }

    fn effective_start_date(&self) -> Option<Date> {
        self.observation_dates.first().copied()
    }

    fn expiry(&self) -> Option<Date> {
        Some(self.expiry)
    }

    crate::impl_focused_pricing_overrides!();
}

crate::impl_empty_cashflow_provider!(
    Autocallable,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod validation_tests {
    use super::*;
    use crate::instruments::Attributes;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;

    fn base_builder() -> crate::instruments::equity::autocallable::types::AutocallableBuilder {
        Autocallable::builder()
            .id(InstrumentId::new("AUTO-TEST"))
            .underlying_ticker("SPX".to_string())
            .observation_dates(vec![date!(2024 - 06 - 28), date!(2024 - 12 - 31)])
            .payment_dates(vec![date!(2024 - 06 - 28), date!(2024 - 12 - 31)])
            .expiry(date!(2024 - 12 - 31))
            .autocall_barriers(vec![1.0, 1.0])
            .coupon_barriers(vec![0.7, 0.7])
            .coupons(vec![0.02, 0.02])
            .final_barrier(0.6)
            .final_payoff_type(FinalPayoffType::Participation { rate: 1.0 })
            .participation_rate(1.0)
            .cap_level(1.5)
            .notional(Money::from((100_000_i64, Currency::USD)))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .path_model(crate::instruments::equity::EquityPathModel::AtmTermGbm)
            .div_yield_id_opt(None)
            .attributes(Attributes::new())
    }

    #[test]
    fn builder_rejects_empty_observation_dates() {
        let result = base_builder().observation_dates(vec![]).build();
        assert!(result.is_err(), "empty observation_dates must be rejected");
    }

    #[test]
    fn builder_rejects_mismatched_barriers_length() {
        let result = base_builder().autocall_barriers(vec![1.0]).build();
        assert!(
            result.is_err(),
            "autocall_barriers length mismatch must be rejected"
        );
    }

    #[test]
    fn builder_rejects_mismatched_coupons_length() {
        let result = base_builder().coupons(vec![0.02]).build();
        assert!(result.is_err(), "coupons length mismatch must be rejected");
    }

    #[test]
    fn builder_rejects_payment_before_observation() {
        let result = base_builder()
            .payment_dates(vec![date!(2024 - 06 - 27), date!(2024 - 12 - 31)])
            .build();
        assert!(
            result.is_err(),
            "payment before observation must be rejected"
        );
    }

    #[test]
    fn builder_rejects_invalid_nested_final_payoff_parameters() {
        assert!(base_builder()
            .final_payoff_type(FinalPayoffType::CapitalProtection { floor: -0.1 })
            .build()
            .is_err());
        assert!(base_builder()
            .final_payoff_type(FinalPayoffType::Participation { rate: -1.0 })
            .build()
            .is_err());
        assert!(base_builder()
            .final_payoff_type(FinalPayoffType::KnockInPut { strike: 0.0 })
            .build()
            .is_err());
    }

    #[test]
    fn builder_rejects_unsorted_observation_dates() {
        let result = base_builder()
            .observation_dates(vec![date!(2024 - 12 - 31), date!(2024 - 06 - 28)])
            .build();
        assert!(
            result.is_err(),
            "unsorted observation_dates must be rejected"
        );
    }

    #[test]
    fn builder_rejects_observation_after_expiry() {
        let result = base_builder()
            .observation_dates(vec![date!(2024 - 12 - 31), date!(2025 - 01 - 31)])
            .expiry(date!(2024 - 12 - 31))
            .build();
        assert!(result.is_err(), "observation after expiry must be rejected");
    }

    #[test]
    fn builder_rejects_negative_barrier() {
        let result = base_builder().autocall_barriers(vec![1.0, -0.1]).build();
        assert!(result.is_err(), "negative barrier must be rejected");
    }

    #[test]
    fn builder_rejects_non_positive_cap_level() {
        let result = base_builder().cap_level(0.0).build();
        assert!(result.is_err(), "non-positive cap_level must be rejected");
    }

    #[test]
    fn serde_requires_explicit_path_model() {
        let option = base_builder().build().expect("valid autocallable");
        let mut value = serde_json::to_value(option).expect("serialize");
        value.as_object_mut().expect("object").remove("path_model");

        let error = serde_json::from_value::<Autocallable>(value)
            .expect_err("missing path model must fail");
        assert!(error.to_string().contains("path_model"));
    }
}
