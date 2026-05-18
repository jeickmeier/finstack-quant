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
//!   Discrete Barrier Options." *Mathematical Finance*, 7(4), 325-349.
//! - Haug, E. G. (2007). *The Complete Guide to Option Pricing Formulas*, Section 4.17.

use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::PricingOverrides;
use finstack_core::dates::Date;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId, PriceId};
use time::macros::date;

/// Final payoff type for autocallable products.
#[derive(
    Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
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
    Clone,
    Debug,
    finstack_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[builder(validate = Autocallable::validate)]
#[serde(deny_unknown_fields, try_from = "AutocallableUnchecked")]
pub struct Autocallable {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Underlying asset ticker symbol
    pub underlying_ticker: crate::instruments::equity::spot::Ticker,
    /// Observation dates for autocall and coupon checks.
    ///
    /// Barriers are monitored **discretely** at these exact dates only.
    /// The Monte Carlo time grid is constructed to include these dates precisely.
    #[schemars(with = "Vec<String>")]
    pub observation_dates: Vec<Date>,
    /// Explicit terminal expiry date for the structure.
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Autocall barrier levels (as ratios of initial spot, e.g., 1.0 = 100%).
    ///
    /// Each barrier corresponds to the observation date at the same index.
    /// If spot ≥ barrier × initial_spot on the observation date, the product autocalls.
    pub autocall_barriers: Vec<f64>,
    /// Coupon amounts paid if observation barrier is met
    pub coupons: Vec<f64>,
    /// Memory ("Phoenix") coupon feature.
    ///
    /// When `true`, any coupons from earlier observation dates whose barrier
    /// was *not* met are accrued ("remembered") and paid in full on the
    /// observation date that finally triggers the autocall (in addition to
    /// that date's own coupon). When `false` (the default), only the coupon
    /// at the autocall date is paid and earlier missed coupons are lost.
    ///
    /// This flag must match the term sheet: a Phoenix/memory autocallable
    /// priced with `memory_coupons = false` is silently underpriced.
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
    pub day_count: finstack_core::dates::DayCount,
    /// Discount curve ID for present value calculations
    pub discount_curve_id: CurveId,
    /// Spot price identifier for underlying asset
    pub spot_id: PriceId,
    /// Volatility surface ID for option pricing
    pub vol_surface_id: CurveId,
    /// Optional dividend yield curve ID.
    ///
    /// `Some(id)`: lookup MUST succeed (a missing or non-unitless scalar
    /// returns an error). `None`: no implicit default; treated as zero
    /// continuous dividend yield. Set explicitly for index underlyings.
    pub div_yield_id: Option<CurveId>,
    /// Pricing overrides (manual price, yield, spread)
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: PricingOverrides,
    /// Attributes for scenario selection and grouping
    pub attributes: Attributes,
}

/// Mirror of `Autocallable` used by serde to apply `validate()` after
/// deserialization. Not part of the public API.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct AutocallableUnchecked {
    id: InstrumentId,
    underlying_ticker: crate::instruments::equity::spot::Ticker,
    #[schemars(with = "Vec<String>")]
    observation_dates: Vec<Date>,
    #[schemars(with = "String")]
    expiry: Date,
    autocall_barriers: Vec<f64>,
    coupons: Vec<f64>,
    #[serde(default)]
    memory_coupons: bool,
    final_barrier: f64,
    final_payoff_type: FinalPayoffType,
    participation_rate: f64,
    cap_level: f64,
    notional: Money,
    day_count: finstack_core::dates::DayCount,
    discount_curve_id: CurveId,
    spot_id: PriceId,
    vol_surface_id: CurveId,
    #[serde(default)]
    div_yield_id: Option<CurveId>,
    #[serde(default)]
    pricing_overrides: PricingOverrides,
    attributes: Attributes,
}

impl TryFrom<AutocallableUnchecked> for Autocallable {
    type Error = finstack_core::Error;

