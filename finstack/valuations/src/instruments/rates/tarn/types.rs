//! Target Redemption Note (TARN) instrument definition.

use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::common_impl::validation;
use crate::instruments::PricingOverrides;
use finstack_core::dates::{Date, DayCount, Tenor};
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};

/// Target Redemption Note (TARN).
///
/// Pays periodic coupons = max(fixed_rate - floating_rate, floor) until the
/// cumulative coupon reaches a target level, at which point the note redeems
/// at par. Path-dependent due to the knockout on cumulative coupon.
///
/// # Coupon Formula
///
/// ```text
/// c_i = max(fixed_rate - L_i, floor)
/// ```
///
/// where L_i is the floating rate (e.g., SOFR) for period i.
///
/// # Target Knockout
///
/// ```text
/// If sum(c_1, ..., c_i) >= target => redeem at par, stop paying coupons
/// ```
///
/// The final coupon is reduced so the cumulative equals the target exactly.
///
/// # References
///
/// - Brigo, D., & Mercurio, F. (2006). *Interest Rate Models - Theory and
///   Practice* (2nd ed.). Springer. Chapter 14: Exotic Derivatives.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Tarn {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Fixed coupon rate (the "strike" rate).
    pub fixed_rate: f64,
    /// Floor on each period's coupon (typically 0.0).
    pub coupon_floor: f64,
    /// Target cumulative coupon level (triggers early redemption).
    ///
    /// A zero target is allowed and represents redemption at the first
    /// future coupon date without coupon accrual.
    pub target_coupon: f64,
    /// Notional amount.
    pub notional: Money,
    /// Coupon payment dates (must be sorted ascending).
    #[schemars(with = "Vec<String>")]
    pub coupon_dates: Vec<Date>,
    /// Floating rate tenor (e.g., "3M", "6M").
    pub floating_tenor: Tenor,
    /// Floating rate index identifier.
    pub floating_index_id: CurveId,
    /// Discount curve ID for PV calculations.
    pub discount_curve_id: CurveId,
    /// Optional normal-vol surface used to infer HW1F short-rate σ for stress scenarios.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vol_surface_id: Option<CurveId>,
    /// Day count convention for coupon accrual.
    pub day_count: DayCount,
    /// Pricing overrides.
    #[serde(default)]
    pub pricing_overrides: PricingOverrides,
    /// Attributes for scenario selection.
    pub attributes: Attributes,
}

impl Tarn {
    /// Validate the TARN parameters.
    ///
    /// Checks:
    /// - At least two coupon dates (need a period to accrue)
    /// - Coupon dates are sorted ascending
    /// - Fixed rate is finite
    /// - Target coupon is non-negative
    /// - Coupon floor is finite and non-negative by convention
    pub fn validate(&self) -> finstack_core::Result<()> {
        validation::require_with(self.coupon_dates.len() >= 2, || {
            "TARN requires at least two coupon dates".to_string()
        })?;

        validation::validate_sorted_strict(&self.coupon_dates, "TARN coupon_dates")?;

        validation::require_with(self.fixed_rate.is_finite(), || {
            format!("TARN fixed_rate ({}) must be finite", self.fixed_rate)
        })?;

        validation::require_with(
            self.target_coupon >= 0.0 && self.target_coupon.is_finite(),
            || {
                format!(
                    "TARN target_coupon ({}) must be non-negative and finite",
                    self.target_coupon
                )
            },
        )?;

        validation::require_with(
            self.coupon_floor >= 0.0 && self.coupon_floor.is_finite(),
            || {
                format!(
                    "TARN coupon_floor ({}) must be non-negative and finite",
                    self.coupon_floor
                )
            },
        )?;

        Ok(())
    }

