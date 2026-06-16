//! DSL tests for Phase 2.

use finstack_quant_core::expr::{ExprNode, Function};
use finstack_quant_statements::dsl::{compile, parse_and_compile, parse_formula, BinOp, StmtExpr};

// ============================================================================
// DSL Parser Tests
// ============================================================================

#[test]
fn test_parse_literal_integer() {
    let result = parse_formula("42").unwrap();
    assert_eq!(result, StmtExpr::Literal(42.0));
}

#[test]
fn test_parse_literal_float() {
    let result = parse_formula("123.456").unwrap();
    assert_eq!(result, StmtExpr::Literal(123.456));
}

#[test]
fn test_parse_literal_negative() {
    let result = parse_formula("-5.0").unwrap();
    match result {
        StmtExpr::UnaryOp { .. } => {}
        _ => panic!("Expected unary negation"),
    }
}

#[test]
fn test_parse_identifier_simple() {
    let result = parse_formula("revenue").unwrap();
    assert_eq!(result, StmtExpr::NodeRef("revenue".into()));
}

#[test]
fn test_parse_identifier_underscore() {
    let result = parse_formula("gross_profit").unwrap();
    assert_eq!(result, StmtExpr::NodeRef("gross_profit".into()));
}

#[test]
fn test_parse_cs_two_part_is_error() {
    let result = parse_formula("cs.interest_expense");
    assert!(
        result.is_err(),
        "Two-part cs.* reference must be a parse error, not a NodeRef"
    );
}

#[test]
fn test_parse_addition() {
    let result = parse_formula("1 + 2").unwrap();
    assert!(
        matches!(result, StmtExpr::BinOp { op: BinOp::Add, .. }),
        "Expected BinOp::Add, got {:?}",
        result
    );
}

#[test]
fn test_parse_subtraction() {
    let result = parse_formula("revenue - cogs").unwrap();
    match result {
        StmtExpr::BinOp { op, left, right } => {
            assert_eq!(op, BinOp::Sub);
            assert_eq!(*left, StmtExpr::NodeRef("revenue".into()));
            assert_eq!(*right, StmtExpr::NodeRef("cogs".into()));
        }
        _ => panic!("Expected BinOp::Sub"),
    }
}

#[test]
fn test_parse_multiplication() {
    let result = parse_formula("revenue * 0.6").unwrap();
    assert!(
        matches!(result, StmtExpr::BinOp { op: BinOp::Mul, .. }),
        "Expected BinOp::Mul, got {:?}",
        result
    );
}

#[test]
fn test_parse_division() {
    let result = parse_formula("gross_profit / revenue").unwrap();
    assert!(
        matches!(result, StmtExpr::BinOp { op: BinOp::Div, .. }),
        "Expected BinOp::Div, got {:?}",
        result
    );
}

#[test]
fn test_parse_modulo() {
    let result = parse_formula("period_num % 4").unwrap();
    assert!(
        matches!(result, StmtExpr::BinOp { op: BinOp::Mod, .. }),
        "Expected BinOp::Mod, got {:?}",
        result
    );
}

#[test]
fn test_parse_parentheses() {
    let result = parse_formula("(1 + 2) * 3").unwrap();
    // Check outer operation
    let StmtExpr::BinOp {
        op: BinOp::Mul,
        left,
        ..
    } = result
    else {
        panic!("Expected Mul at top level, got {:?}", result);
    };
    // Check inner operation
    assert!(
        matches!(*left, StmtExpr::BinOp { op: BinOp::Add, .. }),
        "Expected Add inside parentheses, got {:?}",
        left
    );
}

#[test]
fn test_parse_nested_parentheses() {
    let result = parse_formula("((1 + 2) * 3) / 4").unwrap();
    match result {
        StmtExpr::BinOp { op: BinOp::Div, .. } => {}
        _ => panic!("Expected division at top level"),
    }
}

#[test]
fn test_parse_comparison_gt() {
    let result = parse_formula("revenue > 1000000").unwrap();
    assert!(
        matches!(result, StmtExpr::BinOp { op: BinOp::Gt, .. }),
        "Expected BinOp::Gt, got {:?}",
        result
    );
}

