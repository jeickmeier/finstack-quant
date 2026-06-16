//! Volatility Index Option types and implementation.
//!
//! Defines the `VolatilityIndexOption` instrument for options on VIX, VXN,
//! VSTOXX, and similar volatility indices. These options are European-style,
//! cash-settled, and priced using the Black model.
//!
//! # Contract Specifications
//!
//! VIX options are traded on CBOE with the following standard specs:
//! - Multiplier: $100 per index point
//! - Settlement: Cash-settled to VRO (VIX Settlement Value)
//! - Style: European only
//!
//! # Pricing Model
//!
//! VIX options use the Black model (not Black-Scholes) because:
//! 1. The underlying is the VIX forward, not spot
//! 2. VIX has no cost of carry or dividends
//! 3. The forward price is directly observable from futures
//!
//! ```text
//! Call = DF × [F × N(d1) - K × N(d2)]
//! Put  = DF × [K × N(-d2) - F × N(-d1)]
//!
//! where:
//!   d1 = [ln(F/K) + 0.5σ²T] / (σ√T)
//!   d2 = d1 - σ√T
//!   DF = discount factor to expiry
//! ```
//!
//! # Vol-of-Vol Surface
//!
//! The σ in the Black model is the "vol-of-vol" - the volatility of the
//! volatility index itself. This is typically quoted in a 2D surface indexed
//! by expiry and strike (or moneyness).
//!
//! # References
//!
//! - CBOE (2019). "VIX Options Contract Specifications."
//! - Carr, P., & Lee, R. (2009). "Volatility Derivatives."
//!   *Annual Review of Financial Economics*, 1, 319-339.

use super::pricer;
use crate::cashflow::traits::CashflowProvider;
use crate::contract_specs::{embedded_registry, ContractSpecRegistry};
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::{ExerciseStyle, OptionType};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use time::macros::date;

/// Volatility Index Option instrument.
///
/// European-style options on volatility indices (VIX, VXN, VSTOXX).
/// Priced using the Black model with the vol index forward as underlying.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::equity::vol_index_option::VolatilityIndexOption;
/// use finstack_quant_valuations::instruments::OptionType;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::{Date, DayCount};
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::types::{CurveId, InstrumentId};
/// use time::Month;
///
/// let option = VolatilityIndexOption::builder()
///     .id(InstrumentId::new("VIX-CALL-20-2025M03"))
///     .notional(Money::new(10_000.0, Currency::USD))
///     .strike(20.0)
///     .option_type(OptionType::Call)
///     .expiry(Date::from_calendar_date(2025, Month::March, 19).unwrap())
///     .day_count(DayCount::Act365F)
///     .discount_curve_id(CurveId::new("USD-OIS"))
///     .vol_index_curve_id(CurveId::new("VIX"))
///     .vol_of_vol_surface_id(CurveId::new("VIX-VOLVOL"))
///     .build()
///     .expect("Valid option");
/// ```
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[builder(validate = VolatilityIndexOption::validate)]
#[serde(deny_unknown_fields, try_from = "VolatilityIndexOptionUnchecked")]
pub struct VolatilityIndexOption {
    /// Unique identifier.
    pub id: InstrumentId,
    /// Notional exposure in currency units.
    pub notional: Money,
    /// Strike price (in index points, e.g., 20.0).
    pub strike: f64,
    /// Option type (Call or Put).
    pub option_type: OptionType,
    /// Exercise style (always European for VIX options).
    #[builder(default)]
    #[serde(default)]
    pub exercise_style: ExerciseStyle,
    /// Option expiry date.
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Settlement date (typically same as expiry for cash-settled).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub settlement_date: Option<Date>,
    /// Contract specifications.
    #[builder(default)]
    #[serde(default)]
    pub contract_specs: VolIndexOptionSpecs,
    /// Day count convention (default: Act365F).
    pub day_count: DayCount,
    /// Discount curve identifier for present value calculations.
    pub discount_curve_id: CurveId,
    /// Volatility index forward curve identifier.
    pub vol_index_curve_id: CurveId,
    /// Vol-of-vol surface identifier for option implied volatility.
    pub vol_of_vol_surface_id: CurveId,
    /// Attributes for tagging and selection.
    #[builder(default)]
    #[serde(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    #[serde(default)]
    #[builder(default)]
    pub attributes: Attributes,
}

