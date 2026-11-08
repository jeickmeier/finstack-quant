//! Benchmarks for market-context bumping (greeks/scenario hot path).
//!
//! Single-factor finite-difference greeks bump the shared `MarketContext` many
//! times (two sides per factor, across a portfolio). `MarketContext::bump`
//! returns a fresh perturbed context, so its cost scales with how much of the
//! context is copied per bump; this measures that against context size.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::bumps::{
    BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{
    MarketScalar, ScalarTimeSeries, SeriesInterpolation,
};
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use std::hint::black_box;
use std::sync::Arc;
use time::Month;

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn discount_curve(id: &str) -> DiscountCurve {
    let knots: Vec<(f64, f64)> = (0..40)
        .map(|i| {
            let t = (i as f64) * 0.5;
            (t, (-0.04 * t).exp())
        })
        .collect();
    DiscountCurve::builder(id)
        .base_date(base_date())
        .knots(knots)
        .build()
        .unwrap()
}

/// Build a context with `n_curves` discount curves.
fn build_context(n_curves: usize) -> (MarketContext, CurveId) {
    let mut ctx = MarketContext::new();
    let mut first = None;
    for i in 0..n_curves {
        let id = format!("CURVE_{i}");
        if first.is_none() {
            first = Some(CurveId::from(id.as_str()));
        }
        ctx.insert_mut(discount_curve(&id));
    }
    (ctx, first.expect("at least one curve"))
}

fn check_bump(ctx: &MarketContext, bumped: &MarketContext, target: &CurveId, n_curves: usize) {
    assert_eq!(ctx.len(), n_curves);
    assert_eq!(bumped.len(), n_curves);
    assert_eq!(ctx.curve_ids(), bumped.curve_ids());
    for id in ctx.curve_ids() {
        let original = ctx.get_discount(id.as_str()).expect("original curve");
        let after = bumped.get_discount(id.as_str()).expect("bumped curve");
        assert_eq!(original.id(), after.id());
        assert_eq!(original.knots(), after.knots());
        assert_eq!(original.len(), 40);
        if id == target {
            assert!(!Arc::ptr_eq(&original, &after));
            for (&t, &stored_df) in original.knots().iter().zip(original.dfs()) {
                let unchanged = (-0.04 * t).exp();
                assert_eq!(stored_df.to_bits(), unchanged.to_bits());
                let expected = stored_df * (-0.0001 * t).exp();
                let actual = after.df(t);
                assert!(actual.is_finite() && actual > 0.0);
                assert!(
                    (actual - expected).abs() <= 2e-13 * expected.abs().max(1.0),
                    "parallel bump at {t}: {actual} != {expected}"
                );
            }
            // The shock also applies between stored pillars.
            let t = 2.25;
            let expected = original.df(t) * (-0.0001_f64 * t).exp();
            assert!((after.df(t) - expected).abs() <= 2e-13);
        } else {
            assert!(Arc::ptr_eq(&original, &after), "unrelated curve {id}");
        }
    }
}

fn benchmark_bump(c: &mut Criterion) {
    let mut group = c.benchmark_group("context_bump");

    // The bumped factor is a single curve; the cost difference across context
    // sizes shows how much of the (unbumped) context is copied per bump.
    for &n_curves in &[8usize, 64, 256] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{n_curves}_curves")),
            &n_curves,
            |b, _| {
                let (ctx, target) = build_context(n_curves);
                let checked = ctx
                    .bump([MarketBump::Curve {
                        id: target.clone(),
                        spec: BumpSpec::parallel_bp(1.0),
                    }])
                    .expect("preflight bump");
                check_bump(&ctx, &checked, &target, n_curves);
                drop(checked);
                b.iter(|| {
                    let bumped = black_box(&ctx)
                        .bump([MarketBump::Curve {
                            id: target.clone(),
                            spec: BumpSpec::parallel_bp(1.0),
                        }])
                        .expect("bump");
                    black_box(bumped)
                });
            },
        );
    }

    group.finish();
}

const EXPIRIES: [f64; 3] = [0.25, 1.0, 2.0];
const STRIKES: [f64; 3] = [80.0, 100.0, 120.0];
const VOLS: [f64; 9] = [0.18, 0.19, 0.20, 0.20, 0.21, 0.22, 0.22, 0.23, 0.24];

#[derive(Clone, Copy)]
enum MixedTarget {
    Money,
    Surface,
    Series,
    TypedVol,
}

fn mixed_context(n_curves: usize) -> MarketContext {
    let (ctx, _) = build_context(n_curves);
    let surface = VolSurface::builder("EQ-VOL")
        .expiries(&EXPIRIES)
        .strikes(&STRIKES)
        .row(&VOLS[..3])
        .row(&VOLS[3..6])
        .row(&VOLS[6..])
        .build()
        .expect("fixed varied-vol surface");
    let series = ScalarTimeSeries::new(
        "SPX-CLOSE",
        (0..256)
            .map(|i| {
                (
                    base_date() + time::Duration::days(i),
                    100.0 + 0.1 * i as f64,
                )
            })
            .collect(),
        Some(Currency::USD),
    )
    .expect("fixed close series")
    .with_interpolation(SeriesInterpolation::Step);
    ctx.insert_surface(surface)
        .insert_price(
            "SPX-SPOT",
            MarketScalar::Price(Money::from((100_i64, Currency::USD))),
        )
        .insert_series(series)
}

