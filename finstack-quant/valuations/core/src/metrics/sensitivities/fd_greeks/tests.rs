use super::*;

use crate::instruments::Instrument;
use crate::instruments::pricing_overrides::MetricPricingOverrides;
use crate::instruments::{MarketDependencies, VolatilityDependency};
use crate::metrics::{MetricCalculator, MetricContext, MetricId, MetricRegistry};
use crate::pricer::InstrumentType;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::PriceId;
use std::sync::Arc;
use time::macros::date;

#[derive(Clone)]
enum MockPayoff {
    QuadraticSpot,
    LinearSpot(f64),
    VolLinear {
        surface_terms: Vec<(String, f64)>,
        multiply_by_spot: bool,
    },
}

#[derive(Clone)]
struct MockFdInstrument {
    id: String,
    expiry: Date,
    spot_id: PriceId,
    payoff: MockPayoff,
    overrides: MetricPricingOverrides,
    instrument_overrides: crate::instruments::pricing_overrides::InstrumentPricingOverrides,
    attributes: Attributes,
}

impl MockFdInstrument {
    fn new_with(id: &str, expiry: Date, spot_id: &str, payoff: MockPayoff) -> Self {
        Self {
            id: id.to_string(),
            expiry,
            spot_id: spot_id.into(),
            payoff,
            overrides: MetricPricingOverrides::default(),
            instrument_overrides: Default::default(),
            attributes: Attributes::new(),
        }
    }

    fn quadratic(id: &str, expiry: Date, spot_id: &str) -> Self {
        Self::new_with(id, expiry, spot_id, MockPayoff::QuadraticSpot)
    }

    fn linear_spot(id: &str, expiry: Date, spot_id: &str, slope: f64) -> Self {
        Self::new_with(id, expiry, spot_id, MockPayoff::LinearSpot(slope))
    }

    fn vol_linear(
        id: &str,
        expiry: Date,
        spot_id: &str,
        surface_terms: Vec<(&str, f64)>,
        multiply_by_spot: bool,
    ) -> Self {
        Self::new_with(
            id,
            expiry,
            spot_id,
            MockPayoff::VolLinear {
                surface_terms: surface_terms
                    .into_iter()
                    .map(|(vol_surface_id, slope)| (vol_surface_id.to_string(), slope))
                    .collect(),
                multiply_by_spot,
            },
        )
    }

    fn raw_pv(&self, market: &MarketContext, as_of: Date) -> finstack_quant_core::Result<f64> {
        match &self.payoff {
            MockPayoff::QuadraticSpot => {
                if as_of >= self.expiry {
                    return Ok(0.0);
                }
                let spot = crate::metrics::finite_difference::scalar_numeric_value(
                    market.get_price(self.spot_id.as_str())?,
                );
                Ok(spot * spot)
            }
            MockPayoff::LinearSpot(slope) => {
                let spot = crate::metrics::finite_difference::scalar_numeric_value(
                    market.get_price(self.spot_id.as_str())?,
                );
                Ok(spot * slope)
            }
            MockPayoff::VolLinear {
                surface_terms,
                multiply_by_spot,
            } => {
                let mut value = 0.0;
                for (vol_surface_id, slope) in surface_terms {
                    let surface = market.get_surface(vol_surface_id)?;
                    // Sample the surface at its first grid point: an exact grid
                    // hit so `value_checked` returns the stored vol verbatim.
                    let expiry = surface.expiries().first().copied().ok_or_else(|| {
                        finstack_quant_core::Error::Validation("empty expiries".into())
                    })?;
                    let strike = surface.secondary_values().first().copied().ok_or_else(|| {
                        finstack_quant_core::Error::Validation("empty strikes".into())
                    })?;
                    value += slope
                        * finstack_quant_models::volatility::get_surface_vol(
                            &surface, expiry, strike,
                        )?;
                }
                if *multiply_by_spot {
                    let spot = crate::metrics::finite_difference::scalar_numeric_value(
                        market.get_price(self.spot_id.as_str())?,
                    );
                    value *= spot;
                }
                Ok(value)
            }
        }
    }
}