#[test]
fn test_parse_comparison_gte() {
    let result = parse_formula("revenue >= 1000000").unwrap();
    assert!(
        matches!(result, StmtExpr::BinOp { op: BinOp::Ge, .. }),
        "Expected BinOp::Ge, got {:?}",
        result
    );
}

#[test]
fn test_parse_comparison_lt() {
    let result = parse_formula("margin < 0.1").unwrap();
    match result {
        StmtExpr::BinOp { op, .. } => assert_eq!(op, BinOp::Lt),
        _ => panic!("Expected BinOp::Lt"),
    }
}

#[test]
fn test_parse_comparison_eq() {
    let result = parse_formula("revenue == 1000000").unwrap();
    match result {
        StmtExpr::BinOp { op, .. } => assert_eq!(op, BinOp::Eq),
        _ => panic!("Expected BinOp::Eq"),
    }
}

#[test]
fn test_parse_comparison_ne() {
    let result = parse_formula("revenue != 0").unwrap();
    match result {
        StmtExpr::BinOp { op, .. } => assert_eq!(op, BinOp::Ne),
        _ => panic!("Expected BinOp::Ne"),
    }
}

#[test]
fn test_parse_logical_and() {
    let result = parse_formula("revenue > 1000000 and margin > 0.15").unwrap();
    match result {
        StmtExpr::BinOp { op, .. } => assert_eq!(op, BinOp::And),
        _ => panic!("Expected BinOp::And"),
    }
}

#[test]
fn test_parse_logical_or() {
    let result = parse_formula("revenue < 100000 or expenses > 50000").unwrap();
    match result {
        StmtExpr::BinOp { op, .. } => assert_eq!(op, BinOp::Or),
        _ => panic!("Expected BinOp::Or"),
    }
}

#[test]
fn test_parse_function_call_no_args() {
    // Functions must have parens even with no args
    let result = parse_formula("revenue()");
    // This should parse as a function call, not a node ref
    assert!(result.is_ok());
}

#[test]
fn test_parse_function_call_one_arg() {
    let result = parse_formula("cumsum(revenue)").unwrap();
    match result {
        StmtExpr::Call { func, args } => {
            assert_eq!(func, "cumsum");
            assert_eq!(args.len(), 1);
        }
        _ => panic!("Expected Call"),
    }
}

#[test]
fn test_parse_function_call_two_args() {
    let result = parse_formula("lag(revenue, 1)").unwrap();
    match result {
        StmtExpr::Call { func, args } => {
            assert_eq!(func, "lag");
            assert_eq!(args.len(), 2);
        }
        _ => panic!("Expected Call"),
    }
}

#[test]
fn test_parse_function_call_three_args() {
    let result = parse_formula("if(revenue > 1000000, 1, 0)").unwrap();
    match result {
        StmtExpr::IfThenElse { .. } => {}
        _ => panic!("Expected IfThenElse"),
    }
}

#[test]
fn test_parse_nested_function_calls() {
    let result = parse_formula("rolling_mean(lag(revenue, 1), 4)").unwrap();
    match result {
        StmtExpr::Call { func, args } => {
            assert_eq!(func, "rolling_mean");
            assert_eq!(args.len(), 2);
            match &args[0] {
                StmtExpr::Call { func, .. } => assert_eq!(func, "lag"),
                _ => panic!("Expected nested Call"),
            }
        }
        _ => panic!("Expected Call"),
    }
}

#[test]
fn test_parse_if_then_else() {
    let result = parse_formula("if(revenue > 1000000, revenue * 0.1, 0)").unwrap();
    match result {
        StmtExpr::IfThenElse {
            condition,
            then_expr,
            else_expr,
        } => {
            match *condition {
                StmtExpr::BinOp { op: BinOp::Gt, .. } => {}
                _ => panic!("Expected comparison in condition"),
            }
            match *then_expr {
                StmtExpr::BinOp { op: BinOp::Mul, .. } => {}
                _ => panic!("Expected multiplication in then branch"),
            }
            assert_eq!(*else_expr, StmtExpr::Literal(0.0));
        }
        _ => panic!("Expected IfThenElse"),
    }
}

