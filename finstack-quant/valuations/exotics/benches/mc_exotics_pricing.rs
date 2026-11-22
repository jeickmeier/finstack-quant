//! Monte Carlo, barrier PDE, and analytical barrier FD Greeks exotic pricing benchmarks.
//!
//! `AsianOption`'s `Instrument::value` uses Turnbull–Wakeman for arithmetic averages; the Asian group
//! selects `ModelKey::MonteCarloGBM` through the `EXOTICS` family registry so timings reflect
//! configured Monte Carlo paths.
//!
//! Barrier cases price total at-hit rebates under continuous and quarterly monitoring, plus
//! analytical barrier FD Greeks (`barrier_option_fd_greeks_5_metrics`).
//!
//! Equity path-dependent MC benchmarks (`autocallable_mc`, `cliquet_option_mc`) live in
//! `finstack-quant-valuations-equity`'s `equity_exotics_pricing` target.

#![allow(clippy::unwrap_used)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::{
    VolGridOpts, VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::math::time_grid::TimeGrid;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{
    Attributes, BarrierType, CurveId, InstrumentId, PayoutTiming, PriceId,
};
use finstack_quant_models::monte_carlo::engine::PathCaptureMode;
use finstack_quant_models::monte_carlo::payoff::asian::AveragingMethod;
use finstack_quant_models::monte_carlo::pricer::path_dependent::PathDependentPricerConfig;
use finstack_quant_models::types::OptionType;
use finstack_quant_valuations_core::instruments::pricing::monte_carlo::merged_path_config;
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations_core::instruments::{Instrument, Monitoring, PricingOptions};
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_core::pricer::{InstrumentType, ModelKey, PricerKey, PricerRegistry};
use finstack_quant_valuations_exotics::EXOTICS;
use finstack_quant_valuations_exotics::exotics::asian_option::AsianOption;
use finstack_quant_valuations_exotics::exotics::barrier_option::BarrierOption;
use finstack_quant_valuations_exotics::exotics::lookback_option::{LookbackOption, LookbackType};
use std::hint::black_box;
use time::Month;

use finstack_quant_valuations_test_support::volatility as volatility_support;

#[path = "../../benches/support/record.rs"]
mod record;
#[path = "../../benches/support/mod.rs"]
mod support;

fn exotics_pricer_registry() -> PricerRegistry {
    let mut registry = PricerRegistry::new();
    EXOTICS
        .register_pricers(&mut registry)
        .expect("register exotics pricers");
    registry
}

fn assert_usd_value(value: &Money) {
    assert!(value.amount().is_finite());
    assert_eq!(value.currency(), Currency::USD);
}

fn path_config(
    overrides: &InstrumentPricingOverrides,
    instrument_id: &InstrumentId,
    antithetic_default: bool,
) -> PathDependentPricerConfig {
    let mut config = merged_path_config(
        &PathDependentPricerConfig::default().with_antithetic(antithetic_default),
        overrides,
    )
    .expect("valid MC benchmark configuration");
    config.seed = overrides.model_config.mc_seed(instrument_id);
    config
}

fn path_config_fields(config: &PathDependentPricerConfig) -> serde_json::Value {
    let capture_mode = match &config.path_capture.capture_mode {
        PathCaptureMode::All => serde_json::json!({"mode": "all"}),
        PathCaptureMode::Sample { count, seed } => {
            serde_json::json!({"mode": "sample", "count": count, "seed": seed})
        }
    };
    serde_json::json!({
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
    })
}

fn grid_fields(grid: &TimeGrid) -> serde_json::Value {
    serde_json::json!({
        "source": "reconstructed with the pricing grid builder outside timing",
        "actual_steps": grid.num_steps(),
        "times_bits": grid.times().iter().map(|time| time.to_bits()).collect::<Vec<_>>(),
    })
}

