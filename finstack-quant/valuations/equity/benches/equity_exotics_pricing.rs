//! Equity path-dependent Monte Carlo pricing benchmarks (autocallable and cliquet).

#![allow(clippy::unwrap_used)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::time_grid::TimeGrid;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{Attributes, CurveId, InstrumentId, PriceId};
use finstack_quant_models::monte_carlo::engine::PathCaptureMode;
use finstack_quant_models::monte_carlo::payoff::autocallable::FinalPayoffType;
use finstack_quant_models::monte_carlo::pricer::path_dependent::PathDependentPricerConfig;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::instruments::pricing::monte_carlo::merged_path_config;
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations_core::pricer::ModelKey;
use finstack_quant_valuations_equity::EquityPathModel;
use finstack_quant_valuations_equity::autocallable::Autocallable;
use finstack_quant_valuations_equity::cliquet_option::CliquetOption;
use finstack_quant_valuations_test_support::volatility as volatility_support;
use std::hint::black_box;
use time::Month;

#[path = "../../benches/support/record.rs"]
mod record;
#[path = "../../benches/support/mod.rs"]
mod support;

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

fn autocallable_note(mc_paths: usize) -> Autocallable {
    let observation_dates = vec![
        Date::from_calendar_date(2025, Month::April, 1).unwrap(),
        Date::from_calendar_date(2025, Month::July, 1).unwrap(),
        Date::from_calendar_date(2025, Month::October, 1).unwrap(),
        Date::from_calendar_date(2026, Month::January, 1).unwrap(),
    ];
    let n = observation_dates.len();
    Autocallable {
        id: "AUTO-BENCH".into(),
        underlying_ticker: "SPOT".into(),
        payment_dates: observation_dates.clone(),
        expiry: *observation_dates.last().unwrap(),
        observation_dates,
        autocall_barriers: vec![1.0; n],
        coupons: vec![0.02; n],
        coupon_barriers: vec![0.70; n],
        memory_coupons: false,
        final_barrier: 0.6,
        final_payoff_type: FinalPayoffType::Participation {
            participation_rate: 1.0,
        },
        cap_level: 1.5,
        notional: Money::new(100_000.0, Currency::USD).expect("valid money fixture"),
        day_count: DayCount::Act365F,
        discount_curve_id: CurveId::new("USD-OIS"),
        spot_id: "SPOT".into(),
        vol_surface_id: CurveId::new("SPOT_VOL"),
        path_model: EquityPathModel::AtmTermGbm,
        div_yield_id: Some(PriceId::new("SPOT_DIV")),
        initial_level: None,
        past_fixings: vec![],
        instrument_pricing_overrides: InstrumentPricingOverrides::default().with_mc_paths(mc_paths),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
    }
}

fn bench_autocallable_mc(c: &mut Criterion) {
    let mut group = c.benchmark_group("autocallable_mc");
    let as_of = as_of();
    let paths = 2_500_u64;
    group.throughput(Throughput::Elements(paths));
    group.bench_with_input(BenchmarkId::from_parameter(paths), &paths, |b, &paths| {
        let market = create_mc_market(as_of, 100.0, 0.25, 0.05);
        let note = autocallable_note(paths as usize);
        assert_eq!(note.default_model(), ModelKey::MonteCarloGBM);
        assert!(matches!(note.path_model, EquityPathModel::AtmTermGbm));
        assert_eq!(
            note.instrument_pricing_overrides.model_config.mc_paths,
            Some(paths as usize)
        );
        support::bench(
            b,
            || {
                note.value(black_box(&market), black_box(as_of))
                    .expect("autocallable Monte Carlo price")
            },
            |value| {
                assert_usd_value(value);
                record::capture(&format!("autocallable_mc/{paths}"), || {
                    let config = path_config(&note.instrument_pricing_overrides, &note.id, true);
                    let maturity = note
                        .day_count
                        .year_fraction(as_of, note.expiry, DayCountContext::default())
                        .expect("autocallable maturity");
                    let required_times = future_times(&note.observation_dates, note.day_count, as_of);
                    let grid = config
                        .build_time_grid(maturity, &required_times)
                        .expect("autocallable observation grid");
                    serde_json::json!({
                        "fixture": mc_fixture_fields(&note, &market, as_of, ModelKey::MonteCarloGBM, &config, &grid),
                        "output": {"value": value, "standard_error": null, "diagnostic_status": "Instrument::value returns PV only"},
                    })
                });
            },
        );
    });
    group.finish();
}

fn make_cliquet(as_of: Date, n_resets: usize) -> CliquetOption {
    let reset_dates: Vec<Date> = (1..=n_resets)
        .map(|i| {
            let days = (365 * i / n_resets) as i64;
            as_of + time::Duration::days(days)
        })
        .collect();
    let expiry = *reset_dates.last().unwrap();

    CliquetOption::builder()
        .id(InstrumentId::new("CLIQ-BENCH"))
        .underlying_ticker("SPOT".to_string())
        .reset_dates(reset_dates)
        .expiry(expiry)
        .local_cap(0.05)
        .local_floor(0.0)
        .global_cap(0.20)
        .global_floor(0.0)
        .notional(Money::new(100_000.0, Currency::USD).expect("valid money fixture"))
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id("SPOT".into())
        .vol_surface_id(CurveId::new("SPOT_VOL"))
        .path_model(EquityPathModel::AtmTermGbm)
        .div_yield_id_opt(Some(PriceId::new("SPOT_DIV")))
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .unwrap()
}

fn bench_cliquet_option_mc(c: &mut Criterion) {
    let mut group = c.benchmark_group("cliquet_option_mc");
    let as_of = as_of();
    let n_resets = 12;
    group.bench_with_input(
        BenchmarkId::from_parameter(format!("{n_resets}resets")),
        &n_resets,
        |b, &n_resets| {
            let market = create_mc_market(as_of, 100.0, 0.25, 0.05);
            let option = make_cliquet(as_of, n_resets);
            assert_eq!(option.default_model(), ModelKey::MonteCarloGBM);
            assert!(matches!(option.path_model, EquityPathModel::AtmTermGbm));
            assert_eq!(option.reset_dates.len(), n_resets);
            support::bench(
                b,
                || {
                    option
                        .value(black_box(&market), black_box(as_of))
                        .expect("cliquet Monte Carlo price")
                },
                |value| {
                    assert_usd_value(value);
                    record::capture(&format!("cliquet_option_mc/{n_resets}resets"), || {
                        let config =
                            path_config(&option.instrument_pricing_overrides, &option.id, true);
                        let maturity = option
                            .day_count
                            .year_fraction(as_of, option.expiry, DayCountContext::default())
                            .expect("cliquet maturity");
                        let required_times =
                            future_times(&option.reset_dates, option.day_count, as_of);
                        let grid = config
                            .build_time_grid(maturity, &required_times)
                            .expect("cliquet reset grid");
                        serde_json::json!({
                            "fixture": mc_fixture_fields(&option, &market, as_of, ModelKey::MonteCarloGBM, &config, &grid),
                            "output": {"value": value, "standard_error": null, "diagnostic_status": "Instrument::value returns PV only"},
                        })
                    });
                },
            );
        },
    );
    group.finish();
}

criterion_group!(benches, bench_autocallable_mc, bench_cliquet_option_mc);
criterion_main!(benches);