#[test]
fn test_parse_operator_precedence() {
    // Should parse as 1 + (2 * 3)
    let result = parse_formula("1 + 2 * 3").unwrap();
    match result {
        StmtExpr::BinOp {
            op: BinOp::Add,
            left,
            right,
        } => {
            assert_eq!(*left, StmtExpr::Literal(1.0));
            match *right {
                StmtExpr::BinOp { op: BinOp::Mul, .. } => {}
                _ => panic!("Expected multiplication on right"),
            }
        }
        _ => panic!("Expected addition at top level"),
    }
}

#[test]
fn test_parse_complex_expression() {
    let result = parse_formula("(revenue - cogs) / revenue").unwrap();
    match result {
        StmtExpr::BinOp { op: BinOp::Div, .. } => {}
        _ => panic!("Expected division"),
    }
}

#[test]
fn test_parse_whitespace_tolerance() {
    let result1 = parse_formula("revenue-cogs").unwrap();
    let result2 = parse_formula("revenue - cogs").unwrap();
    let result3 = parse_formula("revenue  -  cogs").unwrap();

    // These should parse to the same structure (subtraction)
    assert_eq!(result1, result2);
    assert_eq!(result2, result3);
    assert!(matches!(result2, StmtExpr::BinOp { op: BinOp::Sub, .. }));
}

#[test]
fn test_parse_error_incomplete() {
    let result = parse_formula("revenue +");
    assert!(result.is_err());
}

#[test]
fn test_parse_error_invalid_operator() {
    let result = parse_formula("revenue @@ cogs");
    assert!(result.is_err());
}

#[test]
fn test_parse_error_unmatched_paren() {
    let result = parse_formula("(revenue - cogs");
    assert!(result.is_err());
}

#[test]
fn test_parse_error_empty() {
    let result = parse_formula("");
    assert!(result.is_err());
}

// ============================================================================
// DSL Compiler Tests
// ============================================================================

#[test]
fn test_compile_literal() {
    let ast = StmtExpr::literal(42.0);
    let expr = compile(&ast).unwrap();

    match expr.node {
        ExprNode::Literal(v) => assert_eq!(v, 42.0),
        _ => panic!("Expected Literal"),
    }
}

#[test]
fn test_compile_node_ref() {
    let ast = StmtExpr::node_ref("revenue");
    let expr = compile(&ast).unwrap();

    match expr.node {
        ExprNode::Column(ref name) => assert_eq!(name, "revenue"),
        _ => panic!("Expected Column"),
    }
}

#[test]
fn test_compile_arithmetic() {
    let ast = StmtExpr::bin_op(BinOp::Add, StmtExpr::literal(1.0), StmtExpr::literal(2.0));

    let expr = compile(&ast).unwrap();

    // Should compile to a BinOp expression
    match expr.node {
        ExprNode::BinOp { .. } => {}
        _ => panic!("Expected BinOp for arithmetic"),
    }
}

#[test]
fn test_compile_function_lag() {
    let ast = StmtExpr::call(
        "lag",
        vec![StmtExpr::node_ref("revenue"), StmtExpr::literal(1.0)],
    );

    let expr = compile(&ast).unwrap();

    match expr.node {
        ExprNode::Call(Function::Lag, args) => {
            assert_eq!(args.len(), 2);
        }
        _ => panic!("Expected Lag function call"),
    }
}

#[test]
fn test_compile_from_parse() {
    let ast = parse_formula("revenue - cogs").unwrap();
    let expr = compile(&ast).unwrap();

    // Should compile successfully to BinOp
    match expr.node {
        ExprNode::BinOp { .. } => {}
        _ => panic!("Expected BinOp for subtraction"),
    }
}

#[test]
fn test_compile_complex_expression() {
    let ast = parse_formula("(revenue - cogs) / revenue").unwrap();
    let expr = compile(&ast);

    assert!(expr.is_ok());
}

#[test]
fn test_parse_and_compile_integration() {
    let expr = parse_and_compile("revenue * 0.6").unwrap();

    match expr.node {
        ExprNode::BinOp { .. } => {}
        _ => panic!("Expected BinOp for multiplication"),
    }
}

// ============================================================================
// Time-Series Operators Tests
// ============================================================================

#[test]
fn test_parse_lag() {
    let result = parse_formula("lag(revenue, 1)").unwrap();
    match result {
        StmtExpr::Call { func, args } => {
            assert_eq!(func, "lag");
            assert_eq!(args.len(), 2);
        }
        _ => panic!("Expected lag call"),
    }
}

