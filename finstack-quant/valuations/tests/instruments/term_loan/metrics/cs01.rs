//! Model-aware term-loan credit-spread sensitivity regressions.

use crate::instruments::common::test_helpers::flat_discount_curve;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
use finstack_quant_valuations::instruments::PricingOptions;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::{standard_pricer_registry, ModelKey};
use time::macros::date;

fn credit_loan_and_market() -> (TermLoan, MarketContext) {
    let as_of = date!(2024 - 01 - 01);
    let mut loan = TermLoan::example().expect("term loan");
    loan.credit_curve_id = Some(CurveId::new("USD-CREDIT"));
    let discount = flat_discount_curve(0.04, as_of, "USD-OIS");
    let source = MarketContext::new().insert(discount);
    let hazard = crate::instruments::test_support::credit::calibrated_hazard_curve(
        &source,
        as_of,
        "USD-CREDIT",
        "USD-CREDIT-ENTITY",
        "USD-OIS",
    )
    .expect("hazard calibration");
    (loan, source.insert(hazard))
}

#[test]
fn cs01_follows_the_selected_pricing_model() {
    let as_of = date!(2024 - 01 - 01);
    let (loan, market) = credit_loan_and_market();

    let tree = standard_pricer_registry()
        .price_with_metrics(
            &loan,
            ModelKey::Tree,
            &market,
            as_of,
            &[MetricId::Cs01, MetricId::BucketedCs01],
            crate::instruments::test_support::credit::pricing_options(),
        )
        .expect("credit-tree metrics");
    let tree_cs01 = *tree.measures.get("cs01").expect("cs01");
    assert!(tree_cs01 < 0.0);
    assert!(tree.measures.contains_key("cs01::USD-CREDIT"));
    assert!(tree
        .measures
        .keys()
        .any(|key| key.as_str().starts_with("bucketed_cs01::USD-CREDIT::")));

    let discounting = standard_pricer_registry()
        .price_with_metrics(
            &loan,
            ModelKey::Discounting,
            &market,
            as_of,
            &[MetricId::Cs01],
            PricingOptions::default(),
        )
        .expect("discounting metrics");
    let zspread_cs01 = *discounting.measures.get("cs01").expect("cs01");
    assert!(zspread_cs01 < 0.0);
    assert!(
        (tree_cs01 - zspread_cs01).abs() > 1.0,
        "tree and discounting CS01 must be distinct model paths: tree={tree_cs01}, discounting={zspread_cs01}"
    );
}

#[test]
fn quoted_z_spread_cs01_matches_settlement_price_bumps() {
    let as_of = date!(2024 - 01 - 15);
    let mut loan = TermLoan::example().expect("term loan");
    let market = MarketContext::new().insert(flat_discount_curve(0.04, as_of, "USD-OIS"));
    let spread = 0.05;
    loan.instrument_pricing_overrides
        .market_quotes
        .quoted_z_spread = Some(spread);
    let registry = standard_pricer_registry();
    let result = registry
        .price_with_metrics(
            &loan,
            ModelKey::Discounting,
            &market,
            as_of,
            &[MetricId::Cs01, MetricId::BucketedCs01],
            PricingOptions::default(),
        )
        .expect("quoted spread metrics");

    // CS01 is a settlement-date quote sensitivity, so compare against actual
    // bumped prices at that same date without a second settlement lag.
    let settlement = loan.settlement_date(as_of).expect("settlement date");
    let mut settlement_loan = loan;
    settlement_loan.settlement_days = 0;
    let mut bumped_values = Vec::with_capacity(2);
    for bumped_spread in [spread - 0.0001, spread + 0.0001] {
        settlement_loan
            .instrument_pricing_overrides
            .market_quotes
            .quoted_z_spread = Some(bumped_spread);
        let price = registry
            .price_with_metrics(
                &settlement_loan,
                ModelKey::Discounting,
                &market,
                settlement,
                &[],
                PricingOptions::default(),
            )
            .expect("bumped settlement price");
        bumped_values.push(price.value.amount());
    }
    let expected = (bumped_values[1] - bumped_values[0]) / 2.0;
    let cs01 = *result.measures.get("cs01").expect("CS01");
    assert!(expected < 0.0);
    assert!(
        (cs01 - expected).abs() < 1e-5,
        "CS01 must differentiate the quoted spread: reported={cs01}, price bumps={expected}"
    );
    let bucketed = *result.measures.get("bucketed_cs01").expect("bucketed CS01");
    let bucket_sum: f64 = result
        .measures
        .iter()
        .filter(|(key, _)| key.as_str().starts_with("bucketed_cs01::"))
        .map(|(_, value)| *value)
        .sum();
    assert!((bucketed - cs01).abs() < 1e-5);
    assert!((bucket_sum - cs01).abs() < 1e-5);
}

#[test]
fn quoted_clean_price_changes_term_loan_cs01() {
    let as_of = date!(2024 - 01 - 15);
    let market = MarketContext::new().insert(flat_discount_curve(0.04, as_of, "USD-OIS"));
    let registry = standard_pricer_registry();
    let mut sensitivities = Vec::with_capacity(3);
    for clean_price in [80.0, 99.0, 110.0] {
        let mut loan = TermLoan::example().expect("term loan");
        loan.instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price_pct = Some(clean_price);
        let result = registry
            .price_with_metrics(
                &loan,
                ModelKey::Discounting,
                &market,
                as_of,
                &[MetricId::Cs01],
                PricingOptions::default(),
            )
            .expect("clean quote CS01");
        sensitivities.push(result.measures.get("cs01").expect("CS01").abs());
    }
    assert!(sensitivities[0] < sensitivities[1]);
    assert!(sensitivities[1] < sensitivities[2]);
}

#[test]
fn impossible_clean_price_does_not_return_model_anchored_risk() {
    let as_of = date!(2024 - 01 - 15);
    let market = MarketContext::new().insert(flat_discount_curve(0.04, as_of, "USD-OIS"));
    let mut loan = TermLoan::example().expect("term loan");
    loan.instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price_pct = Some(-100.0);
    let registry = standard_pricer_registry();
    for metric in [MetricId::Dv01, MetricId::Cs01, MetricId::BucketedCs01] {
        registry
            .price_with_metrics(
                &loan,
                ModelKey::Discounting,
                &market,
                as_of,
                &[metric],
                PricingOptions::default(),
            )
            .expect_err("positive loan cashflows cannot reproduce a negative dirty quote");
    }
}
