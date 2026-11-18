//! Tests for capital structure DSL integration (`cs.*` namespace).

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, PeriodId};
use finstack_quant_core::money::Money;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::dsl::{StmtExpr, parse_formula};
use finstack_quant_statements::types::AmountOrScalar;
use time::Month;

/// Test parsing cs.* references
#[test]
fn test_parse_cs_interest_expense_total() {
    let ast = parse_formula("cs.interest_expense.total").unwrap();

    match ast {
        StmtExpr::CsRef { component, scope } => {
            assert_eq!(component.as_str(), "interest_expense");
            assert_eq!(scope.to_string(), "total");
        }
        _ => panic!("Expected CsRef, got {:?}", ast),
    }
}

#[test]
fn test_parse_cs_principal_payment_instrument() {
    let ast = parse_formula("cs.principal_payment.BOND-001").unwrap();

    match ast {
        StmtExpr::CsRef { component, scope } => {
            assert_eq!(component.as_str(), "principal_payment");
            assert_eq!(scope.to_string(), "BOND-001");
        }
        _ => panic!("Expected CsRef, got {:?}", ast),
    }
}

#[test]
fn test_parse_cs_debt_balance() {
    let ast = parse_formula("cs.debt_balance.SWAP-001").unwrap();

    match ast {
        StmtExpr::CsRef { component, scope } => {
            assert_eq!(component.as_str(), "debt_balance");
            assert_eq!(scope.to_string(), "SWAP-001");
        }
        _ => panic!("Expected CsRef, got {:?}", ast),
    }
}

#[test]
fn test_parse_cs_in_formula() {
    let ast = parse_formula("revenue - cs.interest_expense.total").unwrap();

    // Should parse as a subtraction with a CsRef on the right
    match ast {
        StmtExpr::BinOp { .. } => {
            // Valid structure
        }
        _ => panic!("Expected BinOp, got {:?}", ast),
    }
}

#[test]
fn test_parse_cs_complex_formula() {
    let ast =
        parse_formula("(revenue - cogs - opex - cs.interest_expense.total) / revenue").unwrap();

    // Should parse successfully
    match ast {
        StmtExpr::BinOp { .. } => {
            // Valid structure
        }
        _ => panic!("Expected BinOp, got {:?}", ast),
    }
}

/// Test that invalid cs references produce a parse error
#[test]
fn test_parse_cs_invalid_format() {
    let result = parse_formula("cs.interest_expense");
    assert!(
        result.is_err(),
        "Two-part cs.* reference should produce a parse error"
    );
}

/// Test full model evaluation with capital structure (mock cashflows)
#[test]
fn test_evaluate_model_with_cs_mock() {
    // Create a simple model that references capital structure
    let model_result = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(1_000_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(1_100_000.0),
                ),
            ],
        );
        model_builder.value(
            "cogs",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(600_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(650_000.0),
                ),
            ],
        );
        model_builder.build()
    };

    assert!(model_result.is_ok());
    let model = model_result.unwrap();

    // Verify model structure
    assert_eq!(model.nodes.len(), 2);
    assert!(model.has_node("revenue"));
    assert!(model.has_node("cogs"));
}

/// Test model with bond added via builder
#[test]
fn test_model_with_bond_builder() {
    let issue_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity_date = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let model_result = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder.periods("2025Q1..Q2", None).unwrap();
        model_builder.value(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(1_000_000.0),
            )],
        );
        model_builder.add_debt(
            "BOND-001",
            fixed_bond(
                "BOND-001",
                Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
                0.05,
                issue_date,
                maturity_date,
                "USD-OIS",
            ),
        );
        model_builder.build()
    };

    assert!(model_result.is_ok());
    let model = model_result.unwrap();

    // Verify capital structure was added
    assert!(model.capital_structure.is_some());
    let cs = model.capital_structure.as_ref().unwrap();
    assert_eq!(cs.debt_instruments.len(), 1);
}

/// Test that cs.* references can be compiled
#[test]
fn test_compile_cs_reference() {
    use finstack_quant_statements::dsl::parse_and_compile;

    let expr = parse_and_compile("cs.interest_expense.total").unwrap();

    // Should compile to a typed capital-structure reference.
    use finstack_quant_core::expr::ExprNode;
    match &expr.node {
        ExprNode::CsRef {
            component,
            instrument_or_total,
        } => {
            assert_eq!(component, "interest_expense");
            assert_eq!(instrument_or_total, "total");
        }
        _ => panic!("Expected CsRef node, got {:?}", expr.node),
    }
}

