//! Python bindings for the `finstack-cashflows` crate.

use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

/// Build a cashflow schedule from a JSON spec and return canonical schedule JSON.
///
/// Parameters
/// ----------
/// spec_json : str
///     JSON-encoded `CashflowScheduleBuildSpec`.
/// market_json : str, optional
///     JSON-encoded market context for floating-rate lookups.
///
/// Returns
/// -------
/// str
///     JSON-encoded `CashFlowSchedule`.
#[pyfunction]
#[pyo3(
    signature = (spec_json, market_json = None),
    text_signature = "(spec_json, market_json=None)"
)]
fn build_cashflow_schedule_json(
    py: Python<'_>,
    spec_json: &str,
    market_json: Option<&str>,
) -> PyResult<String> {
    py.detach(|| {
        finstack_cashflows::build_cashflow_schedule_json(spec_json, market_json)
            .map_err(crate::errors::core_to_py)
    })
}

/// Build a stamped cashflow schedule envelope from a JSON spec.
///
/// Parameters
/// ----------
/// spec_json : str
///     JSON-encoded `CashflowScheduleBuildSpec`.
/// market_json : str, optional
///     JSON-encoded market context for floating-rate lookups.
///
/// Returns
/// -------
/// str
///     JSON-encoded `CashflowScheduleEnvelope`.
#[pyfunction]
#[pyo3(
    signature = (spec_json, market_json = None),
    text_signature = "(spec_json, market_json=None)"
)]
fn build_cashflow_schedule_envelope_json(
    py: Python<'_>,
    spec_json: &str,
    market_json: Option<&str>,
) -> PyResult<String> {
    py.detach(|| {
        finstack_cashflows::build_cashflow_schedule_envelope_json(spec_json, market_json)
            .map_err(crate::errors::core_to_py)
    })
}

/// Validate a cashflow schedule JSON string and return it canonicalized.
///
/// Parameters
/// ----------
/// schedule_json : str
///     JSON-encoded `CashFlowSchedule`.
///
/// Returns
/// -------
/// str
///     Canonicalized JSON-encoded `CashFlowSchedule`.
#[pyfunction]
#[pyo3(text_signature = "(schedule_json)")]
fn validate_cashflow_schedule_json(py: Python<'_>, schedule_json: &str) -> PyResult<String> {
    py.detach(|| {
        finstack_cashflows::validate_cashflow_schedule_json(schedule_json)
            .map_err(crate::errors::core_to_py)
    })
}

/// Validate a stamped cashflow schedule envelope and return it canonicalized.
///
/// Parameters
/// ----------
/// envelope_json : str
///     JSON-encoded `CashflowScheduleEnvelope`.
///
/// Returns
/// -------
/// str
///     Canonicalized JSON-encoded `CashflowScheduleEnvelope`.
#[pyfunction]
#[pyo3(text_signature = "(envelope_json)")]
fn validate_cashflow_schedule_envelope_json(
    py: Python<'_>,
    envelope_json: &str,
) -> PyResult<String> {
    py.detach(|| {
        finstack_cashflows::validate_cashflow_schedule_envelope_json(envelope_json)
            .map_err(crate::errors::core_to_py)
    })
}

/// Extract dated flows from a cashflow schedule.
///
/// Parameters
/// ----------
/// schedule_json : str
///     JSON-encoded `CashFlowSchedule`.
///
/// Returns
/// -------
/// str
///     JSON array of `{date, amount}` entries, where `amount` is itself
///     `{amount, currency}`. `CFKind` and accrual metadata are intentionally
///     omitted; parse the full schedule JSON if you need flow classification.
#[pyfunction]
#[pyo3(text_signature = "(schedule_json)")]
fn dated_flows_json(py: Python<'_>, schedule_json: &str) -> PyResult<String> {
    py.detach(|| {
        finstack_cashflows::dated_flows_json(schedule_json).map_err(crate::errors::core_to_py)
    })
}