fn future_times(dates: &[Date], day_count: DayCount, as_of: Date) -> Vec<f64> {
    dates
        .iter()
        .copied()
        .filter(|date| *date > as_of)
        .map(|date| {
            day_count
                .year_fraction(as_of, date, DayCountContext::default())
                .expect("event time")
        })
        .collect()
}

fn mc_fixture_fields<I: serde::Serialize>(
    instrument: &I,
    market: &MarketContext,
    as_of: Date,
    model: ModelKey,
    config: &PathDependentPricerConfig,
    grid: &TimeGrid,
) -> serde_json::Value {
    serde_json::json!({
        "instrument": instrument,
        "market": market,
        "as_of": as_of.to_string(),
        "model": model,
        "requested_metrics": [],
        "mc_config": path_config_fields(config),
        "seed": config.seed,
        "grid": grid_fields(grid),
        "estimator_paths": config.num_paths,
        "physical_paths": config.num_paths * if config.antithetic { 2 } else { 1 },
    })
}

fn as_of() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

/// Equity MC context: USD-OIS discount, flat vol surface, spot scalar (see `tests/metrics/determinism.rs`).
fn create_mc_market(as_of: Date, spot: f64, vol: f64, rate: f64) -> MarketContext {
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0_f64, 1.0_f64),
            (1.0_f64, (-rate).exp()),
            (2.0_f64, (-rate * 2.0_f64).exp()),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let vol_surface = volatility_support::flat_vol_surface(
        "SPOT_VOL",
        &[0.5_f64, 1.0, 2.0],
        &[80.0_f64, 100.0, 120.0],
        vol,
    );

    MarketContext::new()
        .try_insert(disc_curve)
        .expect("insert market curve")
        .insert_surface(vol_surface)
        .insert_price("SPOT", MarketScalar::unitless(spot).expect("valid scalar"))
        .insert_price(
            "SPOT_DIV",
            MarketScalar::unitless(0.0).expect("valid scalar"),
        )
}

fn asian_option(mc_paths: usize) -> AsianOption {
    let expiry = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    AsianOption {
        id: "ASIAN-BENCH".into(),
        underlying_ticker: "SPOT".to_string(),
        spot_id: "SPOT".into(),
        strike: 100.0,
        option_type: OptionType::Call,
        expiry,
        quantity: 1.0,
        currency: Currency::USD,
        averaging_method: AveragingMethod::Arithmetic,
        fixing_dates: vec![
            Date::from_calendar_date(2025, Month::July, 1).unwrap(),
            expiry,
        ],
        day_count: DayCount::Act365F,
        discount_curve_id: "USD-OIS".into(),
        vol_surface_id: "SPOT_VOL".into(),
        div_yield_id: None,
        instrument_pricing_overrides: InstrumentPricingOverrides::default().with_mc_paths(mc_paths),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
        past_fixings: vec![],
    }
}

fn lookback_option(mc_paths: usize) -> LookbackOption {
    let expiry = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    LookbackOption::builder()
        .id("LOOKBACK-BENCH".into())
        .underlying_ticker("SPOT".to_string())
        .strike_opt(Some(100.0))
        .option_type(OptionType::Call)
        .lookback_type(LookbackType::FixedStrike)
        .expiry(expiry)
        .quantity(1.0)
        .currency(Currency::USD)
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id("SPOT".into())
        .vol_surface_id(CurveId::new("SPOT_VOL"))
        .div_yield_id_opt(Some(PriceId::new("SPOT_DIV")))
        .monitoring(Monitoring::Discrete {
            // Monthly contractual observations through expiry.
            observation_dates: (2..=12)
                .map(|month| {
                    Date::from_calendar_date(2025, Month::try_from(month).unwrap(), 1).unwrap()
                })
                .chain(std::iter::once(expiry))
                .collect(),
        })
        .instrument_pricing_overrides(InstrumentPricingOverrides::default().with_mc_paths(mc_paths))
        .attributes(Attributes::new())
        .observed_max_opt(None)
        .build()
        .unwrap()
}

