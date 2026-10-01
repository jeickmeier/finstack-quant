//! Python bindings for the `finstack-quant-statements` crate.
//!
//! Exposes the financial model specification types, builder, evaluator,
//! DSL parser, and EBITDA normalization engine.

mod adjustments;
pub(crate) mod builder;
pub(crate) mod capital_structure;
pub(crate) mod checks;
mod dsl;
pub(crate) mod evaluator;
pub(crate) mod monte_carlo;
mod schema;
pub(crate) mod types;

use crate::bindings::core::money::PyMoney;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::types::AmountOrScalar;
use pyo3::prelude::*;
use pyo3::types::PyList;

/// Human-readable summary of the period-identifier grammar, appended to
/// parse failures so a caller who wrote ``"2025-Q1"`` learns the accepted
/// form instead of a bare "invalid input".
pub(crate) const PERIOD_GRAMMAR: &str = "period ids are <year><kind><index> with no \
    separators: 2025Q1 (quarter), 2025M3 (month), 2025W7 (ISO week), 2025H1 (half-year), \
    2025D45 (day of year), 2025 or FY2025 (year); ranges are <start>..<end> in one kind, \
    e.g. \"2025Q1..Q4\", \"2024M10..2025M03\", \"2025..2030\"";

/// Parse a period identifier, mapping failures to a `ValueError` that names
/// the offending input, the core diagnostic, and the accepted grammar.
pub(crate) fn parse_period_id(s: &str) -> PyResult<PeriodId> {
    s.parse().map_err(|e: finstack_quant_core::Error| {
        crate::errors::value_error(format!("invalid period id {s:?}: {e}; {PERIOD_GRAMMAR}"))
    })
}

/// Extract `(period, value)` pairs from a mapping, a pandas ``Series``, or a
/// sequence of 2-tuples.
///
/// Anything with an ``items()`` method (``dict``, ``Mapping``, ``pd.Series``)
/// is iterated as key/value pairs; otherwise the object is iterated as a
/// sequence of ``(period, value)`` tuples. Keys are rendered with ``str()``
/// so ``PeriodId`` objects and plain strings are both accepted.
pub(crate) fn extract_period_pairs<'py>(
    values: &Bound<'py, PyAny>,
) -> PyResult<Vec<(String, Bound<'py, PyAny>)>> {
    let mut out = Vec::new();
    if values.hasattr("items")? {
        for item in values.call_method0("items")?.try_iter()? {
            let (key, value): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
            out.push((key.str()?.to_string(), value));
        }
        return Ok(out);
    }
    for item in values.try_iter()? {
        let (key, value): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        out.push((key.str()?.to_string(), value));
    }
    Ok(out)
}

/// Extract `(PeriodId, f64)` pairs from any of the shapes accepted by
/// [`extract_period_pairs`].
pub(crate) fn extract_scalar_series(values: &Bound<'_, PyAny>) -> PyResult<Vec<(PeriodId, f64)>> {
    extract_period_pairs(values)?
        .into_iter()
        .map(|(period, value)| Ok((parse_period_id(&period)?, value.extract::<f64>()?)))
        .collect()
}

/// Extract `(PeriodId, Money)` pairs from any of the shapes accepted by
/// [`extract_period_pairs`].
pub(crate) fn extract_money_series(
    values: &Bound<'_, PyAny>,
) -> PyResult<Vec<(PeriodId, finstack_quant_core::money::Money)>> {
    extract_period_pairs(values)?
        .into_iter()
        .map(|(period, value)| {
            Ok((
                parse_period_id(&period)?,
                value.extract::<PyRef<'_, PyMoney>>()?.inner,
            ))
        })
        .collect()
}

/// Extract `(PeriodId, AmountOrScalar)` pairs, accepting ``float`` or
/// ``Money`` cells in any of the shapes accepted by [`extract_period_pairs`].
pub(crate) fn extract_value_series(
    values: &Bound<'_, PyAny>,
) -> PyResult<Vec<(PeriodId, AmountOrScalar)>> {
    extract_period_pairs(values)?
        .into_iter()
        .map(|(period, value)| {
            let amount = if let Ok(money) = value.extract::<PyRef<'_, PyMoney>>() {
                AmountOrScalar::Amount(money.inner)
            } else {
                AmountOrScalar::scalar(value.extract::<f64>()?)
            };
            Ok((parse_period_id(&period)?, amount))
        })
        .collect()
}

/// Render a JSON value as a Python literal (``None`` / ``True`` / ``'text'``
/// / ``{'k': 1.5}``) for ``__repr__`` output.
pub(crate) fn python_literal(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "None".to_string(),
        serde_json::Value::Bool(true) => "True".to_string(),
        serde_json::Value::Bool(false) => "False".to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => {
            format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
        }
        serde_json::Value::Array(items) => {
            let inner: Vec<String> = items.iter().map(python_literal).collect();
            format!("[{}]", inner.join(", "))
        }
        serde_json::Value::Object(map) => {
            let inner: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("'{k}': {}", python_literal(v)))
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
    }
}

/// Register the `statements` submodule on the parent module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "statements")?;
    m.setattr(
        "__doc__",
        "Financial statement modeling: builders, evaluators, forecasts, DSL, adjustments.",
    )?;

    types::register(py, &m)?;
    capital_structure::register(py, &m)?;
    builder::register(py, &m)?;
    evaluator::register(py, &m)?;
    monte_carlo::register(py, &m)?;
    dsl::register(py, &m)?;
    adjustments::register(py, &m)?;
    checks::register(py, &m)?;
    schema::register(py, &m)?;

    let all = PyList::new(
        py,
        [
            "Adjustment",
            "AppliedAdjustment",
            "CapitalStructureCashflows",
            "CheckConfig",
            "CheckFinding",
            "CheckReport",
            "CheckSuiteSpec",
            "EcfSweepSpec",
            "Evaluator",
            "FinancialModelSpec",
            "ForecastMethod",
            "ForecastSpec",
            "FormulaCheckSpec",
            "MetricDefinition",
            "MixedNodeBuilder",
            "ModelBuilder",
            "MonteCarloConfig",
            "MonteCarloResults",
            "NodeId",
            "NodeSpec",
            "NodeType",
            "NormalizationConfig",
            "NormalizationResult",
            "NumericMode",
            "PaymentClassSpec",
            "PikToggleSpec",
            "Registry",
            "StatementResult",
            "WaterfallSpec",
            "normalize",
            "normalize_json",
            "parse_and_compile",
            "parse_formula",
            "schema",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;

    Ok(())
}
