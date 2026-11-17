//! Public-boundary tests for the statements formula DSL.

use finstack_quant_core::expr::{BinOp, ExprNode, Function, UnaryOp};
use finstack_quant_statements::dsl::{StmtExpr, parse_and_compile, parse_formula};

#[test]
fn operators_precedence_and_conditionals_parse_and_compile_consistently() {
    let binary_cases = [
        ("lhs + rhs", BinOp::Add),
        ("lhs - rhs", BinOp::Sub),
        ("lhs * rhs", BinOp::Mul),
        ("lhs / rhs", BinOp::Div),
        ("lhs % rhs", BinOp::Mod),
        ("lhs == rhs", BinOp::Eq),
        ("lhs != rhs", BinOp::Ne),
        ("lhs < rhs", BinOp::Lt),
        ("lhs <= rhs", BinOp::Le),
        ("lhs > rhs", BinOp::Gt),
        ("lhs >= rhs", BinOp::Ge),
        ("lhs and rhs", BinOp::And),
        ("lhs or rhs", BinOp::Or),
    ];

    for (formula, operator) in binary_cases {
        let parsed = parse_formula(formula).expect("binary expression should parse");
        assert!(
            matches!(parsed, StmtExpr::BinOp { op, .. } if op == operator),
            "statement operator mapping changed for {formula}"
        );

        let compiled = parse_and_compile(formula).expect("binary expression should compile");
        assert!(
            matches!(compiled.node, ExprNode::BinOp { op, .. } if op == operator),
            "core operator mapping changed for {formula}"
        );
    }

    let unary_cases = [
        ("-revenue", UnaryOp::Neg),
        ("!flag", UnaryOp::Not),
        ("not revenue > 0", UnaryOp::Not),
    ];

    for (formula, operator) in unary_cases {
        let parsed = parse_formula(formula).expect("unary expression should parse");
        assert!(
            matches!(parsed, StmtExpr::UnaryOp { op, .. } if op == operator),
            "statement unary mapping changed for {formula}"
        );

        let compiled = parse_and_compile(formula).expect("unary expression should compile");
        assert!(
            matches!(compiled.node, ExprNode::UnaryOp { op, .. } if op == operator),
            "core unary mapping changed for {formula}"
        );
    }

    let precedence = parse_formula("1 + 2 * 3").expect("precedence expression should parse");
    let StmtExpr::BinOp {
        op: BinOp::Add,
        left,
        right,
    } = precedence
    else {
        panic!("addition should be the top-level precedence node");
    };
    assert_eq!(*left, StmtExpr::Literal(1.0));
    assert!(matches!(*right, StmtExpr::BinOp { op: BinOp::Mul, .. }));

    let grouped = parse_formula("(1 + 2) * 3").expect("grouped expression should parse");
    let StmtExpr::BinOp {
        op: BinOp::Mul,
        left,
        ..
    } = grouped
    else {
        panic!("multiplication should be the top-level grouped node");
    };
    assert!(matches!(*left, StmtExpr::BinOp { op: BinOp::Add, .. }));

    let nested_grouped = parse_formula("((1 + 2) * 3) / 4").expect("nested grouping should parse");
    assert!(matches!(
        nested_grouped,
        StmtExpr::BinOp { op: BinOp::Div, .. }
    ));

    let compact = parse_formula("revenue-cogs").expect("compact expression should parse");
    let spaced = parse_formula("revenue - cogs").expect("spaced expression should parse");
    let padded = parse_formula("revenue  -  cogs").expect("padded expression should parse");
    assert_eq!(compact, spaced);
    assert_eq!(spaced, padded);

    let conditional = "if(revenue > 1000000, revenue * 0.1, 0)";
    let parsed = parse_formula(conditional).expect("conditional should parse");
    let StmtExpr::IfThenElse {
        condition,
        then_expr,
        else_expr,
    } = parsed
    else {
        panic!("if() should lower to the statements conditional node");
    };
    assert!(matches!(*condition, StmtExpr::BinOp { op: BinOp::Gt, .. }));
    assert!(matches!(*then_expr, StmtExpr::BinOp { op: BinOp::Mul, .. }));
    assert_eq!(*else_expr, StmtExpr::Literal(0.0));

    let compiled = parse_and_compile(conditional).expect("conditional should compile");
    let ExprNode::IfThenElse {
        condition,
        then_expr,
        else_expr,
    } = compiled.node
    else {
        panic!("if() should lower to the core conditional node");
    };
    assert!(matches!(
        condition.node,
        ExprNode::BinOp { op: BinOp::Gt, .. }
    ));
    assert!(matches!(
        then_expr.node,
        ExprNode::BinOp { op: BinOp::Mul, .. }
    ));
    assert!(matches!(else_expr.node, ExprNode::Literal(value) if value == 0.0));
}