/// Test cs.* in complex formulas
#[test]
fn test_compile_cs_in_formula() {
    use finstack_quant_statements::dsl::parse_and_compile;

    // Should compile successfully
    let expr = parse_and_compile("revenue - cogs - cs.interest_expense.total").unwrap();

    // Verify it's a binary operation
    use finstack_quant_core::expr::ExprNode;
    match &expr.node {
        ExprNode::BinOp { .. } => {
            // Valid
        }
        _ => panic!("Expected BinOp node"),
    }
}

/// Test debt service ratio formula
#[test]
fn test_compile_debt_service_ratio() {
    use finstack_quant_statements::dsl::parse_and_compile;

    let expr =
        parse_and_compile("ebitda / (cs.interest_expense.total + cs.principal_payment.total)")
            .unwrap();

    // Should compile successfully
    use finstack_quant_core::expr::ExprNode;
    match &expr.node {
        ExprNode::BinOp { .. } => {
            // Valid
        }
        _ => panic!("Expected BinOp node"),
    }
}

/// Test multiple instrument references
#[test]
fn test_compile_multiple_instruments() {
    use finstack_quant_statements::dsl::parse_and_compile;

    let expr =
        parse_and_compile("cs.interest_expense.BOND-001 + cs.interest_expense.SWAP-001").unwrap();

    // Should compile successfully
    use finstack_quant_core::expr::ExprNode;
    match &expr.node {
        ExprNode::BinOp { .. } => {
            // Valid
        }
        _ => panic!("Expected BinOp node"),
    }
}

// Capital Structure Integration Tests

#[test]
fn test_aggregate_period_flows() {
    use finstack_quant_core::dates::{Date, build_periods};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::instruments::fixed_income::bond::Bond;

    use std::sync::Arc;
    use time::Month;

    let issue = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let bond = Bond::fixed(
        InstrumentId::new("BOND-001"),
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        CurveId::new("USD-OIS"),
    )
    .expect("Bond::fixed should succeed with valid parameters");

    let mut instruments: indexmap::IndexMap<String, Arc<dyn Instrument>> =
        indexmap::IndexMap::new();
    instruments.insert("BOND-001".to_string(), Arc::new(bond));

    let periods = build_periods("2025Q1..2025Q4", None).unwrap().periods;
    let market_ctx = MarketContext::new();

    let cashflows =
        crate::support::aggregate_period_flows(&instruments, &periods, &market_ctx, as_of);

    assert!(cashflows.is_ok());
    let cf = cashflows.unwrap();
    assert!(!cf.totals.is_empty());
}

#[test]
fn test_build_instrument_from_spec_bond_variant() {
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_statements::capital_structure::build_instrument_from_spec;
    use finstack_quant_statements::types::{DebtInstrumentSpec, FinancialStatementInstrument};
    use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
    use time::Month;

    let issue = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let bond = Bond::fixed(
        InstrumentId::new("BOND-002"),
        Money::new(2_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.06).expect("valid rate fixture"),
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        CurveId::new("USD-OIS"),
    )
    .expect("Bond::fixed should succeed with valid parameters");

    let spec = DebtInstrumentSpec {
        id: "BOND-002".to_string(),
        spec: FinancialStatementInstrument::Bond(bond),
    };

    let result = build_instrument_from_spec(&spec);
    assert!(result.is_ok());
}

#[test]
fn test_build_instrument_from_spec_swap_variant() {
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::types::InstrumentId;
    use finstack_quant_statements::capital_structure::build_instrument_from_spec;
    use finstack_quant_statements::types::{DebtInstrumentSpec, FinancialStatementInstrument};
    use finstack_quant_valuations::instruments::PayReceive;
    use time::Month;

    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let swap = crate::rates_support::usd_irs_swap(
        InstrumentId::new("SWAP-002"),
        Money::new(3_000_000.0, Currency::USD).expect("valid money fixture"),
        0.045,
        start,
        maturity,
        PayReceive::Pay,
    )
    .unwrap();

    let spec = DebtInstrumentSpec {
        id: "SWAP-002".to_string(),
        spec: FinancialStatementInstrument::InterestRateSwap(swap),
    };

    let result = build_instrument_from_spec(&spec);
    assert!(result.is_ok());
}

