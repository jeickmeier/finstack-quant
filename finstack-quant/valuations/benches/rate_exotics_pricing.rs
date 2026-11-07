//! Hull-White 1F and range-accrual exotic pricing benchmarks.
//!
//! Covers the rate-note engines that were previously unmeasured:
//! - [`RangeAccrual`]: analytic pricing and cross-currency quanto pricing through analytic/GBM paths
//! - [`CallableRangeAccrual`]: HW1F LSMC with Bermudan call dates
//! - [`Snowball`]: path-dependent HW1F Monte Carlo
//! - [`Tarn`]: target-redemption HW1F Monte Carlo
//!
//! Monte Carlo cases pin `mc_paths` to 2,500 so the target stays usable under
//! `mise run rust-bench` short sampling. Production defaults are 20,000 paths
//! for rate exotics.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_models::monte_carlo::engine::PathCaptureMode;
use finstack_quant_models::monte_carlo::pricer::path_dependent::PathDependentPricerConfig;
use finstack_quant_models::rates::hw1f::Hw1fMcConfig;
use finstack_quant_valuations::instruments::common_impl::helpers::merged_path_config;
use finstack_quant_valuations::instruments::exotics::callable_range_accrual::CallableRangeAccrual;
use finstack_quant_valuations::instruments::exotics::range_accrual::RangeAccrual;
use finstack_quant_valuations::instruments::exotics::snowball::Snowball;
use finstack_quant_valuations::instruments::exotics::tarn::Tarn;
use finstack_quant_valuations::instruments::pricing::hw1f_params::{
    Hw1fParamFamily, resolve_hw1f_params,
};
use finstack_quant_valuations::instruments::pricing_overrides::{
    InstrumentPricingOverrides, ModelConfig,
};
use finstack_quant_valuations::instruments::{Instrument, PricingOptions, QuantoSpec};
use finstack_quant_valuations::pricer::STANDARD;
use finstack_quant_valuations::pricer::{InstrumentType, ModelKey, PricerKey};
use finstack_quant_valuations::results::ValuationResult;
use std::hint::black_box;
use time::Month;

#[path = "support/record.rs"]
mod record;
mod support;

const MC_PATHS: u64 = 2_500;

fn date(year: i32, month: Month, day: u8) -> Date {
    Date::from_calendar_date(year, month, day).unwrap()
}

/// Discount plus projection forwards reconstructed from the same curve.
fn hw1f_market(as_of: Date) -> MarketContext {
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (1.0, 0.97),
            (2.0, 0.94),
            (5.0, 0.85),
            (10.0, 0.70),
        ])
        .build()
        .unwrap();
    let fwd_6m = discount.to_forward_curve("USD-SOFR-6M", 0.5, None).unwrap();
    let fwd_3m = discount
        .to_forward_curve("USD-SOFR-3M", 0.25, None)
        .unwrap();
    MarketContext::new()
        .insert(discount)
        .insert(fwd_6m)
        .insert(fwd_3m)
        .insert_price("SOFR-RATE", MarketScalar::Unitless(0.03))
        .insert_price("USD-OIS_CAPFLOOR_HW1F_KAPPA", MarketScalar::Unitless(0.03))
        .insert_price("USD-OIS_CAPFLOOR_HW1F_SIGMA", MarketScalar::Unitless(0.01))
}

/// Equity-linked range-accrual market matching the instrument example IDs.
fn equity_range_market(as_of: Date) -> MarketContext {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.97), (2.0, 0.94)])
        .build()
        .unwrap();
    let surface = VolSurface::builder("SPX-VOL")
        .expiries(&[0.25, 0.5, 1.0, 2.0])
        .strikes(&[80.0, 100.0, 120.0, 140.0])
        .row(&[0.20, 0.20, 0.20, 0.20])
        .row(&[0.20, 0.20, 0.20, 0.20])
        .row(&[0.20, 0.20, 0.20, 0.20])
        .row(&[0.20, 0.20, 0.20, 0.20])
        .build()
        .unwrap();
    MarketContext::new()
        .insert(curve)
        .insert_surface(surface)
        .insert_price("SPX-SPOT", MarketScalar::Unitless(100.0))
        .insert_price("SPX-DIV", MarketScalar::Unitless(0.02))
}

