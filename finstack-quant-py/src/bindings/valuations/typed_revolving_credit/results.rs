//! Per-path Monte Carlo result of a stochastic revolving credit facility.

use pyo3::prelude::*;

use crate::bindings::core::money::PyMoney;
use crate::bindings::valuations::convert::money_to_py;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::EnhancedMonteCarloResult;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_revolving_credit.results", "EnhancedMonteCarloResult")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "EnhancedMonteCarloResult",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyEnhancedMonteCarloResult {
    /// Inner canonical Rust result.
    pub(crate) inner: EnhancedMonteCarloResult,
}

crate::bindings::macros::wire_methods!(
    PyEnhancedMonteCarloResult,
    EnhancedMonteCarloResult,
    "EnhancedMonteCarloResult",
    module = "finstack_quant.valuations.instruments._typed_revolving_credit.results",
    to_dict
);

#[pymethods]
impl PyEnhancedMonteCarloResult {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_revolving_credit.results", "EnhancedMonteCarloResult.pv_ci_95")]
    #[getter]
    fn pv_ci_95(&self) -> (PyMoney, PyMoney) {
        let (low, high) = self.inner.mc_result.estimate.ci_95;
        (money_to_py(low), money_to_py(high))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_revolving_credit.results", "EnhancedMonteCarloResult.draw_option_cost_ci_95")]
    #[getter]
    fn draw_option_cost_ci_95(&self) -> (PyMoney, PyMoney) {
        let (low, high) = self.inner.draw_option_cost.ci_95;
        (money_to_py(low), money_to_py(high))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_revolving_credit.results", "EnhancedMonteCarloResult.path_pvs")]
    #[getter]
    fn path_pvs(&self) -> Vec<f64> {
        self.inner
            .path_results
            .iter()
            .map(|path| path.pv.amount())
            .collect()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_revolving_credit.results", "EnhancedMonteCarloResult.path_draw_option_costs")]
    #[getter]
    fn path_draw_option_costs(&self) -> Vec<f64> {
        self.inner
            .path_results
            .iter()
            .map(|path| path.draw_option_cost.amount())
            .collect()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_revolving_credit.results", "EnhancedMonteCarloResult.utilization_paths")]
    #[getter]
    fn utilization_paths(&self) -> Vec<Vec<f64>> {
        self.inner
            .path_results
            .iter()
            .filter_map(|path| path.path_data.as_ref())
            .map(|data| data.utilization_path.clone())
            .collect()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_revolving_credit.results", "EnhancedMonteCarloResult.credit_spread_paths")]
    #[getter]
    fn credit_spread_paths(&self) -> Vec<Vec<f64>> {
        self.inner
            .path_results
            .iter()
            .filter_map(|path| path.path_data.as_ref())
            .map(|data| data.credit_spread_path.clone())
            .collect()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_revolving_credit.results", "EnhancedMonteCarloResult.observation_dates")]
    #[getter]
    fn observation_dates(&self) -> Vec<String> {
        self.inner
            .path_results
            .first()
            .and_then(|path| path.path_data.as_ref())
            .map(|data| data.payment_dates.iter().map(ToString::to_string).collect())
            .unwrap_or_default()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "EnhancedMonteCarloResult(pv={}, draw_option_cost={}, paths={})",
            self.inner.mc_result.estimate.mean.amount(),
            self.inner.draw_option_cost.mean.amount(),
            self.inner.path_results.len()
        )
    }
}

impl PyEnhancedMonteCarloResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<crate::bindings::core::table::PyArrowTable> {
        let table = self.inner.to_table().map_err(crate::errors::core_to_py)?;
        crate::bindings::core::table::PyArrowTable::from_envelope(&table)
    }
}

crate::bindings::macros::tabular_methods!(
    PyEnhancedMonteCarloResult,
    "EnhancedMonteCarloResult",
    module = "finstack_quant.valuations.instruments._typed_revolving_credit.results"
);

crate::bindings::macros::py_getters!(PyEnhancedMonteCarloResult |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_revolving_credit.results", "EnhancedMonteCarloResult.pv")]
    pv -> PyMoney = money_to_py(s.inner.mc_result.estimate.mean);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_revolving_credit.results", "EnhancedMonteCarloResult.pv_std_error")]
    pv_std_error -> f64 = s.inner.mc_result.estimate.stderr;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_revolving_credit.results", "EnhancedMonteCarloResult.num_paths")]
    num_paths -> usize = s.inner.mc_result.estimate.num_paths;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_revolving_credit.results", "EnhancedMonteCarloResult.num_simulated_paths")]
    num_simulated_paths -> usize = s.inner.path_results.len();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_revolving_credit.results", "EnhancedMonteCarloResult.draw_option_cost")]
    draw_option_cost -> PyMoney = money_to_py(s.inner.draw_option_cost.mean);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_revolving_credit.results", "EnhancedMonteCarloResult.draw_option_cost_std_error")]
    draw_option_cost_std_error -> f64 = s.inner.draw_option_cost.stderr;
});
