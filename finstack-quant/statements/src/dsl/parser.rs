//! Parser for Statements DSL formulas.

use crate::dsl::ast::{BinOp, StmtExpr, UnaryOp};
use crate::error::{Error, Result};
use crate::types::NodeId;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::{char, multispace0, multispace1},
    combinator::{map, opt, recognize, verify},
    multi::separated_list0,
    number::complete::double,
    sequence::{delimited, pair, preceded},
    IResult, Parser,
};
use std::cell::Cell;

const MAX_PARSE_DEPTH: usize = 64;

thread_local! {
    static PARSE_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// Parse a formula string into a [`StmtExpr`] AST.
///
/// # Arguments
/// * `input` - Text of the DSL formula to parse
///
/// # Returns
/// Parsed AST ready for compilation. On failure the returned [`Error`]
/// includes the line and column where parsing stopped.
pub fn parse_formula(input: &str) -> Result<StmtExpr> {
    PARSE_DEPTH.with(|depth| depth.set(0));
    match expression(input) {
        Ok(("", expr)) => Ok(expr),
        Ok((remaining, _)) => {
            let (line, col) = offset_to_line_col(input, input.len() - remaining.len());
            Err(Error::formula_parse(format!(
                "unexpected input at line {line} col {col}: '{remaining}'"
            )))
        }
        Err(nom::Err::Failure(err)) if err.code == nom::error::ErrorKind::TooLarge => {
            let (line, col) = offset_to_line_col(input, input.len() - err.input.len());
            Err(Error::formula_parse(format!(
                "parse nesting exceeds maximum depth {MAX_PARSE_DEPTH} at line {line} col {col}"
            )))
        }
        Err(nom::Err::Error(err) | nom::Err::Failure(err)) => {
            let (line, col) = offset_to_line_col(input, input.len() - err.input.len());
            let snippet = err.input.chars().take(24).collect::<String>();
            Err(Error::formula_parse(format!(
                "parse error at line {line} col {col}: near '{snippet}' ({:?})",
                err.code
            )))
        }
        Err(nom::Err::Incomplete(_)) => Err(Error::formula_parse(
            "parse error: incomplete input (internal: streaming combinator used)".to_string(),
        )),
    }
}

/// Convert a byte offset into `input` to a 1-indexed (line, column) pair.
fn offset_to_line_col(input: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(input.len());
    let prefix = &input[..offset];
    let line = 1 + prefix.bytes().filter(|&b| b == b'\n').count();
    let last_nl = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let col = 1 + input[last_nl..offset].chars().count();
    (line, col)
}

// Expression parser entry point (handles operator precedence)
fn expression(input: &str) -> IResult<&str, StmtExpr> {
    with_parse_depth(input, logical_or)
}

// Logical OR (lowest precedence)
fn logical_or(input: &str) -> IResult<&str, StmtExpr> {
    let (input, first) = logical_and(input)?;
    let (input, rest) = nom::multi::many0(preceded(
        delimited(multispace0, tag("or"), multispace1),
        logical_and,
    ))
    .parse(input)?;

    Ok((
        input,
        rest.into_iter()
            .fold(first, |acc, expr| StmtExpr::bin_op(BinOp::Or, acc, expr)),
    ))
}

// Logical AND
fn logical_and(input: &str) -> IResult<&str, StmtExpr> {
    let (input, first) = comparison(input)?;
    let (input, rest) = nom::multi::many0(preceded(
        delimited(multispace0, tag("and"), multispace1),
        comparison,
    ))
    .parse(input)?;

    Ok((
        input,
        rest.into_iter()
            .fold(first, |acc, expr| StmtExpr::bin_op(BinOp::And, acc, expr)),
    ))
}

// Comparison operators
fn comparison(input: &str) -> IResult<&str, StmtExpr> {
    let (input, first) = additive(input)?;

    let (input, opt_op_and_expr) = opt((
        delimited(
            multispace0,
            alt((
                map(tag("=="), |_| BinOp::Eq),
                map(tag("!="), |_| BinOp::Ne),
                map(tag("<="), |_| BinOp::Le),
                map(tag(">="), |_| BinOp::Ge),
                map(tag("<"), |_| BinOp::Lt),
                map(tag(">"), |_| BinOp::Gt),
            )),
            multispace0,
        ),
        additive,
    ))
    .parse(input)?;

    match opt_op_and_expr {
        Some((op, second)) => Ok((input, StmtExpr::bin_op(op, first, second))),
        None => Ok((input, first)),
    }
}

// Addition and subtraction
fn additive(input: &str) -> IResult<&str, StmtExpr> {
    let (input, first) = multiplicative(input)?;
    let (input, rest) = nom::multi::many0((
        delimited(
            multispace0,
            alt((
                map(char('+'), |_| BinOp::Add),
                map(char('-'), |_| BinOp::Sub),
            )),
            multispace0,
        ),
        multiplicative,
    ))
    .parse(input)?;

    Ok((
        input,
        rest.into_iter()
            .fold(first, |acc, (op, expr)| StmtExpr::bin_op(op, acc, expr)),
    ))
}

// Multiplication, division, and modulo
fn multiplicative(input: &str) -> IResult<&str, StmtExpr> {
    let (input, first) = unary(input)?;
    let (input, rest) = nom::multi::many0((
        delimited(
            multispace0,
            alt((
                map(char('*'), |_| BinOp::Mul),
                map(char('/'), |_| BinOp::Div),
                map(char('%'), |_| BinOp::Mod),
            )),
            multispace0,
        ),
        unary,
    ))
    .parse(input)?;

    Ok((
        input,
        rest.into_iter()
            .fold(first, |acc, (op, expr)| StmtExpr::bin_op(op, acc, expr)),
    ))
}

// Unary operators
fn unary(input: &str) -> IResult<&str, StmtExpr> {
    with_parse_depth(input, |input| {
        alt((
            map(preceded(char('!'), unary), |expr| {
                StmtExpr::unary_op(UnaryOp::Not, expr)
            }),
            map(preceded((tag("not"), multispace1), unary), |expr| {
                StmtExpr::unary_op(UnaryOp::Not, expr)
            }),
            map(preceded(char('-'), unary), |expr| {
                StmtExpr::unary_op(UnaryOp::Neg, expr)
            }),
            primary,
        ))
        .parse(input)
    })
}

// Primary expressions (literals, identifiers, function calls, parentheses)
fn primary(input: &str) -> IResult<&str, StmtExpr> {
    delimited(
        multispace0,
        alt((
            if_then_else,
            function_call,
            literal,
            identifier,
            parenthesized,
        )),
        multispace0,
    )
    .parse(input)
}

// If-then-else expression
fn if_then_else(input: &str) -> IResult<&str, StmtExpr> {
    let (input, _) = tag("if").parse(input)?;
    let (input, _) = multispace0.parse(input)?;
    let (input, _) = char('(').parse(input)?;
    let (input, condition) = expression(input)?;
    let (input, _) = multispace0.parse(input)?;
    let (input, _) = char(',').parse(input)?;
    let (input, then_expr) = expression(input)?;
    let (input, _) = multispace0.parse(input)?;
    let (input, _) = char(',').parse(input)?;
    let (input, else_expr) = expression(input)?;
    let (input, _) = multispace0.parse(input)?;
    let (input, _) = char(')').parse(input)?;

    Ok((
        input,
        StmtExpr::if_then_else(condition, then_expr, else_expr),
    ))
}

// Function call
fn function_call(input: &str) -> IResult<&str, StmtExpr> {
    let (input, name) = identifier_string(input)?;
    let (input, _) = multispace0.parse(input)?;
    let (input, _) = char('(').parse(input)?;
    let (input, args) =
        separated_list0(delimited(multispace0, char(','), multispace0), expression).parse(input)?;
    let (input, _) = char(')').parse(input)?;

    Ok((input, StmtExpr::call(name, args)))
}

// Literal number (rejects inf/nan)
fn literal(input: &str) -> IResult<&str, StmtExpr> {
    map(verify(double, |v: &f64| v.is_finite()), StmtExpr::literal).parse(input)
}

// Identifier (node reference)
fn identifier(input: &str) -> IResult<&str, StmtExpr> {
    let (input, id_str) = if input.starts_with("cs.") {
        cs_identifier_string(input)?
    } else {
        identifier_string(input)?
    };

    // Check if this is a capital structure reference (cs.component.instrument_or_total)
    if id_str.starts_with("cs.") {
        let parts: Vec<&str> = id_str.split('.').collect();
        if parts.len() == 3 {
            return Ok((
                input,
                StmtExpr::CSRef {
                    component: parts[1].to_string(),
                    instrument_or_total: parts[2].to_string(),
                },
            ));
        }
        return Err(nom::Err::Failure(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    Ok((input, StmtExpr::NodeRef(NodeId::from(id_str.as_str()))))
}

// Identifier string (alphanumeric + underscore + dot).
//
// Hyphens are intentionally excluded from ordinary identifiers so
// `revenue-cogs` parses as subtraction. Capital-structure references use a
// separate parser that permits hyphens in the instrument segment.
fn identifier_string(input: &str) -> IResult<&str, String> {
    map(
        recognize(pair(
            take_while1(|c: char| c.is_alphabetic() || c == '_'),
            nom::bytes::complete::take_while(|c: char| c.is_alphanumeric() || c == '_' || c == '.'),
        )),
        |s: &str| s.to_string(),
    )
    .parse(input)
}

fn cs_identifier_string(input: &str) -> IResult<&str, String> {
    map(
        recognize((
            tag("cs."),
            take_while1(|c: char| c.is_alphanumeric() || c == '_'),
            char('.'),
            take_while1(|c: char| c.is_alphanumeric() || c == '_' || c == '-'),
        )),
        |s: &str| s.to_string(),
    )
    .parse(input)
}

// Parenthesized expression
fn parenthesized(input: &str) -> IResult<&str, StmtExpr> {
    delimited(char('('), expression, char(')')).parse(input)
}

fn with_parse_depth<'a>(
    input: &'a str,
    parse: impl FnOnce(&'a str) -> IResult<&'a str, StmtExpr>,
) -> IResult<&'a str, StmtExpr> {
    PARSE_DEPTH.with(|depth| {
        let current = depth.get();
        if current >= MAX_PARSE_DEPTH {
            Err(nom::Err::Failure(nom::error::Error::new(
                input,
                nom::error::ErrorKind::TooLarge,
            )))
        } else {
            depth.set(current + 1);
            let result = parse(input);
            depth.set(current);
            result
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_literal() {
        let result = parse_formula("42").expect("test should succeed");
        assert_eq!(result, StmtExpr::Literal(42.0));

        let result = parse_formula("123.456").expect("test should succeed");
        assert_eq!(result, StmtExpr::Literal(123.456));
    }

    #[test]
    fn test_parse_identifier() {
        let result = parse_formula("revenue").expect("test should succeed");
        assert_eq!(
            result,
            StmtExpr::NodeRef(crate::types::NodeId::new("revenue"))
        );
    }

    #[test]
    fn test_parse_addition() {
        let result = parse_formula("1 + 2").expect("test should succeed");
        match result {
            StmtExpr::BinOp { op, .. } => assert_eq!(op, BinOp::Add),
            _ => panic!("Expected BinOp"),
        }
    }

    #[test]
    fn test_parse_subtraction() {
        let result = parse_formula("revenue - cogs").expect("test should succeed");
        match result {
            StmtExpr::BinOp { op, left, right } => {
                assert_eq!(op, BinOp::Sub);
                assert_eq!(
                    *left,
                    StmtExpr::NodeRef(crate::types::NodeId::new("revenue"))
                );
                assert_eq!(*right, StmtExpr::NodeRef(crate::types::NodeId::new("cogs")));
            }
            _ => panic!("Expected BinOp"),
        }
    }

    #[test]
    fn test_parse_multiplication() {
        let result = parse_formula("revenue * 0.6").expect("test should succeed");
        match result {
            StmtExpr::BinOp { op, .. } => assert_eq!(op, BinOp::Mul),
            _ => panic!("Expected BinOp"),
        }
    }

    #[test]
    fn test_parse_division() {
        let result = parse_formula("gross_profit / revenue").expect("test should succeed");
        match result {
            StmtExpr::BinOp { op, .. } => assert_eq!(op, BinOp::Div),
            _ => panic!("Expected BinOp"),
        }
    }

    #[test]
    fn test_parse_parentheses() {
        let result = parse_formula("(1 + 2) * 3").expect("test should succeed");
        match result {
            StmtExpr::BinOp {
                op: BinOp::Mul,
                left,
                ..
            } => match *left {
                StmtExpr::BinOp { op: BinOp::Add, .. } => {}
                _ => panic!("Expected Add inside parentheses"),
            },
            _ => panic!("Expected Mul"),
        }
    }

    #[test]
    fn test_parse_function_call() {
        let result = parse_formula("lag(revenue, 1)").expect("test should succeed");
        match result {
            StmtExpr::Call { func, args } => {
                assert_eq!(func, "lag");
                assert_eq!(args.len(), 2);
            }
            _ => panic!("Expected Call"),
        }
    }

    #[test]
    fn test_parse_nested_functions() {
        let result =
            parse_formula("rolling_mean(lag(revenue, 1), 4)").expect("test should succeed");
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
    fn test_parse_comparison() {
        let result = parse_formula("revenue > 1000000").expect("test should succeed");
        match result {
            StmtExpr::BinOp { op, .. } => assert_eq!(op, BinOp::Gt),
            _ => panic!("Expected BinOp"),
        }
    }

    #[test]
    fn test_parse_logical_and() {
        let result =
            parse_formula("revenue > 1000000 and margin > 0.15").expect("test should succeed");
        match result {
            StmtExpr::BinOp { op, .. } => assert_eq!(op, BinOp::And),
            _ => panic!("Expected BinOp"),
        }
    }

    #[test]
    fn test_parse_if_then_else() {
        let result =
            parse_formula("if(revenue > 1000000, revenue * 0.1, 0)").expect("test should succeed");
        match result {
            StmtExpr::IfThenElse { .. } => {}
            _ => panic!("Expected IfThenElse"),
        }
    }

    #[test]
    fn test_parse_complex_expression() {
        let result = parse_formula("(revenue - cogs) / revenue").expect("test should succeed");
        match result {
            StmtExpr::BinOp { op: BinOp::Div, .. } => {}
            _ => panic!("Expected division"),
        }
    }

    #[test]
    fn test_parse_negative_number() {
        let result = parse_formula("-5").expect("test should succeed");
        match result {
            StmtExpr::UnaryOp {
                op: UnaryOp::Neg, ..
            } => {}
            _ => panic!("Expected unary negation"),
        }
    }

    #[test]
    fn test_parse_unary_not_bang() {
        let result = parse_formula("!revenue").expect("test should succeed");
        match result {
            StmtExpr::UnaryOp {
                op: UnaryOp::Not,
                operand,
            } => assert_eq!(
                *operand,
                StmtExpr::NodeRef(crate::types::NodeId::new("revenue"))
            ),
            _ => panic!("Expected unary not"),
        }
    }

    #[test]
    fn test_parse_unary_not_keyword() {
        let result = parse_formula("not revenue").expect("test should succeed");
        match result {
            StmtExpr::UnaryOp {
                op: UnaryOp::Not,
                operand,
            } => assert_eq!(
                *operand,
                StmtExpr::NodeRef(crate::types::NodeId::new("revenue"))
            ),
            _ => panic!("Expected unary not"),
        }
    }

    #[test]
    fn test_parse_not_is_not_identifier_prefix() {
        let result = parse_formula("notional").expect("test should succeed");
        assert_eq!(
            result,
            StmtExpr::NodeRef(crate::types::NodeId::new("notional"))
        );
    }

    #[test]
    fn test_operator_precedence() {
        // Should parse as 1 + (2 * 3)
        let result = parse_formula("1 + 2 * 3").expect("test should succeed");
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
    fn test_parse_error_on_invalid() {
        let result = parse_formula("revenue +");
        assert!(result.is_err());

        let result = parse_formula("revenue @@ cogs");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_error_on_excessive_nesting() {
        let formula = format!(
            "{}1{}",
            "(".repeat(MAX_PARSE_DEPTH + 1),
            ")".repeat(MAX_PARSE_DEPTH + 1)
        );
        let err = parse_formula(&formula).expect_err("deep nesting should fail");
        assert!(
            err.to_string()
                .contains(&format!("maximum depth {MAX_PARSE_DEPTH}")),
            "unexpected error: {err}"
        );
    }
}