fn with_mc_paths<T>(mut inst: T, apply: impl FnOnce(&mut T, InstrumentPricingOverrides)) -> T {
    apply(
        &mut inst,
        InstrumentPricingOverrides::default().with_mc_paths(MC_PATHS as usize),
    );
    inst
}

fn assert_usd(value: &Money) {
    assert!(value.amount().is_finite());
    assert_eq!(value.currency(), Currency::USD);
}

fn capture_range(
    case: &str,
    inst: &RangeAccrual,
    market: &MarketContext,
    as_of: Date,
    model: ModelKey,
    output: impl FnOnce() -> serde_json::Value,
) {
    record::capture(case, || {
        let mc = (model == ModelKey::MonteCarloGBM).then(|| {
            let mut config = merged_path_config(
                &PathDependentPricerConfig::default(),
                &inst.instrument_pricing_overrides,
            )
            .expect("capture range accrual configuration");
            config.seed = inst.instrument_pricing_overrides.model_config.mc_seed(&inst.id);
            let final_date = inst.terms.payment_date.unwrap_or_else(|| {
                *inst.terms.observation_dates.last().expect("observation schedule")
            });
            let horizon = inst
                .terms
                .day_count
                .year_fraction(as_of, final_date, DayCountContext::default())
                .expect("capture payment horizon");
            let observation_times: Vec<_> = inst
                .terms
                .observation_dates
                .iter()
                .filter(|&&date| date > as_of)
                .map(|&date| {
                    inst.terms
                        .day_count
                        .year_fraction(as_of, date, DayCountContext::default())
                        .expect("capture observation time")
                })
                .collect();
            let grid = config
                .build_time_grid(horizon, &observation_times)
                .expect("capture range accrual grid");
            let capture_mode = match &config.path_capture.capture_mode {
                PathCaptureMode::All => serde_json::json!({"mode": "all"}),
                PathCaptureMode::Sample { count, seed } => {
                    serde_json::json!({"mode": "sample", "count": count, "seed": seed})
                }
            };
            serde_json::json!({
                "config": {
                    "num_paths": config.num_paths,
                    "seed": config.seed,
                    "use_parallel": config.use_parallel,
                    "chunk_size": config.chunk_size,
                    "path_capture": {
                        "enabled": config.path_capture.enabled,
                        "capture_mode": capture_mode,
                        "capture_payoffs": config.path_capture.capture_payoffs,
                    },
                    "steps_per_year": config.steps_per_year,
                    "min_steps": config.min_steps,
                    "use_sobol": config.use_sobol,
                    "antithetic": config.antithetic,
                    "use_brownian_bridge": config.use_brownian_bridge,
                },
                "seed": config.seed,
                "independent_estimators": config.num_paths,
                "physical_paths": config.num_paths * if config.antithetic { 2 } else { 1 },
                "grid": {
                    "source": "reconstructed with the production public grid builder",
                    "clock": inst.terms.day_count,
                    "horizon": horizon,
                    "actual_steps": grid.times().len() - 1,
                    "time_bits": grid.times().iter().map(|time| time.to_bits()).collect::<Vec<_>>(),
                    "observation_times": observation_times,
                },
                "uncertainty": {
                    "available": false,
                    "reason": "RangeAccrual GBM pricer returns the mean without publishing MoneyEstimate diagnostics",
                },
            })
        });
        serde_json::json!({
            "fixture": {
                "instrument": inst,
                "market": market,
                "as_of": as_of,
                "model": model,
                "config": {
                    "instrument_pricing_overrides": inst.instrument_pricing_overrides,
                    "mc": mc.as_ref().map(|data| &data["config"]),
                },
                "seed": mc.as_ref().map(|data| &data["seed"]),
                "independent_estimators": mc.as_ref().map(|data| &data["independent_estimators"]),
                "physical_paths": mc.as_ref().map(|data| &data["physical_paths"]),
                "grid": mc.as_ref().map(|data| &data["grid"]),
                "uncertainty": mc.as_ref().map(|data| &data["uncertainty"]),
            },
            "output": output(),
        })
    });
}

