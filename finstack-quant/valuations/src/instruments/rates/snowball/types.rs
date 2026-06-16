//! Snowball / Inverse Floater structured note instrument definition.

use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::common_impl::validation;
use crate::instruments::rates::exotics_shared::bermudan_call::BermudanCallProvision;
use crate::instruments::PricingOverrides;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};

/// Snowball note variant.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SnowballVariant {
    /// Path-dependent snowball: c_i = max(c_{i-1} + fixed - floating, 0).
    Snowball,
    /// Inverse floater: c_i = max(fixed - leverage * floating, 0).
    InverseFloater,
}

impl std::fmt::Display for SnowballVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SnowballVariant::Snowball => write!(f, "snowball"),
            SnowballVariant::InverseFloater => write!(f, "inverse_floater"),
        }
    }
}

/// Snowball structured note.
///
/// The coupon in each period depends on the previous period's coupon,
/// creating a path-dependent "snowball" accumulation:
///
/// ```text
/// c_i = max(c_{i-1} + fixed_rate - L_i, 0)
/// ```
///
/// where L_i is the floating rate and c_0 = initial_coupon.
///
/// If the floating rate stays low, coupons ratchet up over time.
/// If rates spike, the coupon floors at zero and must rebuild.
///
/// # Variants
///
/// - **Snowball**: Coupon depends on previous coupon (path-dependent)
/// - **Inverse Floater**: Coupon = fixed_rate - leverage * floating_rate
///   (simpler, not path-dependent, but often combined with callability)
///
/// # References
///
/// - Brigo, D., & Mercurio, F. (2006). *Interest Rate Models*. Chapter 14.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Snowball {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Snowball or inverse floater variant.
    pub variant: SnowballVariant,
    /// Initial coupon for snowball (c_0); ignored for inverse floater.
    pub initial_coupon: f64,
    /// Fixed rate component.
    pub fixed_rate: f64,
    /// Leverage multiplier on floating rate (1.0 for snowball, variable for inverse floater).
    pub leverage: f64,
    /// Floor on each period coupon (typically 0.0).
    pub coupon_floor: f64,
    /// Optional cap on each period coupon.
    pub coupon_cap: Option<f64>,
    /// Notional amount.
    pub notional: Money,
    /// Coupon payment dates (must be sorted ascending).
    #[schemars(with = "Vec<String>")]
    pub coupon_dates: Vec<Date>,
    /// Floating rate index identifier.
    pub floating_index_id: CurveId,
    /// Floating rate tenor.
    pub floating_tenor: Tenor,
    /// Discount curve ID.
    pub discount_curve_id: CurveId,
    /// Optional normal-vol surface used to infer HW1F short-rate σ for stress scenarios.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vol_surface_id: Option<CurveId>,
    /// Optional Bermudan call provision.
    pub callable: Option<BermudanCallProvision>,
    /// Day count convention.
    pub day_count: DayCount,
    /// Pricing overrides.
    #[serde(default)]
    pub pricing_overrides: PricingOverrides,
    /// Attributes.
    pub attributes: Attributes,
}

impl Snowball {
    /// Validate the snowball parameters.
    ///
    /// Checks:
    /// - At least two coupon dates
    /// - Coupon dates are sorted ascending
    /// - Fixed rate is finite
    /// - Leverage is positive and finite
    /// - Floor is non-negative
    /// - Cap (if set) is greater than floor
    /// - Initial coupon is non-negative for snowball variant
    /// - Callable provision validates (if present)
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        validation::require_with(self.coupon_dates.len() >= 2, || {
            "Snowball requires at least two coupon dates".to_string()
        })?;

        validation::validate_sorted_strict(&self.coupon_dates, "Snowball coupon_dates")?;

        validation::require_with(self.fixed_rate.is_finite(), || {
            format!("Snowball fixed_rate ({}) must be finite", self.fixed_rate)
        })?;

        validation::require_with(self.leverage > 0.0 && self.leverage.is_finite(), || {
            format!(
                "Snowball leverage ({}) must be positive and finite",
                self.leverage
            )
        })?;

        validation::require_with(
            self.coupon_floor >= 0.0 && self.coupon_floor.is_finite(),
            || {
                format!(
                    "Snowball coupon_floor ({}) must be non-negative and finite",
                    self.coupon_floor
                )
            },
        )?;

        if let Some(cap) = self.coupon_cap {
            validation::require_with(cap > self.coupon_floor, || {
                format!(
                    "Snowball coupon_cap ({}) must be greater than coupon_floor ({})",
                    cap, self.coupon_floor
                )
            })?;
        }

        if self.variant == SnowballVariant::Snowball {
            validation::require_with(self.initial_coupon >= 0.0, || {
                format!(
                    "Snowball initial_coupon ({}) must be non-negative",
                    self.initial_coupon
                )
            })?;
        }

        if let Some(ref call) = self.callable {
            call.validate()?;
        }

