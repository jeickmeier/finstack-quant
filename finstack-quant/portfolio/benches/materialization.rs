//! Release-mode portfolio materialization benchmarks and acceptance probes.

mod materialization_fixtures;
mod materialization_gate;

use std::env;
use std::hint::black_box;
use std::path::Path;
use std::time::Instant;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use finstack_quant_core::contract::LoadLimits;
use finstack_quant_portfolio::{
    materialization::InstrumentArtifactCache, materialization::MaterializationReport, Portfolio,
};
use materialization_fixtures::{fixture_a, fixture_b, BenchmarkFixture};
use materialization_gate::{evaluate_latency_gates, p95_millis, parse_sample_count};

#[derive(Default)]
struct ProbeSamples {
    elapsed_millis: Vec<f64>,
    parse_millis: Vec<f64>,
    validate_millis: Vec<f64>,
    decode_millis: Vec<f64>,
    build_positions_millis: Vec<f64>,
    index_build_millis: Vec<f64>,
    last_report: Option<MaterializationReport>,
}

impl ProbeSamples {
    fn push(&mut self, elapsed_millis: f64, report: MaterializationReport) {
        self.elapsed_millis.push(elapsed_millis);
        self.parse_millis
            .push(nanos_to_millis(report.phase_nanos.parse));
        self.validate_millis
            .push(nanos_to_millis(report.phase_nanos.validate_versions));
        self.decode_millis
            .push(nanos_to_millis(report.phase_nanos.decode_instruments));
        self.build_positions_millis
            .push(nanos_to_millis(report.phase_nanos.build_positions));
        self.index_build_millis
            .push(nanos_to_millis(report.phase_nanos.index_build));
        self.last_report = Some(report);
    }

    fn print(&self, case: &str) -> f64 {
        let elapsed_p95 = percentile(&self.elapsed_millis, 95);
        let elapsed_median = percentile(&self.elapsed_millis, 50);
        let report = self
            .last_report
            .as_ref()
            .expect("probe records at least one report");
        println!(
            "materialization_probe case={case} samples={} median_ms={elapsed_median:.3} p95_ms={elapsed_p95:.3} \
             input_bytes={} unique_artifacts={} positions={} dependencies={} cache_hits={} \
             phase_p95_ms=parse:{:.3},validate_versions:{:.3},decode_instruments:{:.3},\
             build_positions:{:.3},index_build:{:.3}",
            self.elapsed_millis.len(),
            report.input_bytes,
            report.unique_instruments,
            report.positions,
            report.dependencies,
            report.cache_hits,
            percentile(&self.parse_millis, 95),
            percentile(&self.validate_millis, 95),
            percentile(&self.decode_millis, 95),
            percentile(&self.build_positions_millis, 95),
            percentile(&self.index_build_millis, 95),
        );
        elapsed_p95
    }
}

