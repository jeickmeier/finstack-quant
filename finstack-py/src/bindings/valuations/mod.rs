//! Python bindings for the `finstack-valuations` crate.
//!
//! Exposes the [`PyValuationResult`] envelope for pricing output,
//! JSON-based instrument loading and the standard pricer pipeline.

mod analytic;
mod calibration;
pub mod correlation;
mod credit;
mod credit_derivatives;
pub(crate) mod credit_factor_model;
mod direct_wrapper;
mod exotic_rates;
mod exotics;
mod fourier;
mod fx;
mod pricing;
mod sabr;

use crate::bindings::pandas_utils::dict_to_dataframe;
use crate::errors::display_to_py;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

/// Parse an ISO 8601 date string into a `time::Date`.
fn parse_date(s: &str) -> PyResult<time::Date> {
    finstack_core::dates::parse_iso_date(s).map_err(display_to_py)
}

// ---------------------------------------------------------------------------
// ValuationResult
// ---------------------------------------------------------------------------

#[pyclass(
    name = "ValuationResult",
    module = "finstack.valuations",
    skip_from_py_object
)]
#[derive(Clone)]
struct PyValuationResult {
    pub(crate) inner: finstack_valuations::results::ValuationResult,
}

#[pymethods]
impl PyValuationResult {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_valuations::results::ValuationResult =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn instrument_id(&self) -> &str {
        &self.inner.instrument_id
    }

    #[getter]
    fn get_price(&self) -> f64 {
        self.inner.value.amount()
    }

    #[getter]
    fn currency(&self) -> String {
        self.inner.value.currency().to_string()
    }

    fn get_metric(&self, key: &str) -> Option<f64> {
        self.inner.metric_str(key)
    }

    fn metric_keys(&self) -> Vec<String> {
        self.inner.measures.keys().map(|k| k.to_string()).collect()
    }

    fn metric_count(&self) -> usize {
        self.inner.measures.len()
    }

    fn all_covenants_passed(&self) -> bool {
        self.inner.all_covenants_passed()
    }

    fn failed_covenants(&self) -> Vec<String> {
        self.inner
            .failed_covenants()
            .into_iter()
            .map(String::from)
            .collect()
    }

    /// Export as a single-row pandas ``DataFrame``.
    ///
    /// Columns include ``instrument_id``, ``price``, ``currency``, plus one
    /// column per metric key.  Useful for stacking multiple results with
    /// ``pd.concat``.
    fn metrics_to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        data.set_item("instrument_id", vec![&self.inner.instrument_id])?;
        data.set_item("price", vec![self.inner.value.amount()])?;
        data.set_item("currency", vec![self.inner.value.currency().to_string()])?;
        for (key, &val) in &self.inner.measures {
            data.set_item(key.to_string(), vec![val])?;
        }
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "ValuationResult(id={:?}, price={:.4}, currency={}, metrics={})",
            self.inner.instrument_id,
            self.inner.value.amount(),
            self.inner.value.currency(),
            self.inner.measures.len()
        )
    }
}

// ---------------------------------------------------------------------------
// InstrumentJson — tagged-union loader
// ---------------------------------------------------------------------------

#[pyfunction]
fn validate_instrument_json(json: &str) -> PyResult<String> {
    let canonical = finstack_valuations::pricer::validate_instrument_json(json)
        .map_err(crate::errors::display_to_py)?;
    let parsed: serde_json::Value = serde_json::from_str(&canonical).map_err(display_to_py)?;
    serde_json::to_string_pretty(&parsed).map_err(display_to_py)
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "valuations")?;
    m.setattr(
        "__doc__",
        "Instrument pricing: bonds, swaps, options, and calibration.",
    )?;

    m.add_class::<PyValuationResult>()?;
    m.add_function(wrap_pyfunction!(validate_instrument_json, &m)?)?;
    pricing::register(py, &m)?;
    analytic::register(py, &m)?;
    sabr::register(py, &m)?;
    credit_factor_model::register(py, &m)?;
    calibration::register(py, &m)?;
    fourier::register(py, &m)?;
    exotic_rates::register(py, &m)?;
    correlation::register(py, &m)?;
    credit::register(py, &m)?;
    credit_derivatives::register(py, &m)?;
    exotics::register(py, &m)?;
    fx::register(py, &m)?;

    let all = PyList::new(
        py,
        [
            "ValuationResult",
            "validate_instrument_json",
            "price_instrument",
            "price_instrument_with_metrics",
            "instrument_cashflows_json",
            "list_standard_metrics",
            "list_standard_metrics_grouped",
            "CalibrationResult",
            "CalibrationEnvelopeError",
            "validate_calibration_json",
            "calibrate",
            "dry_run",
            "dependency_graph_json",
            "bs_cos_price",
            "vg_cos_price",
            "merton_jump_cos_price",
            "tarn_coupon_profile",
            "snowball_coupon_profile",
            "cms_spread_option_intrinsic",
            "callable_range_accrual_accrued",
            "bs_price",
            "bs_greeks",
            "bs_implied_vol",
            "black76_implied_vol",
            "barrier_call",
            "asian_option_price",
            "lookback_option_price",
            "quanto_option_price",
            "SabrParameters",
            "SabrModel",
            "SabrSmile",
            "SabrCalibrator",
            "CreditFactorModel",
            "CreditCalibrator",
            "LevelsAtDate",
            "PeriodDecomposition",
            "FactorCovarianceForecast",
            "decompose_levels",
            "decompose_period",
            "correlation",
            "credit",
            "credit_derivatives",
            "exotics",
            "fx",
        ],
    )?;
    m.setattr("__all__", all)?;
    parent.add_submodule(&m)?;

    let parent_name: String = match parent.getattr("__name__") {
        Ok(attr) => match attr.extract::<String>() {
            Ok(s) => s,
            Err(_) => "finstack.finstack".to_string(),
        },
        Err(_) => "finstack.finstack".to_string(),
    };
    let qual = format!("{parent_name}.valuations");
    m.setattr("__package__", &qual)?;
    let sys = PyModule::import(py, "sys")?;
    let modules = sys.getattr("modules")?;
    modules.set_item(&qual, &m)?;

    Ok(())
}
