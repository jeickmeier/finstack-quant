//! Selected-case registration for the bounded contractual-position query sweep.

#[path = "../../tests/performance/support/accrual_fixture.rs"]
mod fixture;

use criterion::{BenchmarkId, Criterion, Throughput};
use finstack_quant_cashflows::accrual::AccrualIndex;
use std::hint::black_box;

pub(super) fn register(c: &mut Criterion) {
    let mut group = c.benchmark_group("ccm_accrued_contractual_band");
    for case in fixture::cases()
        .into_iter()
        .filter(|case| !matches!(case.shape, fixture::QueryShape::Synthetic))
    {
        group.throughput(Throughput::Elements(case.periods as u64));
        group.bench_with_input(
            BenchmarkId::new(case.shape.label(), case.periods),
            &case,
            |b, &case| {
                // The history, dates and independent checks exist only for selected cases.
                let fixture = fixture::fixture(case);
                fixture::validate(case, &fixture);
                let fixture::Fixture {
                    schedule,
                    config,
                    dates,
                    expected,
                } = fixture;
                drop(expected);
                b.iter(|| {
                    let index = AccrualIndex::build(black_box(&schedule), black_box(&config))
                        .expect("timed accrual index");
                    let mut sum = 0.0;
                    for &date in &dates {
                        sum += black_box(
                            index
                                .accrued_at(black_box(date))
                                .expect("timed query")
                                .amount(),
                        );
                    }
                    black_box(sum)
                });
            },
        );
    }
    group.finish();
}
