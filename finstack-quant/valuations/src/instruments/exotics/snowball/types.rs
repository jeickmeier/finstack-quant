//! Snowball / Inverse Floater structured note instrument definition.

use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::common_impl::validation;
use crate::instruments::rates::hw1f::bermudan_call::BermudanCallProvision;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

/// Snowball note variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SnowballVariant {
    /// Path-dependent snowball: c_i = max(c_{i-1} + fixed - floating, 0).
    Snowball,
    /// Inverse floater: c_i = max(fixed - gearing * floating, 0).
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
/// - **Inverse Floater**: Coupon = fixed_rate - gearing * floating_rate
///   (simpler, not path-dependent, but often combined with callability)
///
/// # References
///
/// - Brigo, D., & Mercurio, F. (2006). *Interest Rate Models*. Chapter 14. `docs/REFERENCES.md#brigo-mercurio-2006-interest-rate-models`
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Snowball {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Snowball or inverse floater variant.
    pub variant: SnowballVariant,
    /// Initial coupon for snowball (c_0); ignored for inverse floater.
    pub initial_coupon: f64,
    /// Fixed rate component as a decimal annual rate (0.05 = 5%).
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub fixed_rate: Decimal,
    /// Multiplier on the floating fixing (1.0 for snowball, variable for inverse floater).
    pub gearing: f64,
    /// Floor on each period coupon (typically 0.0).
    pub coupon_floor: f64,
    /// Optional cap on each period coupon.
    pub coupon_cap: Option<f64>,
    /// Notional amount.
    pub notional: Money,
    /// Accrual start of the first coupon period; also the first in-advance
    /// fixing date.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start_date: Date,
    /// Coupon payment dates, one per period, strictly ascending and after
    /// `start_date`. Period `i` accrues from the previous payment date (or
    /// `start_date`) to `payment_dates[i]` and fixes in advance at its start.
    #[serde(with = "finstack_quant_core::wire::dates")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    pub payment_dates: Vec<Date>,
    /// Rates forward curve that projects the floating index (also the fixing-series key).
    pub forward_curve_id: CurveId,
    /// Contractual tenor of the observed floating index (must match the forward curve tenor).
    pub index_tenor: Tenor,
    /// Discount curve ID.
    pub discount_curve_id: CurveId,
    /// Optional normal-vol surface used to infer HW1F short-rate σ for stress scenarios.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vol_surface_id: Option<CurveId>,
    /// Optional Bermudan call provision.
    pub call_provision: Option<BermudanCallProvision>,
    /// Day count convention.
    pub day_count: DayCount,
    /// Instrument-owned pricing inputs.
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes.
    pub attributes: Attributes,
}

impl Snowball {
    /// Validate the snowball parameters.
    ///
    /// Checks:
    /// - At least one payment date
    /// - `start_date` and the payment dates are strictly ascending
    /// - Fixed rate converts to `f64`
    /// - Gearing is positive and finite
    /// - Floor is non-negative
    /// - Cap (if set) is greater than floor
    /// - Initial coupon is non-negative for snowball variant
    /// - Callable provision validates (if present)
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        validation::require_with(!self.payment_dates.is_empty(), || {
            "Snowball requires at least one payment date".to_string()
        })?;

        validation::validate_sorted_strict(
            &self.period_boundaries(),
            "Snowball start_date followed by payment_dates",
        )?;

        finstack_quant_core::decimal::decimal_to_f64(self.fixed_rate)?;

