//! Shared formula utilities for identifier extraction and manipulation.

use crate::dsl::ast::StmtExpr;
use crate::dsl::parse_formula;
use crate::types::NodeId;
use indexmap::IndexSet;

/// Check if a character is a valid identifier boundary.
///
/// Returns true if the character is NOT part of an identifier (alphanumeric or underscore).
#[inline]
fn is_identifier_boundary(c: char) -> bool {
    !c.is_alphanumeric() && c != '_'
}

/// Check if an identifier at a given position is standalone (not part of a larger identifier).
///
/// # Arguments
///
/// * `formula` - The formula string to check
/// * `start_idx` - Starting position of the identifier
/// * `end_idx` - Ending position of the identifier (exclusive)
/// * `allow_dot_after` - Whether to allow '.' after the identifier (for namespace prefixes)
///
/// # Returns
///
/// Returns true if the identifier is standalone (proper boundaries before and after).
pub(crate) fn is_standalone_identifier(
    formula: &str,
    start_idx: usize,
    end_idx: usize,
    allow_dot_after: bool,
) -> bool {
    // `start_idx`/`end_idx` are BYTE offsets (from `str::find` + `str::len`), so
    // index via byte slices rather than `chars().nth()` (which counts chars and
    // would read the wrong boundary for any non-ASCII formula). Both offsets sit
    // on char boundaries (substring match), so slicing is panic-free. This is
    // also O(1) instead of O(n).
    let before_ok = formula[..start_idx]
        .chars()
        .next_back()
        .is_none_or(|c| is_identifier_boundary(c) && c != '.');

    // Check character after identifier
    let after_ok = formula[end_idx..].chars().next().is_none_or(|c| {
        if allow_dot_after {
            is_identifier_boundary(c)
        } else {
            is_identifier_boundary(c) && c != '.'
        }
    });

    before_ok && after_ok
}

/// Replace every standalone occurrence of one identifier in a formula.
pub(crate) fn replace_standalone_identifier(
    formula: &str,
    identifier: &str,
    replacement: &str,
) -> String {
    const MAX_REPLACE_ITERATIONS: usize = 1_000_000;
    let mut result = formula.to_string();
    let mut idx = 0;
    let mut iterations = 0usize;
    while let Some(pos) = result[idx..].find(identifier) {
        iterations += 1;
        if iterations > MAX_REPLACE_ITERATIONS {
            break;
        }
        let abs_pos = idx + pos;
        let end_pos = abs_pos + identifier.len();
        if is_standalone_identifier(&result, abs_pos, end_pos, false) {
            result.replace_range(abs_pos..end_pos, replacement);
            idx = abs_pos + replacement.len();
        } else {
            idx = end_pos;
        }
    }
    result
}

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
/// ```ignore
/// // Low-level helper; prefer higher-level APIs when possible.
/// use finstack_quant_statements::formula::extract_all_identifiers;
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

/// Extract direct dependencies (ignoring lagged references) from a formula.
///
/// This parses the formula AST and collects identifiers, but skips traversal
/// into `lag()` and `shift()` function calls. This allows breaking cycles
/// using temporal lag.
///
/// # Arguments
///
/// * `formula` - The formula string to analyze
///
/// # Returns
///
/// Returns a set of `NodeId` values that are direct dependencies in the current period.
pub(crate) fn extract_direct_dependencies(formula: &str) -> crate::error::Result<IndexSet<NodeId>> {
    let ast = parse_formula(formula)?;
    let mut identifiers: IndexSet<String> = IndexSet::new();
    collect_identifiers_from_ast(&ast, &mut identifiers, true);
    Ok(identifiers.into_iter().map(NodeId::from).collect())
}