crate::impl_empty_cashflow_provider!(
    MockFdInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl crate::instruments::Instrument for MockFdInstrument {
    fn includes_valuation_date_cashflows(&self) -> bool {
        true
    }
    fn expiry(&self) -> Option<Date> {
        Some(self.expiry)
    }
    fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
        let mut dependencies = MarketDependencies::new();
        dependencies.add_market_scalar_id(self.spot_id.as_str());
        if let MockPayoff::VolLinear { surface_terms, .. } = &self.payoff {
            for (vol_surface_id, _) in surface_terms {
                dependencies.add_volatility_dependency(VolatilityDependency::new(
                    vol_surface_id.clone(),
                    Some(self.spot_id.clone()),
                    None,
                ));
            }
        }
        Ok(dependencies)
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> InstrumentType {
        InstrumentType::Equity
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn clone_box(&self) -> Box<dyn crate::instruments::Instrument> {
        Box::new(self.clone())
    }

    fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    fn attributes_mut(&mut self) -> &mut Attributes {
        &mut self.attributes
    }

    fn get_instrument_pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::InstrumentPricingOverrides> {
        Some(&self.instrument_overrides)
    }

    fn get_instrument_pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::pricing_overrides::InstrumentPricingOverrides> {
        Some(&mut self.instrument_overrides)
    }

    fn get_metric_pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::MetricPricingOverrides> {
        Some(&self.overrides)
    }

    fn get_metric_pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::pricing_overrides::MetricPricingOverrides> {
        Some(&mut self.overrides)
    }

    fn base_value(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        Ok(Money::new(self.raw_pv(market, as_of)?, Currency::USD).expect("valid money fixture"))
    }

    fn base_value_raw(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.raw_pv(market, as_of)
    }

    fn price_with_metrics(
        &self,
        market: &MarketContext,
        as_of: Date,
        metrics: &[MetricId],
        options: crate::instruments::PricingOptions,
    ) -> std::result::Result<crate::results::ValuationResult, crate::pricer::PricingError> {
        let base_value = self.value(market, as_of)?;
        Ok(crate::instruments::lifecycle::build_with_metrics_dyn(
            Arc::from(self.clone_box()),
            Arc::new(market.clone()),
            as_of,
            base_value,
            metrics,
            crate::instruments::lifecycle::MetricBuildOptions {
                pricing: options,
                ..crate::instruments::lifecycle::MetricBuildOptions::default()
            },
        )?)
    }
}

fn registry_for_test() -> MetricRegistry {
    let mut registry = MetricRegistry::new();
    registry
        .register_metric(
            MetricId::Delta,
            Arc::new(GenericFdDelta),
            &[InstrumentType::Equity],
        )
        .expect("unique test metric registration");
    registry
        .register_metric(
            MetricId::Gamma,
            Arc::new(GenericFdGamma),
            &[InstrumentType::Equity],
        )
        .expect("unique test metric registration");
    registry
}

fn market_with_spot(spot_id: &str, price: f64) -> MarketContext {
    MarketContext::new().insert_price(
        spot_id,
        MarketScalar::price(Money::new(price, Currency::USD).expect("valid money fixture")),
    )
}

#[test]
fn seed_scenario_lives_in_model_config() {
    let instrument = MockFdInstrument::quadratic("FD-SEED", date!(2026 - 01 - 01), "SPOT");

    let seeded = clone_with_crn_seed(&instrument).expect("instrument overrides");

    assert!(
        instrument
            .instrument_overrides
            .model_config
            .mc_seed_scenario
            .is_none()
    );
    assert_eq!(
        seeded
            .get_instrument_pricing_overrides()
            .expect("seeded overrides")
            .model_config
            .mc_seed_scenario
            .as_deref(),
        Some(CRN_SEED_SCENARIO)
    );
}

#[test]
fn fd_delta_matches_analytic_for_quadratic_pv() {
    let as_of = date!(2025 - 01 - 01);
    let spot = 100.0;
    let inst = MockFdInstrument::quadratic("FD-TEST", date!(2026 - 01 - 01), "SPOT");
    let market = market_with_spot("SPOT", spot);

    let base_value = inst.value(&market, as_of).expect("base pv");
    let registry = registry_for_test();
    let mut ctx = MetricContext::new(
        Arc::new(inst),
        Arc::new(market),
        as_of,
        base_value,
        MetricContext::default_config(),
    );

    let result = registry
        .compute(&[MetricId::Delta], &mut ctx)
        .expect("delta");
    let delta = *result.get(&MetricId::Delta).expect("delta value");

    // For PV = S^2, dPV/dS = 2S.
    assert!((delta - 2.0 * spot).abs() < 1e-8, "delta mismatch");
}

#[test]
fn fd_gamma_matches_analytic_for_quadratic_pv() {
    let as_of = date!(2025 - 01 - 01);
    let spot = 80.0;
    let inst = MockFdInstrument::quadratic("FD-TEST", date!(2026 - 01 - 01), "SPOT");
    let market = market_with_spot("SPOT", spot);

    let base_value = inst.value(&market, as_of).expect("base pv");
    let registry = registry_for_test();
    let mut ctx = MetricContext::new(
        Arc::new(inst),
        Arc::new(market),
        as_of,
        base_value,
        MetricContext::default_config(),
    );

    let result = registry
        .compute(&[MetricId::Gamma], &mut ctx)
        .expect("gamma");
    let gamma = *result.get(&MetricId::Gamma).expect("gamma value");

    // For PV = S^2, d2PV/dS2 = 2.
    assert!((gamma - 2.0).abs() < 1e-8, "gamma mismatch");
}

#[test]
fn delta_uses_raw_value_when_money_rounds_down() {
    let as_of = date!(2025 - 01 - 01);
    let spot = 100.0;
    let inst = MockFdInstrument::linear_spot("ROUNDING", date!(2026 - 01 - 01), "SENS", 0.00003);
    let market = market_with_spot("SENS", spot);

    // Rounded Money path remains tiny, while the raw path preserves the
    // sensitivity needed for finite differences.
    let base_value = inst.value(&market, as_of).expect("base pv");
    assert!(
        base_value.amount().abs() < 0.01,
        "rounded Money should remain tiny"
    );

    let registry = registry_for_test();
    let mut ctx = MetricContext::new(
        Arc::new(inst),
        Arc::new(market),
        as_of,
        base_value,
        MetricContext::default_config(),
    );

    let result = registry
        .compute(&[MetricId::Delta], &mut ctx)
        .expect("delta");
    let delta = *result.get(&MetricId::Delta).expect("delta value");

    // Raw path preserves sensitivity: raw PV = spot * 0.00003, so delta ≈ 3e-5
    assert!(
        (delta - 0.00003).abs() < 1e-8,
        "delta should reflect raw pv"
    );
}

#[test]
fn fd_greeks_zero_when_expired() {
    let as_of = date!(2027 - 01 - 02);
    let spot = 50.0;
    let inst = MockFdInstrument::quadratic("FD-TEST", date!(2027 - 01 - 01), "SPOT");
    let market = market_with_spot("SPOT", spot);

    let base_value = inst.value(&market, as_of).expect("base pv");
    assert_eq!(base_value.amount(), 0.0, "expired base pv should be zero");

    let registry = registry_for_test();
    let mut ctx = MetricContext::new(
        Arc::new(inst),
        Arc::new(market),
        as_of,
        base_value,
        MetricContext::default_config(),
    );

    let result = registry
        .compute(&[MetricId::Delta, MetricId::Gamma], &mut ctx)
        .expect("greeks");
    assert_eq!(
        *result.get(&MetricId::Delta).expect("delta"),
        0.0,
        "expired delta should be zero"
    );
    assert_eq!(
        *result.get(&MetricId::Gamma).expect("gamma"),
        0.0,
        "expired gamma should be zero"
    );
}

#[test]
fn option_greeks_use_canonical_expiry_before_market_lookups() {
    let expiry = date!(2027 - 01 - 01);
    for as_of in [expiry, date!(2027 - 01 - 02)] {
        let inst = MockFdInstrument::quadratic("FD-EXPIRED", expiry, "MISSING-SPOT");
        let mut ctx = MetricContext::new(
            Arc::new(inst),
            Arc::new(MarketContext::new()),
            as_of,
            Money::from((0_i64, Currency::USD)),
            MetricContext::default_config(),
        );
        assert_eq!(
            GenericFdVanna.calculate(&mut ctx).expect("expired vanna"),
            0.0
        );
        assert_eq!(
            GenericFdVolga.calculate(&mut ctx).expect("expired volga"),
            0.0
        );
    }
}

#[test]
fn delta_errors_on_zero_spot() {
    let as_of = date!(2025 - 01 - 01);
    let inst = MockFdInstrument::quadratic("FD-TEST", date!(2026 - 01 - 01), "SPOT");
    let market = market_with_spot("SPOT", 0.0);

    let base_value = Money::from((0_i64, Currency::USD));
    let registry = registry_for_test();
    let mut ctx = MetricContext::new(
        Arc::new(inst),
        Arc::new(market),
        as_of,
        base_value,
        MetricContext::default_config(),
    );

    let result = registry.compute(&[MetricId::Delta], &mut ctx);
    assert!(result.is_err(), "delta should error on zero spot");
    let err_msg = result.expect_err("already asserted is_err").to_string();
    assert!(
        err_msg.contains("Non-positive") || err_msg.contains("invalid spot"),
        "error should mention non-positive spot: {}",
        err_msg
    );
}

#[test]
fn gamma_errors_on_negative_spot() {
    let as_of = date!(2025 - 01 - 01);
    let inst = MockFdInstrument::quadratic("FD-TEST", date!(2026 - 01 - 01), "SPOT");
    let market = market_with_spot("SPOT", -10.0);

    let base_value = Money::from((0_i64, Currency::USD));
    let registry = registry_for_test();
    let mut ctx = MetricContext::new(
        Arc::new(inst),
        Arc::new(market),
        as_of,
        base_value,
        MetricContext::default_config(),
    );

    let result = registry.compute(&[MetricId::Gamma], &mut ctx);
    assert!(result.is_err(), "gamma should error on negative spot");
}

#[test]
fn spot_bump_applies_minimum_floor_to_applied_relative_bump() {
    // Very small spot should apply a relative bump that matches the floored denominator.
    let tiny_spot = 1e-12;
    let bump_pct = 0.01;
    let bump = super::effective_spot_bump(tiny_spot, bump_pct);

    // The market bump and finite-difference denominator should agree.
    assert_eq!(bump.absolute, super::MIN_ABSOLUTE_BUMP);
    assert_eq!(bump.relative, super::MIN_ABSOLUTE_BUMP / tiny_spot);
    assert!(
        (tiny_spot * bump.relative - bump.absolute).abs() < f64::EPSILON,
        "applied relative bump should reproduce absolute denominator"
    );
}

#[test]
fn validate_spot_rejects_nan() {
    let result = super::validate_spot(f64::NAN, "test");
    assert!(result.is_err(), "NaN spot should be rejected");
}

#[test]
fn validate_spot_rejects_infinity() {
    let result = super::validate_spot(f64::INFINITY, "test");
    assert!(result.is_err(), "infinite spot should be rejected");
}

#[test]
fn validate_spot_accepts_positive_finite() {
    let result = super::validate_spot(100.0, "test");
    assert!(result.is_ok(), "positive finite spot should be accepted");
}

/// Flat vol surface at the given (low) vol level.
fn flat_vol_surface(id: &str, vol: f64) -> finstack_quant_core::market_data::surfaces::VolSurface {
    finstack_quant_core::market_data::surfaces::VolSurface::builder(id)
        .expiries(&[0.5, 1.0])
        .secondary_values(&[90.0, 100.0, 110.0])
        .secondary_axis(finstack_quant_core::market_data::surfaces::VolSurfaceAxis::Strike)
        .quote_type(finstack_quant_core::market_data::surfaces::VolQuoteType::BlackLognormal)
        .interpolation_mode(finstack_quant_core::market_data::surfaces::VolInterpolationMode::Vol)
        .row(&[vol, vol, vol])
        .row(&[vol, vol, vol])
        .build()
        .expect("flat vol surface")
}

#[test]
fn vega_uses_one_sided_difference_when_down_bump_clamps_at_zero() {
    // Surface min vol 0.005 is BELOW the 0.01 default vol bump, so the
    // additive down-bump `σ - h` clamps at 0. A central difference over
    // `2h` would mix a +0.01 up-move with a clamped −0.005 down-move and
    // understate vega; the fix detects the clamp and uses a forward
    // difference `(PV(σ+h) − PV(σ)) / h`.
    let as_of = date!(2025 - 01 - 01);
    let slope = 1_000_000.0; // PV = slope · σ.
    let inst = MockFdInstrument::vol_linear(
        "VEGA-CLAMP",
        date!(2026 - 01 - 01),
        "SPOT",
        vec![("LOWVOL", slope)],
        false,
    );
    let market = market_with_spot("SPOT", 100.0).insert_surface(flat_vol_surface("LOWVOL", 0.005));

    let base_value = inst.value(&market, as_of).expect("base pv");
    let mut registry = MetricRegistry::new();
    registry
        .register_metric(
            MetricId::Vega,
            Arc::new(GenericFdVega),
            &[InstrumentType::Equity],
        )
        .expect("unique test metric registration");
    let mut ctx = MetricContext::new(
        Arc::new(inst),
        Arc::new(market),
        as_of,
        base_value,
        MetricContext::default_config(),
    );

    let result = registry.compute(&[MetricId::Vega], &mut ctx).expect("vega");
    let vega = *result.get(&MetricId::Vega).expect("vega value");

    // PV = slope·σ ⇒ dPV/dσ = slope. Vega is reported per 1 vol point
    // (0.01 absolute vol): expected = slope · 0.01 = 10_000.
    let expected = slope * 0.01;
    assert!(
        (vega - expected).abs() < 1e-6,
        "clamp-aware forward difference must recover the true vega {expected}, got {vega}"
    );
    // The contaminated central difference would have produced
    // slope·0.015 / 2 = 7_500 — assert we are NOT near that wrong value.
    assert!(
        (vega - slope * 0.0075).abs() > 1.0,
        "vega must not equal the contaminated central-difference value"
    );
}

#[test]
fn vega_uses_central_difference_when_no_clamp() {
    // Surface min vol 0.20 is well above the 0.01 bump: standard central
    // difference path, still exact for a PV linear in σ.
    let as_of = date!(2025 - 01 - 01);
    let slope = 500_000.0;
    let inst = MockFdInstrument::vol_linear(
        "VEGA-NOCLAMP",
        date!(2026 - 01 - 01),
        "SPOT",
        vec![("HIVOL", slope)],
        false,
    );
    let market = market_with_spot("SPOT", 100.0).insert_surface(flat_vol_surface("HIVOL", 0.20));

    let base_value = inst.value(&market, as_of).expect("base pv");
    let mut registry = MetricRegistry::new();
    registry
        .register_metric(
            MetricId::Vega,
            Arc::new(GenericFdVega),
            &[InstrumentType::Equity],
        )
        .expect("unique test metric registration");
    let mut ctx = MetricContext::new(
        Arc::new(inst),
        Arc::new(market),
        as_of,
        base_value,
        MetricContext::default_config(),
    );

    let result = registry.compute(&[MetricId::Vega], &mut ctx).expect("vega");
    let vega = *result.get(&MetricId::Vega).expect("vega value");
    let expected = slope * 0.01;
    assert!(
        (vega - expected).abs() < 1e-6,
        "central difference must recover the true vega {expected}, got {vega}"
    );
}

#[test]
fn volatility_greeks_bump_all_present_unique_surfaces() {
    let as_of = date!(2025 - 01 - 01);
    let first_slope = 1_000.0;
    let second_slope = 2_500.0;
    let inst = MockFdInstrument::vol_linear(
        "MULTI-SURFACE",
        date!(2026 - 01 - 01),
        "SPOT",
        vec![("VOL-A", first_slope), ("VOL-B", second_slope)],
        false,
    );
    let market = market_with_spot("SPOT", 100.0)
        .insert_surface(flat_vol_surface("VOL-A", 0.20))
        .insert_surface(flat_vol_surface("VOL-B", 0.30));
    let base_value = inst.value(&market, as_of).expect("base pv");
    let mut registry = MetricRegistry::new();
    registry
        .register_metric(
            MetricId::Vega,
            Arc::new(GenericFdVega),
            &[InstrumentType::Equity],
        )
        .expect("unique test metric registration");
    registry
        .register_metric(
            MetricId::Volga,
            Arc::new(GenericFdVolga),
            &[InstrumentType::Equity],
        )
        .expect("unique test metric registration");
    let mut ctx = MetricContext::new(
        Arc::new(inst),
        Arc::new(market),
        as_of,
        base_value,
        MetricContext::default_config(),
    );

    let result = registry
        .compute(&[MetricId::Vega, MetricId::Volga], &mut ctx)
        .expect("volatility greeks");
    let vega = result[&MetricId::Vega];
    let volga = result[&MetricId::Volga];
    let expected_vega = (first_slope + second_slope) * 0.01;
    assert!((vega - expected_vega).abs() < 1e-9);
    assert!(volga.abs() < 1e-9);
}

#[test]
fn vanna_bumps_all_present_unique_surfaces_with_the_primary_spot() {
    let as_of = date!(2025 - 01 - 01);
    let first_slope = 2.0;
    let second_slope = 3.0;
    let inst = MockFdInstrument::vol_linear(
        "MULTI-SURFACE-VANNA",
        date!(2026 - 01 - 01),
        "SPOT",
        vec![("VOL-A", first_slope), ("VOL-B", second_slope)],
        true,
    );
    let market = market_with_spot("SPOT", 100.0)
        .insert_surface(flat_vol_surface("VOL-A", 0.20))
        .insert_surface(flat_vol_surface("VOL-B", 0.30));
    let base_value = inst.value(&market, as_of).expect("base pv");
    let mut registry = MetricRegistry::new();
    registry
        .register_metric(
            MetricId::Vanna,
            Arc::new(GenericFdVanna),
            &[InstrumentType::Equity],
        )
        .expect("unique test metric registration");
    let mut ctx = MetricContext::new(
        Arc::new(inst),
        Arc::new(market),
        as_of,
        base_value,
        MetricContext::default_config(),
    );

    let result = registry
        .compute(&[MetricId::Vanna], &mut ctx)
        .expect("vanna");
    let vanna = result[&MetricId::Vanna];
    // PV couples spot with Σ slope·σ, so ∂²V/∂S∂σ = Σ slope per decimal
    // vol. Vanna is reported per **vol point** (0.01 absolute vol),
    // consistent with Vega: expected = Σ slope × 0.01.
    let expected = (first_slope + second_slope) * 0.01;
    assert!((vanna - expected).abs() < 1e-9);
}