/// Contract specifications for volatility index options.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct VolIndexOptionSpecs {
    /// Contract multiplier (USD per index point).
    /// VIX options standard: 100 (each point = $100)
    pub multiplier: f64,
    /// Index identifier (e.g., "VIX", "VXN", "VSTOXX").
    pub index_id: String,
}

impl Default for VolIndexOptionSpecs {
    fn default() -> Self {
        Self::vix()
    }
}

#[allow(clippy::expect_used)]
fn contract_spec_registry() -> &'static ContractSpecRegistry {
    embedded_registry().expect("embedded contract-spec registry should load")
}

#[allow(clippy::expect_used)]
fn vol_index_option_specs_from_registry(id: &str) -> VolIndexOptionSpecs {
    contract_spec_registry()
        .vol_index_option_specs(id)
        .expect("embedded volatility index option contract spec should exist")
}

impl VolIndexOptionSpecs {
    /// Create specs for standard VIX options.
    pub fn vix() -> Self {
        vol_index_option_specs_from_registry("cboe.vix_option")
    }

    /// Create specs for VSTOXX options.
    pub fn vstoxx() -> Self {
        vol_index_option_specs_from_registry("eurex.vstoxx_option")
    }
}

/// Mirror of `VolatilityIndexOption` used by serde to apply `validate()`
/// after deserialization. Not part of the public API.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct VolatilityIndexOptionUnchecked {
    id: InstrumentId,
    notional: Money,
    strike: f64,
    option_type: OptionType,
    #[serde(default)]
    exercise_style: ExerciseStyle,
    #[schemars(with = "String")]
    expiry: Date,
    #[serde(default)]
    #[schemars(with = "Option<String>")]
    settlement_date: Option<Date>,
    #[serde(default)]
    contract_specs: VolIndexOptionSpecs,
    day_count: DayCount,
    discount_curve_id: CurveId,
    vol_index_curve_id: CurveId,
    vol_of_vol_surface_id: CurveId,
    #[serde(default)]
    pricing_overrides: crate::instruments::PricingOverrides,
    #[serde(default)]
    attributes: Attributes,
}

impl TryFrom<VolatilityIndexOptionUnchecked> for VolatilityIndexOption {
    type Error = finstack_quant_core::Error;

    fn try_from(value: VolatilityIndexOptionUnchecked) -> std::result::Result<Self, Self::Error> {
        let inst = Self {
            id: value.id,
            notional: value.notional,
            strike: value.strike,
            option_type: value.option_type,
            exercise_style: value.exercise_style,
            expiry: value.expiry,
            settlement_date: value.settlement_date,
            contract_specs: value.contract_specs,
            day_count: value.day_count,
            discount_curve_id: value.discount_curve_id,
            vol_index_curve_id: value.vol_index_curve_id,
            vol_of_vol_surface_id: value.vol_of_vol_surface_id,
            pricing_overrides: value.pricing_overrides,
            attributes: value.attributes,
        };
        inst.validate()?;
        Ok(inst)
    }
}

impl VolatilityIndexOption {
    /// Create a canonical example VIX call option for testing.
    pub fn example() -> finstack_quant_core::Result<Self> {
        // SAFETY: All inputs are compile-time validated constants
        Self::builder()
            .id(InstrumentId::new("VIX-CALL-20-2025M03"))
            .notional(Money::new(10_000.0, Currency::USD))
            .strike(20.0)
            .option_type(OptionType::Call)
            .exercise_style(ExerciseStyle::European)
            .expiry(date!(2025 - 03 - 19))
            .contract_specs(VolIndexOptionSpecs::vix())
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .vol_of_vol_surface_id(CurveId::new("VIX-VOLVOL"))
            .attributes(Attributes::new())
            .build()
    }