fn capture_hw1f<T: Instrument + serde::Serialize>(
    case: &str,
    inst: &T,
    market: &MarketContext,
    as_of: Date,
    discount_curve_id: &str,
    model_config: &ModelConfig,
    result: &ValuationResult,
) {
    record::capture(case, || {
        let config = Hw1fMcConfig::default().with_instrument_overrides(
            &finstack_quant_core::types::InstrumentId::new(inst.id()),
            model_config.mc_paths,
            model_config.mc_seed_scenario.as_deref(),
        );
        let parameters = resolve_hw1f_params(
            Hw1fParamFamily::CapFloor,
            discount_curve_id,
            model_config,
            None,
            case,
            market,
        )
        .expect("capture HW1F parameters");
        assert_eq!(result.measures["mc_num_paths"], config.num_paths as f64);
        assert_eq!(
            result.measures["mc_num_simulated_paths"],
            config.simulated_path_count() as f64
        );
        assert!(result.measures["mc_stderr"] >= 0.0);
        serde_json::json!({
            "fixture": {
                "instrument": inst,
                "market": market,
                "as_of": as_of,
                "model": ModelKey::MonteCarloHullWhite1F,
                "model_parameters": parameters,
                "model_config": model_config,
                "config": config,
                "seed": config.seed,
                "independent_estimators": config.num_paths,
                "physical_paths": config.simulated_path_count(),
                "requested_metrics": [],
                "pricing_options": "PricingOptions::default()",
                "grid": {
                    "available": false,
                    "reason": "the note event-schedule builders and actual runtime grid are private",
                    "policy": "event-aligned HW1F grid",
                    "clock": DayCount::Act365F,
                    "min_steps_between_events": config.min_steps_between_events,
                },
                "theta": {
                    "available": false,
                    "reason": "the note pricer does not expose its fitted theta schedule",
                },
            },
            "output": {
                "value": result.value,
                "value_bits": result.value.amount().to_bits(),
                "measures": result.measures,
            },
        })
    });
}

fn bench_range_accrual_analytic(c: &mut Criterion) {
    let mut group = c.benchmark_group("range_accrual_analytic");
    group.bench_function("12_monthly_obs", |b| {
        let as_of = date(2024, Month::January, 1);
        let market = equity_range_market(as_of);
        let inst = RangeAccrual::example().expect("example");
        assert_eq!(inst.default_model(), ModelKey::StaticReplication);
        assert_eq!(inst.terms.observation_dates.len(), 12);
        assert!(
            inst.terms
                .observation_dates
                .iter()
                .all(|date| *date > as_of)
        );
        assert!(inst.terms.quanto.is_none());
        support::bench(
            b,
            || {
                black_box(&inst)
                    .value(black_box(&market), black_box(as_of))
                    .expect("analytic range accrual PV")
            },
            |value| {
                assert_usd(value);
                capture_range(
                    "range_accrual_analytic/12_monthly_obs",
                    &inst,
                    &market,
                    as_of,
                    ModelKey::StaticReplication,
                    || serde_json::json!({"value": value, "value_bits": value.amount().to_bits()}),
                );
            },
        );
    });
    group.finish();
}

