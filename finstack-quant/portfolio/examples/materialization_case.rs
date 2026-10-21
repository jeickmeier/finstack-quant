//! Dedicated single-case process for manual materialization memory profiling.

use std::env;
use std::fs;

use finstack_quant_core::contract::LoadLimits;
use finstack_quant_portfolio::{materialization::InstrumentArtifactCache, Portfolio};

fn main() {
    let mut args = env::args().skip(1);
    let case = args
        .next()
        .expect("usage: materialization_case <cold|warm|validation> <fixture> <capacity>");
    let fixture_path = args
        .next()
        .expect("usage: materialization_case <cold|warm|validation> <fixture> <capacity>");
    let capacity = args
        .next()
        .expect("missing capacity")
        .parse::<usize>()
        .expect("capacity must be an integer");
    assert!(args.next().is_none(), "unexpected extra arguments");

    let bytes = fs::read(fixture_path).expect("read pre-generated fixture");
    let limits = LoadLimits::default();
    let cache = InstrumentArtifactCache::with_capacity(capacity);
    if case == "warm" {
        Portfolio::from_materialization(&bytes, &cache, &limits).expect("warm cache setup");
    }

    let report = match case.as_str() {
        "cold" | "warm" => {
            let (portfolio, report) =
                Portfolio::from_materialization(&bytes, &cache, &limits).expect("materialize");
            std::hint::black_box(portfolio);
            report
        }
        "validation" => {
            Portfolio::validate_materialization(&bytes, &cache, &limits).expect("validate")
        }
        other => panic!("unsupported case '{other}'; expected cold, warm, or validation"),
    };
    println!(
        "case={case} input_bytes={} unique_artifacts={} positions={} dependencies={} cache_hits={}",
        report.input_bytes,
        report.unique_instruments,
        report.positions,
        report.dependencies,
        report.cache_hits
    );
}