#[test]
fn test_debt_instrument_rejects_invalid_instrument_json() {
    use finstack_quant_statements::types::DebtInstrumentSpec;

    let result = serde_json::from_value::<DebtInstrumentSpec>(serde_json::json!({
        "id": "INVALID",
        "spec": {"invalid_field": "not a valid instrument"}
    }));
    assert!(
        result.is_err(),
        "invalid instrument JSON must fail at the typed boundary"
    );
}

#[test]
fn test_capital_structure_cashflows_accessors() {
    use finstack_quant_statements::capital_structure::{
        CapitalStructureCashflows, CashflowBreakdown,
    };

    let mut cs = CapitalStructureCashflows::new();
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");

    let breakdown = CashflowBreakdown {
        interest_expense_cash: finstack_quant_core::money::Money::new(
            10_000.0,
            finstack_quant_core::currency::Currency::USD,
        )
        .expect("valid money fixture"),
        interest_income_cash: None,
        interest_expense_pik: finstack_quant_core::money::Money::new(
            0.0,
            finstack_quant_core::currency::Currency::USD,
        )
        .expect("valid money fixture"),
        principal_payment: finstack_quant_core::money::Money::new(
            25_000.0,
            finstack_quant_core::currency::Currency::USD,
        )
        .expect("valid money fixture"),
        debt_balance: finstack_quant_core::money::Money::new(
            500_000.0,
            finstack_quant_core::currency::Currency::USD,
        )
        .expect("valid money fixture"),
        fees: finstack_quant_core::money::Money::new(
            1_000.0,
            finstack_quant_core::currency::Currency::USD,
        )
        .expect("valid money fixture"),
        accrued_interest: finstack_quant_core::money::Money::new(
            0.0,
            finstack_quant_core::currency::Currency::USD,
        )
        .expect("valid money fixture"),
        opening_balance: None,
        scheduled_principal: None,
        mandatory_prepayment: None,
        sweep_prepayment: None,
        voluntary_prepayment: None,
        interest_arrears: None,
        principal_pending: None,
        principal_arrears: None,
        principal_advance_reserve: None,
        scheduled_cash_interest_due: None,
        scheduled_principal_due: None,
    };

    let mut instrument_map = indexmap::IndexMap::new();
    instrument_map.insert(period, breakdown.clone());

    cs.by_instrument
        .insert("INST-001".to_string(), instrument_map);
    cs.totals.insert(period, breakdown);

    // Test accessors
    assert_eq!(
        cs.get_component("interest_expense", "INST-001", &period)
            .expect("interest"),
        10_000.0
    );
    assert_eq!(
        cs.get_component("principal_payment", "INST-001", &period)
            .expect("principal"),
        25_000.0
    );
    assert_eq!(
        cs.get_component("debt_balance", "INST-001", &period)
            .expect("balance"),
        500_000.0
    );
    assert_eq!(
        cs.get_component("interest_expense", "total", &period)
            .expect("total interest"),
        10_000.0
    );
    assert_eq!(
        cs.get_component("principal_payment", "total", &period)
            .expect("total principal"),
        25_000.0
    );
    assert_eq!(
        cs.get_component("debt_balance", "total", &period)
            .expect("total balance"),
        500_000.0
    );

    // Test missing instrument
    assert!(
        cs.get_component("interest_expense", "NONEXISTENT", &period)
            .is_err()
    );
    assert!(
        cs.get_component("principal_payment", "NONEXISTENT", &period)
            .is_err()
    );
    assert!(
        cs.get_component("debt_balance", "NONEXISTENT", &period)
            .is_err()
    );
}

/// A fixed-rate bond from the canonical Valuations constructor (US corporate
/// conventions: semi-annual 30/360), ready to attach with `add_debt`.
fn fixed_bond(
    id: &str,
    notional: finstack_quant_core::money::Money,
    coupon_rate: f64,
    issue: finstack_quant_core::dates::Date,
    maturity: finstack_quant_core::dates::Date,
    curve: &str,
) -> finstack_quant_statements::types::FinancialStatementInstrument {
    use finstack_quant_core::types::{CurveId, InstrumentId, Rate};
    finstack_quant_statements::types::FinancialStatementInstrument::Bond(
        finstack_quant_valuations::instruments::fixed_income::bond::Bond::fixed(
            InstrumentId::new(id),
            notional,
            Rate::from_decimal(coupon_rate).expect("valid coupon"),
            issue,
            maturity,
            finstack_quant_core::dates::StubKind::ShortFront,
            CurveId::new(curve),
        )
        .expect("valid bond"),
    )
}