        validation::require_with(self.gearing > 0.0 && self.gearing.is_finite(), || {
            format!(
                "Snowball gearing ({}) must be positive and finite",
                self.gearing
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

        if let Some(ref call) = self.call_provision {
            call.validate()?;
        }

        Ok(())
    }

    /// Create a canonical example snowball for testing.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::currency::Currency;

        let start_date = time::macros::date!(2026 - 06 - 30);
        let payment_dates = vec![
            time::macros::date!(2026 - 12 - 31),
            time::macros::date!(2027 - 06 - 30),
            time::macros::date!(2027 - 12 - 31),
            time::macros::date!(2028 - 06 - 30),
            time::macros::date!(2028 - 12 - 31),
        ];

        Ok(Snowball {
            id: InstrumentId::new("SNOWBALL-USD-3Y"),
            variant: SnowballVariant::Snowball,
            initial_coupon: 0.03,
            fixed_rate: Decimal::new(5, 2),
            gearing: 1.0,
            coupon_floor: 0.0,
            coupon_cap: None,
            notional: Money::from((1_000_000_i64, Currency::USD)),
            start_date,
            payment_dates,
            forward_curve_id: CurveId::new("USD-SOFR-6M"),
            index_tenor: Tenor::semi_annual(),
            discount_curve_id: CurveId::new("USD-OIS"),
            vol_surface_id: Some(CurveId::new("USD-SOFR-HW-VOL")),
            call_provision: None,
            day_count: DayCount::Act360,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        })
    }

    /// Create a canonical example inverse floater for testing.
    pub fn example_inverse_floater() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::currency::Currency;

        let start_date = time::macros::date!(2026 - 03 - 31);
        let payment_dates = vec![
            time::macros::date!(2026 - 06 - 30),
            time::macros::date!(2026 - 09 - 30),
            time::macros::date!(2026 - 12 - 31),
        ];

        Ok(Snowball {
            id: InstrumentId::new("INV-FLOATER-USD-1Y"),
            variant: SnowballVariant::InverseFloater,
            initial_coupon: 0.0, // ignored for inverse floater
            fixed_rate: Decimal::new(8, 2),
            gearing: 1.5,
            coupon_floor: 0.0,
            coupon_cap: Some(0.10),
            notional: Money::from((500_000_i64, Currency::USD)),
            start_date,
            payment_dates,
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            index_tenor: Tenor::quarterly(),
            discount_curve_id: CurveId::new("USD-OIS"),
            vol_surface_id: Some(CurveId::new("USD-SOFR-HW-VOL")),
            call_provision: None,
            day_count: DayCount::Act360,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        })
    }

    /// Coupon period boundaries: `start_date` followed by every payment date.
    ///
    /// Period `i` accrues over `[boundaries[i], boundaries[i + 1]]`, fixes in
    /// advance at `boundaries[i]` and pays at `boundaries[i + 1]`.
    pub(crate) fn period_boundaries(&self) -> Vec<Date> {
        std::iter::once(self.start_date)
            .chain(self.payment_dates.iter().copied())
            .collect()
    }