/// Compute accrued interest for a schedule as of a given date.
///
/// Parameters
/// ----------
/// schedule_json : str
///     JSON-encoded `CashFlowSchedule`.
/// as_of : str
///     ISO-8601 date (YYYY-MM-DD) for the accrual snapshot.
/// config_json : str, optional
///     JSON-encoded `AccrualConfig` overriding defaults.
///
/// Returns
/// -------
/// float
///     Accrued interest in the schedule's settlement currency, returned as a
///     host-language double. The Rust engine computes from the canonical
///     schedule and then crosses the binding boundary as `f64`; for large
///     notionals, compare results with an absolute tolerance scaled to the
///     schedule notional rather than expecting decimal-string equality.
#[pyfunction]
#[pyo3(
    signature = (schedule_json, as_of, config_json = None),
    text_signature = "(schedule_json, as_of, config_json=None)"
)]
fn accrued_interest_json(
    py: Python<'_>,
    schedule_json: &str,
    as_of: &str,
    config_json: Option<&str>,
) -> PyResult<f64> {
    py.detach(|| {
        finstack_cashflows::accrued_interest_json(schedule_json, as_of, config_json)
            .map_err(crate::errors::core_to_py)
    })
}

/// Construct a tagged Bond instrument JSON from a cashflow schedule.
///
/// Convenience wrapper that crosses crates: it materializes a
/// `finstack_valuations::instruments::fixed_income::bond::Bond` from the
/// supplied schedule and wraps it in the tagged `InstrumentJson` envelope.
///
/// Parameters
/// ----------
/// instrument_id : str
///     Identifier for the Bond instrument.
/// schedule_json : str
///     JSON-encoded `CashFlowSchedule`.
/// discount_curve_id : str
///     Identifier of the discount curve used for pricing.
/// quoted_clean : float, optional
///     Clean quoted price used to calibrate yield on construction.
///
/// Returns
/// -------
/// str
///     JSON-encoded tagged `InstrumentJson::Bond`.
#[pyfunction]
#[pyo3(
    signature = (instrument_id, schedule_json, discount_curve_id, quoted_clean = None),
    text_signature = "(instrument_id, schedule_json, discount_curve_id, quoted_clean=None)"
)]
fn bond_from_cashflows_json(
    py: Python<'_>,
    instrument_id: &str,
    schedule_json: &str,
    discount_curve_id: &str,
    quoted_clean: Option<f64>,
) -> PyResult<String> {
    py.detach(|| {
        finstack_valuations::bond_from_cashflows_json(
            instrument_id,
            schedule_json,
            discount_curve_id,
            quoted_clean,
        )
        .map_err(crate::errors::core_to_py)
    })
}

/// Register the `finstack.cashflows` Python namespace.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "cashflows")?;
    m.setattr(
        "__doc__",
        "Cashflow schedule JSON construction, validation, and bond conversion.",
    )?;

    m.add_function(wrap_pyfunction!(accrued_interest_json, &m)?)?;
    m.add_function(wrap_pyfunction!(bond_from_cashflows_json, &m)?)?;
    m.add_function(wrap_pyfunction!(build_cashflow_schedule_envelope_json, &m)?)?;
    m.add_function(wrap_pyfunction!(build_cashflow_schedule_json, &m)?)?;
    m.add_function(wrap_pyfunction!(dated_flows_json, &m)?)?;
    m.add_function(wrap_pyfunction!(
        validate_cashflow_schedule_envelope_json,
        &m
    )?)?;
    m.add_function(wrap_pyfunction!(validate_cashflow_schedule_json, &m)?)?;

    let all = PyList::new(
        py,
        [
            "accrued_interest_json",
            "bond_from_cashflows_json",
            "build_cashflow_schedule_envelope_json",
            "build_cashflow_schedule_json",
            "dated_flows_json",
            "validate_cashflow_schedule_envelope_json",
            "validate_cashflow_schedule_json",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "cashflows",
        crate::bindings::module_utils::ROOT_PACKAGE,
        crate::bindings::module_utils::ParentNameSource::Name,
    )?;

    Ok(())
}