#[test]
fn function_names_arities_and_core_mappings_stay_in_sync() {
    let cases = [
        ("lag(revenue, 1)", "lag", 2, Function::Lag),
        ("diff(revenue, 1)", "diff", 2, Function::Diff),
        (
            "pct_change(revenue, 1)",
            "pct_change",
            2,
            Function::PctChange,
        ),
        ("cumsum(revenue)", "cumsum", 1, Function::CumSum),
        ("cumprod(revenue)", "cumprod", 1, Function::CumProd),
        ("cummin(revenue)", "cummin", 1, Function::CumMin),
        ("cummax(revenue)", "cummax", 1, Function::CumMax),
        (
            "rolling_mean(revenue, 4)",
            "rolling_mean",
            2,
            Function::RollingMean,
        ),
        (
            "rolling_sum(revenue, 4)",
            "rolling_sum",
            2,
            Function::RollingSum,
        ),
        (
            "rolling_std(revenue, 4)",
            "rolling_std",
            2,
            Function::RollingStd,
        ),
        (
            "rolling_var(revenue, 4)",
            "rolling_var",
            2,
            Function::RollingVar,
        ),
        (
            "rolling_median(revenue, 4)",
            "rolling_median",
            2,
            Function::RollingMedian,
        ),
        (
            "rolling_min(revenue, 4)",
            "rolling_min",
            2,
            Function::RollingMin,
        ),
        (
            "rolling_max(revenue, 4)",
            "rolling_max",
            2,
            Function::RollingMax,
        ),
        (
            "rolling_count(revenue, 4)",
            "rolling_count",
            2,
            Function::RollingCount,
        ),
        ("ewm_mean(revenue, 0.5)", "ewm_mean", 2, Function::EwmMean),
        ("ewm_std(revenue, 0.5)", "ewm_std", 2, Function::EwmStd),
        ("ewm_var(revenue, 0.5)", "ewm_var", 2, Function::EwmVar),
        ("std(revenue)", "std", 1, Function::Std),
        ("var(revenue)", "var", 1, Function::Var),
        ("median(revenue)", "median", 1, Function::Median),
        ("shift(revenue, 1)", "shift", 2, Function::Shift),
        ("rank(revenue)", "rank", 1, Function::Rank),
        ("quantile(revenue, 0.5)", "quantile", 2, Function::Quantile),
        ("sum(revenue, cogs)", "sum", 2, Function::Sum),
        ("mean(revenue, cogs)", "mean", 2, Function::Mean),
        ("min(revenue, cogs)", "min", 2, Function::Min),
        ("max(revenue, cogs)", "max", 2, Function::Max),
        ("annualize(revenue, 4)", "annualize", 2, Function::Annualize),
        (
            "annualize_rate(rate, 4, 1)",
            "annualize_rate",
            3,
            Function::AnnualizeRate,
        ),
        ("ttm(ebitda)", "ttm", 1, Function::Ttm),
        ("ytd(revenue)", "ytd", 1, Function::Ytd),
        ("qtd(revenue)", "qtd", 1, Function::Qtd),
        (
            "fiscal_ytd(revenue, 4)",
            "fiscal_ytd",
            2,
            Function::FiscalYtd,
        ),
        ("coalesce(revenue, 0)", "coalesce", 2, Function::Coalesce),
        ("abs(revenue)", "abs", 1, Function::Abs),
        ("sign(revenue)", "sign", 1, Function::Sign),
        (
            "growth_rate(revenue, 4)",
            "growth_rate",
            2,
            Function::GrowthRate,
        ),
        ("pow(growth, 2)", "pow", 2, Function::Pow),
        ("round(revenue, 2)", "round", 2, Function::Round),
        ("floor(revenue)", "floor", 1, Function::Floor),
        ("ceil(revenue)", "ceil", 1, Function::Ceil),
        ("ln(revenue)", "ln", 1, Function::Ln),
        ("exp(growth)", "exp", 1, Function::Exp),
        ("log10(revenue)", "log10", 1, Function::Log10),
        ("sqrt(revenue)", "sqrt", 1, Function::Sqrt),
        ("clamp(margin, 0, 1)", "clamp", 3, Function::Clamp),
        ("is_missing(revenue)", "is_missing", 1, Function::IsMissing),
    ];

    for (formula, expected_name, expected_arity, expected_function) in cases {
        let parsed = parse_formula(formula).expect("supported function should parse");
        let StmtExpr::Call { func, args } = parsed else {
            panic!("{formula} should parse as a function call");
        };
        assert_eq!(
            func, expected_name,
            "DSL function name changed for {formula}"
        );
        assert_eq!(
            args.len(),
            expected_arity,
            "DSL argument count changed for {formula}"
        );

        let compiled = parse_and_compile(formula).expect("supported function should compile");
        let ExprNode::Call(function, args) = compiled.node else {
            panic!("{formula} should compile as a core function call");
        };
        assert_eq!(
            function, expected_function,
            "core function mapping changed for {formula}"
        );
        assert_eq!(
            args.len(),
            expected_arity,
            "compiled argument count changed for {formula}"
        );
    }

    let nested = "rolling_mean(lag(revenue, 1), 4)";
    let parsed = parse_formula(nested).expect("nested function should parse");
    let StmtExpr::Call { func, args } = parsed else {
        panic!("nested expression should have a function-call root");
    };
    assert_eq!(func, "rolling_mean");
    assert_eq!(args.len(), 2);
    assert!(matches!(
        &args[0],
        StmtExpr::Call { func, args } if func == "lag" && args.len() == 2
    ));

    let compiled = parse_and_compile(nested).expect("nested function should compile");
    let ExprNode::Call(Function::RollingMean, args) = compiled.node else {
        panic!("nested expression should compile with rolling_mean at the root");
    };
    assert_eq!(args.len(), 2);
    assert!(matches!(
        &args[0].node,
        ExprNode::Call(Function::Lag, lag_args) if lag_args.len() == 2
    ));
}