// Note: lead() function test removed - lead() is intentionally not supported
// in financial modeling to prevent forward-looking bias

#[test]
fn test_parse_diff() {
    let result = parse_formula("diff(revenue, 1)").unwrap();
    match result {
        StmtExpr::Call { func, .. } => assert_eq!(func, "diff"),
        _ => panic!("Expected diff call"),
    }
}

#[test]
fn test_parse_pct_change() {
    let result = parse_formula("pct_change(revenue, 1)").unwrap();
    match result {
        StmtExpr::Call { func, .. } => assert_eq!(func, "pct_change"),
        _ => panic!("Expected pct_change call"),
    }
}

#[test]
fn test_compile_time_series_operators() {
    // Note: 'lead' is intentionally not supported in financial modeling
    // to prevent forward-looking bias
    let functions = vec!["lag", "diff", "pct_change"];

    for func in functions {
        let formula = format!("{}(revenue, 1)", func);
        let expr = parse_and_compile(&formula).unwrap();

        assert!(
            matches!(expr.node, ExprNode::Call(..)),
            "Expected Call for {}, got {:?}",
            func,
            expr.node
        );
    }
}

// ============================================================================
// Rolling Window Functions Tests
// ============================================================================

#[test]
fn test_parse_rolling_mean() {
    let result = parse_formula("rolling_mean(revenue, 4)").unwrap();
    match result {
        StmtExpr::Call { func, args } => {
            assert_eq!(func, "rolling_mean");
            assert_eq!(args.len(), 2);
        }
        _ => panic!("Expected rolling_mean call"),
    }
}

#[test]
fn test_parse_rolling_sum() {
    let result = parse_formula("rolling_sum(revenue, 12)").unwrap();
    match result {
        StmtExpr::Call { func, .. } => assert_eq!(func, "rolling_sum"),
        _ => panic!("Expected rolling_sum call"),
    }
}

#[test]
fn test_parse_rolling_std() {
    let result = parse_formula("rolling_std(revenue, 4)").unwrap();
    match result {
        StmtExpr::Call { func, .. } => assert_eq!(func, "rolling_std"),
        _ => panic!("Expected rolling_std call"),
    }
}

#[test]
fn test_compile_rolling_functions() {
    let functions = vec!["rolling_mean", "rolling_sum", "rolling_std"];

    for func in functions {
        let formula = format!("{}(revenue, 4)", func);
        let expr = parse_and_compile(&formula).unwrap();

        match expr.node {
            ExprNode::Call(..) => {}
            _ => panic!("Expected Call for {}", func),
        }
    }
}

#[test]
fn test_parse_ttm_equivalent() {
    // TTM is typically rolling_sum(revenue, 4) for quarterly or 12 for monthly
    let result = parse_formula("rolling_sum(revenue, 4)").unwrap();
    match result {
        StmtExpr::Call { func, .. } => {
            assert_eq!(func, "rolling_sum");
            // TTM = Trailing Twelve Months
            // For quarterly data: 4 periods
            // For monthly data: 12 periods
        }
        _ => panic!("Expected rolling_sum for TTM"),
    }
}

// ============================================================================
// Statistical Functions Tests
// ============================================================================

#[test]
fn test_parse_statistical_functions() {
    let functions = vec!["mean", "median", "std", "var"];

    for func in functions {
        let formula = format!("{}(revenue)", func);
        let result = parse_formula(&formula).unwrap();

        match result {
            StmtExpr::Call { func: f, .. } => assert_eq!(f, func),
            _ => panic!("Expected {} call", func),
        }
    }
}

#[test]
fn test_compile_statistical_functions() {
    let expr = parse_and_compile("std(revenue)").unwrap();

    match expr.node {
        ExprNode::Call(Function::Std, args) => {
            assert_eq!(args.len(), 1);
        }
        _ => panic!("Expected Std function call"),
    }
}

// ============================================================================
// Custom Functions Tests
// ============================================================================

#[test]
fn test_parse_custom_sum() {
    let result = parse_formula("sum(revenue, other_income)").unwrap();
    match result {
        StmtExpr::Call { func, args } => {
            assert_eq!(func, "sum");
            assert_eq!(args.len(), 2);
        }
        _ => panic!("Expected sum call"),
    }
}

