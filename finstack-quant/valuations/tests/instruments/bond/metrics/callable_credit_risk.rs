//! Risky-callable (callable + credit curve) quoted-bond risk coverage.
//!
//! A callable bond that also carries a `credit_curve_id`, priced against a
//! `quoted_clean_price`, must produce non-zero, call-aware CS01/DV01: the OAS
//! clone retains the credit tag and reprices on the two-factor `RatesCreditTree`,
//! so CS01 bumps the hazard and DV01 bumps the discount curve, both at the
//! constant calibrated OAS. This locks that (previously untested) path.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::bond::{Bond, CallPut, CallPutSchedule};
use finstack_quant_valuations::instruments::{
    Instrument, InstrumentPricingOverrides, PricingOptions,
};
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

fn build_callable_credit_bond(as_of: time::Date) -> Bond {
    let mut bond = Bond::fixed(
        "CALL-CREDIT",
        Money::new(1_000_000.0, Currency::USD),
        0.06,
        as_of,
        date!(2032 - 01 - 01),
        "USD-OIS",
    )
    .expect("callable credit bond should build");
    bond.settlement_convention = None;
    bond.credit_curve_id = Some(CurveId::new("USD-CREDIT"));
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: date!(2028 - 01 - 01),
            end_date: date!(2028 - 01 - 01),
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![],
    });
    bond
}

fn build_market(as_of: time::Date) -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (3.0, 0.91), (7.0, 0.78)])
        .build()
        .expect("discount curve should build");
    let hazard = HazardCurve::builder("USD-CREDIT")
        .base_date(as_of)
        .recovery_rate(0.4)
        .knots([(0.0, 0.015), (7.0, 0.015)])
        .build()
        .expect("hazard curve should build");
    MarketContext::new().insert(disc).insert(hazard)
}

#[test]
fn test_quoted_callable_credit_bond_risk_nonzero_and_call_aware() {
    let as_of = date!(2025 - 01 - 01);
    let market = build_market(as_of);

    // Unquoted reference: callable + credit prices on the two-factor tree at OAS=0;
    // CS01 bumps the hazard and reprices through the same tree.
    let mut unquoted = build_callable_credit_bond(as_of);
    unquoted.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_implied_vol(0.02);
    let base = unquoted
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01, MetricId::CleanPrice],
            PricingOptions::default(),
        )
        .expect("unquoted callable-credit bond should price");
    let base_cs01 = *base.measures.get("cs01").unwrap();
    let model_clean = *base.measures.get("clean_price").unwrap() / 1_000_000.0 * 100.0;
    assert!(
        base_cs01.abs() > 1e-3,
        "sanity: unquoted callable-credit CS01 non-zero, got {base_cs01}"
    );

    // Quoted at the model price → OAS calibration ≈ reproduces the quote.
    let mut quoted = build_callable_credit_bond(as_of);
    quoted.instrument_pricing_overrides = InstrumentPricingOverrides::default()
        .with_quoted_clean_price(model_clean)
        .with_implied_vol(0.02);
    let result = quoted
        .price_with_metrics(
            &market,
            as_of,
            &[
                MetricId::Cs01,
                MetricId::BucketedCs01,
                MetricId::Dv01,
                MetricId::BucketedDv01,
                MetricId::EmbeddedOptionValue,
            ],
            PricingOptions::default(),
        )
        .expect("quoted callable-credit bond should price");

    let cs01 = *result.measures.get("cs01").unwrap();
    let dv01 = *result.measures.get("dv01").unwrap();
    let eov = *result.measures.get("embedded_option_value").unwrap();

    // The embedded call is live → the option-adjusted two-factor tree is active.
    assert!(
        eov < -1e-3,
        "embedded call should be live (EmbeddedOptionValue < 0), got {eov}"
    );

    // Non-zero, correct sign (long bond: wider spread & higher rates both lower PV).
    assert!(
        cs01.abs() > 1e-3,
        "callable-credit CS01 must be non-zero, got {cs01}"
    );
    assert!(
        dv01.abs() > 1e-3,
        "callable-credit DV01 must be non-zero, got {dv01}"
    );
    assert!(cs01 < 0.0, "long-bond CS01 should be negative, got {cs01}");
    assert!(dv01 < 0.0, "long-bond DV01 should be negative, got {dv01}");

    // Bucketed metrics populated.
    let bcs = result
        .measures
        .iter()
        .filter(|(k, v)| k.as_str().starts_with("bucketed_cs01") && v.abs() > 1e-6)
        .count();
    let bdv = result
        .measures
        .iter()
        .filter(|(k, v)| k.as_str().starts_with("bucketed_dv01") && v.abs() > 1e-6)
        .count();
    assert!(bcs >= 1, "bucketed_cs01 must be populated, got {bcs}");
    assert!(bdv >= 1, "bucketed_dv01 must be populated, got {bdv}");

    // Quoted ≈ unquoted: the OAS calibration reproduces the quote on the two-factor
    // tree, so the quoted CS01 reconciles with the unquoted model CS01.
    assert!(
        (cs01 - base_cs01).abs() < (base_cs01.abs() * 0.05 + 1.0),
        "quoted callable-credit CS01 ({cs01:.4}) should reconcile with unquoted ({base_cs01:.4})"
    );
}
