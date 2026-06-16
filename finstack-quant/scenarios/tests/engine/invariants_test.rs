//! Post-shock invariant tests.
//!
//! Verifies that after applying scenario shocks, market data objects maintain
//! their fundamental invariants:
//! - Discount curves: DF(t) strictly decreasing
//! - Forward curves: forward rates finite and non-negative
//! - Base correlation: all values in [0, 1]
//! - Vol surfaces: all grid values ≥ 0

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::BaseCorrelationCurve;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_scenarios::{
    CurveKind, ExecutionContext, OperationSpec, ScenarioEngine, ScenarioSpec,
};
use finstack_quant_statements::FinancialModelSpec;
use time::macros::date;

#[test]
fn test_discount_curve_df_monotonic_after_parallel_shock() {
    let mut market = MarketContext::new();
    let as_of = date!(2025 - 01 - 01);

    // Build a sample discount curve
    let curve = DiscountCurve::builder("USD_SOFR")
        .base_date(as_of)
        .day_count(finstack_quant_core::dates::DayCount::Thirty360)
        .knots(vec![
            (0.25, 0.99),
            (0.5, 0.98),
            (1.0, 0.96),
            (2.0, 0.92),
            (5.0, 0.85),
            (10.0, 0.70),
        ])
        .build()
        .unwrap();

    market = market.insert(curve);

    // Apply parallel shock
    let scenario = ScenarioSpec {
        id: "parallel_shock".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD_SOFR".into(),
            discount_curve_id: None,
            bp: 50.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let mut model = FinancialModelSpec::new("test", vec![]);
    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of,
    };

    engine.apply(&scenario, &mut ctx).unwrap();

    // Check monotonicity: DF should be strictly decreasing
    let curve = market.get_discount("USD_SOFR").unwrap();
    let knots = curve.knots();
    let dfs = curve.dfs();

    for i in 1..knots.len() {
        assert!(
            dfs[i] < dfs[i - 1],
            "DF not strictly decreasing: DF({}) = {} >= DF({}) = {}",
            knots[i],
            dfs[i],
            knots[i - 1],
            dfs[i - 1]
        );
    }
}

#[test]
fn test_discount_curve_df_monotonic_after_node_shock() {
    let mut market = MarketContext::new();
    let as_of = date!(2025 - 01 - 01);

    // Build a sample discount curve
    let curve = DiscountCurve::builder("USD_SOFR")
        .base_date(as_of)
        .day_count(finstack_quant_core::dates::DayCount::Thirty360)
        .knots(vec![
            (0.25, 0.99),
            (0.5, 0.98),
            (1.0, 0.96),
            (2.0, 0.92),
            (5.0, 0.85),
            (10.0, 0.70),
        ])
        .build()
        .unwrap();

    market = market.insert(curve);

    // Apply node shock (2Y key-rate bump)
    let scenario = ScenarioSpec {
        id: "node_shock".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveNodeBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD_SOFR".into(),
            discount_curve_id: None,
            nodes: vec![("2Y".into(), 25.0)],
            match_mode: finstack_quant_scenarios::TenorMatchMode::Interpolate,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let mut model = FinancialModelSpec::new("test", vec![]);
    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of,
    };

    engine.apply(&scenario, &mut ctx).unwrap();

    // Check monotonicity
    let curve = market.get_discount("USD_SOFR").unwrap();
    let knots = curve.knots();
    let dfs = curve.dfs();

    for i in 1..knots.len() {
        assert!(
            dfs[i] < dfs[i - 1],
            "DF not strictly decreasing after node shock: DF({}) = {} >= DF({}) = {}",
            knots[i],
            dfs[i],
            knots[i - 1],
            dfs[i - 1]
        );
    }
}

#[test]
fn test_forward_curve_rates_finite_after_parallel_shock() {
    let mut market = MarketContext::new();
    let as_of = date!(2025 - 01 - 01);

    // Build a sample forward curve (3M = 0.25 years)
    let curve = ForwardCurve::builder("USD_LIBOR_3M", 0.25)
        .base_date(as_of)
        .knots(vec![
            (0.25, 0.05),
            (0.5, 0.051),
            (1.0, 0.052),
            (2.0, 0.053),
            (5.0, 0.055),
        ])
        .build()
        .unwrap();

    market = market.insert(curve);

    // Apply parallel shock
    let scenario = ScenarioSpec {
        id: "parallel_shock".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Forward,
            curve_id: "USD_LIBOR_3M".into(),
            discount_curve_id: None,
            bp: 100.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let mut model = FinancialModelSpec::new("test", vec![]);
    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of,
    };

    engine.apply(&scenario, &mut ctx).unwrap();

    // Check that all forwards are finite and non-negative
    let curve = market.get_forward("USD_LIBOR_3M").unwrap();
    let forwards = curve.forwards();

    for (i, &fwd) in forwards.iter().enumerate() {
        assert!(
            fwd.is_finite(),
            "Forward rate at index {} is not finite: {}",
            i,
            fwd
        );
        assert!(
            fwd >= 0.0,
            "Forward rate at index {} is negative: {}",
            i,
            fwd
        );
    }
}

