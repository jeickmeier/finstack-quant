//! Quoted credit-bond risk regression.
//!
//! A bond with a credit (hazard) curve AND a `quoted_clean_price` must still
//! produce non-zero direct hazard CS01 — the engine calibrates a flat hazard
//! shift that reproduces the quote and bumps that shifted curve, mirroring the
//! same bond priced WITHOUT a quote. Before the fix, `Bond::base_value`
//! short-circuits to the constant quoted price, so the hazard bump reprices the
//! same constant and CS01 collapses to zero.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::{
    BondRiskBasis, Instrument, InstrumentPricingOverrides, PricingOptions,
};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::ModelKey;
use time::macros::date;

fn build_credit_bond(as_of: time::Date) -> Bond {
    let mut bond = Bond::fixed(
        "CREDIT-Q",
        Money::new(1_000_000.0, Currency::USD),
        finstack_quant_core::types::Rate::from_decimal(0.05),
        as_of,
        date!(2030 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("credit bond should build");
    bond.credit_curve_id = Some(CurveId::new("USD-CREDIT"));
    bond
}

fn build_market(as_of: time::Date) -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([
            (0.0, 1.0),
            (1.0, 0.97),
            (2.0, 0.94),
            (3.0, 0.91),
            (5.0, 0.83),
        ])
        .build()
        .expect("discount curve should build");
    let source = MarketContext::new().insert(disc);
    let hazard = crate::test_support::credit::calibrated_hazard_curve(
        &source,
        as_of,
        "USD-CREDIT",
        "USD-CREDIT-ENTITY",
        "USD-OIS",
    )
    .expect("hazard calibration should succeed");
    source.insert(hazard)
}

#[test]
fn test_quoted_credit_bond_hazard_cs01_nonzero_and_matches_unquoted() {
    let as_of = date!(2025 - 01 - 01);
    let market = build_market(as_of);

    // Unquoted: model clean price + reference CS01.
    let unquoted = build_credit_bond(as_of);
    let base = unquoted
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01Hazard, MetricId::CleanPrice],
            PricingOptions::default(),
        )
        .expect("unquoted credit bond should price");
    let base_cs01 = *base.measures.get("cs01_hazard").unwrap();
    let model_clean_pct = *base.measures.get("clean_price").unwrap() / 1_000_000.0 * 100.0;
    assert!(
        base_cs01.abs() > 1e-3,
        "sanity: unquoted credit CS01 should be non-zero, got {base_cs01}"
    );

    // Quoted at the model clean price → calibrated hazard shift ≈ 0 → risk ≈ unquoted.
    let mut quoted = build_credit_bond(as_of);
    quoted.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(model_clean_pct);
    let result = quoted
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01Hazard, MetricId::BucketedCs01Hazard],
            PricingOptions::default(),
        )
        .expect("quoted credit bond should price");

    let cs01 = *result.measures.get("cs01_hazard").unwrap();
    assert!(
        cs01.abs() > 1e-3,
        "quoted credit CS01 must be non-zero (was 0 before the fix), got {cs01}"
    );

    let bucket_series_prefix = "bucketed_cs01_hazard::USD-CREDIT::";
    let bucketed_nonzero = result
        .measures
        .iter()
        .filter(|(k, v)| k.as_str().starts_with(bucket_series_prefix) && v.abs() > 1e-6)
        .count();
    assert!(
        bucketed_nonzero >= 1,
        "quoted credit bucketed CS01 series '{bucket_series_prefix}' must be populated, \
         got {bucketed_nonzero}"
    );

    assert!(
        (cs01 - base_cs01).abs() < (base_cs01.abs() * 0.05 + 1.0),
        "quoted CS01 ({cs01:.4}) should reconcile with unquoted ({base_cs01:.4})"
    );
}

#[test]
fn test_quoted_credit_bond_offmodel_changes_hazard_cs01() {
    let as_of = date!(2025 - 01 - 01);
    let market = build_market(as_of);

    let unquoted = build_credit_bond(as_of);
    let base = unquoted
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::CleanPrice],
            PricingOptions::default(),
        )
        .unwrap();
    let model_clean_pct = *base.measures.get("clean_price").unwrap() / 1_000_000.0 * 100.0;

    let hazard_cs01_at = |clean_pct: f64| -> f64 {
        let mut q = build_credit_bond(as_of);
        q.instrument_pricing_overrides =
            InstrumentPricingOverrides::default().with_quoted_clean_price(clean_pct);
        let r = q
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Cs01Hazard],
                PricingOptions::default(),
            )
            .unwrap();
        *r.measures.get("cs01_hazard").unwrap()
    };

    // Quoting 8pts below model recalibrates the hazard wider, so CS01 must differ
    // from the at-model quote. If the calibrated shift were silently discarded
    // (curve-id bug), both would equal the unquoted CS01 and this would fail.
    let cs01_model = hazard_cs01_at(model_clean_pct);
    let cs01_distressed = hazard_cs01_at(model_clean_pct - 8.0);
    assert!(
        (cs01_distressed - cs01_model).abs() > 1e-2,
        "off-model quote should recalibrate the hazard and change CS01: \
         model={cs01_model:.4}, distressed={cs01_distressed:.4}"
    );
}