    /// Validate structural invariants required by the Black-model pricer.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `strike` is non-positive or non-finite (Black formula needs `ln(F/K)`)
    /// - `notional.amount()` is non-finite
    /// - `contract_specs.multiplier` is non-positive or non-finite
    /// - `settlement_date` is set but earlier than `expiry`
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if !self.strike.is_finite() || self.strike <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "VolatilityIndexOption strike must be finite and positive, got {}",
                self.strike
            )));
        }
        if !self.notional.amount().is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "VolatilityIndexOption notional amount must be finite".into(),
            ));
        }
        if !self.contract_specs.multiplier.is_finite()
            || self.contract_specs.multiplier <= f64::MIN_POSITIVE
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "VolatilityIndexOption contract spec value must be finite and positive, got {}",
                self.contract_specs.multiplier
            )));
        }
        if let Some(settle) = self.settlement_date {
            if settle < self.expiry {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "VolatilityIndexOption settlement_date {} must be on or after expiry {}",
                    settle, self.expiry
                )));
            }
        }
        Ok(())
    }

    /// Create a VIX call option.
    ///
    /// # Errors
    ///
    /// Returns an error if the builder fails validation.
    pub fn vix_call(
        id: impl Into<String>,
        strike: f64,
        expiry: Date,
        notional: Money,
    ) -> finstack_quant_core::Result<Self> {
        Self::builder()
            .id(InstrumentId::new(id.into()))
            .notional(notional)
            .strike(strike)
            .option_type(OptionType::Call)
            .exercise_style(ExerciseStyle::European)
            .expiry(expiry)
            .contract_specs(VolIndexOptionSpecs::vix())
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .vol_of_vol_surface_id(CurveId::new("VIX-VOLVOL"))
            .attributes(Attributes::new())
            .build()
    }

    /// Create a VIX put option.
    ///
    /// # Errors
    ///
    /// Returns an error if the builder fails validation.
    pub fn vix_put(
        id: impl Into<String>,
        strike: f64,
        expiry: Date,
        notional: Money,
    ) -> finstack_quant_core::Result<Self> {
        Self::builder()
            .id(InstrumentId::new(id.into()))
            .notional(notional)
            .strike(strike)
            .option_type(OptionType::Put)
            .exercise_style(ExerciseStyle::European)
            .expiry(expiry)
            .contract_specs(VolIndexOptionSpecs::vix())
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .vol_of_vol_surface_id(CurveId::new("VIX-VOLVOL"))
            .attributes(Attributes::new())
            .build()
    }

    /// Calculate the number of contracts based on notional.
    ///
    /// # Formula
    /// ```text
    /// contracts = notional / (multiplier × strike)
    /// ```
    pub fn num_contracts(&self) -> f64 {
        let contract_value = self.contract_specs.multiplier * self.strike;
        if contract_value > 0.0 {
            self.notional.amount() / contract_value
        } else {
            0.0
        }
    }

    /// Calculate the raw present value as f64.
    ///
    /// # Arguments
    ///
    /// * `context` - Market context with vol index curves and surfaces
    /// * `as_of` - Valuation date for time to expiry calculation
    pub fn npv_raw(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        pricer::compute_pv_raw(self, context, as_of)
    }

    /// Calculate the Black model option price (undiscounted).
    ///
    /// # Arguments
    /// * `forward` - Forward vol index level
    /// * `sigma` - Vol-of-vol (volatility of the vol index)
    /// * `t` - Time to expiry in years
    ///
    /// # Returns
    /// Undiscounted option price per index point
    pub fn black_price(&self, forward: f64, sigma: f64, t: f64) -> f64 {
        pricer::black_price(self, forward, sigma, t)
    }

    /// Get the forward volatility level at expiry.
    ///
    /// # Arguments
    /// * `context` - Market context with vol index curves
    /// * `as_of` - Valuation date for time to expiry calculation
    pub fn forward_vol(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        pricer::forward_vol(self, context, as_of)
    }

    /// Calculate delta (sensitivity to forward vol level).
    ///
    /// # Arguments
    /// * `context` - Market context with curves and surfaces
    /// * `as_of` - Valuation date for time to expiry calculation
    ///
    /// # Returns
    /// Delta per contract (change in option value per 1-point change in forward)
    pub fn delta(&self, context: &MarketContext, as_of: Date) -> finstack_quant_core::Result<f64> {
        pricer::delta(self, context, as_of)
    }

    /// Calculate gamma (second derivative w.r.t. forward vol level).
    ///
    /// # Arguments
    /// * `context` - Market context with curves and surfaces
    /// * `as_of` - Valuation date for time to expiry calculation
    pub fn gamma(&self, context: &MarketContext, as_of: Date) -> finstack_quant_core::Result<f64> {
        pricer::gamma(self, context, as_of)
    }

    /// Calculate vega (sensitivity to vol-of-vol).
    ///
    /// # Arguments
    /// * `context` - Market context with curves and surfaces
    /// * `as_of` - Valuation date for time to expiry calculation
    ///
    /// # Returns
    /// Change in option value for a 1% change in vol-of-vol
    pub fn vega(&self, context: &MarketContext, as_of: Date) -> finstack_quant_core::Result<f64> {
        pricer::vega(self, context, as_of)
    }

    /// Calculate theta (time decay, per day).
    ///
    /// # Arguments
    /// * `context` - Market context with curves and surfaces
    /// * `as_of` - Valuation date for time to expiry calculation
    ///
    /// # Returns
    /// Change in option value for 1 day passing
    pub fn theta(&self, context: &MarketContext, as_of: Date) -> finstack_quant_core::Result<f64> {
        pricer::theta(self, context, as_of)
    }

    /// Calculate intrinsic value.
    ///
    /// # Arguments
    /// * `context` - Market context with vol index curves
    /// * `as_of` - Valuation date for time to expiry calculation
    pub fn intrinsic_value(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        pricer::intrinsic_value(self, context, as_of)
    }

    /// Calculate time value (option value minus intrinsic value).
    ///
    /// # Arguments
    /// * `context` - Market context with curves and surfaces
    /// * `as_of` - Valuation date for time to expiry calculation
    pub fn time_value(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        pricer::time_value(self, context, as_of)
    }
}

