//! Shared formula utilities for identifier extraction and manipulation.

use crate::dsl::ast::StmtExpr;
use crate::dsl::parse_formula;
use crate::types::NodeId;
use indexmap::IndexSet;

/// Extract ALL identifiers from a formula by parsing the AST.
///
/// This function parses the formula and extracts all node references and
/// qualified references (e.g., `fin.ebitda`), regardless of whether they
/// are known or not. This is useful for validation.
///
/// # Arguments
///
/// * `formula` - The formula string to analyze
///
/// # Returns
///
/// Returns a set of all identifiers found in the formula. For qualified
/// references (e.g., `fin.ebitda`), returns the qualified name.
///
/// # Example
///
/// ```
/// // Low-level helper; prefer higher-level APIs when possible.
/// use finstack_quant_statements::dsl::extract_all_identifiers;
/// ```
///
/// Function names and numeric literals are not identifiers. Unlike direct
/// dependency extraction, this function includes references inside `lag()` and
/// `shift()` because callers use it for complete validation and alias
/// normalization rather than same-period scheduling.
///
/// # Errors
///
/// Returns a formula-parse error if `formula` is not a complete valid
/// Statements DSL expression. It does not check whether extracted identifiers
/// are defined by a particular model.
pub fn extract_all_identifiers(formula: &str) -> crate::error::Result<IndexSet<String>> {
    let ast = parse_formula(formula)?;
    let mut identifiers = IndexSet::new();
    collect_identifiers_from_ast(&ast, &mut identifiers, false);
    Ok(identifiers)
}

/// Return whether a formula contains a parsed capital-structure reference.
///
/// This distinguishes the dedicated `cs.<component>.<instrument-or-total>` AST
/// node from ordinary identifiers that merely contain the characters `cs.`.
///
/// # Arguments
///
/// * `formula` - Complete Statements DSL expression to parse and inspect.
///
/// # Errors
///
/// Returns a formula-parse error when `formula` is not a valid expression.
pub(crate) fn contains_capital_structure_reference(formula: &str) -> crate::error::Result<bool> {
    Ok(extract_all_identifiers(formula)?
        .iter()
        .any(|identifier| identifier.starts_with("cs.")))
}

/// Extract direct dependencies (ignoring lagged references) from a formula.
///
/// This parses the formula AST and collects identifiers, but skips traversal
/// into `lag()` and `shift()` function calls. This allows breaking cycles
/// using temporal lag.
///
/// # Arguments
///
/// * `ast` - Parsed formula whose same-period references are needed.
///
/// # Returns
///
/// Returns a set of `NodeId` values that are direct dependencies in the current period.
pub(crate) fn extract_direct_dependencies(ast: &StmtExpr) -> IndexSet<NodeId> {
    let mut identifiers: IndexSet<String> = IndexSet::new();
    collect_identifiers_from_ast(ast, &mut identifiers, true);
    identifiers.into_iter().map(NodeId::from).collect()
}

/// Recursively collect identifiers from an AST node.
///
/// # Arguments
/// * `expr` - Expression to traverse
/// * `identifiers` - Set to collect identifiers into
/// * `ignore_lag` - If true, do not traverse into `lag()` or `shift()` calls
///   (except when the offset is a literal 0, which is a current-period dependency)
pub(crate) fn collect_identifiers_from_ast(
    expr: &StmtExpr,
    identifiers: &mut IndexSet<String>,
    ignore_lag: bool,
) {
    match expr {
        StmtExpr::Literal(_) => {}
        StmtExpr::NodeRef(name) => {
            identifiers.insert(name.as_str().to_string());
        }
        StmtExpr::CsRef { component, scope } => {
            // Encode cs.* references in the same format as the evaluator
            identifiers.insert(format!("cs.{component}.{scope}"));
        }
        StmtExpr::BinOp { left, right, .. } => {
            collect_identifiers_from_ast(left, identifiers, ignore_lag);
            collect_identifiers_from_ast(right, identifiers, ignore_lag);
        }
        StmtExpr::UnaryOp { operand, .. } => {
            collect_identifiers_from_ast(operand, identifiers, ignore_lag);
        }
        StmtExpr::AsMoney { value, .. } => {
            collect_identifiers_from_ast(value, identifiers, ignore_lag);
        }
        StmtExpr::Call { func, args } => {
            // A `lag`/`shift` value is read from history only when its offset
            // is a positive integer literal. Zero, a non-integer literal and
            // any dynamic offset can read the current period, so the value
            // keeps its same-period edge.
            if ignore_lag && (func == "lag" || func == "shift") {
                let past_only = matches!(
                    args.get(1),
                    Some(StmtExpr::Literal(offset)) if *offset >= 1.0 && offset.fract() == 0.0
                );
                if !past_only && let Some(first) = args.first() {
                    collect_identifiers_from_ast(first, identifiers, ignore_lag);
                }
                // The offset expression and any further args ARE evaluated in
                // the current period, so they are always dependencies.
                for arg in args.iter().skip(1) {
                    collect_identifiers_from_ast(arg, identifiers, ignore_lag);
                }
                return;
            }
            for arg in args {
                collect_identifiers_from_ast(arg, identifiers, ignore_lag);
            }
        }
        StmtExpr::IfThenElse {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_identifiers_from_ast(condition, identifiers, ignore_lag);
            collect_identifiers_from_ast(then_expr, identifiers, ignore_lag);
            collect_identifiers_from_ast(else_expr, identifiers, ignore_lag);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capital_structure_detection_uses_parsed_reference_kind() {
        assert!(
            contains_capital_structure_reference("revenue - cs.interest_expense.total")
                .expect("capital-structure formula should parse")
        );
        assert!(
            !contains_capital_structure_reference("economics.sales + cs_value.total")
                .expect("ordinary qualified identifiers should parse")
        );
    }

    #[test]
    fn test_extract_direct_dependencies_ignore_lag() {
        let formula = "revenue + lag(cogs, 1)";
        let deps = extract_direct_dependencies(&parse_formula(formula).expect("should parse"));
        assert!(deps.contains("revenue"));
        assert!(!deps.contains("cogs"));
    }

    #[test]
    fn test_extract_direct_dependencies_zero_shift_included() {
        // shift(x, 0) should include x as a direct dependency since it refers to current period
        let formula = "shift(revenue, 0)";
        let deps = extract_direct_dependencies(&parse_formula(formula).expect("should parse"));
        assert!(deps.contains("revenue"));
    }

    #[test]
    fn test_extract_direct_dependencies_zero_lag_included() {
        // lag(x, 0) should include x as a direct dependency since it refers to current period
        let formula = "lag(revenue, 0)";
        let deps = extract_direct_dependencies(&parse_formula(formula).expect("should parse"));
        assert!(deps.contains("revenue"));
    }

    #[test]
    fn test_extract_direct_dependencies_nonzero_shift_excluded() {
        // shift(x, 1) should NOT include x as a direct dependency
        let formula = "shift(revenue, 1)";
        let deps = extract_direct_dependencies(&parse_formula(formula).expect("should parse"));
        assert!(!deps.contains("revenue"));
    }
}