    /// Compute the coupon for a given period based on the variant.
    ///
    /// For snowball: c_i = max(prev_coupon + fixed_rate - floating, floor)
    /// For inverse floater: c_i = max(fixed_rate - gearing * floating, floor)
    ///
    /// Applies optional cap after floor.
    ///
    /// # Arguments
    ///
    /// * `floating_rate` - Period floating rate in decimal (e.g. 0.03 for 3%) observed
    ///   for this coupon period.
    /// * `prev_coupon` - Previous period's coupon in decimal; used as `c_{i-1}` for the
    ///   snowball variant and ignored for inverse floater.
    pub fn compute_coupon(&self, floating_rate: f64, prev_coupon: f64) -> f64 {
        // `validate` guarantees the conversion; a rate `Decimal` always fits in `f64`.
        let fixed_rate = self.fixed_rate.to_f64().unwrap_or(f64::NAN);
        let raw = match self.variant {
            SnowballVariant::Snowball => prev_coupon + fixed_rate - floating_rate,
            SnowballVariant::InverseFloater => fixed_rate - self.gearing * floating_rate,
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

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        Snowball::validate(self)
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        match self.variant {
            // Snowball is path-dependent, needs MC.
            // An inverse-floater coupon `max(fixed − gearing·float, floor)`
            // (optionally capped) is an option on the floating rate: the floor
            // is a floorlet, the cap a caplet. The `Discounting` pricer applies
            // `compute_coupon` to a single deterministic forward, so it captures
            // only intrinsic value and drops the floor/cap time value (and
            // geared convexity). Default to the HW1F MC pricer, which prices
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
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        deps.add_forward_curve(self.forward_curve_id.clone());
        if let Some(vol_surface_id) = &self.vol_surface_id {
            deps.add_volatility_dependency(
                crate::instruments::common_impl::dependencies::VolatilityDependency::new(
                    vol_surface_id.clone(),
                    None,
                    None,
                ),
            );
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
        Some(self.start_date)
    }

    crate::impl_focused_pricing_overrides!();
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
        let s = Snowball::example().expect("example");
        assert!(s.validate().is_ok());
    }

    #[test]
    // schema-rejection-test
    fn retired_callable_key_is_rejected() {
        let mut json = serde_json::to_value(Snowball::example().expect("example")).expect("json");
        let obj = json.as_object_mut().expect("object");
        let provision = obj.remove("call_provision").expect("call_provision");
        obj.insert("callable".to_string(), provision);
        let err = serde_json::from_value::<Snowball>(json).expect_err("callable is retired");
        assert!(err.to_string().contains("callable"), "{err}");
    }

    #[test]
    fn example_inverse_floater_validates() {
        let s = Snowball::example_inverse_floater().expect("example");
        assert!(s.validate().is_ok());
    }

    #[test]
    fn snowball_coupon_accumulation() {
        let s = Snowball::example().expect("example");
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
        let inv = Snowball::example_inverse_floater().expect("example");
        assert_eq!(inv.coupon_floor, 0.0);
        assert!(inv.coupon_cap.is_some());
        assert_eq!(inv.default_model(), ModelKey::MonteCarloHullWhite1F);
    }

    #[test]
    fn snowball_coupon_floors_at_zero() {
        let s = Snowball::example().expect("example");
        // c_prev = 0.01, fixed = 0.05, floating = 0.20
        // raw = 0.01 + 0.05 - 0.20 = -0.14 => floor at 0
        let c = s.compute_coupon(0.20, 0.01);
        assert!((c).abs() < 1e-12);
    }

    #[test]
    fn inverse_floater_coupon() {
        let s = Snowball::example_inverse_floater().expect("example");
        // fixed = 0.08, gearing = 1.5, floating = 0.03
        // c = max(0.08 - 1.5 * 0.03, 0) = max(0.035, 0) = 0.035
        let c = s.compute_coupon(0.03, 0.0);
        assert!((c - 0.035).abs() < 1e-12);
    }

    #[test]
    fn inverse_floater_coupon_with_cap() {
        let s = Snowball::example_inverse_floater().expect("example");
        // fixed = 0.08, gearing = 1.5, floating = 0.0
        // c = max(0.08 - 0.0, 0) = 0.08, but cap = 0.10 => 0.08
        let c = s.compute_coupon(0.0, 0.0);
        assert!((c - 0.08).abs() < 1e-12);
    }

    #[test]
    fn inverse_floater_floors_at_zero() {
        let s = Snowball::example_inverse_floater().expect("example");
        // fixed = 0.08, gearing = 1.5, floating = 0.10
        // c = max(0.08 - 0.15, 0) = max(-0.07, 0) = 0
        let c = s.compute_coupon(0.10, 0.0);
        assert!((c).abs() < 1e-12);
    }

    #[test]
    fn snowball_negative_initial_coupon_fails() {
        let mut s = Snowball::example().expect("example");
        s.initial_coupon = -0.01;
        assert!(s.validate().is_err());
    }

    #[test]
    fn snowball_cap_below_floor_fails() {
        let mut s = Snowball::example().expect("example");
        s.coupon_cap = Some(0.0);
        s.coupon_floor = 0.01;
        assert!(s.validate().is_err());
    }

    #[test]
    fn snowball_instrument_trait() {
        use crate::instruments::common_impl::traits::Instrument;
        let s = Snowball::example().expect("example");
        assert_eq!(s.id(), "SNOWBALL-USD-3Y");
        assert_eq!(s.key(), crate::pricer::InstrumentType::Snowball);
    }

    #[test]
    fn snowball_serde_roundtrip() {
        let s = Snowball::example().expect("example");
        let json = serde_json::to_string(&s).expect("serialize");
        let deser: Snowball = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.id, s.id);
        assert_eq!(deser.variant, s.variant);
    }

    #[test]
    fn value_requires_explicit_pricer() {
        use crate::instruments::common_impl::traits::Instrument;
        use finstack_quant_core::market_data::context::MarketContext;
        use time::macros::date;

        let snowball = Snowball::example().expect("example");
        let market = MarketContext::default();
        let as_of = date!(2025 - 01 - 01);
        let err = snowball
            .value(&market, as_of)
            .expect_err("value must require Monte Carlo pricer");
        let msg = err.to_string();
        assert!(
            msg.contains("Monte Carlo"),
            "expected Monte Carlo pricer in error message, got: {msg}"
        );
        assert!(
            msg.contains("inverse floater"),
            "expected inverse floater explicit path in error message, got: {msg}"
        );
    }
}