#[test]
fn malformed_and_unsupported_formulas_fail_at_the_public_boundary() {
    let invalid_formulas = [
        "",
        "revenue +",
        "revenue @@ cogs",
        "(revenue - cogs",
        "cs.interest_expense",
    ];

    for formula in invalid_formulas {
        assert!(
            parse_formula(formula).is_err(),
            "malformed formula unexpectedly parsed: {formula}"
        );
    }

    let compile_failures = [
        "revenue()",
        "lead(revenue, 1)",
        "lag(revenue)",
        "diff()",
        "pct_change(revenue, 1, 2)",
        "cumsum()",
        "rolling_mean(revenue)",
        "ewm_mean(revenue)",
        "ewm_std(revenue)",
        "std()",
        "shift(revenue)",
        "rank()",
        "quantile(revenue)",
        "sum()",
        "mean()",
        "min()",
        "max()",
        "annualize()",
        "annualize_rate(rate, 4)",
        "ttm(revenue, 4)",
        "ltm(revenue)",
        "ltm(revenue, 4)",
        "ytd()",
        "qtd(revenue, 4)",
        "fiscal_ytd(revenue)",
        "coalesce(revenue)",
        "abs(revenue, 1)",
        "sign()",
        "growth_rate(revenue, 1, 2)",
        "pow(revenue)",
        "pow(revenue, 2, 3)",
        "round()",
        "round(revenue, 2, 3)",
        "floor(revenue, 1)",
        "ceil()",
        "ln(revenue, 1)",
        "exp()",
        "log10(revenue, 1)",
        "sqrt()",
        "clamp(revenue, 0)",
        "clamp(revenue, 0, 1, 2)",
        "is_missing()",
        "is_missing(revenue, 1)",
    ];

    for formula in compile_failures {
        assert!(
            parse_formula(formula).is_ok(),
            "compile-failure fixture must remain syntactically valid: {formula}"
        );
        assert!(
            parse_and_compile(formula).is_err(),
            "unsupported function or arity unexpectedly compiled: {formula}"
        );
    }
}