    fn try_from(value: AutocallableUnchecked) -> std::result::Result<Self, Self::Error> {
        let inst = Self {
            id: value.id,
            underlying_ticker: value.underlying_ticker,
            observation_dates: value.observation_dates,
            expiry: value.expiry,
            autocall_barriers: value.autocall_barriers,
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
            div_yield_id: value.div_yield_id,
            pricing_overrides: value.pricing_overrides,
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
    /// - `autocall_barriers.len() != observation_dates.len()`
    /// - `coupons.len() != observation_dates.len()`
    /// - any barrier is negative or non-finite
    /// - any coupon is non-finite
    /// - `final_barrier`, `participation_rate`, or `cap_level` are non-finite
    /// - `cap_level <= 0`
    /// - `notional.amount()` is not finite
    pub fn validate(&self) -> finstack_core::Result<()> {
        let n = self.observation_dates.len();
        if n == 0 {
            return Err(finstack_core::Error::Validation(
                "Autocallable requires at least one observation date".into(),
            ));
        }
        for window in self.observation_dates.windows(2) {
            if window[0] >= window[1] {
                return Err(finstack_core::Error::Validation(format!(
                    "Autocallable observation_dates must be strictly increasing; got {} >= {}",
                    window[0], window[1]
                )));
            }
        }
        // Safe: observation_dates is non-empty (checked above).
        if let Some(&last_obs) = self.observation_dates.last() {
            if last_obs > self.expiry {
                return Err(finstack_core::Error::Validation(format!(
                    "Autocallable last observation date {} is after expiry {}",
                    last_obs, self.expiry
                )));
            }
        }
        if self.autocall_barriers.len() != n {
            return Err(finstack_core::Error::Validation(format!(
                "Autocallable autocall_barriers.len() ({}) must match observation_dates.len() ({})",
                self.autocall_barriers.len(),
                n
            )));
        }
        if self.coupons.len() != n {
            return Err(finstack_core::Error::Validation(format!(
                "Autocallable coupons.len() ({}) must match observation_dates.len() ({})",
                self.coupons.len(),
                n
            )));
        }
        for (i, b) in self.autocall_barriers.iter().enumerate() {
            if !b.is_finite() || *b < 0.0 {
                return Err(finstack_core::Error::Validation(format!(
                    "Autocallable autocall_barriers[{}] = {} must be finite and non-negative",
                    i, b
                )));
            }
        }
        for (i, c) in self.coupons.iter().enumerate() {
            if !c.is_finite() {
                return Err(finstack_core::Error::Validation(format!(
                    "Autocallable coupons[{}] = {} must be finite",
                    i, c
                )));
            }
        }
        if !self.final_barrier.is_finite() || self.final_barrier < 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "Autocallable final_barrier = {} must be finite and non-negative",
                self.final_barrier
            )));
        }
        if !self.participation_rate.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Autocallable participation_rate = {} must be finite",
                self.participation_rate
            )));
        }
        if !self.cap_level.is_finite() || self.cap_level <= 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "Autocallable cap_level = {} must be finite and positive",
                self.cap_level
            )));
        }
        if !self.notional.amount().is_finite() {
            return Err(finstack_core::Error::Validation(
                "Autocallable notional amount must be finite".into(),
            ));
        }
        Ok(())
    }

    /// Create a canonical example autocallable (quarterly observations, simple barriers/coupons).
    pub fn example() -> finstack_core::Result<Self> {
        use finstack_core::currency::Currency;
        use finstack_core::dates::DayCount;
        let observation_dates = vec![
            date!(2024 - 03 - 29),
            date!(2024 - 06 - 28),
            date!(2024 - 09 - 30),
            date!(2024 - 12 - 31),
        ];
        let autocall_barriers = vec![1.0, 1.0, 1.0, 1.0]; // 100% of initial
        let coupons = vec![0.02, 0.02, 0.02, 0.02]; // 2% per observation if called
        Autocallable::builder()
            .id(InstrumentId::new("AUTO-SPX-QTR"))
            .underlying_ticker("SPX".to_string())
            .observation_dates(observation_dates)
            .expiry(date!(2024 - 12 - 31))
            .autocall_barriers(autocall_barriers)
            .coupons(coupons)
            .final_barrier(0.6) // 60% final KI barrier
            .final_payoff_type(FinalPayoffType::Participation { rate: 1.0 })
            .participation_rate(1.0)
            .cap_level(1.5) // 150% cap
            .notional(Money::new(100_000.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
    }
}

impl crate::instruments::common_impl::traits::Instrument for Autocallable {
    impl_instrument_base!(crate::pricer::InstrumentType::Autocallable);

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::MonteCarloGBM
    }

    fn market_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::dependencies::MarketDependencies>
    {
        crate::instruments::common_impl::dependencies::MarketDependencies::from_curves_and_equity(
            self,
        )
    }

    fn base_value(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<finstack_core::money::Money> {
        use crate::instruments::equity::autocallable::pricer;
        pricer::compute_pv(self, market, as_of)
    }

    fn effective_start_date(&self) -> Option<Date> {
        self.observation_dates.first().copied()
    }

    fn expiry(&self) -> Option<Date> {
        Some(self.expiry)
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
    Autocallable,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod validation_tests {
    use super::*;
    use crate::instruments::Attributes;
    use finstack_core::currency::Currency;
    use finstack_core::dates::DayCount;

    fn base_builder() -> crate::instruments::equity::autocallable::types::AutocallableBuilder {
        Autocallable::builder()
            .id(InstrumentId::new("AUTO-TEST"))
            .underlying_ticker("SPX".to_string())
            .observation_dates(vec![date!(2024 - 06 - 28), date!(2024 - 12 - 31)])
            .expiry(date!(2024 - 12 - 31))
            .autocall_barriers(vec![1.0, 1.0])
            .coupons(vec![0.02, 0.02])
            .final_barrier(0.6)
            .final_payoff_type(FinalPayoffType::Participation { rate: 1.0 })
            .participation_rate(1.0)
            .cap_level(1.5)
            .notional(Money::new(100_000.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(None)
            .pricing_overrides(PricingOverrides::default())
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
}
