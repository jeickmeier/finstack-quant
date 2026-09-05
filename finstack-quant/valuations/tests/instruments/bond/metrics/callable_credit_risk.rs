//! Risky-callable (callable + credit curve) quoted-bond risk coverage.
//!
//! A callable bond that also carries a `credit_curve_id`, priced against a
//! `quoted_clean_price`, must produce non-zero, call-aware CS01/DV01: the OAS
//! clone retains the credit tag and reprices on the two-factor `RatesCreditTree`,
//! so CS01 bumps the hazard and DV01 bumps the discount curve, both at the
//! constant calibrated OAS. This locks that (previously untested) path.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::bond::{Bond, CallPut, CallPutSchedule};
use finstack_quant_valuations::instruments::{
    Instrument, InstrumentPricingOverrides, PricingOptions,
};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::ModelKey;
use time::macros::date;

fn build_callable_credit_bond(as_of: time::Date) -> Bond {
    let mut bond = Bond::fixed(
        "CALL-CREDIT",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.06).expect("valid rate fixture"),
        as_of,
        date!(2027 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("callable credit bond should build");
    bond.settlement_convention = None;
    bond.credit_curve_id = Some(CurveId::new("USD-CREDIT"));
    bond.instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.01);
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: date!(2026 - 01 - 01),
            end_date: date!(2026 - 01 - 01),
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
        .knots([(0.0, 1.0), (1.0, 0.96), (2.0, 0.91), (5.0, 0.78)])
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
fn test_quoted_callable_credit_bond_risk_nonzero_and_call_aware() {
    let as_of = date!(2025 - 01 - 01);
    let market = build_market(as_of);

    // Unquoted reference: callable + credit prices on the two-factor tree at OAS=0;
    // CS01 bumps the hazard and reprices through the same tree.
    let mut unquoted = build_callable_credit_bond(as_of);
    unquoted.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_hw1f_sigma(0.0);
    let base = unquoted
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01, MetricId::CleanPrice],
            crate::test_support::credit::pricing_options(),
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
        .with_hw1f_sigma(0.0);
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
            crate::test_support::credit::pricing_options(),
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
        .filter(|(k, v)| k.as_str().starts_with("bucketed_cs01::USD-CREDIT::") && v.abs() > 1e-6)
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

#[test]
fn test_unquoted_callable_explicit_models_skip_quote_spread_dependencies() {
    let as_of = date!(2025 - 01 - 01);
    let market = build_market(as_of);
    let mut bond = build_callable_credit_bond(as_of);
    bond.instrument_pricing_overrides = InstrumentPricingOverrides::default().with_hw1f_sigma(0.0);

    let rates_credit = bond
        .price_with_metrics(
            &market,
            as_of,
            &[
                MetricId::Cs01,
                MetricId::BucketedCs01,
                MetricId::BucketedDv01,
            ],
            crate::test_support::credit::pricing_options().with_model(ModelKey::RatesCredit),
        )
        .expect("unquoted callable RatesCredit risk should not require Z-spread");
    for metric in ["cs01", "bucketed_cs01", "bucketed_dv01"] {
        let value = rates_credit.measures[metric];
        assert!(
            value.is_finite() && value.abs() > 1e-6,
            "{metric} should be finite and non-zero, got {value}"
        );
    }
    assert!(rates_credit.measures.iter().any(|(key, value)| {
        key.as_str().starts_with("bucketed_cs01::USD-CREDIT::") && value.abs() > 1e-6
    }));
    assert!(rates_credit
        .measures
        .iter()
        .any(|(key, value)| { key.as_str().starts_with("bucketed_dv01::") && value.abs() > 1e-6 }));

    let tree = bond
        .price_with_metrics(
            &market,
            as_of,
            &[
                MetricId::Cs01,
                MetricId::BucketedCs01,
                MetricId::BucketedDv01,
            ],
            PricingOptions::default().with_model(ModelKey::Tree),
        )
        .expect("unquoted callable Tree risk should not require Z-spread");
    for metric in ["cs01", "bucketed_cs01", "bucketed_dv01"] {
        let value = tree.measures[metric];
        assert!(
            value.is_finite() && value.abs() > 1e-6,
            "Tree {metric} should be finite and non-zero, got {value}"
        );
    }
    let bucketed_dv01 = tree.measures["bucketed_dv01"];
    assert!(bucketed_dv01.is_finite() && bucketed_dv01.abs() > 1e-6);
    assert!(tree
        .measures
        .iter()
        .any(|(key, value)| key.as_str().starts_with("bucketed_dv01::") && value.abs() > 1e-6));
}