#[test]
fn test_quoted_credit_bond_quote_space_cs01_preserves_hazard_replay() {
    let as_of = date!(2025 - 01 - 01);
    let market = build_market(as_of);

    let unquoted = build_credit_bond(as_of);
    let base = unquoted
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::CleanPrice],
            PricingOptions::default(),
        )
        .expect("unquoted credit bond should price");
    let model_clean_pct = base.measures["clean_price"] / 1_000_000.0 * 100.0;

    let mut quoted = build_credit_bond(as_of);
    quoted.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(model_clean_pct);
    let result = quoted
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01, MetricId::BucketedCs01],
            crate::test_support::credit::pricing_options(),
        )
        .expect("quote-space CS01 should replay the calibrated hazard curve");

    let cs01 = result.measures["cs01"];
    assert!(
        cs01.is_finite() && cs01.abs() > 1e-3,
        "quote-space CS01 should be finite and non-zero, got {cs01}"
    );
    let bucket_prefix = "bucketed_cs01::USD-CREDIT::";
    assert!(
        result
            .measures
            .iter()
            .any(|(key, value)| key.as_str().starts_with(bucket_prefix) && value.abs() > 1e-6),
        "quote-space bucketed CS01 should populate '{bucket_prefix}'"
    );
}

#[test]
fn test_quoted_credit_bond_callable_oas_dv01_matches_unquoted_model_risk() {
    let as_of = date!(2025 - 01 - 01);
    let market = build_market(as_of);

    let mut unquoted = build_credit_bond(as_of);
    unquoted.metric_pricing_overrides = unquoted
        .metric_pricing_overrides
        .with_bond_risk_basis(BondRiskBasis::CallableOas);
    let base = unquoted
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::CleanPrice, MetricId::Dv01, MetricId::BucketedDv01],
            PricingOptions::default(),
        )
        .expect("unquoted OAS-basis credit risk should price");
    let model_clean_pct = base.measures["clean_price"] / 1_000_000.0 * 100.0;
    let base_dv01 = base.measures["dv01"];
    let base_bucketed = base.measures["bucketed_dv01"];

    let mut quoted = build_credit_bond(as_of);
    quoted.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(model_clean_pct);
    quoted.metric_pricing_overrides = quoted
        .metric_pricing_overrides
        .with_bond_risk_basis(BondRiskBasis::CallableOas);
    let result = quoted
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01, MetricId::BucketedDv01],
            PricingOptions::default(),
        )
        .expect("quoted OAS-basis credit risk should retain the model quote anchor");
    let quoted_dv01 = result.measures["dv01"];
    let quoted_bucketed = result.measures["bucketed_dv01"];

    assert!(
        quoted_dv01.abs() > 1e-3 && quoted_bucketed.abs() > 1e-3,
        "quoted OAS-basis DV01 must not collapse to zero: scalar={quoted_dv01}, bucketed={quoted_bucketed}"
    );
    assert!(
        (quoted_dv01 - base_dv01).abs() < base_dv01.abs() * 0.05 + 1.0,
        "quoted scalar DV01 ({quoted_dv01}) should match unquoted model risk ({base_dv01})"
    );
    assert!(
        (quoted_bucketed - base_bucketed).abs() < base_bucketed.abs() * 0.05 + 1.0,
        "quoted bucketed DV01 ({quoted_bucketed}) should match unquoted model risk ({base_bucketed})"
    );
}

#[test]
fn test_direct_hazard_risk_propagates_unattainable_quote_calibration_error() {
    let as_of = date!(2025 - 01 - 01);
    let market = build_market(as_of);
    let mut quoted = build_credit_bond(as_of);
    quoted.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(10_000.0);

    let err = quoted
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01Hazard],
            PricingOptions::default(),
        )
        .expect_err("an unattainable quote must fail hazard-shift calibration");
    let message = err.to_string();
    assert!(
        message.contains("no sign change") || message.contains("convergence"),
        "error should preserve the typed root-solve failure, got: {message}"
    );
}

#[test]
fn test_explicit_discounting_with_attached_credit_uses_plain_rate_bucketed_dv01() {
    let as_of = date!(2025 - 01 - 01);
    let market = build_market(as_of);
    let mut quoted = build_credit_bond(as_of);
    quoted.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(99.0);

    let result = quoted
        .price_with_metrics(
            &market,
            as_of,
            &[
                MetricId::BucketedDv01,
                MetricId::Cs01Hazard,
                MetricId::BucketedCs01Hazard,
            ],
            PricingOptions::default().with_model(ModelKey::Discounting),
        )
        .expect("explicit Discounting must ignore the attached credit curve for rate risk");

    let aggregate = result.measures["bucketed_dv01"];
    assert!(aggregate.is_finite() && aggregate.abs() > 1e-6);
    assert!(result
        .measures
        .iter()
        .any(|(key, value)| key.as_str().starts_with("bucketed_dv01::") && value.abs() > 1e-6));
    assert_eq!(result.measures["cs01_hazard"], 0.0);
    assert_eq!(result.measures["bucketed_cs01_hazard"], 0.0);
}