/// STATEMENTS-CONTRACTS-004 clause 3: the seven cumulative and full-history
/// functions take exactly one argument.
#[test]
fn unary_history_functions_reject_surplus_arguments() {
    for name in [
        "cumsum", "cumprod", "cummin", "cummax", "std", "var", "median",
    ] {
        parse_and_compile(&format!("{name}(revenue)")).expect("one argument compiles");
        for surplus in [format!("{name}(revenue, cogs)"), format!("{name}()")] {
            let error = parse_and_compile(&surplus).expect_err("wrong arity is rejected");
            assert!(
                error.to_string().contains("exactly 1 argument"),
                "{surplus}: {error}"
            );
        }
    }
}

/// Clause 1: `!` binds at unary precedence, `not` outside a comparison, and
/// nested comparisons need parentheses.
#[test]
fn negation_and_comparison_grouping_follow_the_accepted_grammar() {
    let a_gt_b = || StmtExpr::bin_op(BinOp::Gt, StmtExpr::node_ref("a"), StmtExpr::node_ref("b"));
    assert_eq!(
        parse_formula("!a > b").expect("parses"),
        StmtExpr::bin_op(
            BinOp::Gt,
            StmtExpr::unary_op(UnaryOp::Not, StmtExpr::node_ref("a")),
            StmtExpr::node_ref("b"),
        )
    );
    assert_eq!(
        parse_formula("not a > b").expect("parses"),
        StmtExpr::unary_op(UnaryOp::Not, a_gt_b())
    );
    parse_formula("a > b > c").expect_err("a chained comparison needs parentheses");
    parse_formula("(a > b) > c").expect("an explicit grouping parses");
}

/// Clause 1: canonical rendering re-parses to the same grouping, for parsed
/// text and for trees a caller builds directly.
#[test]
fn canonical_rendering_preserves_grouping() {
    for source in [
        "!a > b",
        "not a > b",
        "-!a",
        "!-a",
        "!(a and b)",
        "not (a or b) and c",
        "(a > b) == (c > d)",
        "(a > b) > c",
        "a - (b - c)",
        "a / (b * c)",
        "-(a + b) * c",
        "if(not a > b, -(a - b), !a)",
        "min(a, -b, (a > b) + 1)",
    ] {
        let ast = parse_formula(source).expect("valid source");
        let rendered = ast.to_string();
        assert_eq!(
            parse_formula(&rendered).expect("rendered text parses"),
            ast,
            "{source} rendered as {rendered}"
        );
    }

    let a = || StmtExpr::node_ref("a");
    let b = || StmtExpr::node_ref("b");
    let built = [
        StmtExpr::bin_op(
            BinOp::Lt,
            StmtExpr::bin_op(BinOp::Gt, a(), b()),
            StmtExpr::bin_op(BinOp::Eq, a(), b()),
        ),
        StmtExpr::bin_op(BinOp::Gt, StmtExpr::unary_op(UnaryOp::Not, a()), b()),
        StmtExpr::unary_op(UnaryOp::Neg, StmtExpr::unary_op(UnaryOp::Not, a())),
        StmtExpr::unary_op(
            UnaryOp::Not,
            StmtExpr::bin_op(BinOp::Or, a(), StmtExpr::bin_op(BinOp::And, a(), b())),
        ),
        StmtExpr::bin_op(BinOp::Sub, a(), StmtExpr::bin_op(BinOp::Sub, a(), b())),
        StmtExpr::bin_op(
            BinOp::Mul,
            StmtExpr::bin_op(BinOp::And, a(), b()),
            StmtExpr::unary_op(UnaryOp::Neg, StmtExpr::bin_op(BinOp::Add, a(), b())),
        ),
    ];
    for ast in built {
        let rendered = ast.to_string();
        assert_eq!(
            parse_formula(&rendered).expect("rendered text parses"),
            ast,
            "built tree rendered as {rendered}"
        );
    }
}

/// Clause 2: identifiers are nonempty dot-separated segments.
#[test]
fn identifiers_need_nonempty_dot_separated_segments() {
    for valid in ["fin.gross_margin", "a.b.c", "_private", "rev.2024"] {
        assert_eq!(
            parse_formula(valid).expect("valid identifier"),
            StmtExpr::node_ref(valid)
        );
    }
    for invalid in [
        "a..b",
        "a.",
        ".a",
        "a.b.",
        "cs.debt_balance",
        "cs.debt_balance.",
    ] {
        parse_formula(invalid).expect_err(invalid);
    }
}

