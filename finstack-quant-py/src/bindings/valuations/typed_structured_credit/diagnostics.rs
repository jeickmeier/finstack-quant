//! Deal-level accounting of a deterministic structured-credit simulation.

use pyo3::prelude::*;

use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::date_to_py;
use crate::bindings::pandas_utils::{serde_rows_to_dataframe_with_schema, serde_to_py};
use crate::bindings::valuations::convert::money_to_py;
use crate::errors::{display_to_py, serde_json_to_py};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::SimulationDiagnostics;

/// Deal-level accounting of one deterministic simulation
/// (``StructuredCredit.run_simulation_with_diagnostics``'s return value):
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
/// >>> diagnostics = deal.run_simulation_with_diagnostics(market, as_of)
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

    /// Collateral draws funded from the reserve account over the simulation.
    #[getter]
    fn draws_from_reserve(&self) -> PyMoney {
        money_to_py(self.inner.draws_from_reserve)
    }

    /// Collateral draws funded from principal collections over the simulation.
    #[getter]
    fn draws_from_principal(&self) -> PyMoney {
        money_to_py(self.inner.draws_from_principal)
    }

    /// Collateral draws that could not be funded over the simulation.
    #[getter]
    fn unfunded_draws(&self) -> PyMoney {
        money_to_py(self.inner.unfunded_draws)
    }

    /// Revolver repayments diverted to replenish the reserve over the simulation.
    #[getter]
    fn reserve_replenished(&self) -> PyMoney {
        money_to_py(self.inner.reserve_replenished)
    }

    /// Per-period deal record as ``PeriodDiagnostics`` serde dicts:
    /// ``payment_date``, ``pool_balance``, ``pool_factor``,
    /// ``weighted_avg_coupon``, ``weighted_avg_spread_bp``, ``warf``, the
    /// period's collections, defaults, recoveries, reinvested par, fees paid,
    /// the cash-account balances, ``delinquent_balance``, ``excess_spread``
    /// and the ``coverage_tests`` the executor evaluated.
    #[getter]
    fn periods<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.periods)
    }

    /// One row per simulated period as a pandas ``DataFrame``.
    ///
    /// Columns: ``date`` (ISO 8601 string), ``pool_balance``,
    /// ``pool_factor``, ``weighted_avg_coupon`` (decimal),
    /// ``weighted_avg_spread_bp``, ``warf``, ``interest_collections``,
    /// ``principal_collections``, ``defaults``, ``recoveries``,
    /// ``reinvested_par``, ``fees_paid``, ``reserve_balance``,
    /// ``reserve_interest``, ``spread_account``, ``funding_account``,
    /// ``delinquent_balance`` and ``excess_spread`` (annualized decimal);
    /// amounts in currency units.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///     The period record.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the rows cannot be serialized.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .periods
            .iter()
            .map(|period| {
                let reserve_interest = self
                    .inner
                    .reserve_interest_paid
                    .iter()
                    .find(|(paid, _)| *paid == period.payment_date)
                    .map_or(0.0, |(_, amount)| amount.amount());
                serde_json::json!({
                    "date": period.payment_date.to_string(),
                    "pool_balance": period.pool_balance.amount(),
                    "pool_factor": period.pool_factor,
                    "weighted_avg_coupon": period.weighted_avg_coupon,
                    "weighted_avg_spread_bp": period.weighted_avg_spread_bp,
                    "warf": period.warf,
                    "interest_collections": period.interest_collections.amount(),
                    "principal_collections": period.principal_collections.amount(),
                    "defaults": period.defaults.amount(),
                    "recoveries": period.recoveries.amount(),
                    "reinvested_par": period.reinvested_par.amount(),
                    "fees_paid": period.fees_paid.amount(),
                    "reserve_balance": period.reserve_balance.amount(),
                    "reserve_interest": reserve_interest,
                    "spread_account": period.spread_account.amount(),
                    "funding_account": period.funding_account.amount(),
                    "delinquent_balance": period.delinquent_balance.amount(),
                    "excess_spread": period.excess_spread,
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(
            py,
            &rows,
            &[
                ("date", "str"),
                ("pool_balance", "float64"),
                ("pool_factor", "float64"),
                ("weighted_avg_coupon", "float64"),
                ("weighted_avg_spread_bp", "float64"),
                ("warf", "float64"),
                ("interest_collections", "float64"),
                ("principal_collections", "float64"),
                ("defaults", "float64"),
                ("recoveries", "float64"),
                ("reinvested_par", "float64"),
                ("fees_paid", "float64"),
                ("reserve_balance", "float64"),
                ("reserve_interest", "float64"),
                ("spread_account", "float64"),
                ("funding_account", "float64"),
                ("delinquent_balance", "float64"),
                ("excess_spread", "float64"),
            ],
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
