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
    CurveKind, ExecutionContext, OperationSpec, ScenarioEngine, ScenarioSpec, Warning,
};
use finstack_quant_statements::types::FinancialModelSpec;
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
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
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

    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );
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
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
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

    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );
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

/// Projection-factor curve with 5.0%-5.5% continuously compounded zero rates.
fn sample_forward_curve(as_of: time::Date) -> ForwardCurve {
    ForwardCurve::from_zero_rates(
        "USD_LIBOR_3M",
        as_of,
        &[
            (0.25, 0.05),
            (0.5, 0.051),
            (1.0, 0.052),
            (2.0, 0.053),
            (5.0, 0.055),
        ],
        finstack_quant_core::math::Compounding::Continuous,
        finstack_quant_core::dates::DayCount::Act365F,
        finstack_quant_core::math::interp::InterpStyle::LogLinear,
        finstack_quant_core::math::interp::ExtrapolationPolicy::Extend,
    )
    .unwrap()
}

/// Assert finite positive factors and finite non-negative simple forwards
/// between consecutive knots.
fn assert_period_forwards_finite_non_negative(curve: &ForwardCurve, label: &str) {
    for (i, &factor) in curve.factors().iter().enumerate() {
        assert!(
            factor.is_finite() && factor > 0.0,
            "Projection factor at index {i} is invalid after {label}: {factor}"
        );
    }
    for window in curve.knots().windows(2) {
        let (t1, t2) = (window[0], window[1]);
        let fwd = curve.rate_between(t1, t2, t2 - t1).unwrap();
        assert!(
            fwd.is_finite() && fwd >= 0.0,
            "Forward rate over [{t1}, {t2}] is invalid after {label}: {fwd}"
        );
    }
}

#[test]
fn test_forward_curve_rates_finite_after_parallel_shock() {
    let mut market = MarketContext::new();
    let as_of = date!(2025 - 01 - 01);

    let curve = sample_forward_curve(as_of);

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

    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );
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

    let curve = market.get_forward("USD_LIBOR_3M").unwrap();
    assert_period_forwards_finite_non_negative(&curve, "parallel shock");
}

#[test]
fn test_forward_curve_rates_finite_after_node_shock() {
    let mut market = MarketContext::new();
    let as_of = date!(2025 - 01 - 01);

    let curve = sample_forward_curve(as_of);

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

    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );
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

    let curve = market.get_forward("USD_LIBOR_3M").unwrap();
    assert_period_forwards_finite_non_negative(&curve, "node shock");
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

    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );
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
fn parallel_base_correlation_clamp_warning_names_the_parallel_op() {
    let curve = BaseCorrelationCurve::builder("CDX_IG")
        .knots(vec![(0.03, 0.20), (0.30, 0.70)])
        .build()
        .unwrap();
    let mut market = MarketContext::new().insert(curve);
    let scenario = ScenarioSpec {
        id: "parallel_clamp".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::BaseCorrParallelPts {
            surface_id: "CDX_IG".into(),
            points: 0.40,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: date!(2025 - 01 - 01),
    };

    let report = ScenarioEngine::new().apply(&scenario, &mut ctx).unwrap();

    let details: Vec<&str> = report
        .warnings
        .iter()
        .filter_map(|warning| match warning {
            Warning::CorrelationClamped { detail, .. } => Some(detail.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(details.len(), 1, "{:?}", report.warnings);
    assert!(
        details[0].starts_with("BaseCorrParallelPts on CDX_IG detachment 0.3"),
        "{}",
        details[0]
    );
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
            detachment_bp: Some(vec![700, 1500]),
            points: 0.25,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );
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
        .secondary_values(&[90.0, 95.0, 100.0, 105.0, 110.0])
        .secondary_axis(finstack_quant_core::market_data::surfaces::VolSurfaceAxis::Strike)
        .quote_type(finstack_quant_core::market_data::surfaces::VolQuoteType::BlackLognormal)
        .interpolation_mode(finstack_quant_core::market_data::surfaces::VolInterpolationMode::Vol)
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
            vol_surface_id: "SPX_VOL".into(),
            pct: 20.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );
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
    let strikes = surface.secondary_values().to_vec();

    for &expiry in &expiries {
        for &strike in &strikes {
            let vol = finstack_quant_models::volatility::get_surface_vol(&surface, expiry, strike)
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
    let as_of = date!(2025 - 01 - 01);
    let clock = finstack_quant_core::dates::VolatilityClock::try_new(
        as_of,
        finstack_quant_core::dates::DayCount::Act365F,
        finstack_quant_core::dates::DayCountContextState::default(),
    )
    .expect("valid clock");
    let expiries: Vec<f64> = ["3M", "6M", "1Y", "2Y"]
        .iter()
        .map(|s| {
            let d = finstack_quant_core::dates::Tenor::parse(s)
                .unwrap()
                .add_to_date(
                    as_of,
                    None,
                    finstack_quant_core::dates::BusinessDayConvention::Unadjusted,
                )
                .unwrap();
            clock.expiry_time(d).unwrap()
        })
        .collect();

    let mut market = MarketContext::new();

    // Build a sample vol surface
    let surface = finstack_quant_core::market_data::surfaces::VolSurface::builder("SPX_VOL")
        .expiries(&expiries)
        .secondary_values(&[90.0, 95.0, 100.0, 105.0, 110.0])
        .secondary_axis(finstack_quant_core::market_data::surfaces::VolSurfaceAxis::Strike)
        .quote_type(finstack_quant_core::market_data::surfaces::VolQuoteType::BlackLognormal)
        .interpolation_mode(finstack_quant_core::market_data::surfaces::VolInterpolationMode::Vol)
        .expiry_clock(clock)
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
            vol_surface_id: "SPX_VOL".into(),
            tenors: Some(vec!["3M".into(), "1Y".into()]),
            strikes: Some(vec![95.0, 100.0]),
            pct: 30.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let mut model = FinancialModelSpec::new(
        "test",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            vec![],
        )
        .expect("valid period plan"),
    );
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

    // Check that all vol values are non-negative
    let surface = market.get_surface("SPX_VOL").unwrap();
    let expiries = surface.expiries().to_vec();
    let strikes = surface.secondary_values().to_vec();

    for &expiry in &expiries {
        for &strike in &strikes {
            let vol = finstack_quant_models::volatility::get_surface_vol(&surface, expiry, strike)
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