fn bench_asian_option_mc(c: &mut Criterion) {
    let mut group = c.benchmark_group("asian_option_mc");
    // Register the 10,000-path/12-fixing pivot only once across the two axes.
    for (name, paths, fixing_count, seasoned, expected_steps) in [
        ("2500", 2_500_usize, 2, false, 253),
        ("paths_2500_fixings_12", 2_500, 12, false, 263),
        ("paths_10000_fixings_12", 10_000, 12, false, 263),
        ("paths_50000_fixings_12", 50_000, 12, false, 263),
        ("paths_10000_fixings_2", 10_000, 2, false, 253),
        ("paths_10000_fixings_52", 10_000, 52, false, 303),
        (
            "seasoned_paths_10000_fixings_12_past_6",
            10_000,
            12,
            true,
            132,
        ),
    ] {
        group.throughput(Throughput::Elements(paths as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), &paths, |b, &paths| {
            let as_of = if seasoned {
                Date::from_calendar_date(2025, Month::July, 1).expect("seasoned valuation date")
            } else {
                as_of()
            };
            let market = create_mc_market(as_of, 100.0, 0.25, 0.05);
            let mut option = asian_option(paths);
            if fixing_count == 12 {
                option.fixing_dates = (2..=12)
                    .map(|month| Date::from_calendar_date(2025, Month::try_from(month).unwrap(), 1).unwrap())
                    .chain(std::iter::once(option.expiry))
                    .collect();
            } else if fixing_count == 52 {
                option.fixing_dates = (1..=52)
                    .map(|i| as_of + time::Duration::days(365 * i / 52))
                    .collect();
            }
            if seasoned {
                option.past_fixings = option.fixing_dates.iter().copied().take(6)
                    .zip([98.0, 101.0, 99.0, 102.0, 103.0, 100.0])
                    .collect();
            }
            let registry = exotics_pricer_registry();
            assert_eq!(option.instrument_pricing_overrides.model_config.mc_paths, Some(paths));
            assert_eq!(option.fixing_dates.len(), fixing_count);
            assert_eq!(option.past_fixings.len(), if seasoned { 6 } else { 0 });
            support::bench(
                b,
                || {
                    registry
                        .price_with_metrics(
                            &option,
                            ModelKey::MonteCarloGBM,
                            black_box(&market),
                            black_box(as_of),
                            &[],
                            PricingOptions::default(),
                        )
                        .expect("Asian Monte Carlo price")
                },
                |result| {
                    assert_usd_value(&result.value);
                    assert_eq!(result.provenance.as_ref().expect("pricing provenance").model, ModelKey::MonteCarloGBM);
                    record::capture(&format!("asian_option_mc/{name}"), || {
                        let config = path_config(&option.instrument_pricing_overrides, &option.id, false);
                        let maturity = option.day_count.year_fraction(as_of, option.expiry, DayCountContext::default()).expect("Asian maturity");
                        let required_times = future_times(&option.fixing_dates, option.day_count, as_of);
                        let base_steps = ((maturity * config.steps_per_year).round() as usize).max(config.min_steps);
                        let grid = TimeGrid::uniform_with_required_times(maturity, base_steps, &required_times).expect("Asian fixing grid");
                        assert_eq!(grid.num_steps(), expected_steps);
                        let fixing_steps: Vec<usize> = required_times.iter().map(|required| {
                            grid.get_index(*required).expect("exact fixing step")
                        }).collect();
                        serde_json::json!({
                            "fixture": {
                                "instrument": option,
                                "market": market,
                                "as_of": as_of.to_string(),
                                "model": ModelKey::MonteCarloGBM,
                                "requested_metrics": [],
                                "configured_path_settings": path_config_fields(&config),
                                "seed": config.seed,
                                "base_steps": base_steps,
                                "grid": grid_fields(&grid),
                                "fixing_steps": fixing_steps,
                                "estimator_paths": config.num_paths,
                                "physical_paths_per_payoff": config.num_paths * if config.antithetic { 2 } else { 1 },
                            },
                            "output": {
                                "valuation": result,
                                "implementation_workload": {
                                    "payoff_passes": 1,
                                    "physical_path_traversals": config.num_paths,
                                    "effective_path_capture": {"enabled": false, "capture_mode": "all", "capture_payoffs": false},
                                },
                                "paired_moments": null,
                                "standard_error": null,
                                "diagnostic_status": "Asian public pricing exposes PV only; paired moments and adjusted SE are not included in this record and require the separate untimed test-only panel",
                            },
                        })
                    });
                },
            );
        });
    }
    group.finish();
}

