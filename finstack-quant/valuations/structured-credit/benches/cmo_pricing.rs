//! Agency CMO waterfall pricing benchmarks.
//!
//! Covers sequential, PAC/support, and IO/PO collateralized mortgage obligation
//! waterfalls across monthly collateral cashflow projections.

#![allow(clippy::unwrap_used)]

#[path = "../../benches/support/mod.rs"]
mod support;

use criterion::{Criterion, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_fixed_income::mbs_passthrough::AgencyProgram;
use finstack_quant_valuations_structured_credit::cmo::{AgencyCmo, CmoTranche, CmoWaterfall};
use finstack_quant_valuations_test_support::discount_forward_curves as discount_forward_curve_support;
use std::hint::black_box;
use time::Month;
use time::macros::date;

fn as_of() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn check_money(value: &Money, currency: Currency) {
    assert_eq!(value.currency(), currency);
    assert!(value.amount().is_finite());
}

/// Five-tranche sequential CMO waterfall allocation.
///
/// Measures the sequential/PAC waterfall distribution over a monthly cashflow
/// projection horizon for the five-tranche fixture.
fn bench_cmo_waterfall_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("cmo_waterfall_pv");
    group.bench_function("5t", |b| {
        let as_of = as_of();
        let market = MarketContext::new()
            .try_insert(discount_forward_curve_support::flat_discount(
                "USD-OIS", as_of, 0.05,
            ))
            .expect("insert market curve");
        let n_tranches = 5;
        let tranches: Vec<CmoTranche> = (0..n_tranches)
            .map(|i| {
                // Keep total coupon demand under the ~4% collateral
                // pass-through so the deal passes interest-coverage
                // validation at build.
                let coupon = 0.03 + 0.001 * i as f64;
                let balance = 10_000_000.0;
                CmoTranche::sequential(
                    format!("T{}", i).as_str(),
                    Money::new(balance, Currency::USD).expect("valid money fixture"),
                    coupon,
                    (i + 1) as u32,
                )
            })
            .collect();

        let cmo = AgencyCmo::builder()
            .id(InstrumentId::new("CMO-BENCH"))
            .deal_name("BENCH-CMO".into())
            .agency(AgencyProgram::Fnma)
            .issue_date(date!(2024 - 01 - 01))
            .waterfall(CmoWaterfall::new(tranches))
            .reference_tranche_id("T0".to_string())
            .collateral_wac(0.045)
            .collateral_wam_months(360)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .unwrap();

        assert_eq!(cmo.waterfall.tranches.len(), n_tranches);
        support::bench(
            b,
            || {
                cmo.value(black_box(&market), black_box(as_of))
                    .expect("CMO waterfall PV")
            },
            |value| check_money(value, Currency::USD),
        );
    });
    group.finish();
}

/// Compare CMO structure types: sequential, PAC/support, IO/PO.
fn bench_cmo_structure_type(c: &mut Criterion) {
    let mut group = c.benchmark_group("cmo_structure_type");
    for (label, fixture) in [
        (
            "sequential",
            AgencyCmo::example as fn() -> finstack_quant_core::Result<AgencyCmo>,
        ),
        ("pac_support", AgencyCmo::example_pac_support),
        ("io_po", AgencyCmo::example_io_po),
    ] {
        group.bench_function(label, |b| {
            let as_of = as_of();
            let market = MarketContext::new()
                .try_insert(discount_forward_curve_support::flat_discount(
                    "USD-OIS", as_of, 0.05,
                ))
                .expect("insert market curve");
            let cmo = fixture().expect("CMO fixture");
            assert!(!cmo.waterfall.tranches.is_empty());
            support::bench(
                b,
                || {
                    cmo.value(black_box(&market), black_box(as_of))
                        .expect(label)
                },
                |value| check_money(value, Currency::USD),
            );
        });
    }
    group.finish();
}

criterion_group!(benches, bench_cmo_waterfall_pv, bench_cmo_structure_type);
criterion_main!(benches);
