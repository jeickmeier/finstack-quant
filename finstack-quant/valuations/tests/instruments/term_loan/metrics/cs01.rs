//! Model-aware term-loan credit-spread sensitivity regressions.

use crate::common::test_helpers::flat_discount_curve;
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
    let hazard = crate::test_support::credit::calibrated_hazard_curve(
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
            crate::test_support::credit::pricing_options(),
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