fn mixed_bump(target: MixedTarget) -> MarketBump {
    match target {
        MixedTarget::Money => MarketBump::Curve {
            id: CurveId::from("SPX-SPOT"),
            spec: BumpSpec::multiplier(1.01),
        },
        MixedTarget::Surface => MarketBump::Curve {
            id: CurveId::from("EQ-VOL"),
            spec: BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::Fraction,
                value: 0.01,
                bump_type: BumpType::Parallel,
            },
        },
        MixedTarget::Series => MarketBump::Curve {
            id: CurveId::from("SPX-CLOSE"),
            spec: BumpSpec::multiplier(1.01),
        },
        MixedTarget::TypedVol => MarketBump::VolBucketPct {
            vol_surface_id: CurveId::from("EQ-VOL"),
            expiries: None,
            strikes: None,
            pct: 5.0,
        },
    }
}

fn check_mixed_bump(
    base: &MarketContext,
    bumped: &MarketContext,
    n_curves: usize,
    target: MixedTarget,
) {
    assert_eq!(base.len(), n_curves + 3);
    assert_eq!(bumped.len(), base.len());
    assert_eq!(base.curve_ids(), bumped.curve_ids());
    for ctx in [base, bumped] {
        let stats = ctx.stats();
        assert_eq!(stats.total_curves, n_curves);
        assert_eq!(stats.surface_count, 1);
        assert_eq!(stats.price_count, 1);
        assert_eq!(stats.series_count, 1);
        assert_eq!(stats.credit_index_count, 0);
    }
    for id in base.curve_ids() {
        let original = base.get_discount(id.as_str()).expect("base curve");
        let after = bumped.get_discount(id.as_str()).expect("unchanged curve");
        assert!(Arc::ptr_eq(&original, &after), "unrelated curve {id}");
        assert_eq!(original.id(), after.id());
        assert_eq!(original.base_date(), base_date());
        assert_eq!(original.knots(), after.knots());
        assert_eq!(original.len(), 40);
        for (&time, &df) in original.knots().iter().zip(original.dfs()) {
            assert_eq!(df.to_bits(), (-0.04 * time).exp().to_bits());
        }
    }

    for (ctx, changed) in [(base, false), (bumped, true)] {
        let surface = ctx.get_surface("EQ-VOL").expect("surface");
        assert_eq!(surface.id().as_str(), "EQ-VOL");
        assert_eq!(surface.expiries(), EXPIRIES);
        assert_eq!(surface.strikes(), STRIKES);
        assert_eq!(surface.vols().len(), VOLS.len());
        for (&actual, original) in surface.vols().iter().zip(VOLS) {
            let expected = match (changed, target) {
                (true, MixedTarget::Surface) => original + 0.01,
                (true, MixedTarget::TypedVol) => original * 1.05,
                _ => original,
            };
            assert!(actual.is_finite());
            assert_eq!(actual.to_bits(), expected.to_bits());
        }
        let MarketScalar::Price(spot) = ctx.get_price("SPX-SPOT").expect("spot") else {
            panic!("Money spot fixture changed variant");
        };
        assert_eq!(spot.currency(), Currency::USD);
        let expected_spot = if changed && matches!(target, MixedTarget::Money) {
            101_i64
        } else {
            100_i64
        };
        assert_eq!(
            spot.amount_decimal(),
            Money::from((expected_spot, Currency::USD)).amount_decimal()
        );

        let series = ctx.get_series("SPX-CLOSE").expect("close series");
        assert_eq!(series.id().as_str(), "SPX-CLOSE");
        assert_eq!(series.currency(), Some(Currency::USD));
        assert_eq!(series.interpolation(), SeriesInterpolation::Step);
        let observations = series.observations();
        assert_eq!(observations.len(), 256);
        for (i, (date, actual)) in observations.into_iter().enumerate() {
            assert_eq!(date, base_date() + time::Duration::days(i as i64));
            let original = 100.0 + 0.1 * i as f64;
            let expected = if changed && matches!(target, MixedTarget::Series) {
                original * 1.01
            } else {
                original
            };
            assert!(actual.is_finite());
            assert_eq!(actual.to_bits(), expected.to_bits());
        }
    }
    let original_surface = base.get_surface("EQ-VOL").expect("base surface");
    let after_surface = bumped.get_surface("EQ-VOL").expect("bumped surface");
    assert_eq!(
        original_surface.interpolation_mode(),
        after_surface.interpolation_mode()
    );
    if matches!(target, MixedTarget::Money | MixedTarget::Series) {
        assert!(Arc::ptr_eq(&original_surface, &after_surface));
    }
}

fn benchmark_mixed_bump(c: &mut Criterion) {
    let mut group = c.benchmark_group("context_bump_mixed");
    // 1024 is a large-context stress case, not a claim about typical markets.
    for (name, target, n_curves) in [
        ("money_multiplier", MixedTarget::Money, 8usize),
        ("money_multiplier", MixedTarget::Money, 64),
        ("money_multiplier", MixedTarget::Money, 256),
        ("money_multiplier", MixedTarget::Money, 1024),
        ("surface_abs", MixedTarget::Surface, 256),
        ("series_multiplier", MixedTarget::Series, 256),
        ("typed_vol_bucket", MixedTarget::TypedVol, 256),
    ] {
        group.bench_with_input(
            BenchmarkId::new(name, format!("curves_{n_curves}")),
            &n_curves,
            |b, &n_curves| {
                let base = mixed_context(n_curves);
                let checked = base
                    .bump([mixed_bump(target)])
                    .expect("selected-case mixed preflight");
                check_mixed_bump(&base, &checked, n_curves, target);
                drop(checked);
                b.iter(|| {
                    let bumped = black_box(&base)
                        .bump([mixed_bump(target)])
                        .expect("mixed bump");
                    black_box(bumped)
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, benchmark_bump, benchmark_mixed_bump);
criterion_main!(benches);
