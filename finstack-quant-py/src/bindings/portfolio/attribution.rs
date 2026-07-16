//! Portfolio-level P&L attribution bindings.

use crate::bindings::core::money::PyMoney;
use crate::bindings::extract::{extract_market_ref, extract_portfolio_ref};
use crate::bindings::module_utils::py_to_json_string;
use crate::errors::{display_to_py, portfolio_to_py, serde_json_to_py};
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Portfolio-level P&L attribution result.
///
/// Aggregate fields are currency-tagged :class:`~finstack_quant.core.money.Money`
/// values computed by Rust. Per-position and detailed breakdowns remain available
/// through the canonical nested JSON payload.
#[pyclass(
    name = "PortfolioAttribution",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyPortfolioAttribution {
    inner: finstack_quant_portfolio::attribution::PortfolioAttribution,
}

impl PyPortfolioAttribution {
    fn from_inner(inner: finstack_quant_portfolio::attribution::PortfolioAttribution) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPortfolioAttribution {
    /// Serialize the complete canonical attribution payload to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Serialize the position-native nested attribution map to compact JSON.
    ///
    /// Position keys retain the canonical Rust ``IndexMap`` insertion order.
    fn by_position_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner.by_position).map_err(display_to_py)
    }

    /// Check that aggregate factor P&L reconciles to total P&L.
    fn reconciliation_check<'py>(
        &self,
        py: Python<'py>,
        tolerance: f64,
    ) -> PyResult<Bound<'py, PyDict>> {
        let report = self.inner.reconciliation_check(tolerance);
        let result = PyDict::new(py);
        result.set_item("total_residual", report.total_residual)?;
        result.set_item("is_reconciled", report.is_reconciled)?;
        result.set_item("tolerance", report.tolerance)?;
        Ok(result)
    }

    #[getter]
    fn total_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.total_pnl)
    }

    #[getter]
    fn carry(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.carry)
    }

    #[getter]
    fn rates_curves_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.rates_curves_pnl)
    }

    #[getter]
    fn credit_curves_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.credit_curves_pnl)
    }

    #[getter]
    fn inflation_curves_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.inflation_curves_pnl)
    }

    #[getter]
    fn correlations_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.correlations_pnl)
    }

    #[getter]
    fn fx_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.fx_pnl)
    }

    #[getter]
    fn fx_translation_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.fx_translation_pnl)
    }

    #[getter]
    fn cross_factor_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.cross_factor_pnl)
    }

    #[getter]
    fn vol_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.vol_pnl)
    }

    #[getter]
    fn model_params_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.model_params_pnl)
    }

    #[getter]
    fn market_scalars_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.market_scalars_pnl)
    }

    #[getter]
    fn residual(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.residual)
    }

    #[getter]
    fn result_invalid(&self) -> bool {
        self.inner.result_invalid
    }

    fn __repr__(&self) -> String {
        format!(
            "PortfolioAttribution(total_pnl={}, positions={}, result_invalid={})",
            self.inner.total_pnl,
            self.inner.by_position.len(),
            self.inner.result_invalid,
        )
    }
}

/// Attribute portfolio P&L between two market snapshots.
///
/// ``portfolio`` and both markets accept either typed binding objects or their
/// canonical JSON representations. ``method`` uses the same serde shape as
/// instrument attribution (for example ``"Parallel"`` or
/// ``{"Waterfall": ["Carry", "RatesCurves"]}``). ``config`` is an optional
/// canonical ``FinstackConfig`` dictionary or JSON string.
#[pyfunction]
#[pyo3(signature = (portfolio, market_t0, market_t1, as_of_t0, as_of_t1, method, config=None))]
#[allow(clippy::too_many_arguments)]
fn attribute_portfolio_pnl(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    market_t0: &Bound<'_, PyAny>,
    market_t1: &Bound<'_, PyAny>,
    as_of_t0: &str,
    as_of_t1: &str,
    method: &Bound<'_, PyAny>,
    config: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyPortfolioAttribution> {
    let portfolio = extract_portfolio_ref(portfolio)?;
    let market_t0 = extract_market_ref(market_t0)?;
    let market_t1 = extract_market_ref(market_t1)?;
    let as_of_t0 = super::parse_date(as_of_t0)?;
    let as_of_t1 = super::parse_date(as_of_t1)?;

    let method_json = py_to_json_string(py, method, "method")?;
    let method = serde_json::from_str(&method_json)
        .map_err(|error| serde_json_to_py(error, "invalid attribution method"))?;
    let config = config
        .map(|value| {
            let json = py_to_json_string(py, value, "config")?;
            serde_json::from_str(&json)
                .map_err(|error| serde_json_to_py(error, "invalid finstack config"))
        })
        .transpose()?
        .unwrap_or_default();

    let portfolio_ref: &finstack_quant_portfolio::Portfolio = &portfolio;
    let market_t0_ref: &finstack_quant_core::market_data::context::MarketContext = &market_t0;
    let market_t1_ref: &finstack_quant_core::market_data::context::MarketContext = &market_t1;
    let result = py
        .detach(|| {
            finstack_quant_portfolio::attribution::attribute_portfolio_pnl(
                portfolio_ref,
                market_t0_ref,
                market_t1_ref,
                as_of_t0,
                as_of_t1,
                &config,
                method,
            )
        })
        .map_err(portfolio_to_py)?;
    Ok(PyPortfolioAttribution::from_inner(result))
}

pub fn register(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyPortfolioAttribution>()?;
    module.add_function(pyo3::wrap_pyfunction!(attribute_portfolio_pnl, module)?)?;
    Ok(())
}
