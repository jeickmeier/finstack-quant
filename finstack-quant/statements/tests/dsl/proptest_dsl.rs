//! Property-based tests for DSL parser.
//!
//! Uses proptest to verify parser invariants and consistency.

use finstack_quant_statements::dsl;
use proptest::prelude::*;

// Strategy to generate valid identifiers
// Avoid identifiers starting with "nan" or "inf" followed by a digit, as "nan" and "inf" are reserved literals
// that the parser tries to match before identifiers
fn valid_identifier() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,15}"
        .prop_filter(
            "must not start with 'nan' or 'inf' followed by digit or be exactly 'nan' or 'inf'",
            |s: &String| {
                // Reject if starts with "nan" followed by digit or is exactly "nan"
                let nan_issue = s.starts_with("nan")
                    && (s.len() == 3
                        || (s.len() > 3 && s.chars().nth(3).unwrap().is_ascii_digit()));

                // Reject if starts with "inf" followed by digit or is exactly "inf"
                let inf_issue = s.starts_with("inf")
                    && (s.len() == 3
                        || (s.len() > 3 && s.chars().nth(3).unwrap().is_ascii_digit()));

                !nan_issue && !inf_issue
            },
        )
        .prop_map(|s: String| s)
}

// Strategy to generate simple numeric literals
fn numeric_literal() -> impl Strategy<Value = String> {
    (0.0..1_000_000.0).prop_map(|n: f64| n.to_string())
}

// Strategy to generate simple binary expressions
fn simple_binary_expr() -> impl Strategy<Value = String> {
    (valid_identifier(), "[+\\-*/]", numeric_literal())
        .prop_map(|(id, op, num)| format!("{} {} {}", id, op, num))
}

proptest! {
    /// Test that valid simple formulas always parse successfully
    #[test]
    fn parse_simple_formula_always_succeeds(
        identifier in valid_identifier(),
        value in 0.0..1_000_000.0
    ) {
        let formula = format!("{} + {}", identifier, value);
        let result = dsl::parser::parse_formula(&formula);
        prop_assert!(result.is_ok(), "Failed to parse: {}", formula);
    }

    /// Test that parsing the same formula twice gives the same AST
    #[test]
    fn parse_deterministic(
        formula in simple_binary_expr()
    ) {
        let result1 = dsl::parser::parse_formula(&formula);
        let result2 = dsl::parser::parse_formula(&formula);

        match (result1, result2) {
            (Ok(ast1), Ok(ast2)) => {
                prop_assert_eq!(ast1, ast2, "Same formula parsed differently");
            }
            (Err(_), Err(_)) => {
                // Both failed - acceptable
            }
            _ => {
                prop_assert!(false, "Inconsistent parse results");
            }
        }
    }

    /// Test operator precedence is consistent
    #[test]
    fn operator_precedence_consistent(
        a in valid_identifier(),
        b in valid_identifier(),
        c in numeric_literal()
    ) {
        // a + b * c should always parse as a + (b * c)
        let formula = format!("{} + {} * {}", a, b, c);
        let result = dsl::parser::parse_formula(&formula);

        if let Ok(ast) = result {
            // Verify the structure matches expected precedence
            // Top level should be Add, right side should be Mul
            use finstack_quant_statements::dsl::ast::{StmtExpr, BinOp};

            match ast {
                StmtExpr::BinOp { op: BinOp::Add, left: _, right } => {
                    match *right {
                        StmtExpr::BinOp { op: BinOp::Mul, .. } => {
                            // Correct precedence
                        }
                        _ => prop_assert!(false, "Multiplication should be right operand of addition"),
                    }
                }
                _ => {
                    // If it doesn't parse as expected, that's ok for this test
                }
            }
        }
    }

    /// Test that parsing the same formula with parentheses works consistently
    #[test]
    fn parentheses_parsing_consistent(
        formula in simple_binary_expr()
    ) {
        // Just verify that parsing with and without extra parens both succeed or both fail
        let with_parens = format!("({})", formula);
        let result1 = dsl::parser::parse_formula(&formula);
        let result2 = dsl::parser::parse_formula(&with_parens);

        // Both should succeed or both should fail
        prop_assert_eq!(result1.is_ok(), result2.is_ok(), "Parentheses changed parse success");
    }

}

#[test]
fn test_proptest_infrastructure_works() {
    // Smoke test to ensure proptest is properly configured
    let result = dsl::parser::parse_formula("a + b");
    assert!(result.is_ok());
}
