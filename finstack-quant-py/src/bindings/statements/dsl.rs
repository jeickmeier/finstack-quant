//! Python wrappers for the statement DSL (parser + compiler).

use crate::errors::statements_to_py;
use pyo3::prelude::*;

/// Parse a DSL formula and return its canonical source rendering.
///
/// The formula is parsed into the statements AST and rendered back to
/// canonical text: whitespace normalised, operators spaced, and
/// parentheses kept only where operator precedence requires them. Parsing
/// the returned string again yields the same AST, so the output is a stable
/// surface for diffing, hashing, or displaying formulas — not a debug dump.
///
/// Parameters
/// ----------
/// formula : str
///     DSL expression to parse (e.g. ``"revenue - cogs"``).
///
/// Returns
/// -------
/// str
///     Canonical formula text, e.g. ``"(revenue - cogs) / revenue"``.
///
/// Raises
/// ------
/// ValueError
///     If the formula is not one complete, well-formed DSL expression.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import parse_formula
/// >>> parse_formula("revenue-cogs")
/// 'revenue - cogs'
#[pyfunction]
#[pyo3(text_signature = "(formula, /)")]
fn parse_formula(formula: &str) -> PyResult<String> {
    let ast = finstack_quant_statements::dsl::parse_formula(formula).map_err(statements_to_py)?;
    Ok(ast.to_string())
}

/// Parse and compile a formula, raising if either step fails.
///
/// Compilation produces the shared expression AST for the period-aware statements
/// evaluator. It checks supported functions, arities and capital-structure references.
///
/// Parameters
/// ----------
/// formula : str
///     DSL expression to validate.
///
/// Returns
/// -------
/// None
///     Returns nothing on success. Validation is reported by raising, so
///     ``if parse_and_compile(f):`` is not a validity check — call it bare
///     and catch ``ValueError``.
///
/// Raises
/// ------
/// ValueError
///     If the formula fails to parse or compile.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import parse_and_compile
/// >>> parse_and_compile("revenue * (1 + growth_rate)") is None
/// True
#[pyfunction]
#[pyo3(text_signature = "(formula, /)")]
fn parse_and_compile(formula: &str) -> PyResult<()> {
    finstack_quant_statements::dsl::parse_and_compile(formula).map_err(statements_to_py)?;
    Ok(())
}

/// Register DSL functions.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(parse_formula, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(parse_and_compile, m)?)?;
    Ok(())
}
