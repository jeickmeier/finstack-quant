//! Python wrappers for the statement DSL (parser + compiler).

use crate::errors::display_to_py;
use pyo3::prelude::*;

/// Parse a DSL formula string and return a debug string for the AST.
///
/// Useful for inspecting formula structure without compiling.
///
/// Parameters
/// ----------
/// formula : str
///     DSL expression to parse (e.g. ``"revenue - cogs"``).
///
/// Returns
/// -------
/// str
///     Debug representation of the parsed AST.
#[pyfunction]
#[pyo3(text_signature = "(formula, /)")]
fn parse_formula(formula: &str) -> PyResult<String> {
    let ast = finstack_quant_statements::dsl::parse_formula(formula).map_err(display_to_py)?;
    Ok(format!("{ast:?}"))
}

/// Validate that a formula parses and compiles successfully.
///
/// Parameters
/// ----------
/// formula : str
///     DSL expression to validate.
///
/// Returns
/// -------
/// bool
///     ``True`` if the formula is valid.
///
/// Raises
/// ------
/// ValueError
///     If the formula fails to parse or compile.
#[pyfunction]
#[pyo3(text_signature = "(formula, /)")]
fn validate_formula(formula: &str) -> PyResult<bool> {
    finstack_quant_statements::dsl::parse_and_compile(formula).map_err(display_to_py)?;
    Ok(true)
}

/// Register DSL functions.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(parse_formula, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(validate_formula, m)?)?;
    Ok(())
}
