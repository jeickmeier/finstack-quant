//! Python bindings for the `finstack-quant-valuations` crate.
//!
//! Exposes the [`PyValuationResult`] envelope for pricing output,
//! JSON-based instrument loading and the standard pricer pipeline.

mod analytic;
mod calibration;
pub mod correlation;
mod credit;
mod exotic_rates;
mod fourier;
pub(crate) mod instruments;
mod pricing;
mod sabr;
mod structured_credit;

use crate::bindings::pandas_utils::dict_to_dataframe;
use crate::errors::display_to_py;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

// ---------------------------------------------------------------------------
// ValuationResult
// ---------------------------------------------------------------------------

#[pyclass(
    name = "ValuationResult",
    module = "finstack_quant.valuations",
    skip_from_py_object
)]
#[derive(Clone)]
struct PyValuationResult {
    pub(crate) inner: finstack_quant_valuations::results::ValuationResult,
}

#[pymethods]
impl PyValuationResult {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_valuations::results::ValuationResult =
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

    /// Return the exact Decimal price as a string, without a float round-trip.
    ///
    /// Unlike the ``price`` property (a lossy ``float``), this preserves the
    /// internal Decimal representation exactly. Pass the result to
    /// ``decimal.Decimal`` for lossless arithmetic in Python.
    ///
    /// Returns
    /// -------
    /// str
    ///     Exact decimal string of the valuation amount, e.g. ``"1000000.00"``.
    #[pyo3(text_signature = "($self)")]
    fn price_decimal(&self) -> String {
        self.inner.value.amount_decimal().to_string()
    }

    #[getter]
    fn currency(&self) -> String {
        self.inner.value.currency().to_string()
    }

    fn get_metric(&self, key: &str) -> Option<f64> {
        self.inner.metric_str(key)
    }

    /// Return decoded component vectors and values for a composite base metric.
    ///
    /// Results preserve the underlying ``measures`` insertion order. Legacy
    /// malformed escapes remain literal, and decoded-coordinate collisions
    /// fall back to literal wire components so every value remains visible.
    fn metric_series(&self, base: &str) -> Vec<(Vec<String>, f64)> {
        let base = finstack_quant_valuations::metrics::MetricId::custom(base);
        self.inner.metric_series(&base)
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
    finstack_quant_valuations::pricer::validate_instrument_json(json)
        .map_err(crate::errors::display_to_py)
}

/// Construct tagged bond instrument JSON from a cashflow schedule.
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
        finstack_quant_valuations::instruments::fixed_income::bond::bond_from_cashflows_json(
            instrument_id,
            schedule_json,
            discount_curve_id,
            quoted_clean,
        )
        .map_err(crate::errors::core_to_py)
    })
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "valuations")?;
    let qual = crate::bindings::module_utils::set_submodule_package(
        parent,
        &m,
        "valuations",
        crate::bindings::module_utils::ROOT_PACKAGE,
        crate::bindings::module_utils::ParentNameSource::Name,
    )?;
    m.setattr(
        "__doc__",
        "Instrument pricing: bonds, swaps, options, and calibration.",
    )?;

    m.add_class::<PyValuationResult>()?;
    analytic::register(py, &m)?;
    sabr::register(py, &m)?;
    calibration::register(py, &m)?;
    fourier::register(py, &m)?;
    exotic_rates::register(py, &m)?;
    correlation::register(py, &m)?;
    register_instruments(py, &m)?;
    register_models(py, &m)?;

    let all = PyList::new(
        py,
        [
            "ValuationResult",
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
            "inverse_floater_coupon_profile",
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
            "correlation",
            "instruments",
            "models",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule_at(py, parent, &m, &qual)?;

    Ok(())
}

fn register_instruments(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "instruments")?;
    let qual = crate::bindings::module_utils::set_submodule_package_by_package(
        parent,
        &m,
        "instruments",
        "finstack_quant.finstack_quant.valuations",
    )?;
    m.setattr(
        "__doc__",
        "JSON validation, pricing, metric, and cashflow helpers for valuation workflows.",
    )?;

    m.add_function(wrap_pyfunction!(validate_instrument_json, &m)?)?;
    m.add_function(wrap_pyfunction!(bond_from_cashflows_json, &m)?)?;
    m.getattr("bond_from_cashflows_json")?
        .setattr("__module__", "finstack_quant.valuations.instruments")?;
    instruments::register(py, &m)?;
    pricing::register(py, &m)?;
    structured_credit::register(&m)?;
    let mut exports = vec![
        "Bond",
        "TermLoan",
        "bond_from_cashflows_json",
        "instrument_cashflows_json",
        "list_models",
        "list_models_grouped",
        "list_standard_metrics",
        "list_standard_metrics_grouped",
        "price_instrument",
        "price_instrument_with_metrics",
    ];
    exports.extend_from_slice(structured_credit::EXPORTS);
    exports.push("validate_instrument_json");
    let all = PyList::new(py, exports)?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule_at(py, parent, &m, &qual)?;
    Ok(())
}

fn register_models(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "models")?;
    let qual = crate::bindings::module_utils::set_submodule_package_by_package(
        parent,
        &m,
        "models",
        "finstack_quant.finstack_quant.valuations",
    )?;
    m.setattr("__doc__", "Pricing model wrappers for valuation workflows.")?;

    credit::register(py, &m)?;

    let all = PyList::new(py, ["credit"])?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule_at(py, parent, &m, &qual)?;
    Ok(())
}