fn bench_lookback_option_mc(c: &mut Criterion) {
    let mut group = c.benchmark_group("lookback_option_mc");
    let as_of = as_of();
    let paths = 2_500_u64;
    group.throughput(Throughput::Elements(paths));
    group.bench_with_input(BenchmarkId::from_parameter(paths), &paths, |b, &paths| {
        let market = create_mc_market(as_of, 100.0, 0.25, 0.05);
        let option = lookback_option(paths as usize);
        assert_eq!(option.default_model(), ModelKey::MonteCarloGBM);
        assert_eq!(
            option.instrument_pricing_overrides.model_config.mc_paths,
            Some(paths as usize)
        );
        support::bench(
            b,
            || {
                option
                    .value(black_box(&market), black_box(as_of))
                    .expect("lookback Monte Carlo price")
            },
            |value| {
                assert_usd_value(value);
                record::capture(&format!("lookback_option_mc/{paths}"), || {
                    let config = path_config(&option.instrument_pricing_overrides, &option.id, false);
                    let maturity = option.day_count.year_fraction(as_of, option.expiry, DayCountContext::default()).expect("lookback maturity");
                    let (grid, _) = option.monitoring.time_grid(as_of, option.day_count, None, maturity, &config).expect("lookback monitoring grid");
                    serde_json::json!({
                        "fixture": mc_fixture_fields(&option, &market, as_of, ModelKey::MonteCarloGBM, &config, &grid),
                        "output": {"value": value, "standard_error": null, "diagnostic_status": "Instrument::value returns PV only"},
                    })
                });
            },
        );
    });
    group.finish();
}

