//! Deal-level accounting of a deterministic structured-credit simulation.

use pyo3::prelude::*;

use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::date_to_py;
use crate::bindings::pandas_utils::{serde_rows_to_dataframe_with_schema, serde_to_py};
use crate::bindings::valuations::convert::money_to_py;
use crate::errors::{display_to_py, serde_json_to_py};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::SimulationDiagnostics;

/// Deal-level accounting of one deterministic simulation
/// (the diagnostics member of ``StructuredCredit.run_simulation``'s result):
/// the per-period pool, collections, cash-account and coverage-test record
/// plus the reserve-account and draw-funding totals.
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
/// ...     StandardDealParams, StructuredCredit,
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
/// ...         asset_type={"type": "first_lien_loan"},
/// ...     )
/// ... ])
/// >>> note = (
/// ...     Tranche
/// ...     .builder()
/// ...     .id("A")
/// ...     .attach_pct(0.0)
/// ...     .detach_pct(100.0)
/// ...     .seniority("senior")
/// ...     .original_balance(Money(80_000_000.0, Currency("USD")))
/// ...     .coupon_fixed(0.05)
/// ...     .maturity(maturity)
/// ...     .build()
/// ... )
/// >>> deal = StructuredCredit.new_standard("abs", StandardDealParams("ABS-1", pool, TrancheStructure([note]), as_of, maturity, "USD-SOFR-DISC"))
/// >>> envelope = json.loads(deal.to_json())
/// >>> envelope["instrument"]["spec"]["calendar_id"] = "nyse"
/// >>> deal = StructuredCredit.from_json(json.dumps(envelope))
/// >>> market = MarketContext().insert(DiscountCurve.flat("USD-SOFR-DISC", as_of, 0.03))
/// >>> diagnostics = deal.run_simulation(market, as_of).diagnostics
/// >>> diagnostics.unfunded_draws.amount
/// 0.0
/// >>> list(diagnostics.to_dataframe().columns)[:3]
/// ['date', 'pool_balance', 'pool_factor']
/// >>> len(diagnostics.periods) == len(diagnostics.to_dataframe())
/// True
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "SimulationDiagnostics",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PySimulationDiagnostics {
    /// Inner canonical Rust diagnostics.
    pub(crate) inner: SimulationDiagnostics,
}

#[pymethods]
impl PySimulationDiagnostics {
    /// Deserialize from the JSON produced by ``to_json``.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON-encoded ``SimulationDiagnostics`` (the shape ``to_json`` writes).
    ///
    /// Returns
    /// -------
    /// SimulationDiagnostics
    ///     The decoded diagnostics.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not valid JSON for the diagnostics shape.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import SimulationDiagnostics
    /// >>> try:
    /// ...     SimulationDiagnostics.from_json("{}")
    /// ... except ValueError:
    /// ...     print("rejected")
    /// rejected
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|err| serde_json_to_py(err, "invalid SimulationDiagnostics JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize to the JSON shape ``from_json`` accepts.
    ///
    /// Returns
    /// -------
    /// str
    ///     JSON-encoded diagnostics.
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
    ///     Serde form of the Rust diagnostics.
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

    /// Reserve-account balance at the end of each simulated period, as
    /// ``(datetime.date, Money)`` pairs.
    #[getter]
    fn reserve_balance_path<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .reserve_balance_path
            .iter()
            .map(|(date, amount)| Ok((date_to_py(py, *date)?, money_to_py(*amount))))
            .collect()
    }

