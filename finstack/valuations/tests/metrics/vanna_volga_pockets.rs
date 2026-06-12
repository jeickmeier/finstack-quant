//! Coverage tests for vanna/volga metric calculators that were previously uncovered.
//!
//! These tests are intentionally small and deterministic:
//! - Exercise the calculators through the standard registry (public API).
//! - Validate results against an explicit finite-difference reference implementation.

use crate::finstack_test_utils::{
    date, equity_option_european_call, flat_discount_with_tenor, flat_vol_surface,
    fx_option_european_call,
};
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCountContext};
use finstack_core::market_data::bumps::{BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use finstack_valuations::instruments::equity::equity_option::EquityOption;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::{standard_registry, MetricContext, MetricId};
use std::sync::Arc;

const SPOT_BUMP_PCT: f64 = 0.01;
const VOL_BUMP_PCT: f64 = 0.01;

fn bump_scalar_price(
    context: &MarketContext,
    price_id: &str,
    bump_pct: f64,
) -> finstack_core::Result<MarketContext> {
    let current = context.get_price(price_id)?;
    let bumped_value = match current {
        MarketScalar::Unitless(v) => MarketScalar::Unitless(v * (1.0 + bump_pct)),
        MarketScalar::Price(m) => {
            MarketScalar::Price(Money::new(m.amount() * (1.0 + bump_pct), m.currency()))
        }
    };
    Ok(context.clone().insert_price(price_id, bumped_value))
}

fn bump_surface_vol_absolute(
    context: &MarketContext,
    vol_surface_id: &str,
    bump_abs: f64,
) -> finstack_core::Result<MarketContext> {
    context.bump([MarketBump::Curve {
        id: CurveId::from(vol_surface_id),
        spec: BumpSpec {
            mode: BumpMode::Additive,
            units: BumpUnits::Fraction,
            value: bump_abs,
            bump_type: BumpType::Parallel,
        },
    }])
}

fn approx_eq(actual: f64, expected: f64, tol: f64) {
    let diff = (actual - expected).abs();
    assert!(
        diff <= tol,
        "expected {expected}, got {actual} (diff {diff} > {tol})"
    );
}

fn equity_market(as_of: Date, spot: f64, vol: f64, rate: f64, div_yield: f64) -> MarketContext {
    // IDs chosen to align with EquityOption::european_call defaults.
    let expiries = [0.25, 0.5, 1.0, 2.0];
    let strikes = [80.0, 90.0, 100.0, 110.0, 120.0];

    MarketContext::new()
        .insert(flat_discount_with_tenor("USD-OIS", as_of, rate, 5.0))
        .insert_surface(flat_vol_surface("EQUITY-VOL", &expiries, &strikes, vol))
        .insert_price("EQUITY-SPOT", MarketScalar::Unitless(spot))
        .insert_price("EQUITY-DIVYIELD", MarketScalar::Unitless(div_yield))
}

fn equity_option(as_of: Date, expiry: Date, strike: f64) -> EquityOption {
    let _ = as_of;
    equity_option_european_call("EQ-VANNA-VOLGA", "SPX", strike, expiry, 100.0)
        .expect("equity option should build for vanna/volga tests")
}

fn equity_delta_fd(
    opt: &EquityOption,
    curves: &MarketContext,
    as_of: Date,
    spot_bump_pct: f64,
) -> finstack_core::Result<f64> {
    // Central delta via bump-and-reprice.
    let curves_up = bump_scalar_price(curves, &opt.spot_id, spot_bump_pct)?;
    let curves_dn = bump_scalar_price(curves, &opt.spot_id, -spot_bump_pct)?;
    let pv_up = opt.value(&curves_up, as_of)?.amount();
    let pv_dn = opt.value(&curves_dn, as_of)?.amount();

    // Convert bump_pct into absolute spot bump for denominator.
    let spot = match curves.get_price(&opt.spot_id)? {
        MarketScalar::Unitless(v) => *v,
        MarketScalar::Price(m) => m.amount(),
    };
    let h = spot * spot_bump_pct;
    Ok((pv_up - pv_dn) / (2.0 * h))
}

#[test]
fn equity_vanna_and_volga_match_reference_fd() -> finstack_core::Result<()> {
    let as_of = date(2025, 1, 2);
    let expiry = date(2025, 7, 2);
    let spot = 100.0;
    let strike = 100.0;
    let vol = 0.20;
    let rate = 0.03;
    let div_yield = 0.01;

    let opt = equity_option(as_of, expiry, strike);
    let market = equity_market(as_of, spot, vol, rate, div_yield);

    let pv = opt.value(&market, as_of)?;
    let mut ctx = MetricContext::new(
        Arc::new(opt.clone()),
        Arc::new(market.clone()),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let registry = standard_registry();

    let res = registry.compute(&[MetricId::Vanna, MetricId::Volga], &mut ctx)?;
    let vanna = *res.get(&MetricId::Vanna).expect("vanna present");
    let volga = *res.get(&MetricId::Volga).expect("volga present");

    // Reference finite differences using the same bump convention:
    // - Spot: ±1% (relative)
    // - Vol: absolute +/- 1 vol point
    let vol_bump_abs = VOL_BUMP_PCT;
    let delta_sigma = vol_bump_abs;

    let curves_vol_up =
        bump_surface_vol_absolute(&market, opt.vol_surface_id.as_str(), vol_bump_abs)?;
    let curves_vol_dn =
        bump_surface_vol_absolute(&market, opt.vol_surface_id.as_str(), -vol_bump_abs)?;

    let spot_bump_pct = SPOT_BUMP_PCT;
    let delta_up = equity_delta_fd(&opt, &curves_vol_up, as_of, spot_bump_pct)?;
    let delta_dn = equity_delta_fd(&opt, &curves_vol_dn, as_of, spot_bump_pct)?;
    let vanna_ref = (delta_up - delta_dn) / (2.0 * delta_sigma);

    let pv_up = opt.value(&curves_vol_up, as_of)?.amount();
    let pv_0 = opt.value(&market, as_of)?.amount();
    let pv_dn = opt.value(&curves_vol_dn, as_of)?.amount();
    // Volga is reported per **vol point squared** (consistent with vega, which
    // is per vol point), so normalize by the bump expressed in vol points.
    let volga_width = delta_sigma * 100.0;
    let volga_ref = (pv_up - 2.0 * pv_0 + pv_dn) / (volga_width * volga_width);

    // Tolerances: these are FD-vs-FD comparisons, so keep them tight but not brittle.
    approx_eq(vanna, vanna_ref, 5e-6);
    approx_eq(volga, volga_ref, 5e-4);
    Ok(())
}

#[test]
fn equity_vanna_and_volga_are_zero_when_expired() -> finstack_core::Result<()> {
    let as_of = date(2025, 7, 2);
    let expiry = date(2025, 7, 2); // expired at as_of
    let opt = equity_option(as_of, expiry, 100.0);
    let market = equity_market(as_of, 100.0, 0.20, 0.03, 0.0);

    let pv = opt.value(&market, as_of)?;
    let mut ctx = MetricContext::new(
        Arc::new(opt),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let registry = standard_registry();

    let res = registry.compute(&[MetricId::Vanna, MetricId::Volga], &mut ctx)?;
    assert_eq!(*res.get(&MetricId::Vanna).unwrap(), 0.0);
    assert_eq!(*res.get(&MetricId::Volga).unwrap(), 0.0);
    Ok(())
}

// -------------------- FX option vanna/volga --------------------

struct TestFx {
    spot: f64,
}
impl FxProvider for TestFx {
    fn rate(
        &self,
        from: Currency,
        to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> finstack_core::Result<f64> {
        // Support EUR/USD and USD/EUR for tests.
        if from == Currency::EUR && to == Currency::USD {
            Ok(self.spot)
        } else if from == Currency::USD && to == Currency::EUR {
            Ok(1.0 / self.spot)
        } else if from == to {
            Ok(1.0)
        } else {
            Err(finstack_core::Error::Validation(
                "FX rate not found".to_string(),
            ))
        }
    }
}

fn fx_market(as_of: Date, spot: f64, vol: f64, r_d: f64, r_f: f64) -> MarketContext {
    let expiries = [0.25, 0.5, 1.0, 2.0];
    let strikes = [0.9, 1.0, 1.1, 1.2, 1.3];

    let fx = FxMatrix::new(Arc::new(TestFx { spot }));
    MarketContext::new()
        .insert_fx(fx)
        .insert(flat_discount_with_tenor("USD-OIS", as_of, r_d, 5.0))
        .insert(flat_discount_with_tenor("EUR-OIS", as_of, r_f, 5.0))
        .insert_surface(flat_vol_surface("EURUSD-VOL", &expiries, &strikes, vol))
}

#[test]
fn fx_vanna_and_volga_match_reference_fd() -> finstack_core::Result<()> {
    let as_of = date(2025, 1, 2);
    let expiry = date(2025, 7, 2);

    let spot = 1.10;
    let strike = 1.10;
    let vol = 0.12;
    let r_d = 0.04;
    let r_f = 0.02;

    let opt = fx_option_european_call(
        "FX-VANNA-VOLGA",
        Currency::EUR,
        Currency::USD,
        strike,
        expiry,
        Money::new(1_000_000.0, Currency::EUR),
        "EURUSD-VOL",
    )
    .unwrap();

    let market = fx_market(as_of, spot, vol, r_d, r_f);
    let pv = opt.value(&market, as_of)?;

    let mut ctx = MetricContext::new(
        Arc::new(opt.clone()),
        Arc::new(market.clone()),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let registry = standard_registry();

    let res = registry.compute(&[MetricId::Vanna, MetricId::Volga], &mut ctx)?;
    let vanna = *res.get(&MetricId::Vanna).expect("vanna present");
    let volga = *res.get(&MetricId::Volga).expect("volga present");

    // Explicit reference using the calculator's own bump conventions:
    // bump a single surface point by ±1% and divide by the corresponding absolute Δσ.
    let t = opt
        .day_count
        .year_fraction(as_of, expiry, DayCountContext::default())?;
    let surf = market.get_surface(opt.vol_surface_id.as_str())?;
    let sigma = surf.value_clamped(t, opt.strike);
    let vol_bump_pct = VOL_BUMP_PCT;
    let delta_sigma = (sigma * vol_bump_pct).abs().max(1e-12);

    let curves_up = {
        let bumped = surf.bump_point(t, opt.strike, vol_bump_pct)?;
        market.clone().insert_surface(bumped)
    };
    let curves_dn = {
        let bumped = surf.bump_point(t, opt.strike, -vol_bump_pct)?;
        market.insert_surface(bumped)
    };

    let metrics = [MetricId::Delta, MetricId::Vega];
    let result_up = opt.price_with_metrics(
        &curves_up,
        as_of,
        &metrics,
        finstack_valuations::instruments::PricingOptions::default(),
    )?;
    let result_dn = opt.price_with_metrics(
        &curves_dn,
        as_of,
        &metrics,
        finstack_valuations::instruments::PricingOptions::default(),
    )?;
    let delta_up = result_up.measures[&MetricId::Delta];
    let delta_dn = result_dn.measures[&MetricId::Delta];
    let vanna_ref = (delta_up - delta_dn) / (2.0 * delta_sigma);

    let vega_up = result_up.measures[&MetricId::Vega];
    let vega_dn = result_dn.measures[&MetricId::Vega];
    let volga_ref = (vega_up - vega_dn) / (2.0 * delta_sigma) * 0.01;

    approx_eq(vanna, vanna_ref, 1e-10);
    approx_eq(volga, volga_ref, 1e-10);
    Ok(())
}

#[test]
fn fx_volga_returns_zero_when_surface_vol_is_zero() -> finstack_core::Result<()> {
    let as_of = date(2025, 1, 2);
    let expiry = date(2025, 7, 2);

    let opt = fx_option_european_call(
        "FX-ZERO-VOLGA",
        Currency::EUR,
        Currency::USD,
        1.10,
        expiry,
        Money::new(1_000_000.0, Currency::EUR),
        "EURUSD-VOL",
    )
    .unwrap();

    let market = fx_market(as_of, 1.10, 0.0, 0.02, 0.01);
    let pv = opt.value(&market, as_of)?;
    let mut ctx = MetricContext::new(
        Arc::new(opt),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let registry = standard_registry();

    let res = registry.compute(&[MetricId::Volga], &mut ctx)?;
    assert_eq!(*res.get(&MetricId::Volga).unwrap(), 0.0);
    Ok(())
}
