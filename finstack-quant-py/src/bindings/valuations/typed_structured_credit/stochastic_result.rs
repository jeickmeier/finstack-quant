//! Typed stochastic pricing result of a structured-credit deal.

use pyo3::prelude::*;

use crate::bindings::core::money::PyMoney;
use crate::bindings::pandas_utils::{serde_rows_to_dataframe_with_schema, serde_to_py};
use crate::bindings::valuations::convert::money_to_py;
use crate::errors::{display_to_py, serde_json_to_py};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::StochasticPricingResult;

/// Stochastic (scenario-waterfall) pricing result of a structured-credit deal
/// (``StructuredCredit.price_stochastic``'s return value).
///
/// Deal-level present value, loss statistics and Monte Carlo error sit next
/// to one ``TranchePricingResult`` per tranche (as dicts). Pools of real
/// instruments add the reserve-funding diagnostics
/// (``unfunded_draw_path_fraction``, ``expected_collateral_draws``) and the
/// draw option cost with its per-path distribution.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> import json
/// >>> from finstack_quant.core.currency import Currency
/// >>> from finstack_quant.core.dates import DayCount
/// >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
/// >>> from finstack_quant.core.money import Money
/// >>> from finstack_quant.valuations.instruments import (
/// ...     AssetPool,
/// ...     RepLine,
/// ...     StructuredCredit,
/// ...     Tranche,
/// ...     TrancheStructure,
/// ... )
/// >>> as_of, maturity = datetime.date(2024, 1, 15), datetime.date(2031, 1, 15)
/// >>> pool = AssetPool("POOL-1", "abs", Currency("USD")).with_rep_lines([
/// ...     RepLine(
/// ...         "LINE-1",
/// ...         Money(80_000_000.0, Currency("USD")),
/// ...         0.07,
/// ...         maturity,
/// ...         12,
/// ...         DayCount.ACT_360,
/// ...         asset_type={"type": "first_lien_loan", "industry": None},
/// ...     )
/// ... ])
/// >>> note = (
/// ...     Tranche
/// ...     .builder()
/// ...     .id("A")
/// ...     .attachment_point(0.0)
/// ...     .detachment_point(100.0)
/// ...     .seniority("senior")
/// ...     .original_balance(Money(80_000_000.0, Currency("USD")))
/// ...     .coupon_fixed(0.05)
/// ...     .maturity(maturity)
/// ...     .build()
/// ... )
/// >>> deal = StructuredCredit.new_abs("ABS-1", pool, TrancheStructure([note]), as_of, maturity, "USD-SOFR-DISC")
/// >>> envelope = json.loads(deal.to_json())
/// >>> envelope["instrument"]["spec"]["payment_calendar_id"] = "nyse"
/// >>> deal = StructuredCredit.from_json(json.dumps(envelope))
/// >>> market = MarketContext().insert(DiscountCurve.flat("USD-SOFR-DISC", as_of, 0.03))
/// >>> result = deal.price_stochastic(market, as_of, num_paths=8)
/// >>> (result.num_paths, [t["tranche_id"] for t in result.tranche_results])
/// (8, ['A'])
/// >>> result.unfunded_draw_path_fraction
/// 0.0
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
    /// Deserialize from the JSON produced by ``to_json``.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON-encoded ``StochasticPricingResult`` (the shape ``to_json`` writes).
    ///
    /// Returns
    /// -------
    /// StochasticPricingResult
    ///     The decoded result.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not valid JSON for the result shape.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import StochasticPricingResult
    /// >>> try:
    /// ...     StochasticPricingResult.from_json("{}")
    /// ... except ValueError:
    /// ...     print("rejected")
    /// rejected
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|err| serde_json_to_py(err, "invalid StochasticPricingResult JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize to the JSON shape ``from_json`` accepts.
    ///
    /// Returns
    /// -------
    /// str
    ///     JSON-encoded result.
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

    /// Mean deal present value across paths (pool currency).
    #[getter]
    fn npv(&self) -> PyMoney {
        money_to_py(self.inner.npv)
    }

    /// Dirty price as a percentage of the pool notional.
    #[getter]
    fn dirty_price(&self) -> f64 {
        self.inner.dirty_price
    }

    /// Expected (mean) loss across paths.
    #[getter]
    fn expected_loss(&self) -> PyMoney {
        money_to_py(self.inner.expected_loss)
    }

    /// Unexpected loss (standard deviation of the path losses).
    #[getter]
    fn unexpected_loss(&self) -> PyMoney {
        money_to_py(self.inner.unexpected_loss)
    }

    /// Expected shortfall of the path losses at ``es_confidence``.
    #[getter]
    fn expected_shortfall(&self) -> PyMoney {
        money_to_py(self.inner.expected_shortfall)
    }

    /// Confidence level of ``expected_shortfall`` (decimal).
    #[getter]
    fn es_confidence(&self) -> f64 {
        self.inner.es_confidence
    }

    /// Standard error of the mean present value, in currency units.
    #[getter]
    fn pv_std_error(&self) -> f64 {
        self.inner.pv_std_error
    }

    /// 95% confidence interval of the mean present value, in currency units.
    #[getter]
    fn pv_confidence_interval(&self) -> (f64, f64) {
        self.inner.pv_confidence_interval
    }

    /// Number of scenario paths.
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.num_paths
    }

    /// Pricing mode used, as its serde ``dict``.
    #[getter]
    fn pricing_mode<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.pricing_mode)
    }

    /// Fraction of paths on which a collateral draw could not be funded from
    /// the reserve account and that period's principal collections.
    #[getter]
    fn unfunded_draw_path_fraction(&self) -> f64 {
        self.inner.unfunded_draw_path_fraction
    }

    /// Mean over paths of the collateral draws funded through the reserve
    /// account and principal collections.
    #[getter]
    fn expected_collateral_draws(&self) -> PyMoney {
        money_to_py(self.inner.expected_collateral_draws)
    }

    /// Mean draw option cost across paths: the value to the deal of its
    /// revolvers' draws having been made at the contractual margin instead
    /// of each path's fair spread (negative when spreads widen).
    #[getter]
    fn draw_option_cost(&self) -> PyMoney {
        money_to_py(self.inner.draw_option_cost)
    }

    /// Per-path draw option cost in path order (empty for pools without
    /// stochastic revolvers).
    #[getter]
    fn draw_option_cost_paths(&self) -> Vec<f64> {
        self.inner.draw_option_cost_paths.clone()
    }

    /// Tranche-level results as a list of dicts (``TranchePricingResult``
    /// serde shape, in capital-structure order).
    #[getter]
    fn tranche_results<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.tranche_results)
    }

    /// One row per tranche as a pandas ``DataFrame``.
    ///
    /// Columns: ``tranche_id``, ``seniority``, ``npv``, ``expected_loss``,
    /// ``unexpected_loss``, ``expected_shortfall`` (currency units),
    /// ``attachment``, ``detachment`` (decimal), ``average_life`` (years),
    /// ``credit_duration`` and ``draw_option_cost`` (currency units).
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///     The tranche summary.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the rows cannot be serialized.
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
                    "expected_loss": tranche.expected_loss.amount(),
                    "unexpected_loss": tranche.unexpected_loss.amount(),
                    "expected_shortfall": tranche.expected_shortfall.amount(),
                    "attachment": tranche.attachment,
                    "detachment": tranche.detachment,
                    "average_life": tranche.average_life,
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
                ("expected_loss", "float64"),
                ("unexpected_loss", "float64"),
                ("expected_shortfall", "float64"),
                ("attachment", "float64"),
                ("detachment", "float64"),
                ("average_life", "float64"),
                ("credit_duration", "float64"),
                ("draw_option_cost", "float64"),
            ],
        )
    }

    /// The per-path draw option cost distribution as a pandas ``DataFrame``.
    ///
    /// Columns: ``path`` (index in path order) and ``draw_option_cost``
    /// (currency units). Empty for pools without stochastic revolvers.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///     One row per path.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the rows cannot be serialized.
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