// ================================================================================================
// Option risk metric providers (metrics adapters)
// ================================================================================================

impl crate::instruments::common_impl::traits::OptionGreeksProvider for VolatilityIndexOption {
    fn option_delta(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(Some(self.delta(market, as_of)?))
    }

    fn option_vega(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(Some(self.vega(market, as_of)?))
    }
}

// =============================================================================
// Trait Implementations
// =============================================================================

impl crate::instruments::common_impl::traits::Instrument for VolatilityIndexOption {
    impl_instrument_base!(crate::pricer::InstrumentType::VolatilityIndexOption);

    fn base_value(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        pricer::compute_pv(self, curves, as_of)
    }

    fn value_raw(&self, curves: &MarketContext, as_of: Date) -> finstack_quant_core::Result<f64> {
        pricer::compute_pv_raw(self, curves, as_of)
    }

    fn effective_start_date(&self) -> Option<Date> {
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

impl CashflowProvider for VolatilityIndexOption {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn cashflow_schedule(
        &self,
        _curves: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        Ok(crate::cashflow::traits::schedule_from_classified_flows(
            Vec::new(),
            self.day_count,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: self.notional(),
                representation: crate::cashflow::builder::CashflowRepresentation::Placeholder,
                ..Default::default()
            },
        ))
    }
}

impl crate::instruments::common_impl::traits::CurveDependencies for VolatilityIndexOption {
    fn curve_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::InstrumentCurves>
    {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .build()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::market_data::term_structures::VolatilityIndexCurve;
    use time::Month;

    fn setup_market() -> (MarketContext, Date) {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        // Create discount curve (4% flat rate)
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (1.0, 0.96)])
            .build()
            .expect("valid discount curve");

        // Create VIX forward curve - contango structure
        let vix = VolatilityIndexCurve::builder("VIX")
            .base_date(base_date)
            .spot_level(18.0)
            .knots([(0.0, 18.0), (0.25, 20.0), (0.5, 21.0), (1.0, 22.0)])
            .build()
            .expect("valid VIX curve");