fn quanto_range_fixture(as_of: Date) -> (RangeAccrual, MarketContext) {
    let market = equity_range_market(as_of)
        .insert(
            DiscountCurve::builder("EUR-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (2.0, (-0.2_f64).exp())])
                .build()
                .unwrap(),
        )
        .insert_price(
            "SPX-SPOT",
            MarketScalar::Price(Money::from((100_i64, Currency::EUR))),
        )
        .insert_price("EURUSD", MarketScalar::Unitless(1.1))
        .insert_surface(
            VolSurface::builder("FX-VOL")
                .expiries(&[1.0])
                .strikes(&[1.1])
                .row(&[0.1])
                .build()
                .unwrap(),
        );
    let mut inst = RangeAccrual::example().expect("example");
    inst.terms.quanto = Some(QuantoSpec {
        asset_currency: Currency::EUR,
        asset_discount_curve_id: "EUR-OIS".into(),
        correlation: 0.5,
        fx_vol_surface_id: "FX-VOL".into(),
        fx_spot_id: "EURUSD".into(),
    });
    inst.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_mc_paths(MC_PATHS as usize);
    (inst, market)
}

fn bench_quanto_range_accrual(c: &mut Criterion) {
    let mut group = c.benchmark_group("quanto_range_accrual");
    group.bench_function("analytic_12_observations", |b| {
        let as_of = date(2024, Month::January, 1);
        let (inst, market) = quanto_range_fixture(as_of);
        assert_eq!(inst.default_model(), ModelKey::StaticReplication);
        assert_eq!(inst.terms.observation_dates.len(), 12);
        assert!(inst.terms.quanto.is_some());
        support::bench(
            b,
            || {
                inst.value(black_box(&market), black_box(as_of))
                    .expect("analytic quanto range accrual PV")
            },
            |value| {
                assert_usd(value);
                capture_range(
                    "quanto_range_accrual/analytic_12_observations",
                    &inst,
                    &market,
                    as_of,
                    ModelKey::StaticReplication,
                    || serde_json::json!({"value": value, "value_bits": value.amount().to_bits()}),
                );
            },
        );
    });
    group.bench_function("gbm_2500_paths_12_observations", |b| {
        let as_of = date(2024, Month::January, 1);
        let (inst, market) = quanto_range_fixture(as_of);
        let registry = STANDARD.pricers();
        let key = PricerKey::new(InstrumentType::RangeAccrual, ModelKey::MonteCarloGBM);
        let pricer = registry.get_pricer(key).expect("GBM range accrual pricer");
        assert_eq!(pricer.key(), key);
        assert_eq!(
            inst.instrument_pricing_overrides.model_config.mc_paths,
            Some(MC_PATHS as usize)
        );
        assert_eq!(inst.terms.observation_dates.len(), 12);
        assert!(inst.terms.quanto.is_some());
        support::bench(
            b,
            || {
                pricer
                    .price_dyn(black_box(&inst), black_box(&market), black_box(as_of))
                    .expect("GBM quanto range accrual PV")
            },
            |result| {
                assert_usd(&result.value);
                assert!(result.measures.values().all(|value| value.is_finite()));
                capture_range(
                    "quanto_range_accrual/gbm_2500_paths_12_observations",
                    &inst,
                    &market,
                    as_of,
                    ModelKey::MonteCarloGBM,
                    || {
                        serde_json::json!({
                            "value": result.value,
                            "value_bits": result.value.amount().to_bits(),
                            "measures": result.measures,
                        })
                    },
                );
            },
        );
    });
    group.finish();
}

fn bench_callable_range_accrual_lsmc(c: &mut Criterion) {
    let mut group = c.benchmark_group("callable_range_accrual_lsmc");
    group.throughput(Throughput::Elements(MC_PATHS));
    group.bench_with_input(BenchmarkId::from_parameter(MC_PATHS), &MC_PATHS, |b, _| {
        let as_of = date(2026, Month::January, 1);
        let market = hw1f_market(as_of);
        let inst = with_mc_paths(
            CallableRangeAccrual::example().expect("example"),
            |inst, overrides| {
                inst.instrument_pricing_overrides = overrides;
            },
        );
        assert_eq!(inst.default_model(), ModelKey::MonteCarloHullWhite1F);
        assert_eq!(
            inst.instrument_pricing_overrides.model_config.mc_paths,
            Some(MC_PATHS as usize)
        );
        assert_eq!(inst.range_accrual.observation_dates.len(), 12);
        assert!(!inst.call_provision.eligible_call_dates().is_empty());
        support::bench(
            b,
            || {
                black_box(&inst)
                    .price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&[]),
                        PricingOptions::default(),
                    )
                    .expect("callable range accrual HW1F LSMC PV")
            },
            |result| {
                assert_usd(&result.value);
                assert!(result.measures.values().all(|value| value.is_finite()));
                capture_hw1f(
                    "callable_range_accrual_lsmc/2500",
                    &inst,
                    &market,
                    as_of,
                    inst.range_accrual.discount_curve_id.as_str(),
                    &inst.instrument_pricing_overrides.model_config,
                    result,
                );
            },
        );
    });
    group.finish();
}