#[test]
fn test_parse_custom_annualize() {
    let result = parse_formula("annualize(net_income, 4)").unwrap();
    match result {
        StmtExpr::Call { func, args } => {
            assert_eq!(func, "annualize");
            assert_eq!(args.len(), 2);
        }
        _ => panic!("Expected annualize call"),
    }
}

#[test]
fn test_parse_custom_ttm() {
    let result = parse_formula("ttm(ebitda)").unwrap();
    match result {
        StmtExpr::Call { func, args } => {
            assert_eq!(func, "ttm");
            assert_eq!(args.len(), 1);
        }
        _ => panic!("Expected ttm call"),
    }
}

#[test]
fn test_parse_custom_ltm() {
    let result = parse_formula("ltm(revenue)").unwrap();
    match result {
        StmtExpr::Call { func, args } => {
            assert_eq!(func, "ltm");
            assert_eq!(args.len(), 1);
        }
        _ => panic!("Expected ltm call"),
    }
}

#[test]
fn test_parse_custom_ytd() {
    let result = parse_formula("ytd(revenue)").unwrap();
    match result {
        StmtExpr::Call { func, args } => {
            assert_eq!(func, "ytd");
            assert_eq!(args.len(), 1);
        }
        _ => panic!("Expected ytd call"),
    }
}

#[test]
fn test_parse_custom_qtd() {
    let result = parse_formula("qtd(revenue)").unwrap();
    match result {
        StmtExpr::Call { func, args } => {
            assert_eq!(func, "qtd");
            assert_eq!(args.len(), 1);
        }
        _ => panic!("Expected qtd call"),
    }
}

#[test]
fn test_parse_custom_fiscal_ytd() {
    let result = parse_formula("fiscal_ytd(revenue, 4)").unwrap();
    match result {
        StmtExpr::Call { func, args } => {
            assert_eq!(func, "fiscal_ytd");
            assert_eq!(args.len(), 2);
        }
        _ => panic!("Expected fiscal_ytd call"),
    }
}

#[test]
fn test_parse_custom_coalesce() {
    let result = parse_formula("coalesce(bonus, 0)").unwrap();
    match result {
        StmtExpr::Call { func, args } => {
            assert_eq!(func, "coalesce");
            assert_eq!(args.len(), 2);
        }
        _ => panic!("Expected coalesce call"),
    }
}

#[test]
fn test_compile_custom_functions() {
    // Custom functions are compiled to core expression functions
    let tests = vec![
        ("sum(revenue, cogs)", true),
        ("mean(revenue, cogs)", true),
        ("annualize(revenue, 4)", true),
        ("ttm(revenue)", true),
        ("ltm(revenue)", true),
        ("ytd(revenue)", true),
        ("qtd(revenue)", true),
        ("fiscal_ytd(revenue, 4)", true),
        ("coalesce(revenue, 0)", true),
    ];

    for (formula, should_succeed) in tests {
        let expr = parse_and_compile(formula);
        assert_eq!(
            expr.is_ok(),
            should_succeed,
            "Formula '{}' compilation result unexpected",
            formula
        );
    }
}

// ============================================================================
// Complex Integration Tests
// ============================================================================

#[test]
fn test_complex_margin_calculation() {
    let formula = "(revenue - cogs) / revenue";
    let expr = parse_and_compile(formula);
    assert!(expr.is_ok());
}

#[test]
fn test_complex_yoy_growth() {
    let formula = "pct_change(revenue, 4)";
    let expr = parse_and_compile(formula);
    assert!(expr.is_ok());
}

#[test]
fn test_complex_conditional_bonus() {
    let formula = "if(revenue > 1000000, revenue * 0.1, 0)";
    let expr = parse_and_compile(formula);
    assert!(expr.is_ok());
}

#[test]
fn test_complex_nested_operations() {
    let formula = "rolling_mean(pct_change(revenue, 1), 4)";
    let expr = parse_and_compile(formula);
    assert!(expr.is_ok());
}

#[test]
fn test_complex_leverage_ratio() {
    let formula = "debt_balance / rolling_sum(ebitda, 4)";
    let expr = parse_and_compile(formula);
    assert!(expr.is_ok());
}