        // Create vol-of-vol surface (flat 80% vol-of-vol)
        let volvol = VolSurface::builder("VIX-VOLVOL")
            .expiries(&[0.25, 0.5, 1.0])
            .strikes(&[15.0, 20.0, 25.0])
            .row(&[0.8, 0.8, 0.8])
            .row(&[0.8, 0.8, 0.8])
            .row(&[0.8, 0.8, 0.8])
            .build()
            .expect("valid vol-of-vol surface");

        let ctx = MarketContext::new()
            .insert(disc)
            .insert(vix)
            .insert_surface(volvol);

        (ctx, base_date)
    }

    #[test]
    fn test_atm_call_has_positive_value() {
        let (market, as_of) = setup_market();

        // Create ATM call (strike = forward ~20)
        let option = VolatilityIndexOption::builder()
            .id(InstrumentId::new("VIX-CALL-ATM"))
            .notional(Money::new(2_000.0, Currency::USD)) // 1 contract
            .strike(20.0)
            .option_type(OptionType::Call)
            .expiry(Date::from_calendar_date(2025, Month::April, 1).expect("valid date"))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .vol_of_vol_surface_id(CurveId::new("VIX-VOLVOL"))
            .build()
            .expect("valid option");

        let npv = option.value(&market, as_of).expect("value calculation");
        assert!(npv.amount() > 0.0, "ATM call should have positive value");
    }

    #[test]
    fn test_put_call_parity() {
        let (market, as_of) = setup_market();
        let expiry = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let strike = 20.0;

        let call = VolatilityIndexOption::builder()
            .id(InstrumentId::new("VIX-CALL"))
            .notional(Money::new(2_000.0, Currency::USD))
            .strike(strike)
            .option_type(OptionType::Call)
            .expiry(expiry)
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .vol_of_vol_surface_id(CurveId::new("VIX-VOLVOL"))
            .build()
            .expect("valid call");

        let put = VolatilityIndexOption::builder()
            .id(InstrumentId::new("VIX-PUT"))
            .notional(Money::new(2_000.0, Currency::USD))
            .strike(strike)
            .option_type(OptionType::Put)
            .expiry(expiry)
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .vol_of_vol_surface_id(CurveId::new("VIX-VOLVOL"))
            .build()
            .expect("valid put");

        let call_pv = call.npv_raw(&market, as_of).expect("call npv");
        let put_pv = put.npv_raw(&market, as_of).expect("put npv");
        let forward = call.forward_vol(&market, as_of).expect("forward vol");

        // Get DF for put-call parity
        let disc = market.get_discount("USD-OIS").expect("discount curve");
        let df = disc
            .df_between_dates(as_of, expiry)
            .expect("discount factor");

        let contracts = call.num_contracts();
        let mult = call.contract_specs.multiplier;

        // Put-Call Parity: C - P = (F - K) × DF × mult × contracts
        let parity_diff = (forward - strike) * df * mult * contracts;
        let actual_diff = call_pv - put_pv;

        assert!(
            (actual_diff - parity_diff).abs() < 1.0,
            "Put-call parity violated: expected diff {}, got {}",
            parity_diff,
            actual_diff
        );
    }

    #[test]
    fn test_deep_itm_call_approximates_intrinsic() {
        let (market, as_of) = setup_market();

        // Deep ITM call (strike << forward)
        let option = VolatilityIndexOption::builder()
            .id(InstrumentId::new("VIX-CALL-DITM"))
            .notional(Money::new(1_000.0, Currency::USD))
            .strike(10.0) // Way below forward of ~20
            .option_type(OptionType::Call)
            .expiry(Date::from_calendar_date(2025, Month::April, 1).expect("valid date"))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .vol_of_vol_surface_id(CurveId::new("VIX-VOLVOL"))
            .build()
            .expect("valid option");

        let npv = option.npv_raw(&market, as_of).expect("npv");
        let intrinsic = option.intrinsic_value(&market, as_of).expect("intrinsic");

        // Deep ITM option should be close to intrinsic
        assert!(
            (npv - intrinsic).abs() / intrinsic < 0.2,
            "Deep ITM call should be close to intrinsic: NPV={}, intrinsic={}",
            npv,
            intrinsic
        );
    }