fn bench_snowball_mc(c: &mut Criterion) {
    let mut group = c.benchmark_group("snowball_hw1f_mc");
    group.throughput(Throughput::Elements(MC_PATHS));
    group.bench_with_input(BenchmarkId::from_parameter(MC_PATHS), &MC_PATHS, |b, _| {
        let as_of = date(2026, Month::January, 1);
        let market = hw1f_market(as_of);
        let inst = with_mc_paths(Snowball::example().expect("example"), |inst, overrides| {
            inst.instrument_pricing_overrides = overrides;
        });
        assert_eq!(inst.default_model(), ModelKey::MonteCarloHullWhite1F);
        assert_eq!(
            inst.instrument_pricing_overrides.model_config.mc_paths,
            Some(MC_PATHS as usize)
        );
        assert!(inst.start_date > as_of);
        assert_eq!(inst.payment_dates.len(), 5);
        support::bench(
            b,
            || {
                black_box(&inst)
                    .price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&[]),
                        PricingOptions::default(),
                    )
                    .expect("snowball HW1F Monte Carlo PV")
            },
            |result| {
                assert_usd(&result.value);
                assert!(result.measures.values().all(|value| value.is_finite()));
                capture_hw1f(
                    "snowball_hw1f_mc/2500",
                    &inst,
                    &market,
                    as_of,
                    inst.discount_curve_id.as_str(),
                    &inst.instrument_pricing_overrides.model_config,
                    result,
                );
            },
        );
    });
    group.finish();
}

fn bench_tarn_mc(c: &mut Criterion) {
    let mut group = c.benchmark_group("tarn_hw1f_mc");
    group.throughput(Throughput::Elements(MC_PATHS));
    group.bench_with_input(BenchmarkId::from_parameter(MC_PATHS), &MC_PATHS, |b, _| {
        let as_of = date(2026, Month::January, 1);
        let market = hw1f_market(as_of);
        let inst = with_mc_paths(Tarn::example().expect("example"), |inst, overrides| {
            inst.instrument_pricing_overrides = overrides;
        });
        assert_eq!(inst.default_model(), ModelKey::MonteCarloHullWhite1F);
        assert_eq!(
            inst.instrument_pricing_overrides.model_config.mc_paths,
            Some(MC_PATHS as usize)
        );
        assert!(inst.start_date > as_of);
        assert_eq!(inst.payment_dates.len(), 9);
        support::bench(
            b,
            || {
                black_box(&inst)
                    .price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&[]),
                        PricingOptions::default(),
                    )
                    .expect("TARN HW1F Monte Carlo PV")
            },
            |result| {
                assert_usd(&result.value);
                assert!(result.measures.values().all(|value| value.is_finite()));
                capture_hw1f(
                    "tarn_hw1f_mc/2500",
                    &inst,
                    &market,
                    as_of,
                    inst.discount_curve_id.as_str(),
                    &inst.instrument_pricing_overrides.model_config,
                    result,
                );
            },
        );
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_range_accrual_analytic,
    bench_quanto_range_accrual,
    bench_callable_range_accrual_lsmc,
    bench_snowball_mc,
    bench_tarn_mc,
);
criterion_main!(benches);