/// Clause 3: an invalid expression is rejected in a branch evaluation would
/// skip, and a literal future read is rejected for every offset function.
#[test]
fn untaken_branches_and_future_reads_are_rejected() {
    for formula in [
        "if(1, revenue, unknown_function(revenue))",
        "if(0, cumsum(revenue, cogs), revenue)",
        "1 or lag(revenue)",
        "if(1, revenue, cs.not_a_component.total)",
        "if(1, revenue, lead(revenue, 1))",
    ] {
        parse_and_compile(formula).expect_err(formula);
    }
    for name in ["lag", "shift", "diff", "pct_change", "growth_rate"] {
        parse_and_compile(&format!("{name}(revenue, 1)")).expect("a past read compiles");
        for offset in ["-1", "-(1)", "- 2"] {
            let formula = format!("if(1, revenue, {name}(revenue, {offset}))");
            let error = parse_and_compile(&formula).expect_err("a future read is rejected");
            assert!(
                error.to_string().contains("future period"),
                "{formula}: {error}"
            );
        }
    }
}

/// Clause 4: text is limited to 64 levels of nesting and 256 terms.
#[test]
fn text_limits_are_inclusive_at_the_boundary() {
    let chain = |terms: usize| {
        std::iter::repeat_n("1", terms)
            .collect::<Vec<_>>()
            .join(" + ")
    };
    parse_and_compile(&chain(256)).expect("256 terms are accepted");
    let error = parse_and_compile(&chain(257)).expect_err("257 terms are rejected");
    assert!(
        error.to_string().contains("maximum of 256 terms"),
        "{error}"
    );

    let negated = |levels: usize| format!("{}1", "-".repeat(levels));
    parse_and_compile(&negated(61)).expect("nesting within the limit is accepted");
    let error = parse_and_compile(&negated(65)).expect_err("65 levels are rejected");
    assert!(error.to_string().contains("maximum depth 64"), "{error}");
}

/// Clause 4: a caller-built tree is validated before any recursive pass.
#[test]
fn caller_built_trees_are_preflighted() {
    use finstack_quant_statements::dsl::compile;

    for literal in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let ast = StmtExpr::bin_op(
            BinOp::Add,
            StmtExpr::node_ref("revenue"),
            StmtExpr::if_then_else(
                StmtExpr::literal(1.0),
                StmtExpr::literal(2.0),
                StmtExpr::literal(literal),
            ),
        );
        let error = compile(&ast).expect_err("a non-finite literal is rejected");
        assert!(error.to_string().contains("finite"), "{error}");
    }

    for identifier in ["", "a..b", "a.", "1a", "a b", "cs.debt_balance.total"] {
        compile(&StmtExpr::node_ref(identifier)).expect_err(identifier);
    }
    for (component, target) in [
        ("debt_balance", ""),
        ("debt_balance", "a b"),
        ("nope", "total"),
    ] {
        let ast = StmtExpr::CsRef {
            component: component.into(),
            instrument_or_total: target.into(),
        };
        compile(&ast).expect_err("an invalid capital-structure reference is rejected");
    }
    compile(&StmtExpr::call("if", vec![StmtExpr::literal(1.0); 3]))
        .expect_err("`if` is a conditional, not a callable function");
    compile(&StmtExpr::call("median", vec![StmtExpr::node_ref("a"); 2]))
        .expect_err("surplus arguments are rejected for a built call");

    let sum_of = |terms: usize| {
        (1..terms).fold(StmtExpr::literal(1.0), |acc, _| {
            StmtExpr::bin_op(BinOp::Add, acc, StmtExpr::literal(1.0))
        })
    };
    compile(&sum_of(256)).expect("256 terms are accepted");
    let error = compile(&sum_of(257)).expect_err("257 terms are rejected");
    assert!(
        error.to_string().contains("maximum of 256 terms"),
        "{error}"
    );

    // One term under 100,000 negations: only the depth limit can reject it,
    // and a recursive validator would overflow the stack first.
    let negated = |levels: usize| {
        (0..levels).fold(StmtExpr::node_ref("a"), |acc, _| {
            StmtExpr::unary_op(UnaryOp::Neg, acc)
        })
    };
    compile(&negated(319)).expect("a tree 320 levels deep is accepted");
    let too_deep = negated(100_000);
    let error = compile(&too_deep).expect_err("a deeper tree is rejected");
    assert!(
        error.to_string().contains("maximum depth of 320"),
        "{error}"
    );
    // The derived `Drop` recurses to the tree's depth; this test only
    // qualifies the validator.
    std::mem::forget(too_deep);
}