    #[test]
    fn test_delta_call_positive() {
        let (market, as_of) = setup_market();

        let call = VolatilityIndexOption::builder()
            .id(InstrumentId::new("VIX-CALL"))
            .notional(Money::new(2_000.0, Currency::USD))
            .strike(20.0)
            .option_type(OptionType::Call)
            .expiry(Date::from_calendar_date(2025, Month::April, 1).expect("valid date"))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .vol_of_vol_surface_id(CurveId::new("VIX-VOLVOL"))
            .build()
            .expect("valid call");

        let delta = call.delta(&market, as_of).expect("delta");
        assert!(delta > 0.0, "Call delta should be positive");
    }

    #[test]
    fn test_delta_put_negative() {
        let (market, as_of) = setup_market();

        let put = VolatilityIndexOption::builder()
            .id(InstrumentId::new("VIX-PUT"))
            .notional(Money::new(2_000.0, Currency::USD))
            .strike(20.0)
            .option_type(OptionType::Put)
            .expiry(Date::from_calendar_date(2025, Month::April, 1).expect("valid date"))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .vol_of_vol_surface_id(CurveId::new("VIX-VOLVOL"))
            .build()
            .expect("valid put");

        let delta = put.delta(&market, as_of).expect("delta");
        assert!(delta < 0.0, "Put delta should be negative");
    }

    #[test]
    fn test_vega_positive() {
        let (market, as_of) = setup_market();

        let option = VolatilityIndexOption::builder()
            .id(InstrumentId::new("VIX-CALL"))
            .notional(Money::new(2_000.0, Currency::USD))
            .strike(20.0)
            .option_type(OptionType::Call)
            .expiry(Date::from_calendar_date(2025, Month::April, 1).expect("valid date"))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .vol_of_vol_surface_id(CurveId::new("VIX-VOLVOL"))
            .build()
            .expect("valid option");

        let vega = option.vega(&market, as_of).expect("vega");
        assert!(
            vega > 0.0,
            "Vega should be positive for both calls and puts"
        );
    }

    #[test]
    fn test_serde_round_trip() {
        let option =
            VolatilityIndexOption::example().expect("VolatilityIndexOption example is valid");
        let json = serde_json::to_string(&option).expect("json serialization");
        let recovered: VolatilityIndexOption =
            serde_json::from_str(&json).expect("json deserialization");
        assert_eq!(option.id, recovered.id);
        assert!((option.strike - recovered.strike).abs() < 1e-10);
    }

    fn base_builder() -> super::VolatilityIndexOptionBuilder {
        VolatilityIndexOption::builder()
            .id(InstrumentId::new("VIX-TEST"))
            .notional(Money::new(10_000.0, Currency::USD))
            .strike(20.0)
            .option_type(OptionType::Call)
            .expiry(date!(2025 - 03 - 19))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .vol_of_vol_surface_id(CurveId::new("VIX-VOLVOL"))
    }

    #[test]
    fn builder_rejects_zero_strike() {
        let result = base_builder().strike(0.0).build();
        assert!(result.is_err(), "zero strike must be rejected");
    }

    #[test]
    fn builder_rejects_negative_strike() {
        let result = base_builder().strike(-1.0).build();
        assert!(result.is_err(), "negative strike must be rejected");
    }

    #[test]
    fn builder_rejects_non_finite_strike() {
        let result = base_builder().strike(f64::NAN).build();
        assert!(result.is_err(), "NaN strike must be rejected");
    }

    #[test]
    fn builder_rejects_settlement_before_expiry() {
        let result = base_builder()
            .expiry(date!(2025 - 03 - 19))
            .settlement_date_opt(Some(date!(2025 - 03 - 18)))
            .build();
        assert!(
            result.is_err(),
            "settlement_date earlier than expiry must be rejected"
        );
    }
}
