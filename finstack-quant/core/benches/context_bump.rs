//! Benchmarks for market-context bumping (greeks/scenario hot path).
//!
//! Single-factor finite-difference greeks bump the shared `MarketContext` many
//! times (two sides per factor, across a portfolio). `MarketContext::bump`
//! returns a fresh perturbed context, so its cost scales with how much of the
//! context is copied per bump; this measures that against context size.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::types::CurveId;
use std::hint::black_box;
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

fn benchmark_bump(c: &mut Criterion) {
    let mut group = c.benchmark_group("context_bump");

    // The bumped factor is a single curve; the cost difference across context
    // sizes shows how much of the (unbumped) context is copied per bump.
    for &n_curves in &[8usize, 64, 256] {
        let (ctx, target) = build_context(n_curves);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{n_curves}_curves")),
            &n_curves,
            |b, _| {
                b.iter(|| {
                    let bumped = ctx
                        .bump([MarketBump::Curve {
                            id: target.clone(),
                            spec: BumpSpec::parallel_bp(1.0),
                        }])
                        .expect("bump");
                    black_box(bumped.curve(target.as_str()).is_some())
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, benchmark_bump);
criterion_main!(benches);
