//! Bond `recovery01` on a hazard curve carrying its calibration recipe.
//!
//! The curve is re-bootstrapped to the same par spreads at the bumped recovery,
//! as the CDS metric does. Hazard rates rise with recovery (`h ≈ S / (1 − R)`),
//! which offsets most of the gain from recovering more.

use crate::instruments::test_support::credit::{
    CalibratedHazardSpec, calibrated_hazard_curve_from_spec, pricing_options,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

const AS_OF: Date = date!(2025 - 01 - 01);

fn bond() -> Bond {
    bond_with_coupon(0.05)
}

fn bond_with_coupon(coupon: f64) -> Bond {
    let mut bond = Bond::fixed(
        "RECOVERY-RECAL",
        Money::new(1_000_000.0, Currency::USD).expect("money"),
        finstack_quant_core::types::Rate::from_decimal(coupon).expect("rate"),
        AS_OF,
        date!(2030 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.settlement_convention = None;
    bond.credit_curve_id = Some(CurveId::new("USD-CREDIT"));
    bond
}

fn discount_only() -> MarketContext {
    MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(AS_OF)
            .knots([(0.0, 1.0), (5.0, 0.86), (10.0, 0.74)])
            .build()
            .expect("discount"),
    )
}

/// 250bp flat par spreads, wide enough for the recovery effect to show.
fn hazard_at(recovery_rate: f64) -> HazardCurve {
    calibrated_hazard_curve_from_spec(CalibratedHazardSpec {
        source_market: &discount_only(),
        base_date: AS_OF,
        curve_id: "USD-CREDIT",
        entity: "USD-CREDIT-ENTITY",
        discount_curve_id: "USD-OIS",
        pillars: &[
            (date!(2026 - 01 - 01), 250.0),
            (date!(2028 - 01 - 01), 250.0),
            (date!(2030 - 01 - 01), 250.0),
            (date!(2035 - 01 - 01), 250.0),
        ],
        recovery_rate,
        cds_valuation_convention: None,
        doc_clause: None,
    })
    .expect("calibration")
}

#[test]
fn recovery01_is_the_central_difference_across_recalibrated_curves() {
    let bond = bond();
    let pv_at = |recovery: f64| {
        bond.value_raw(&discount_only().insert(hazard_at(recovery)), AS_OF)
            .expect("pv")
    };
    let expected = (pv_at(0.41) - pv_at(0.39)) / 2.0;

    let market = discount_only().insert(hazard_at(0.40));
    let recalibrated = bond
        .price_with_metrics(&market, AS_OF, &[MetricId::Recovery01], pricing_options())
        .expect("recovery01")
        .measures["recovery01"];
    assert!(
        (recalibrated - expected).abs() < 1e-8,
        "{recalibrated} vs {expected}"
    );

    // Holding the same hazard rates only adds the recovery gain, so it is far
    // larger: 1,767.89 frozen against 8.03 recalibrated here.
    let frozen = {
        let up = market.get_hazard("USD-CREDIT").expect("curve");
        let pv = |recovery: f64| {
            let curve = up.with_recovery_rate(recovery).expect("curve");
            bond.value_raw(&market.clone().insert(curve), AS_OF)
                .expect("pv")
        };
        (pv(0.41) - pv(0.39)) / 2.0
    };
    assert!(frozen > 100.0 * recalibrated, "{frozen} vs {recalibrated}");
    assert!(recalibrated > 0.0, "{recalibrated}");
}

#[test]
fn a_recipe_with_no_recalibration_provider_is_an_error_not_a_frozen_bump() {
    let market = discount_only().insert(hazard_at(0.40));
    let error = bond()
        .price_with_metrics(
            &market,
            AS_OF,
            &[MetricId::Recovery01],
            PricingOptions::default(),
        )
        .expect_err("no provider")
        .to_string();
    assert!(error.contains("re-bootstrap failed"), "{error}");
}

#[test]
fn a_premium_bond_loses_when_recovery_rises_at_fixed_spreads() {
    let market = discount_only().insert(hazard_at(0.40));
    let recovery01 = bond_with_coupon(0.12)
        .price_with_metrics(&market, AS_OF, &[MetricId::Recovery01], pricing_options())
        .expect("recovery01")
        .measures["recovery01"];
    assert!(recovery01 < 0.0, "{recovery01}");
}