fn bench_barrier_monitoring(c: &mut Criterion) {
    let as_of = as_of();
    let mut group = c.benchmark_group("barrier_monitoring");
    for (monitoring_name, monitoring) in [
        ("continuous", Monitoring::Continuous),
        (
            "quarterly",
            Monitoring::Discrete {
                observation_dates: [91, 182, 273, 365]
                    .map(|days| as_of + time::Duration::days(days))
                    .to_vec(),
            },
        ),
    ] {
        for (model_name, model) in [
            ("gbm_2500_paths", ModelKey::MonteCarloGBM),
            ("heston_2500_paths", ModelKey::MonteCarloHeston),
            ("pde", ModelKey::PdeCrankNicolson1D),
        ] {
            group.bench_function(format!("{monitoring_name}/{model_name}"), |b| {
                let market = create_mc_market(as_of, 100.0, 0.25, 0.05)
                    .insert_price("HESTON_KAPPA", MarketScalar::unitless(2.0).expect("valid scalar"))
                    .insert_price("HESTON_THETA", MarketScalar::unitless(0.0625).expect("valid scalar"))
                    .insert_price("HESTON_SIGMA_V", MarketScalar::unitless(0.3).expect("valid scalar"))
                    .insert_price("HESTON_RHO", MarketScalar::unitless(-0.5).expect("valid scalar"))
                    .insert_price("HESTON_V0", MarketScalar::unitless(0.0625).expect("valid scalar"));
                let mut option = BarrierOption::example().unwrap();
                option.expiry = as_of + time::Duration::days(365);
                option.strike = 100.0;
                option.barrier = 120.0;
                option.barrier_type = BarrierType::UpAndOut;
                option.quantity = 1_000.0;
                option.rebate = Some(Money::from((25_i64, Currency::USD)));
                option.rebate_timing = Some(PayoutTiming::AtHit);
                option.spot_id = "SPOT".into();
                option.vol_surface_id = "SPOT_VOL".into();
                option.div_yield_id = Some("SPOT_DIV".into());
                option.instrument_pricing_overrides =
                    InstrumentPricingOverrides::default().with_mc_paths(2_500);
                option.monitoring = monitoring.clone();
                let registry = exotics_pricer_registry();
                let key = PricerKey::new(InstrumentType::BarrierOption, model);
                let pricer = registry.get_pricer(key).expect("barrier model available");
                assert_eq!(pricer.key(), key);
                support::bench(
                    b,
                    || {
                        pricer
                            .price_dyn(black_box(&option), black_box(&market), black_box(as_of))
                            .expect("barrier model price")
                    },
                    |result| {
                        assert_usd_value(&result.value);
                        record::capture(&format!("barrier_monitoring/{monitoring_name}/{model_name}"), || {
                            let maturity = option.day_count.year_fraction(as_of, option.expiry, DayCountContext::default()).expect("barrier maturity");
                            let fixture = match model {
                                ModelKey::MonteCarloGBM => {
                                    let config = path_config(&option.instrument_pricing_overrides, &option.id, false);
                                    let (grid, _) = option.monitoring.time_grid(as_of, option.day_count, None, maturity, &config).expect("GBM barrier grid");
                                    mc_fixture_fields(&option, &market, as_of, model, &config, &grid)
                                }
                                ModelKey::MonteCarloHeston => {
                                    // This model constructs McEngineConfig directly and has a
                                    // ten-step floor, independent of PathDependentPricer defaults.
                                    let grid_config = PathDependentPricerConfig {
                                        steps_per_year: 252.0,
                                        min_steps: 10,
                                        ..Default::default()
                                    };
                                    let (grid, _) = option.monitoring.time_grid(as_of, option.day_count, None, maturity, &grid_config).expect("Heston barrier grid");
                                    let engine = finstack_quant_models::monte_carlo::engine::McEngineConfig::new(2_500, grid);
                                    serde_json::json!({
                                        "instrument": option,
                                        "market": market,
                                        "as_of": as_of.to_string(),
                                        "model": model,
                                        "requested_metrics": [],
                                        "config": {
                                            "steps_per_year": grid_config.steps_per_year,
                                            "min_steps": grid_config.min_steps,
                                            "num_paths": engine.num_paths,
                                            "use_parallel": engine.use_parallel,
                                            "chunk_size": engine.chunk_size,
                                            "target_ci_half_width": engine.target_ci_half_width,
                                            "antithetic": engine.antithetic,
                                            "path_capture_enabled": engine.path_capture.enabled,
                                            "path_capture_mode": "all",
                                            "capture_payoffs": engine.path_capture.capture_payoffs,
                                            "discretization": "qe_heston",
                                        },
                                        "seed": option.instrument_pricing_overrides.model_config.mc_seed(&option.id),
                                        "grid": grid_fields(&engine.time_grid),
                                        "estimator_paths": engine.num_paths,
                                        "physical_paths": engine.num_paths * if engine.antithetic { 2 } else { 1 },
                                    })
                                }
                                ModelKey::PdeCrankNicolson1D => serde_json::json!({
                                    "instrument": option,
                                    "market": market,
                                    "as_of": as_of.to_string(),
                                    "model": model,
                                    "requested_metrics": [],
                                    "config": {"space_points": 200, "time_steps": 100, "rannacher_implicit_steps": 2},
                                    "grid": null,
                                    "grid_status": "private PDE mesh is not exposed; full inputs and default mesh configuration are retained",
                                    "estimator_paths": null,
                                    "physical_paths": null,
                                }),
                                _ => unreachable!("only the three benchmarked barrier models are recorded"),
                            };
                            serde_json::json!({"fixture": fixture, "output": {"valuation": result}})
                        });
                    },
                );
            });
        }
    }
    group.finish();
}