    /// Create a canonical example TARN for testing.
    #[allow(clippy::expect_used)]
    pub fn example() -> Self {
        use finstack_core::currency::Currency;
        use time::Month;

        let coupon_dates = vec![
            Date::from_calendar_date(2026, Month::June, 30).expect("valid"),
            Date::from_calendar_date(2026, Month::December, 31).expect("valid"),
            Date::from_calendar_date(2027, Month::June, 30).expect("valid"),
            Date::from_calendar_date(2027, Month::December, 31).expect("valid"),
            Date::from_calendar_date(2028, Month::June, 30).expect("valid"),
            Date::from_calendar_date(2028, Month::December, 31).expect("valid"),
            Date::from_calendar_date(2029, Month::June, 30).expect("valid"),
            Date::from_calendar_date(2029, Month::December, 31).expect("valid"),
            Date::from_calendar_date(2030, Month::June, 30).expect("valid"),
            Date::from_calendar_date(2030, Month::December, 31).expect("valid"),
        ];

        Tarn {
            id: InstrumentId::new("TARN-USD-5Y"),
            fixed_rate: 0.06,
            coupon_floor: 0.0,
            target_coupon: 0.15,
            notional: Money::new(1_000_000.0, Currency::USD),
            coupon_dates,
            floating_tenor: Tenor::semi_annual(),
            floating_index_id: CurveId::new("USD-SOFR-6M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            vol_surface_id: Some(CurveId::new("USD-SOFR-HW-VOL")),
            day_count: DayCount::Act360,
            pricing_overrides: PricingOverrides::default(),
            attributes: Attributes::new(),
        }
    }
}

impl crate::instruments::common_impl::traits::Instrument for Tarn {
    impl_instrument_base!(crate::pricer::InstrumentType::Tarn);

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::MonteCarloHullWhite1F
    }

    fn market_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::dependencies::MarketDependencies>
    {
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
        _market: &finstack_core::market_data::context::MarketContext,
        _as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<finstack_core::money::Money> {
        self.validate()?;
        Err(finstack_core::Error::Validation(
            "TARN pricing requires Monte Carlo simulation. \
             Use price_with_metrics with a MC pricer."
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

impl crate::instruments::common_impl::traits::CurveDependencies for Tarn {
    fn curve_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::traits::InstrumentCurves> {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .forward(self.floating_index_id.clone())
            .build()
    }
}

crate::impl_empty_cashflow_provider!(
    Tarn,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_tarn_validates() {
        let tarn = Tarn::example();
        assert!(tarn.validate().is_ok());
    }

    #[test]
    fn tarn_too_few_coupon_dates() {
        use finstack_core::currency::Currency;
        use time::Month;

        let tarn = Tarn {
            id: InstrumentId::new("TARN-BAD"),
            fixed_rate: 0.05,
            coupon_floor: 0.0,
            target_coupon: 0.10,
            notional: Money::new(100_000.0, Currency::USD),
            coupon_dates: vec![Date::from_calendar_date(2026, Month::June, 30).expect("valid")],
            floating_tenor: Tenor::semi_annual(),
            floating_index_id: CurveId::new("USD-SOFR-6M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            vol_surface_id: Some(CurveId::new("USD-SOFR-HW-VOL")),
            day_count: DayCount::Act360,
            pricing_overrides: PricingOverrides::default(),
            attributes: Attributes::new(),
        };
        assert!(tarn.validate().is_err());
    }

    #[test]
    fn tarn_negative_target_fails() {
        let mut tarn = Tarn::example();
        tarn.target_coupon = -0.01;
        assert!(tarn.validate().is_err());
    }

    #[test]
    fn tarn_negative_floor_fails() {
        let mut tarn = Tarn::example();
        tarn.coupon_floor = -0.01;
        assert!(tarn.validate().is_err());
    }

    #[test]
    fn tarn_instrument_trait() {
        use crate::instruments::common_impl::traits::Instrument;
        let tarn = Tarn::example();
        assert_eq!(tarn.id(), "TARN-USD-5Y");
        assert_eq!(tarn.key(), crate::pricer::InstrumentType::Tarn);
    }

    #[test]
    fn tarn_serde_roundtrip() {
        let tarn = Tarn::example();
        let json = serde_json::to_string(&tarn).expect("serialize");
        let deser: Tarn = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.id, tarn.id);
        assert!((deser.fixed_rate - tarn.fixed_rate).abs() < 1e-12);
        assert!((deser.target_coupon - tarn.target_coupon).abs() < 1e-12);
    }
}