#[test]
fn test_forward_curve_rates_finite_after_node_shock() {
    let mut market = MarketContext::new();
    let as_of = date!(2025 - 01 - 01);

    // Build a sample forward curve (3M = 0.25 years)
    let curve = ForwardCurve::builder("USD_LIBOR_3M", 0.25)
        .base_date(as_of)
        .knots(vec![
            (0.25, 0.05),
            (0.5, 0.051),
            (1.0, 0.052),
            (2.0, 0.053),
            (5.0, 0.055),
        ])
        .build()
        .unwrap();

    market = market.insert(curve);

    // Apply node shock
    let scenario = ScenarioSpec {
        id: "node_shock".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveNodeBp {
            curve_kind: CurveKind::Forward,
            curve_id: "USD_LIBOR_3M".into(),
            discount_curve_id: None,
            nodes: vec![("1Y".into(), 50.0)],
            match_mode: finstack_quant_scenarios::TenorMatchMode::Interpolate,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let mut model = FinancialModelSpec::new("test", vec![]);
    let engine = ScenarioEngine::new();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of,
    };

    engine.apply(&scenario, &mut ctx).unwrap();

    // Check that all forwards are finite and non-negative
    let curve = market.get_forward("USD_LIBOR_3M").unwrap();
    let forwards = curve.forwards();

    for (i, &fwd) in forwards.iter().enumerate() {
        assert!(
            fwd.is_finite(),
            "Forward rate at index {} is not finite after node shock: {}",
            i,
            fwd
        );
        assert!(
            fwd >= 0.0,
            "Forward rate at index {} is negative after node shock: {}",
            i,
            fwd
        );
    }
}

#[test]
fn test_base_correlation_bounds_after_parallel_shock() {
    let mut market = MarketContext::new();

    // Build a sample base correlation curve
    let curve = BaseCorrelationCurve::builder("CDX_IG")
        .knots(vec![
            (0.03, 0.20),
            (0.07, 0.35),
            (0.10, 0.45),
            (0.15, 0.55),
            (0.30, 0.70),
        ])
        .build()
        .unwrap();

    market = market.insert(curve);

    // Apply parallel shock (additive)
    let scenario = ScenarioSpec {
        id: "parallel_shock".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::BaseCorrParallelPts {
            surface_id: "CDX_IG".into(),
            points: 0.15,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let mut model = FinancialModelSpec::new("test", vec![]);
    let engine = ScenarioEngine::new();
    let as_of = date!(2025 - 01 - 01);
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of,
    };

    engine.apply(&scenario, &mut ctx).unwrap();

    // Check that all correlations are in [0, 1]
    let curve = market.get_base_correlation("CDX_IG").unwrap();
    let correlations = curve.correlations();

    for (i, &corr) in correlations.iter().enumerate() {
        assert!(
            (0.0..=1.0).contains(&corr),
            "Correlation at index {} is out of bounds [0, 1]: {}",
            i,
            corr
        );
    }
}

#[test]
fn test_base_correlation_bounds_after_bucket_shock() {
    let mut market = MarketContext::new();

    // Build a sample base correlation curve
    let curve = BaseCorrelationCurve::builder("CDX_IG")
        .knots(vec![
            (0.03, 0.20),
            (0.07, 0.35),
            (0.10, 0.45),
            (0.15, 0.55),
            (0.30, 0.70),
        ])
        .build()
        .unwrap();

    market = market.insert(curve);

    // Apply bucket shock to 7% and 15% detachment points
    let scenario = ScenarioSpec {
        id: "bucket_shock".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::BaseCorrBucketPts {
            surface_id: "CDX_IG".into(),
            detachment_bps: Some(vec![700, 1500]),
            maturities: None,
            points: 0.25,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let mut model = FinancialModelSpec::new("test", vec![]);
    let engine = ScenarioEngine::new();
    let as_of = date!(2025 - 01 - 01);
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of,
    };

    engine.apply(&scenario, &mut ctx).unwrap();

    // Check that all correlations are in [0, 1]
    let curve = market.get_base_correlation("CDX_IG").unwrap();
    let correlations = curve.correlations();

    for (i, &corr) in correlations.iter().enumerate() {
        assert!(
            (0.0..=1.0).contains(&corr),
            "Correlation at index {} is out of bounds [0, 1] after bucket shock: {}",
            i,
            corr
        );
    }
}

#[test]
fn test_vol_surface_non_negative_after_parallel_shock() {
    let mut market = MarketContext::new();

    // Build a sample vol surface
    let surface = finstack_quant_core::market_data::surfaces::VolSurface::builder("SPX_VOL")
        .expiries(&[0.25, 0.5, 1.0, 2.0])
        .strikes(&[90.0, 95.0, 100.0, 105.0, 110.0])
        .row(&[0.25, 0.22, 0.20, 0.22, 0.25])
        .row(&[0.24, 0.21, 0.19, 0.21, 0.24])
        .row(&[0.23, 0.20, 0.18, 0.20, 0.23])
        .row(&[0.22, 0.19, 0.17, 0.19, 0.22])
        .build()
        .unwrap();

    market = market.insert_surface(surface);

    // Apply parallel shock
    let scenario = ScenarioSpec {
        id: "parallel_shock".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::VolSurfaceParallelPct {
            surface_kind: finstack_quant_scenarios::VolSurfaceKind::Equity,
            surface_id: "SPX_VOL".into(),
            pct: 20.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let mut model = FinancialModelSpec::new("test", vec![]);
    let engine = ScenarioEngine::new();
    let as_of = date!(2025 - 01 - 01);
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of,
    };

    engine.apply(&scenario, &mut ctx).unwrap();

    // Check that all vol values are non-negative
    let surface = market.get_surface("SPX_VOL").unwrap();
    let expiries = surface.expiries().to_vec();
    let strikes = surface.strikes().to_vec();

    for &expiry in &expiries {
        for &strike in &strikes {
            let vol = surface
                .value_checked(expiry, strike)
                .expect("grid points are in bounds");
            assert!(
                vol >= 0.0,
                "Vol at (expiry={}, strike={}) is negative: {}",
                expiry,
                strike,
                vol
            );
        }
    }
}

#[test]
fn test_vol_surface_non_negative_after_bucket_shock() {
    let mut market = MarketContext::new();

    // Build a sample vol surface
    let surface = finstack_quant_core::market_data::surfaces::VolSurface::builder("SPX_VOL")
        .expiries(&[0.25, 0.5, 1.0, 2.0])
        .strikes(&[90.0, 95.0, 100.0, 105.0, 110.0])
        .row(&[0.25, 0.22, 0.20, 0.22, 0.25])
        .row(&[0.24, 0.21, 0.19, 0.21, 0.24])
        .row(&[0.23, 0.20, 0.18, 0.20, 0.23])
        .row(&[0.22, 0.19, 0.17, 0.19, 0.22])
        .build()
        .unwrap();

    market = market.insert_surface(surface);

    // Apply bucket shock to specific strikes
    let scenario = ScenarioSpec {
        id: "bucket_shock".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::VolSurfaceBucketPct {
            surface_kind: finstack_quant_scenarios::VolSurfaceKind::Equity,
            surface_id: "SPX_VOL".into(),
            tenors: Some(vec!["3M".into(), "1Y".into()]),
            strikes: Some(vec![95.0, 100.0]),
            pct: 30.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let mut model = FinancialModelSpec::new("test", vec![]);
    let engine = ScenarioEngine::new();
    let as_of = date!(2025 - 01 - 01);
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of,
    };

    engine.apply(&scenario, &mut ctx).unwrap();

    // Check that all vol values are non-negative
    let surface = market.get_surface("SPX_VOL").unwrap();
    let expiries = surface.expiries().to_vec();
    let strikes = surface.strikes().to_vec();

    for &expiry in &expiries {
        for &strike in &strikes {
            let vol = surface
                .value_checked(expiry, strike)
                .expect("grid points are in bounds");
            assert!(
                vol >= 0.0,
                "Vol at (expiry={}, strike={}) is negative after bucket shock: {}",
                expiry,
                strike,
                vol
            );
        }
    }
}
