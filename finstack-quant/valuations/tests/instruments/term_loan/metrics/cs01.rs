//! Model-aware term-loan credit-spread sensitivity regressions.

use crate::common::test_helpers::flat_discount_curve;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
use finstack_quant_valuations::instruments::PricingOptions;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::{standard_registry, ModelKey};
use time::macros::date;

fn credit_loan_and_market() -> (TermLoan, MarketContext) {
    let as_of = date!(2024 - 01 - 01);
    let mut loan = TermLoan::example().expect("term loan");
    loan.credit_curve_id = Some(CurveId::new("USD-CREDIT"));
    let discount = flat_discount_curve(0.04, as_of, "USD-OIS");
    let hazard = HazardCurve::builder("USD-CREDIT")
        .base_date(as_of)
        .recovery_rate(0.4)
        .knots([(0.0, 0.02), (5.0, 0.02)])
        .build()
        .expect("hazard curve");
    (loan, MarketContext::new().insert(discount).insert(hazard))
}

#[test]
fn cs01_routes_to_hazard_only_when_the_active_model_consumes_credit() {
    let as_of = date!(2024 - 01 - 01);
    let (loan, market) = credit_loan_and_market();
    let metrics = [MetricId::Cs01, MetricId::Cs01Hazard];

    let tree = standard_registry()
        .price_with_metrics(
            &loan,
            ModelKey::Tree,
            &market,
            as_of,
            &metrics,
            PricingOptions::default(),
        )
        .expect("credit-tree metrics");
    let tree_cs01 = *tree.measures.get("cs01").expect("cs01");
    let tree_hazard = *tree.measures.get("cs01_hazard").expect("cs01_hazard");
    assert!(tree_cs01 < 0.0);
    assert!((tree_cs01 - tree_hazard).abs() < 1e-10);

    let discounting = standard_registry()
        .price_with_metrics(
            &loan,
            ModelKey::Discounting,
            &market,
            as_of,
            &metrics,
            PricingOptions::default(),
        )
        .expect("discounting metrics");
    let zspread_cs01 = *discounting.measures.get("cs01").expect("cs01");
    let discounting_hazard = *discounting
        .measures
        .get("cs01_hazard")
        .expect("cs01_hazard");
    assert!(zspread_cs01 < 0.0);
    assert_eq!(discounting_hazard, 0.0);
}