/// Recursively collect identifiers from an AST node.
///
/// # Arguments
/// * `expr` - Expression to traverse
/// * `identifiers` - Set to collect identifiers into
/// * `ignore_lag` - If true, do not traverse into `lag()` or `shift()` calls
///   (except when the offset is a literal 0, which is a current-period dependency)
fn collect_identifiers_from_ast(
    expr: &StmtExpr,
    identifiers: &mut IndexSet<String>,
    ignore_lag: bool,
) {
    match expr {
        StmtExpr::Literal(_) => {}
        StmtExpr::NodeRef(name) => {
            identifiers.insert(name.as_str().to_string());
        }
        StmtExpr::CSRef {
            component,
            instrument_or_total,
        } => {
            // Encode cs.* references in the same format as the evaluator
            let encoded = format!("cs.{}.{}", component, instrument_or_total);
            identifiers.insert(encoded);
        }
        StmtExpr::BinOp { left, right, .. } => {
            collect_identifiers_from_ast(left, identifiers, ignore_lag);
            collect_identifiers_from_ast(right, identifiers, ignore_lag);
        }
        StmtExpr::UnaryOp { operand, .. } => {
            collect_identifiers_from_ast(operand, identifiers, ignore_lag);
        }
        StmtExpr::Call { func, args } => {
            // If ignoring lag, skip traversal for lag/shift functions
            // EXCEPT when the offset is a literal 0 (which means current-period dependency)
            if ignore_lag && (func == "lag" || func == "shift") {
                // The lagged value (args[0]) is read from history, not the
                // current period, so it is normally NOT a current-period
                // dependency. Exceptions:
                //   * a literal-0 offset reads args[0] in the current period;
                //   * a dynamic (non-literal) offset could evaluate to 0, so
                //     conservatively treat args[0] as a current-period dep.
                match args.get(1) {
                    Some(StmtExpr::Literal(offset)) if *offset == 0.0 => {
                        collect_identifiers_from_ast(&args[0], identifiers, ignore_lag);
                    }
                    Some(StmtExpr::Literal(_)) => {}
                    _ => {
                        if let Some(first) = args.first() {
                            collect_identifiers_from_ast(first, identifiers, ignore_lag);
                        }
                    }
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

/// Extract identifiers from a formula that match a given set of known identifiers.
///
/// This function finds all standalone occurrences of identifiers from the provided set
/// within the formula string. It handles:
/// - Proper identifier boundaries (not part of larger identifiers)
/// - Exclusion of cs.* namespace references (capital structure)
///
/// # Arguments
///
/// * `formula` - The formula string to analyze
/// * `known_identifiers` - Set of identifiers to look for
///
/// # Returns
///
/// Returns a set of identifiers that appear in the formula as standalone references.
///
/// # Example
///
/// ```ignore
/// // Low-level helper; prefer higher-level APIs when possible.
/// use crate::utils::formula::extract_identifiers;
/// ```
pub(crate) fn extract_identifiers(
    formula: &str,
    known_identifiers: &IndexSet<String>,
) -> crate::error::Result<IndexSet<String>> {
    let ast = parse_formula(formula)?;
    let mut identifiers = IndexSet::new();
    collect_identifiers_from_ast(&ast, &mut identifiers, false);

    Ok(identifiers
        .into_iter()
        .filter(|id| known_identifiers.contains(id))
        .collect())
}

/// Replace all occurrences of identifiers in a formula with qualified versions.
///
/// This function performs in-place replacement of unqualified identifiers with
/// their qualified equivalents (e.g., "gross_profit" → "fin.gross_profit").
///
/// # Arguments
///
/// * `formula` - The formula string to modify
/// * `identifiers` - Set of identifiers to replace (unqualified)
/// * `namespace` - Namespace to prepend (e.g., "fin")
///
/// # Returns
///
/// Returns the modified formula string with qualified identifiers.
///
/// # Example
///
/// ```ignore
/// // Low-level helper; prefer higher-level APIs when possible.
/// use crate::utils::formula::qualify_identifiers;
/// ```
pub(crate) fn qualify_identifiers(
    formula: &str,
    identifiers: &IndexSet<String>,
    namespace: &str,
) -> String {
    let mut result = formula.to_string();

    // Sort by length descending to replace longer IDs first
    // This prevents "ebitda_margin" from being partially replaced as "ebitda"
    let mut sorted: Vec<_> = identifiers.iter().cloned().collect();
    sorted.sort_by_key(|id| std::cmp::Reverse(id.len()));

    for identifier in sorted {
        let qualified = format!("{}.{}", namespace, identifier);
        result = replace_standalone_identifier(&result, &identifier, &qualified);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_standalone_identifier() {
        let formula = "revenue - cogs";

        // "revenue" at position 0-7 should be standalone
        assert!(is_standalone_identifier(formula, 0, 7, false));

        // "cogs" at position 10-14 should be standalone
        assert!(is_standalone_identifier(formula, 10, 14, false));
    }

    #[test]
    fn test_is_standalone_identifier_not_standalone() {
        let formula = "my_revenue - cogs";
        let start = formula
            .find("revenue")
            .expect("substring should exist in test formula");
        let end = start + "revenue".len();

        assert!(
            !is_standalone_identifier(formula, start, end, false),
            "embedded identifiers should not be treated as standalone"
        );
    }

    #[test]
    fn test_extract_identifiers_basic() {
        let formula = "revenue - cogs";
        let mut known = IndexSet::new();
        known.insert("revenue".to_string());
        known.insert("cogs".to_string());
        known.insert("gross_profit".to_string());

        let deps = extract_identifiers(formula, &known).expect("formula should parse");

        assert_eq!(deps.len(), 2);
        assert!(deps.contains("revenue"));
        assert!(deps.contains("cogs"));
        assert!(!deps.contains("gross_profit"));
    }

    #[test]
    fn test_extract_identifiers_excludes_cs_refs() {
        let formula = "revenue - cs.interest_expense.total";
        let mut known = IndexSet::new();
        known.insert("revenue".to_string());
        known.insert("interest_expense".to_string());

        let deps = extract_identifiers(formula, &known).expect("formula should parse");

        assert_eq!(deps.len(), 1);
        assert!(deps.contains("revenue"));
        assert!(!deps.contains("interest_expense")); // Should be excluded (part of cs.*)
    }

    #[test]
    fn test_extract_identifiers_partial_match() {
        let formula = "ebitda_margin + ebitda";
        let mut known = IndexSet::new();
        known.insert("ebitda".to_string());
        known.insert("ebitda_margin".to_string());

        let deps = extract_identifiers(formula, &known).expect("formula should parse");

        assert_eq!(deps.len(), 2);
        assert!(deps.contains("ebitda"));
        assert!(deps.contains("ebitda_margin"));
    }

    #[test]
    fn test_qualify_identifiers_basic() {
        let formula = "gross_profit / revenue";
        let mut identifiers = IndexSet::new();
        identifiers.insert("gross_profit".to_string());

        let result = qualify_identifiers(formula, &identifiers, "fin");

        assert_eq!(result, "fin.gross_profit / revenue");
    }

    #[test]
    fn test_qualify_identifiers_multiple() {
        let formula = "gross_profit - opex";
        let mut identifiers = IndexSet::new();
        identifiers.insert("gross_profit".to_string());
        identifiers.insert("opex".to_string());

        let result = qualify_identifiers(formula, &identifiers, "fin");

        assert_eq!(result, "fin.gross_profit - fin.opex");
    }

    #[test]
    fn test_qualify_identifiers_prefix_handling() {
        let formula = "ebitda_margin + ebitda";
        let mut identifiers = IndexSet::new();
        identifiers.insert("ebitda".to_string());
        identifiers.insert("ebitda_margin".to_string());

        let result = qualify_identifiers(formula, &identifiers, "fin");

        // Should replace longer identifiers first to avoid partial replacement
        assert_eq!(result, "fin.ebitda_margin + fin.ebitda");
    }

    #[test]
    fn test_extract_direct_dependencies_ignore_lag() {
        let formula = "revenue + lag(cogs, 1)";
        let deps = extract_direct_dependencies(formula).expect("should parse");
        assert!(deps.contains("revenue"));
        assert!(!deps.contains("cogs"));
    }

    #[test]
    fn test_extract_direct_dependencies_zero_shift_included() {
        // shift(x, 0) should include x as a direct dependency since it refers to current period
        let formula = "shift(revenue, 0)";
        let deps = extract_direct_dependencies(formula).expect("should parse");
        assert!(deps.contains("revenue"));
    }

    #[test]
    fn test_extract_direct_dependencies_zero_lag_included() {
        // lag(x, 0) should include x as a direct dependency since it refers to current period
        let formula = "lag(revenue, 0)";
        let deps = extract_direct_dependencies(formula).expect("should parse");
        assert!(deps.contains("revenue"));
    }

    #[test]
    fn test_extract_direct_dependencies_nonzero_shift_excluded() {
        // shift(x, 1) should NOT include x as a direct dependency
        let formula = "shift(revenue, 1)";
        let deps = extract_direct_dependencies(formula).expect("should parse");
        assert!(!deps.contains("revenue"));
    }
}
