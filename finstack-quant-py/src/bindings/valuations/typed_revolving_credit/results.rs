//! Per-path Monte Carlo result of a stochastic revolving credit facility.

use pyo3::prelude::*;

use crate::bindings::core::money::PyMoney;
use crate::bindings::pandas_utils::{serde_rows_to_dataframe_with_schema, serde_to_py};
use crate::bindings::valuations::convert::money_to_py;
use crate::errors::{display_to_py, serde_json_to_py};
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::EnhancedMonteCarloResult;

/// Monte Carlo result of a stochastic revolving credit facility with every
/// simulated path retained (``RevolvingCredit.price_with_paths``'s return
/// value).
///
/// The present value and the draw option cost are antithetic-aware estimates
/// (mean, standard error, 95% interval); ``path_pvs`` and
/// ``path_draw_option_costs`` carry the per-path values in path order and
/// :meth:`to_dataframe` tabulates them.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> import json
/// >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
/// >>> from finstack_quant.valuations.instruments import RevolvingCredit
/// >>> envelope = json.loads(RevolvingCredit.example().to_json())
/// >>> spec = envelope["instrument"]["spec"]
/// >>> spec["base_rate_spec"] = {"fixed": {"rate": 0.06}}
/// >>> spec["draw_repay_spec"] = {
/// ...     "stochastic": {
/// ...         "utilization_process": {"mean_reverting": {"target_rate": 0.6, "speed": 1.0, "volatility": 0.25}},
/// ...         "num_paths": 16,
/// ...         "seed": 42,
/// ...         "mc_config": {
/// ...             "recovery_rate": spec["recovery_rate"],
/// ...             "credit_spread_process": {"constant": 0.025},
/// ...         },
/// ...     }
/// ... }
/// >>> facility = RevolvingCredit.from_json(json.dumps(envelope))
/// >>> as_of = datetime.date(2024, 1, 15)
/// >>> market = MarketContext().insert(DiscountCurve.flat("USD-OIS", as_of, 0.03))
/// >>> result = facility.price_with_paths(market, as_of)
/// >>> (result.num_simulated_paths, len(result.path_pvs), result.pv.currency.code)
/// (16, 16, 'USD')
/// >>> list(result.to_dataframe().columns)
/// ['path', 'pv', 'draw_option_cost']
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

#[pymethods]
impl PyEnhancedMonteCarloResult {
    /// Deserialize from the JSON produced by ``to_json``.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON-encoded ``EnhancedMonteCarloResult`` (the exact shape
    ///     ``to_json`` writes, including every path's cashflow schedule).
    ///
    /// Returns
    /// -------
    /// EnhancedMonteCarloResult
    ///     The decoded result.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not valid JSON for the result shape.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import EnhancedMonteCarloResult
    /// >>> try:
    /// ...     EnhancedMonteCarloResult.from_json("{}")
    /// ... except ValueError:
    /// ...     print("rejected")
    /// rejected
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|err| serde_json_to_py(err, "invalid EnhancedMonteCarloResult JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize to the JSON shape ``from_json`` accepts.
    ///
    /// Returns
    /// -------
    /// str
    ///     JSON-encoded result, including every path's cashflow schedule and
    ///     factor paths.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the value cannot be serialized to JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Return every field as a plain ``dict`` (canonical serde shape).
    ///
    /// Returns
    /// -------
    /// dict
    ///     Serde form of the Rust result.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the value cannot be serialized.
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    /// Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Mean present value across paths (facility currency).
    #[getter]
    fn pv(&self) -> PyMoney {
        money_to_py(self.inner.mc_result.estimate.mean)
    }

    /// Standard error of the mean present value, in currency units.
    #[getter]
    fn pv_std_error(&self) -> f64 {
        self.inner.mc_result.estimate.stderr
    }

    /// 95% confidence interval of the mean present value.
    #[getter]
    fn pv_ci_95(&self) -> (PyMoney, PyMoney) {
        let (low, high) = self.inner.mc_result.estimate.ci_95;
        (money_to_py(low), money_to_py(high))
    }

    /// Number of independent path estimators (antithetic pairs count once).
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.mc_result.estimate.num_paths
    }

    /// Number of simulated paths, including both members of antithetic pairs.
    #[getter]
    fn num_simulated_paths(&self) -> usize {
        self.inner.path_results.len()
    }

    /// Mean draw option cost across paths (facility currency): the value to
    /// the lender of the simulated draws having been made at the contractual
    /// margin instead of each path's fair spread. Negative when spreads
    /// widen after draws.
    #[getter]
    fn draw_option_cost(&self) -> PyMoney {
        money_to_py(self.inner.draw_option_cost.mean)
    }

    /// Standard error of the mean draw option cost, in currency units.
    #[getter]
    fn draw_option_cost_std_error(&self) -> f64 {
        self.inner.draw_option_cost.stderr
    }

    /// 95% confidence interval of the mean draw option cost.
    #[getter]
    fn draw_option_cost_ci_95(&self) -> (PyMoney, PyMoney) {
        let (low, high) = self.inner.draw_option_cost.ci_95;
        (money_to_py(low), money_to_py(high))
    }

    /// Present value of every simulated path, in path order.
    #[getter]
    fn path_pvs(&self) -> Vec<f64> {
        self.inner
            .path_results
            .iter()
            .map(|path| path.pv.amount())
            .collect()
    }

    /// Draw option cost of every simulated path, in path order.
    #[getter]
    fn path_draw_option_costs(&self) -> Vec<f64> {
        self.inner
            .path_results
            .iter()
            .map(|path| path.draw_option_cost.amount())
            .collect()
    }

    /// Simulated utilization trajectories, one list per path, aligned with
    /// ``observation_dates``.
    #[getter]
    fn utilization_paths(&self) -> Vec<Vec<f64>> {
        self.inner
            .path_results
            .iter()
            .filter_map(|path| path.path_data.as_ref())
            .map(|data| data.utilization_path.clone())
            .collect()
    }

    /// Simulated credit-spread trajectories (decimal), one list per path,
    /// aligned with ``observation_dates``.
    #[getter]
    fn credit_spread_paths(&self) -> Vec<Vec<f64>> {
        self.inner
            .path_results
            .iter()
            .filter_map(|path| path.path_data.as_ref())
            .map(|data| data.credit_spread_path.clone())
            .collect()
    }

    /// Observation dates of the factor trajectories (ISO 8601 strings).
    #[getter]
    fn observation_dates(&self) -> Vec<String> {
        self.inner
            .path_results
            .first()
            .and_then(|path| path.path_data.as_ref())
            .map(|data| data.payment_dates.iter().map(ToString::to_string).collect())
            .unwrap_or_default()
    }

    /// One row per simulated path as a pandas ``DataFrame``.
    ///
    /// Columns: ``path`` (index in path order), ``pv`` and
    /// ``draw_option_cost`` (facility currency units).
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///     The per-path distribution of present value and draw option cost.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the rows cannot be serialized.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .path_results
            .iter()
            .enumerate()
            .map(|(path, result)| {
                serde_json::json!({
                    "path": path,
                    "pv": result.pv.amount(),
                    "draw_option_cost": result.draw_option_cost.amount(),
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(
            py,
            &rows,
            &[
                ("path", "int64"),
                ("pv", "float64"),
                ("draw_option_cost", "float64"),
            ],
        )
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
