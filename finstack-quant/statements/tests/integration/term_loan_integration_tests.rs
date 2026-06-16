//! Tests for TermLoan capital structure integration.
//!
//! This test suite validates that TermLoan instruments from the valuations crate
//! can be properly integrated into financial statement models.

use finstack_quant_statements::types::DebtInstrumentSpec;

// ============================================================================
// TermLoan Variant Tests
// ============================================================================

#[test]
fn test_term_loan_variant_serialization() {
    let spec = DebtInstrumentSpec {
        id: "TL-001".to_string(),
        spec: serde_json::json!({
            "type": "term_loan",
            "spec": {
                "id": "TL-001",
                "notional": { "amount": 5000000.0, "currency": "USD" }
            }
        }),
    };

    // Test serialization roundtrip
    let json = serde_json::to_string(&spec).unwrap();
    let deserialized: DebtInstrumentSpec = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, "TL-001");
}

// ============================================================================
// Capital Structure with TermLoan
// ============================================================================

#[test]
fn term_loan_capital_structure_evaluates_with_market() {
    use finstack_quant_cashflows::builder::specs::CouponType;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use finstack_quant_statements::builder::ModelBuilder;
    use finstack_quant_statements::evaluator::Evaluator;
    use finstack_quant_valuations::instruments::fixed_income::term_loan::{
        AmortizationSpec, RateSpec, TermLoan,
    };
    use finstack_quant_valuations::instruments::InstrumentJson;
    use time::macros::date;

    let issue = date!(2025 - 01 - 01);
    let maturity = date!(2026 - 01 - 01);
    let loan = TermLoan::builder()
        .id("TL-001".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(5_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .rate(RateSpec::Fixed { rate_bp: 800 })
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .pricing_overrides(Default::default())
        .attributes(Default::default())
        .build()
        .expect("valid term loan");

    let tagged = serde_json::to_value(InstrumentJson::TermLoan(loan))
        .expect("term loan should serialize to tagged registry payload");
    let model = ModelBuilder::new("term-loan-cs")
        .periods("2025Q1..2025Q4", None)
        .expect("valid periods")
        .add_custom_debt("TL-001", tagged)
        .compute("tl_interest", "cs.interest_expense.TL-001")
        .expect("valid cs interest formula")
        .compute("tl_balance", "cs.debt_balance.TL-001")
        .expect("valid cs debt balance formula")
        .build()
        .expect("valid capital structure model");

    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots(vec![(0.0, 1.0), (1.0, 0.95), (5.0, 0.80)])
        .build()
        .expect("valid discount curve");
    let market = MarketContext::new().insert(discount_curve);

    let mut evaluator = Evaluator::new();
    let result = evaluator
        .evaluate_with_market(&model, &market, issue)
        .expect("term loan capital structure should evaluate with market");

    let cashflows = result
        .cs_cashflows
        .as_ref()
        .expect("capital structure cashflows should be populated");
    assert!(
        cashflows.by_instrument.contains_key("TL-001"),
        "term loan should produce per-instrument capital-structure cashflows"
    );

    let q2 = finstack_quant_core::dates::PeriodId::quarter(2025, 2);
    let q2_interest = result
        .get("tl_interest", &q2)
        .expect("Q2 term loan interest should evaluate");
    assert!(
        q2_interest > 0.0,
        "expected non-zero Q2 term loan interest, got {q2_interest}"
    );

    let q1 = finstack_quant_core::dates::PeriodId::quarter(2025, 1);
    let q1_balance = result
        .get("tl_balance", &q1)
        .expect("Q1 term loan balance should evaluate");
    assert!(
        q1_balance > 0.0,
        "expected non-zero Q1 term loan debt balance, got {q1_balance}"
    );
}