fn create_barrier_fd_option() -> BarrierOption {
    let as_of = as_of();
    BarrierOption {
        expiry_fixing: None,
        id: "BARRIER-12M".into(),
        underlying_ticker: "SPX".into(),
        strike: 100.0,
        barrier: 80.0,
        rebate: None,
        rebate_timing: None,
        option_type: OptionType::Call,
        barrier_type: BarrierType::DownAndOut,
        expiry: as_of + time::Duration::days(365),
        observed_barrier_breached: None,
        quantity: 100.0,
        currency: Currency::USD,
        day_count: DayCount::Act365F,
        monitoring: Monitoring::Continuous,
        discount_curve_id: "USD_DISC".into(),
        spot_id: "SPX".into(),
        vol_surface_id: "SPX_VOL".into(),
        div_yield_id: Some("SPX_DIV".into()),
        instrument_pricing_overrides: InstrumentPricingOverrides::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

fn create_barrier_fd_market() -> MarketContext {
    let as_of = as_of();
    let disc = DiscountCurve::builder("USD_DISC")
        .base_date(as_of)
        .knots([
            (0.0, 1.0),
            (0.5, 0.98),
            (1.0, 0.96),
            (2.0, 0.92),
            (5.0, 0.80),
        ])
        .interp(InterpStyle::Linear)
        .day_count(DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let vol_surface = VolSurface::from_grid(
        "SPX_VOL",
        &[0.25, 0.5, 1.0, 2.0],
        &[80.0, 90.0, 100.0, 110.0, 120.0],
        &[0.25; 20],
        VolGridOpts {
            secondary_axis: VolSurfaceAxis::Strike,
            quote_type: VolQuoteType::BlackLognormal,
            interpolation_mode: VolInterpolationMode::Vol,
        },
        None,
    )
    .unwrap();

    MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .insert_surface(vol_surface)
        .insert_price(
            "SPX",
            MarketScalar::price(Money::new(100.0, Currency::USD).expect("valid money fixture")),
        )
        .insert_price(
            "SPX_DIV",
            MarketScalar::unitless(0.02).expect("valid scalar"),
        )
}

fn bench_barrier_option_fd_greeks(c: &mut Criterion) {
    c.bench_function("barrier_option_fd_greeks_5_metrics", |b| {
        let market = create_barrier_fd_market();
        let option = create_barrier_fd_option();
        let as_of = as_of();
        let greeks = [
            MetricId::Delta,
            MetricId::Gamma,
            MetricId::Vega,
            MetricId::Vanna,
            MetricId::Volga,
        ];
        assert_eq!(option.default_model(), ModelKey::BarrierBSContinuous);
        assert!(option.expiry > as_of);
        assert!(option.barrier < 100.0);
        support::bench(
            b,
            || {
                option
                    .price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&greeks),
                        PricingOptions::default(),
                    )
                    .expect("barrier option finite-difference Greeks")
            },
            |result| {
                assert_eq!(result.value.currency(), Currency::USD);
                assert!(result.value.amount().is_finite());
                for metric in &greeks {
                    assert!(
                        result
                            .measures
                            .get(metric)
                            .expect("requested Greek")
                            .is_finite()
                    );
                }
            },
        );
    });
}

criterion_group!(
    benches,
    bench_asian_option_mc,
    bench_lookback_option_mc,
    bench_barrier_monitoring,
    bench_barrier_option_fd_greeks,
);
criterion_main!(benches);