    /// Reserve interest earned each period, before routing, as
    /// ``(datetime.date, Money)`` pairs.
    #[getter]
    fn reserve_interest_paid<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .reserve_interest_paid
            .iter()
            .map(|(date, amount)| Ok((date_to_py(py, *date)?, money_to_py(*amount))))
            .collect()
    }

    /// Payment date of the early-amortization event (or coverage-test
    /// acceleration) that ended the revolving period, or ``None``.
    #[getter]
    fn early_amortization_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .early_amortization_date
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    /// Lender draws applied to notes as ``(tranche_id, datetime.date, Money)``
    /// triples.
    #[getter]
    fn tranche_draws<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Vec<(String, Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .tranche_draws
            .iter()
            .map(|(id, date, amount)| {
                Ok((id.clone(), date_to_py(py, *date)?, money_to_py(*amount)))
            })
            .collect()
    }

    /// Per-period deal record as ``PeriodDiagnostics`` serde dicts:
    /// ``payment_date``, ``pool_balance``, ``pool_factor``,
    /// ``wac``, ``weighted_avg_spread_bp``, ``warf``, the
    /// period's collections, defaults, recoveries, reinvested par, fees paid,
    /// the cash-account balances, ``delinquent_balance``,
    /// ``servicer_advances_outstanding``, ``excess_spread``
    /// and the ``coverage_tests`` the executor evaluated.
    #[getter]
    fn periods<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.periods)
    }

    /// One row per simulated period as a pandas ``DataFrame``.
    ///
    /// Columns: ``date`` (ISO 8601 string), ``pool_balance``,
    /// ``pool_factor``, ``wac`` (decimal),
    /// ``weighted_avg_spread_bp``, ``warf``, ``interest_collections``,
    /// ``principal_collections``, ``defaults``, ``recoveries``,
    /// ``reinvested_par``, ``fees_paid``, ``reserve_balance``,
    /// ``reserve_interest``, ``spread_account``, ``funding_account``,
    /// ``delinquent_balance``, ``servicer_advances_outstanding`` and
    /// ``excess_spread`` (annualized decimal);
    /// amounts in their stored currency units without FX conversion. Rows keep
    /// stored period order and duplicate dates. Reserve interest uses the first
    /// matching date in ``reserve_interest_paid``; missing matches are zero.
    /// An empty result retains all nineteen columns with a string date and
    /// float64 numeric dtypes.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///     The period record.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the canonical table cannot be built or converted.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::table_to_dataframe(
            py,
            &self.inner.to_table().map_err(crate::errors::core_to_py)?,
        )
    }

    /// Coverage-test evaluations as a long pandas ``DataFrame``.
    ///
    /// Columns: ``date`` (ISO 8601 string), ``test_id``, ``ratio``,
    /// ``trigger_level``, ``cushion`` (ratio minus trigger) and ``passing``.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///     One row per test per period.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the rows cannot be serialized.
    #[pyo3(text_signature = "($self)")]
    fn coverage_tests_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .periods
            .iter()
            .flat_map(|period| {
                period.coverage_tests.iter().map(|test| {
                    serde_json::json!({
                        "date": period.payment_date.to_string(),
                        "test_id": test.test_id,
                        "ratio": test.ratio,
                        "trigger_level": test.trigger_level,
                        "cushion": test.cushion,
                        "passing": test.passing,
                    })
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(
            py,
            &rows,
            &[
                ("date", "str"),
                ("test_id", "str"),
                ("ratio", "float64"),
                ("trigger_level", "float64"),
                ("cushion", "float64"),
                ("passing", "bool"),
            ],
        )
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "SimulationDiagnostics(periods={}, draws_from_reserve={}, unfunded_draws={})",
            self.inner.periods.len(),
            self.inner.draws_from_reserve.amount(),
            self.inner.unfunded_draws.amount()
        )
    }
}

crate::bindings::macros::py_getters!(PySimulationDiagnostics |s| {
    /// Collateral draws funded from the reserve account over the simulation.
    draws_from_reserve -> PyMoney = money_to_py(s.inner.draws_from_reserve);

    /// Collateral draws funded from principal collections over the simulation.
    draws_from_principal -> PyMoney = money_to_py(s.inner.draws_from_principal);

    /// Collateral draws that could not be funded over the simulation.
    unfunded_draws -> PyMoney = money_to_py(s.inner.unfunded_draws);

    /// Revolver repayments diverted to replenish the reserve over the simulation.
    reserve_replenished -> PyMoney = money_to_py(s.inner.reserve_replenished);
});
