#![cfg(test)]

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_valuations::instruments::credit_derivatives::cds::PayReceive;
use finstack_valuations::instruments::credit_derivatives::cds_index::{
    CDSIndex, CDSIndexConstituent, CDSIndexParams,
};
use finstack_valuations::instruments::CreditParams;
use finstack_valuations::instruments::Instrument;
use time::Month;

use crate::common::test_helpers::flat_discount_curve;

fn flat_discount(
    id: &'static str,
    base: Date,
) -> finstack_core::market_data::term_structures::DiscountCurve {
    // Use a very small rate to create a nearly flat but valid (decreasing) curve
    flat_discount_curve(0.0001, base, id)
}

fn flat_hazard(
    id: &'static str,
    base: Date,
    rec: f64,
    hz: f64,
) -> finstack_core::market_data::term_structures::HazardCurve {
    use finstack_core::market_data::term_structures::HazardCurve;
    HazardCurve::builder(id)
        .base_date(base)
        .recovery_rate(rec)
        .knots(vec![(1.0, hz), (10.0, hz)])
        .par_spreads(vec![(1.0, hz * 10000.0 * (1.0 - rec))])
        .build()
        .unwrap()
}

#[test]
fn imm_20th_schedule_for_index_synthetic() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let start = as_of;
    let end = Date::from_calendar_date(2028, Month::January, 1).unwrap();

    let idx = CDSIndex::from_preset(
        &CDSIndexParams::cdx_na_ig(42, 1, 100.0),
        "CDX-IMM",
        Money::new(1_000_000.0, Currency::USD),
        PayReceive::Pay,
        start,
        end,
        0.40,
        "USD-OIS",
        "HZ-IDX",
    )
    .expect("valid test parameters");
    let cds = idx.to_synthetic_cds();
    let schedule_dates = cds.isda_coupon_schedule().unwrap();
    // Internal coupon dates (excluding first and last) should be near the 20th.
    for d in schedule_dates
        .iter()
        .skip(1)
        .take(schedule_dates.len().saturating_sub(2))
    {
        assert!(
            (18..=23).contains(&d.day()),
            "ISDA coupon dates should be near the 20th (got day {})",
            d.day()
        );
    }
}

#[test]
fn index_factor_scales_pv() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let start = as_of;
    let end = Date::from_calendar_date(2028, Month::January, 1).unwrap();

    let disc = flat_discount("USD-OIS", as_of);
    let rec = finstack_valuations::constants::isda::STANDARD_RECOVERY_SENIOR;
    let hz = 0.02;
    let names = [
        ("N1", "HZ1"),
        ("N2", "HZ2"),
        ("N3", "HZ3"),
        ("N4", "HZ4"),
        ("N5", "HZ5"),
    ];
    let mut ctx = MarketContext::new().insert(disc);
    for (_, hid) in names.iter() {
        ctx = ctx.insert(flat_hazard(hid, as_of, rec, hz));
    }

    let cons: Vec<CDSIndexConstituent> = names
        .iter()
        .map(|(n, hid)| {
            CDSIndexConstituent::active(CreditParams::corporate_standard(*n, *hid), 1.0 / 5.0)
        })
        .collect();

    let preset = CDSIndexParams::cdx_na_ig(42, 1, 100.0);

    let idx_base = CDSIndex::from_preset(
        &preset,
        "CDX-BASE",
        Money::new(10_000_000.0, Currency::USD),
        PayReceive::Pay,
        start,
        end,
        rec,
        "USD-OIS",
        "HZ-IDX",
    )
    .expect("valid test parameters")
    .with_constituents(cons.clone());

    let idx_scaled = CDSIndex::from_preset(
        &preset,
        "CDX-SCALED",
        Money::new(10_000_000.0, Currency::USD),
        PayReceive::Pay,
        start,
        end,
        rec,
        "USD-OIS",
        "HZ-IDX",
    )
    .expect("valid test parameters")
    .with_index_factor(0.8)
    .with_constituents(cons);

    let pv_base = idx_base.value(&ctx, as_of).unwrap().amount();
    let pv_scaled = idx_scaled.value(&ctx, as_of).unwrap().amount();
    // PV should scale approximately with index_factor. Allow small numerical tolerance.
    let ratio = pv_scaled / (pv_base * 0.8);
    assert!((ratio - 1.0).abs() < 5e-6);
}