fn benchmark_materialization(criterion: &mut Criterion) {
    let fixture_a = fixture_a();
    let fixture_b = fixture_b();
    let limits = LoadLimits::default();
    let probe_samples = probe_sample_count();
    println!(
        "materialization_fixture name={} input_bytes={} unique_artifacts={} positions={} dependencies={}",
        fixture_a.name,
        fixture_a.bytes.len(),
        fixture_a.artifact_count,
        fixture_a.position_count,
        fixture_a.dependency_count,
    );
    println!(
        "materialization_fixture name={} input_bytes={} unique_artifacts={} positions={} dependencies={}",
        fixture_b.name,
        fixture_b.bytes.len(),
        fixture_b.artifact_count,
        fixture_b.position_count,
        fixture_b.dependency_count,
    );

    let cold_a = probe_cold(&fixture_a, &limits, probe_samples);
    let cold_b = probe_cold(&fixture_b, &limits, probe_samples);
    let warm_b = probe_warm(&fixture_b, &limits, probe_samples);
    let validation_b = probe_validation(&fixture_b, &limits, probe_samples);
    let cold_a_p95 = cold_a.print("cold_a_5000_unique");
    let _cold_b_p95 = cold_b.print("cold_b_5000_50");
    let warm_b_p95 = warm_b.print("warm_b_5000_50");
    validation_b.print("validation_b_5000_50");
    write_raw_samples(&cold_a, &cold_b, &warm_b, &validation_b);
    let verdict = evaluate_latency_gates(cold_a_p95, warm_b_p95);
    println!(
        "materialization_gates cold_hard={} cold_500ms_target={} warm_250ms_target={}",
        pass_miss(verdict.cold_hard_pass),
        pass_miss(verdict.cold_target_pass),
        pass_miss(verdict.warm_target_pass),
    );
    assert!(
        verdict.cold_hard_pass,
        "cold fixture A p95 {cold_a_p95:.3} ms must be below the 1000 ms hard gate"
    );

    let mut group = criterion.benchmark_group("portfolio_materialization");
    group.throughput(Throughput::Bytes(fixture_a.bytes.len() as u64));
    group.bench_function("cold_a_5000_unique", |bencher| {
        bencher.iter_batched(
            || InstrumentArtifactCache::with_capacity(fixture_a.artifact_count),
            |cache| {
                black_box(
                    Portfolio::from_materialization(&fixture_a.bytes, &cache, &limits)
                        .expect("cold fixture A materialization"),
                )
            },
            BatchSize::SmallInput,
        );
    });

    group.throughput(Throughput::Bytes(fixture_b.bytes.len() as u64));
    group.bench_function("cold_b_5000_50", |bencher| {
        bencher.iter_batched(
            || InstrumentArtifactCache::with_capacity(fixture_b.artifact_count),
            |cache| {
                black_box(
                    Portfolio::from_materialization(&fixture_b.bytes, &cache, &limits)
                        .expect("cold fixture B materialization"),
                )
            },
            BatchSize::SmallInput,
        );
    });

    let warm_cache = InstrumentArtifactCache::with_capacity(fixture_b.artifact_count);
    Portfolio::from_materialization(&fixture_b.bytes, &warm_cache, &limits)
        .expect("warm fixture B setup");
    group.bench_function("warm_b_5000_50", |bencher| {
        bencher.iter(|| {
            black_box(
                Portfolio::from_materialization(&fixture_b.bytes, &warm_cache, &limits)
                    .expect("warm fixture B materialization"),
            )
        });
    });
    group.bench_function("validation_b_5000_50", |bencher| {
        bencher.iter_batched(
            || InstrumentArtifactCache::with_capacity(fixture_b.artifact_count),
            |cache| {
                black_box(
                    Portfolio::validate_materialization(&fixture_b.bytes, &cache, &limits)
                        .expect("fixture B validation"),
                )
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn probe_cold(
    fixture: &BenchmarkFixture,
    limits: &LoadLimits,
    sample_count: usize,
) -> ProbeSamples {
    let caches: Vec<_> = (0..sample_count)
        .map(|_| InstrumentArtifactCache::with_capacity(fixture.artifact_count))
        .collect();
    let mut samples = ProbeSamples::default();
    for cache in caches {
        let started = Instant::now();
        let (_portfolio, report) = Portfolio::from_materialization(&fixture.bytes, &cache, limits)
            .expect("cold acceptance probe");
        samples.push(started.elapsed().as_secs_f64() * 1_000.0, report);
    }
    samples
}

fn probe_warm(
    fixture: &BenchmarkFixture,
    limits: &LoadLimits,
    sample_count: usize,
) -> ProbeSamples {
    let cache = InstrumentArtifactCache::with_capacity(fixture.artifact_count);
    Portfolio::from_materialization(&fixture.bytes, &cache, limits).expect("warm probe setup");
    let mut samples = ProbeSamples::default();
    for _ in 0..sample_count {
        let started = Instant::now();
        let (_portfolio, report) = Portfolio::from_materialization(&fixture.bytes, &cache, limits)
            .expect("warm acceptance probe");
        samples.push(started.elapsed().as_secs_f64() * 1_000.0, report);
    }
    samples
}

fn probe_validation(
    fixture: &BenchmarkFixture,
    limits: &LoadLimits,
    sample_count: usize,
) -> ProbeSamples {
    let caches: Vec<_> = (0..sample_count)
        .map(|_| InstrumentArtifactCache::with_capacity(fixture.artifact_count))
        .collect();
    let mut samples = ProbeSamples::default();
    for cache in caches {
        let started = Instant::now();
        let report = Portfolio::validate_materialization(&fixture.bytes, &cache, limits)
            .expect("validation acceptance probe");
        samples.push(started.elapsed().as_secs_f64() * 1_000.0, report);
    }
    samples
}

fn probe_sample_count() -> usize {
    let value = env::var("FQ_MATERIALIZATION_P95_SAMPLES").ok();
    let allow_smoke = env::var("FQ_MATERIALIZATION_SMOKE").as_deref() == Ok("1");
    parse_sample_count(value.as_deref(), allow_smoke).unwrap_or_else(|error| panic!("{error}"))
}

fn percentile(samples: &[f64], percentile: usize) -> f64 {
    let mut values = samples.to_vec();
    if percentile == 95 {
        return p95_millis(&mut values);
    }
    values.sort_by(f64::total_cmp);
    let rank = (values.len() * percentile).div_ceil(100);
    values[rank.saturating_sub(1)]
}

fn write_raw_samples(
    cold_a: &ProbeSamples,
    cold_b: &ProbeSamples,
    warm_b: &ProbeSamples,
    validation_b: &ProbeSamples,
) {
    let Ok(path) = env::var("FQ_MATERIALIZATION_RAW_OUTPUT") else {
        return;
    };
    let payload = serde_json::json!({
        "language": "rust",
        "timing_boundary": "fixture bytes and cache exist before start; elapsed surrounds only the Rust validation/materialization call and returned values",
        "cases": {
            "cold_a_5000_unique": &cold_a.elapsed_millis,
            "cold_b_5000_50": &cold_b.elapsed_millis,
            "warm_b_5000_50": &warm_b.elapsed_millis,
            "validation_b_5000_50": &validation_b.elapsed_millis,
        },
        "counters": {
            "cold_a_5000_unique": report_counters(cold_a),
            "cold_b_5000_50": report_counters(cold_b),
            "warm_b_5000_50": report_counters(warm_b),
            "validation_b_5000_50": report_counters(validation_b),
        },
    });
    if let Some(parent) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(parent).expect("create raw benchmark output directory");
    }
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&payload).expect("serialize raw benchmark samples"),
    )
    .expect("write raw benchmark samples");
}

fn report_counters(samples: &ProbeSamples) -> serde_json::Value {
    let report = samples
        .last_report
        .as_ref()
        .expect("probe records at least one report");
    serde_json::json!({
        "input_bytes": report.input_bytes,
        "unique_instruments": report.unique_instruments,
        "positions": report.positions,
        "dependencies": report.dependencies,
        "cache_hits": report.cache_hits,
        "phase_nanos": report.phase_nanos,
    })
}

fn nanos_to_millis(nanos: u64) -> f64 {
    nanos as f64 / 1_000_000.0
}

fn pass_miss(value: bool) -> &'static str {
    if value {
        "PASS"
    } else {
        "MISS"
    }
}

fn criterion_config() -> Criterion {
    let output = env::var("FQ_MATERIALIZATION_CRITERION_DIR")
        .unwrap_or_else(|_| "target/materialization-criterion/criterion".to_string());
    Criterion::default().output_directory(Path::new(&output))
}

criterion_group!(
    name = benches;
    config = criterion_config();
    targets = benchmark_materialization
);
criterion_main!(benches);
