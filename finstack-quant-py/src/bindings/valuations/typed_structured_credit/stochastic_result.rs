//! Typed stochastic pricing result of a structured-credit deal.

use pyo3::prelude::*;

use crate::bindings::core::money::PyMoney;
use crate::bindings::pandas_utils::{serde_rows_to_dataframe_with_schema, serde_to_py};
use crate::bindings::valuations::convert::money_to_py;
use crate::errors::{display_to_py, serde_json_to_py};
use finstack_quant_valuations::results::structured_credit::StochasticPricingResult;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "StochasticPricingResult",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyStochasticPricingResult {
    /// Inner canonical Rust result.
    pub(crate) inner: StochasticPricingResult,
}

#[pymethods]
impl PyStochasticPricingResult {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|err| serde_json_to_py(err, "invalid StochasticPricingResult JSON"))?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult.to_dict")]
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult.pricing_mode")]
    #[getter]
    fn pricing_mode<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.pricing_mode)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult.tranche_results")]
    #[getter]
    fn tranche_results<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.tranche_results)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult.to_dataframe")]
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .tranche_results
            .iter()
            .map(|tranche| {
                serde_json::json!({
                    "tranche_id": tranche.tranche_id,
                    "seniority": serde_json::to_value(tranche.seniority).unwrap_or_default(),
                    "npv": tranche.npv.amount(),
                    "price_pct": tranche.price_pct,
                    "expected_loss": tranche.expected_loss.amount(),
                    "unexpected_loss": tranche.unexpected_loss.amount(),
                    "expected_shortfall": tranche.expected_shortfall.amount(),
                    "attach_pct": tranche.attach_pct,
                    "detach_pct": tranche.detach_pct,
                    "wal": tranche.wal,
                    "paths_with_principal": tranche.paths_with_principal,
                    "credit_duration": tranche.credit_duration,
                    "draw_option_cost": tranche.draw_option_cost.amount(),
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(
            py,
            &rows,
            &[
                ("tranche_id", "str"),
                ("seniority", "str"),
                ("npv", "float64"),
                ("price_pct", "float64"),
                ("expected_loss", "float64"),
                ("unexpected_loss", "float64"),
                ("expected_shortfall", "float64"),
                ("attach_pct", "float64"),
                ("detach_pct", "float64"),
                ("wal", "float64"),
                ("paths_with_principal", "int64"),
                ("credit_duration", "float64"),
                ("draw_option_cost", "float64"),
            ],
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult.draw_option_cost_dataframe")]
    #[pyo3(text_signature = "($self)")]
    fn draw_option_cost_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .draw_option_cost_paths
            .iter()
            .enumerate()
            .map(|(path, cost)| serde_json::json!({ "path": path, "draw_option_cost": cost }))
            .collect();
        serde_rows_to_dataframe_with_schema(
            py,
            &rows,
            &[("path", "int64"), ("draw_option_cost", "float64")],
        )
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "StochasticPricingResult(npv={}, expected_loss={}, num_paths={}, tranches={})",
            self.inner.npv.amount(),
            self.inner.expected_loss.amount(),
            self.inner.num_paths,
            self.inner.tranche_results.len()
        )
    }
}

crate::bindings::macros::py_getters!(PyStochasticPricingResult |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult.npv")]
    npv -> PyMoney = money_to_py(s.inner.npv);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult.dirty_price")]
    dirty_price -> f64 = s.inner.dirty_price;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult.expected_loss")]
    expected_loss -> PyMoney = money_to_py(s.inner.expected_loss);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult.unexpected_loss")]
    unexpected_loss -> PyMoney = money_to_py(s.inner.unexpected_loss);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult.expected_shortfall")]
    expected_shortfall -> PyMoney = money_to_py(s.inner.expected_shortfall);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult.es_confidence")]
    es_confidence -> f64 = s.inner.es_confidence;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult.pv_std_error")]
    pv_std_error -> f64 = s.inner.pv_std_error;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult.pv_confidence_interval")]
    pv_confidence_interval -> (f64, f64) = s.inner.pv_confidence_interval;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult.num_paths")]
    num_paths -> usize = s.inner.num_paths;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult.unfunded_draw_path_fraction")]
    unfunded_draw_path_fraction -> f64 = s.inner.unfunded_draw_path_fraction;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult.expected_collateral_draws")]
    expected_collateral_draws -> PyMoney = money_to_py(s.inner.expected_collateral_draws);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult.draw_option_cost")]
    draw_option_cost -> PyMoney = money_to_py(s.inner.draw_option_cost);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result", "StochasticPricingResult.draw_option_cost_paths")]
    draw_option_cost_paths -> Vec<f64> = s.inner.draw_option_cost_paths.clone();
});