        Ok(())
    }

    /// Create a canonical example snowball for testing.
    #[allow(clippy::expect_used)]
    pub fn example_snowball() -> Self {
        use finstack_quant_core::currency::Currency;
        use time::Month;

        let coupon_dates = vec![
            Date::from_calendar_date(2026, Month::June, 30).expect("valid"),
            Date::from_calendar_date(2026, Month::December, 31).expect("valid"),
            Date::from_calendar_date(2027, Month::June, 30).expect("valid"),
            Date::from_calendar_date(2027, Month::December, 31).expect("valid"),
            Date::from_calendar_date(2028, Month::June, 30).expect("valid"),
            Date::from_calendar_date(2028, Month::December, 31).expect("valid"),
        ];

        Snowball {
            id: InstrumentId::new("SNOWBALL-USD-3Y"),
            variant: SnowballVariant::Snowball,
            initial_coupon: 0.03,
            fixed_rate: 0.05,
            leverage: 1.0,
            coupon_floor: 0.0,
            coupon_cap: None,
            notional: Money::new(1_000_000.0, Currency::USD),
            coupon_dates,
            floating_index_id: CurveId::new("USD-SOFR-6M"),
            floating_tenor: Tenor::semi_annual(),
            discount_curve_id: CurveId::new("USD-OIS"),
            vol_surface_id: Some(CurveId::new("USD-SOFR-HW-VOL")),
            callable: None,
            day_count: DayCount::Act360,
            pricing_overrides: PricingOverrides::default(),
            attributes: Attributes::new(),
        }
    }

    /// Create a canonical example inverse floater for testing.
    #[allow(clippy::expect_used)]
    pub fn example_inverse_floater() -> Self {
        use finstack_quant_core::currency::Currency;
        use time::Month;

        let coupon_dates = vec![
            Date::from_calendar_date(2026, Month::March, 31).expect("valid"),
            Date::from_calendar_date(2026, Month::June, 30).expect("valid"),
            Date::from_calendar_date(2026, Month::September, 30).expect("valid"),
            Date::from_calendar_date(2026, Month::December, 31).expect("valid"),
        ];

        Snowball {
            id: InstrumentId::new("INV-FLOATER-USD-1Y"),
            variant: SnowballVariant::InverseFloater,
            initial_coupon: 0.0, // ignored for inverse floater
            fixed_rate: 0.08,
            leverage: 1.5,
            coupon_floor: 0.0,
            coupon_cap: Some(0.10),
            notional: Money::new(500_000.0, Currency::USD),
            coupon_dates,
            floating_index_id: CurveId::new("USD-SOFR-3M"),
            floating_tenor: Tenor::quarterly(),
            discount_curve_id: CurveId::new("USD-OIS"),
            vol_surface_id: Some(CurveId::new("USD-SOFR-HW-VOL")),
            callable: None,
            day_count: DayCount::Act360,
            pricing_overrides: PricingOverrides::default(),
            attributes: Attributes::new(),
        }
    }

    /// Compute the coupon for a given period based on the variant.
    ///
    /// For snowball: c_i = max(prev_coupon + fixed_rate - floating, floor)
    /// For inverse floater: c_i = max(fixed_rate - leverage * floating, floor)
    ///
    /// Applies optional cap after floor.
    pub fn compute_coupon(&self, floating_rate: f64, prev_coupon: f64) -> f64 {
        let raw = match self.variant {
            SnowballVariant::Snowball => prev_coupon + self.fixed_rate - floating_rate,
            SnowballVariant::InverseFloater => self.fixed_rate - self.leverage * floating_rate,
        };

        let floored = raw.max(self.coupon_floor);

        match self.coupon_cap {
            Some(cap) => floored.min(cap),
            None => floored,
        }
    }
}

impl crate::instruments::common_impl::traits::Instrument for Snowball {
    impl_instrument_base!(crate::pricer::InstrumentType::Snowball);

    fn default_model(&self) -> crate::pricer::ModelKey {
        match self.variant {
            // Snowball is path-dependent, needs MC.
            // An inverse-floater coupon `max(fixed − leverage·float, floor)`
            // (optionally capped) is an option on the floating rate: the floor
            // is a floorlet, the cap a caplet. The `Discounting` pricer applies
            // `compute_coupon` to a single deterministic forward, so it captures
            // only intrinsic value and drops the floor/cap time value (and
            // leveraged convexity). Default to the HW1F MC pricer, which prices
            // the embedded optionality correctly; `Discounting` remains an
            // explicit, fast, intrinsic-only approximation for callers who opt in.
            SnowballVariant::Snowball | SnowballVariant::InverseFloater => {
                crate::pricer::ModelKey::MonteCarloHullWhite1F
            }
        }
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps =
            crate::instruments::common_impl::dependencies::MarketDependencies::from_curve_dependencies(
                self,
            )?;
        if let Some(surface_id) = &self.vol_surface_id {
            deps.add_vol_surface_id(surface_id.as_str());
        }
        Ok(deps)
    }

