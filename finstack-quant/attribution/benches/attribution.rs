//! Fixed-size hot-path benchmarks for `finstack-quant-attribution`.
//!
//! Answers "how expensive is this call" at one representative size. Size
//! sweeps live in `attribution_scale.rs`. Existing ids (`parallel_1_bond`,
//! `waterfall_1_bond`, `parallel_5_bonds`) are preserved for baseline compare.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

#[path = "support/fixtures.rs"]
mod fixtures;

use criterion::{criterion_group, criterion_main, Criterion};
use finstack_quant_attribution::{
    attribute_pnl, attribute_pnl_metrics_based, attribute_return_contribution,
    attribute_return_contribution_json, default_waterfall_order,
    long_rows::pnl_attribution_long_rows, pnl_bridge, translate_to_target_currency,
    AttributionMethod, AttributionRequest, ExecutionPolicy, MarketRestoreFlags, MarketSnapshot,
    TaylorAttributionConfig,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::PricingOptions;
use finstack_quant_valuations::metrics::MetricId;
use fixtures::{
    equity_markets, eur_fx_markets, parallel_spec_envelope, return_contribution_spec, rich_markets,
    sample_bond, sample_equity, sample_eur_bond, BondMarkets,
};
use std::hint::black_box;
use std::sync::Arc;

fn bench_attribution_parallel_1_bond(c: &mut Criterion) {
    let fx = BondMarkets::new(5.0);
    let bond: Arc<dyn Instrument> = Arc::new(sample_bond("BENCH-BOND-1", 5));

    c.bench_function("parallel_1_bond", |b| {
        b.iter(|| {
            let attr = attribute_pnl(
                &AttributionMethod::Parallel,
                &AttributionRequest {
                    execution_policy: ExecutionPolicy::Parallel,
                    ..AttributionRequest::new(
                        &bond,
                        black_box(&fx.market_t0),
                        black_box(&fx.market_t1),
                        black_box(fx.as_of_t0),
                        black_box(fx.as_of_t1),
                        &fx.config,
                    )
                },
            )
            .unwrap();
            black_box(attr);
        });
    });
}

fn bench_attribution_waterfall_1_bond(c: &mut Criterion) {
    let fx = BondMarkets::new(5.0);
    let bond: Arc<dyn Instrument> = Arc::new(sample_bond("BENCH-BOND-1", 5));
    let factor_order = default_waterfall_order();

    c.bench_function("waterfall_1_bond", |b| {
        b.iter(|| {
            let attr = attribute_pnl(
                &AttributionMethod::Waterfall(factor_order.clone()),
                &AttributionRequest {
                    strict_validation: false,
                    model_params_t0: None,
                    ..AttributionRequest::new(
                        &bond,
                        black_box(&fx.market_t0),
                        black_box(&fx.market_t1),
                        black_box(fx.as_of_t0),
                        black_box(fx.as_of_t1),
                        &fx.config,
                    )
                },
            )
            .unwrap();
            black_box(attr);
        });
    });
}

fn bench_attribution_parallel_5_bonds(c: &mut Criterion) {
    let fx = BondMarkets::new(5.0);
    let bonds: Vec<Arc<dyn Instrument>> = (0..5)
        .map(|i| {
            let id = format!("BENCH-BOND-{i}");
            Arc::new(sample_bond(id.as_str(), 3 + i * 2)) as Arc<dyn Instrument>
        })
        .collect();

    c.bench_function("parallel_5_bonds", |b| {
        b.iter(|| {
            for bond in &bonds {
                let attr = attribute_pnl(
                    &AttributionMethod::Parallel,
                    &AttributionRequest {
                        execution_policy: ExecutionPolicy::Parallel,
                        ..AttributionRequest::new(
                            bond,
                            black_box(&fx.market_t0),
                            black_box(&fx.market_t1),
                            black_box(fx.as_of_t0),
                            black_box(fx.as_of_t1),
                            &fx.config,
                        )
                    },
                )
                .unwrap();
                black_box(attr);
            }
        });
    });
}

fn bench_hot_paths(c: &mut Criterion) {
    let mut group = c.benchmark_group("attribution_hot_paths");
    group.sample_size(20);

    let lean = BondMarkets::new(5.0);
    let fat = rich_markets(5.0);
    let fx_mkts = eur_fx_markets(5.0);
    let eq_mkts = equity_markets();
    let bond: Arc<dyn Instrument> = Arc::new(sample_bond("BENCH-BOND-1", 5));
    let eur_bond: Arc<dyn Instrument> = Arc::new(sample_eur_bond("BENCH-EUR-BOND", 5));
    let equity = sample_equity();
    let waterfall_order = default_waterfall_order();
    let taylor_cfg = TaylorAttributionConfig::default();
    let taylor_gamma = TaylorAttributionConfig {
        include_gamma: true,
        ..TaylorAttributionConfig::default()
    };
    let metrics = vec![MetricId::Dv01, MetricId::Theta, MetricId::Convexity];
    let opts = PricingOptions::default();
    let val_t0 = bond
        .price_with_metrics(&lean.market_t0, lean.as_of_t0, &metrics, opts.clone())
        .unwrap();
    let val_t1 = bond
        .price_with_metrics(&lean.market_t1, lean.as_of_t1, &metrics, opts)
        .unwrap();
    let rc_1k = return_contribution_spec(1_000, false);
    let rc_brinson_1k = return_contribution_spec(1_000, true);
    let rc_json = serde_json::to_string(&rc_1k).unwrap();
    let spec_envelope = parallel_spec_envelope(5.0);

    let template = attribute_pnl(
        &AttributionMethod::Parallel,
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Serial,
            ..AttributionRequest::new(
                &eur_bond,
                &fx_mkts.market_t0,
                &fx_mkts.market_t1,
                fx_mkts.as_of_t0,
                fx_mkts.as_of_t1,
                &fx_mkts.config,
            )
        },
    )
    .unwrap();
    let val_t0_eur = eur_bond
        .value(&fx_mkts.market_t0, fx_mkts.as_of_t0)
        .unwrap();
    let long_src = attribute_pnl(
        &AttributionMethod::Parallel,
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Serial,
            ..AttributionRequest::new(
                &bond,
                &lean.market_t0,
                &lean.market_t1,
                lean.as_of_t0,
                lean.as_of_t1,
                &lean.config,
            )
        },
    )
    .unwrap();

    group.bench_function("simple_bridge_1_bond", |b| {
        b.iter(|| {
            let pnl = pnl_bridge(
                &bond,
                black_box(&lean.market_t0),
                black_box(&lean.market_t1),
                black_box(lean.as_of_t0),
                black_box(lean.as_of_t1),
                Currency::USD,
            )
            .unwrap();
            black_box(pnl);
        });
    });

    group.bench_function("metrics_based_precomputed_1_bond", |b| {
        b.iter(|| {
            let attr = attribute_pnl_metrics_based(
                &bond,
                black_box(&lean.market_t0),
                black_box(&lean.market_t1),
                black_box(&val_t0),
                black_box(&val_t1),
                black_box(lean.as_of_t0),
                black_box(lean.as_of_t1),
            )
            .unwrap();
            black_box(attr);
        });
    });

    group.bench_function("taylor_1_bond", |b| {
        b.iter(|| {
            let attr = attribute_pnl(
                &AttributionMethod::Taylor(taylor_cfg.clone()),
                &AttributionRequest {
                    execution_policy: ExecutionPolicy::Serial,
                    ..AttributionRequest::new(
                        &bond,
                        black_box(&lean.market_t0),
                        black_box(&lean.market_t1),
                        black_box(lean.as_of_t0),
                        black_box(lean.as_of_t1),
                        &finstack_quant_core::config::FinstackConfig::default(),
                    )
                },
            )
            .unwrap();
            black_box(attr);
        });
    });

    group.bench_function("taylor_gamma_1_bond", |b| {
        b.iter(|| {
            let attr = attribute_pnl(
                &AttributionMethod::Taylor(taylor_gamma.clone()),
                &AttributionRequest {
                    execution_policy: ExecutionPolicy::Serial,
                    ..AttributionRequest::new(
                        &bond,
                        black_box(&lean.market_t0),
                        black_box(&lean.market_t1),
                        black_box(lean.as_of_t0),
                        black_box(lean.as_of_t1),
                        &finstack_quant_core::config::FinstackConfig::default(),
                    )
                },
            )
            .unwrap();
            black_box(attr);
        });
    });

    group.bench_function("parallel_serial_1_bond", |b| {
        b.iter(|| {
            let attr = attribute_pnl(
                &AttributionMethod::Parallel,
                &AttributionRequest {
                    execution_policy: ExecutionPolicy::Serial,
                    ..AttributionRequest::new(
                        &bond,
                        black_box(&lean.market_t0),
                        black_box(&lean.market_t1),
                        black_box(lean.as_of_t0),
                        black_box(lean.as_of_t1),
                        &lean.config,
                    )
                },
            )
            .unwrap();
            black_box(attr);
        });
    });

    group.bench_function("parallel_fat_market_1_bond", |b| {
        b.iter(|| {
            let attr = attribute_pnl(
                &AttributionMethod::Parallel,
                &AttributionRequest {
                    execution_policy: ExecutionPolicy::Serial,
                    ..AttributionRequest::new(
                        &bond,
                        black_box(&fat.market_t0),
                        black_box(&fat.market_t1),
                        black_box(fat.as_of_t0),
                        black_box(fat.as_of_t1),
                        &fat.config,
                    )
                },
            )
            .unwrap();
            black_box(attr);
        });
    });

    group.bench_function("waterfall_fat_market_1_bond", |b| {
        b.iter(|| {
            let attr = attribute_pnl(
                &AttributionMethod::Waterfall(waterfall_order.clone()),
                &AttributionRequest {
                    strict_validation: false,
                    model_params_t0: None,
                    ..AttributionRequest::new(
                        &bond,
                        black_box(&fat.market_t0),
                        black_box(&fat.market_t1),
                        black_box(fat.as_of_t0),
                        black_box(fat.as_of_t1),
                        &fat.config,
                    )
                },
            )
            .unwrap();
            black_box(attr);
        });
    });

    group.bench_function("equity_parallel_1", |b| {
        b.iter(|| {
            let attr = attribute_pnl(
                &AttributionMethod::Parallel,
                &AttributionRequest {
                    execution_policy: ExecutionPolicy::Serial,
                    ..AttributionRequest::new(
                        &equity,
                        black_box(&eq_mkts.market_t0),
                        black_box(&eq_mkts.market_t1),
                        black_box(eq_mkts.as_of_t0),
                        black_box(eq_mkts.as_of_t1),
                        &eq_mkts.config,
                    )
                },
            )
            .unwrap();
            black_box(attr);
        });
    });

    group.bench_function("fx_translate_1_bond", |b| {
        b.iter(|| {
            let mut attr = template.clone();
            translate_to_target_currency(
                &mut attr,
                val_t0_eur,
                Currency::USD,
                black_box(&fx_mkts.market_t0),
                black_box(&fx_mkts.market_t1),
                black_box(fx_mkts.as_of_t0),
                black_box(fx_mkts.as_of_t1),
            )
            .unwrap();
            black_box(attr);
        });
    });

    group.bench_function("long_rows_1_bond", |b| {
        b.iter(|| {
            let rows = pnl_attribution_long_rows(black_box(&long_src));
            black_box(rows);
        });
    });

    group.bench_function("snapshot_extract_restore_rates", |b| {
        b.iter(|| {
            let snap =
                MarketSnapshot::extract(black_box(&lean.market_t0), MarketRestoreFlags::RATES);
            let restored = MarketSnapshot::restore_market(
                black_box(&lean.market_t1),
                &snap,
                MarketRestoreFlags::RATES,
            );
            black_box(restored);
        });
    });

    group.bench_function("snapshot_extract_restore_all_fat", |b| {
        b.iter(|| {
            let snap =
                MarketSnapshot::extract(black_box(&fat.market_t0), MarketRestoreFlags::all());
            let restored = MarketSnapshot::restore_market(
                black_box(&fat.market_t1),
                &snap,
                MarketRestoreFlags::all(),
            );
            black_box(restored);
        });
    });

    group.bench_function("return_contribution_1k", |b| {
        b.iter(|| {
            let result = attribute_return_contribution(black_box(&rc_1k)).unwrap();
            black_box(result);
        });
    });

    group.bench_function("return_contribution_brinson_1k", |b| {
        b.iter(|| {
            let result = attribute_return_contribution(black_box(&rc_brinson_1k)).unwrap();
            black_box(result);
        });
    });

    group.bench_function("return_contribution_json_1k", |b| {
        b.iter(|| {
            let json = attribute_return_contribution_json(black_box(&rc_json)).unwrap();
            black_box(json);
        });
    });

    group.bench_function("spec_execute_1_bond", |b| {
        b.iter(|| {
            let result = spec_envelope.execute().unwrap();
            black_box(result);
        });
    });

    group.finish();
}

criterion_group!(
    attribution_parallel_1_bond,
    bench_attribution_parallel_1_bond
);
criterion_group!(
    attribution_waterfall_1_bond,
    bench_attribution_waterfall_1_bond
);
criterion_group!(
    attribution_parallel_5_bonds,
    bench_attribution_parallel_5_bonds
);
criterion_group!(attribution_hot_paths, bench_hot_paths);
criterion_main!(
    attribution_parallel_1_bond,
    attribution_waterfall_1_bond,
    attribution_parallel_5_bonds,
    attribution_hot_paths
);