    fn base_value(
        &self,
        _market: &finstack_quant_core::market_data::context::MarketContext,
        _as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        self.validate()?;
        Err(finstack_quant_core::Error::Validation(
            "Snowball/InverseFloater pricing requires Monte Carlo simulation \
             for the snowball variant, or forward curve projection for inverse floater."
                .to_string(),
        ))
    }

    fn effective_start_date(&self) -> Option<Date> {
        self.coupon_dates.first().copied()
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

impl crate::instruments::common_impl::traits::CurveDependencies for Snowball {
    fn curve_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::InstrumentCurves>
    {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .forward(self.floating_index_id.clone())
            .build()
    }
}

crate::impl_empty_cashflow_provider!(
    Snowball,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_snowball_validates() {
        let s = Snowball::example_snowball();
        assert!(s.validate().is_ok());
    }

    #[test]
    fn example_inverse_floater_validates() {
        let s = Snowball::example_inverse_floater();
        assert!(s.validate().is_ok());
    }

    #[test]
    fn snowball_coupon_accumulation() {
        let s = Snowball::example_snowball();
        // c_0 = 0.03, fixed = 0.05, floating = 0.02
        // c_1 = max(0.03 + 0.05 - 0.02, 0) = 0.06
        let c1 = s.compute_coupon(0.02, 0.03);
        assert!((c1 - 0.06).abs() < 1e-12);

        // c_2 = max(0.06 + 0.05 - 0.03, 0) = 0.08
        let c2 = s.compute_coupon(0.03, c1);
        assert!((c2 - 0.08).abs() < 1e-12);

        // c_3 = max(0.08 + 0.05 - 0.06, 0) = 0.07
        let c3 = s.compute_coupon(0.06, c2);
        assert!((c3 - 0.07).abs() < 1e-12);
    }

    #[test]
    fn inverse_floater_defaults_to_mc_to_capture_floor_optionality() {
        use crate::instruments::common_impl::traits::Instrument;
        use crate::pricer::ModelKey;
        // A floored/capped inverse floater carries floorlet/caplet optionality,
        // so the default model must be the HW1F MC pricer (which prices the
        // floor's time value), not the intrinsic-only Discounting pricer.
        let inv = Snowball::example_inverse_floater();
        assert_eq!(inv.coupon_floor, 0.0);
        assert!(inv.coupon_cap.is_some());
        assert_eq!(inv.default_model(), ModelKey::MonteCarloHullWhite1F);
    }

    #[test]
    fn snowball_coupon_floors_at_zero() {
        let s = Snowball::example_snowball();
        // c_prev = 0.01, fixed = 0.05, floating = 0.20
        // raw = 0.01 + 0.05 - 0.20 = -0.14 => floor at 0
        let c = s.compute_coupon(0.20, 0.01);
        assert!((c).abs() < 1e-12);
    }

    #[test]
    fn inverse_floater_coupon() {
        let s = Snowball::example_inverse_floater();
        // fixed = 0.08, leverage = 1.5, floating = 0.03
        // c = max(0.08 - 1.5 * 0.03, 0) = max(0.035, 0) = 0.035
        let c = s.compute_coupon(0.03, 0.0);
        assert!((c - 0.035).abs() < 1e-12);
    }

    #[test]
    fn inverse_floater_coupon_with_cap() {
        let s = Snowball::example_inverse_floater();
        // fixed = 0.08, leverage = 1.5, floating = 0.0
        // c = max(0.08 - 0.0, 0) = 0.08, but cap = 0.10 => 0.08
        let c = s.compute_coupon(0.0, 0.0);
        assert!((c - 0.08).abs() < 1e-12);
    }

    #[test]
    fn inverse_floater_floors_at_zero() {
        let s = Snowball::example_inverse_floater();
        // fixed = 0.08, leverage = 1.5, floating = 0.10
        // c = max(0.08 - 0.15, 0) = max(-0.07, 0) = 0
        let c = s.compute_coupon(0.10, 0.0);
        assert!((c).abs() < 1e-12);
    }

    #[test]
    fn snowball_negative_initial_coupon_fails() {
        let mut s = Snowball::example_snowball();
        s.initial_coupon = -0.01;
        assert!(s.validate().is_err());
    }

    #[test]
    fn snowball_cap_below_floor_fails() {
        let mut s = Snowball::example_snowball();
        s.coupon_cap = Some(0.0);
        s.coupon_floor = 0.01;
        assert!(s.validate().is_err());
    }

    #[test]
    fn snowball_instrument_trait() {
        use crate::instruments::common_impl::traits::Instrument;
        let s = Snowball::example_snowball();
        assert_eq!(s.id(), "SNOWBALL-USD-3Y");
        assert_eq!(s.key(), crate::pricer::InstrumentType::Snowball);
    }

    #[test]
    fn snowball_serde_roundtrip() {
        let s = Snowball::example_snowball();
        let json = serde_json::to_string(&s).expect("serialize");
        let deser: Snowball = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.id, s.id);
        assert_eq!(deser.variant, s.variant);
    }
}
