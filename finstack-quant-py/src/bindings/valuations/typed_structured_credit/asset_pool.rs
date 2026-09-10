use pyo3::prelude::*;

use crate::bindings::core::money::PyMoney;
use crate::bindings::valuations::convert::{currency_from_py, enum_to_py_string, money_to_py};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, DealType, PoolAsset,
};

use super::super::instruments::enum_from_str;
use super::PyRepLine;

/// Typed wrapper for the Rust `AssetPool` (structured-credit collateral pool).
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "AssetPool",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyAssetPool {
    /// Inner canonical Rust asset pool.
    pub(crate) inner: AssetPool,
}

#[pymethods]
impl PyAssetPool {
    /// Structured-credit collateral pool.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Pool identifier.
    /// deal_type : {"clo", "cbo", "abs", "rmbs", "cmbs", "auto", "card"}
    ///     Deal classification for pool-level assumptions.
    /// base_currency : Currency | str
    ///     Base currency for every asset and pool-level account.
    ///
    /// Returns
    /// -------
    /// AssetPool
    ///     A new, empty asset pool. Use :meth:`with_rep_lines` and/or
    ///     :meth:`assets` to attach collateral.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``deal_type`` is not a recognized deal type.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.core.currency import Currency
    /// >>> from finstack_quant.valuations.instruments import AssetPool
    /// >>> pool = AssetPool("POOL-1", "abs", Currency("USD"))
    /// >>> "POOL-1" in repr(pool)
    /// True
    #[new]
    #[pyo3(text_signature = "(id, deal_type, base_currency)")]
    fn new(id: &str, deal_type: &str, base_currency: &Bound<'_, PyAny>) -> PyResult<Self> {
        let deal_type: DealType = enum_from_str(deal_type, "deal_type")?;
        let base_currency = currency_from_py(base_currency, "base_currency")?;
        let inner = AssetPool::new(id, deal_type, base_currency);
        Ok(Self { inner })
    }

    /// Attach representative pool lines, returning a new pool.
    ///
    /// Parameters
    /// ----------
    /// rep_lines : list[RepLine]
    ///     Aggregated representative lines the pricing engine will use
    ///     instead of individual assets.
    ///
    /// Returns
    /// -------
    /// AssetPool
    ///     A new pool with ``rep_lines`` set (the original is unchanged).
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If an element of ``rep_lines`` is not a ``RepLine``.
    ///
    /// Examples
    /// --------
    /// >>> import datetime
    /// >>> from finstack_quant.core.currency import Currency
    /// >>> from finstack_quant.core.dates import DayCount
    /// >>> from finstack_quant.core.money import Money
    /// >>> from finstack_quant.valuations.instruments import AssetPool, RepLine
    /// >>> pool = AssetPool("POOL-1", "abs", Currency("USD")).with_rep_lines([
    /// ...     RepLine(
    /// ...         "LINE-1", Money(80_000_000.0, Currency("USD")), 0.07,
    /// ...         datetime.date(2031, 1, 15), 12, DayCount.ACT_360, asset_type={"type": "first_lien_loan", "industry": None},
    /// ...     )
    /// ... ])
    /// >>> "POOL-1" in repr(pool)
    /// True
    #[pyo3(text_signature = "($self, rep_lines)")]
    fn with_rep_lines(&self, rep_lines: Vec<PyRef<'_, PyRepLine>>) -> Self {
        let mut inner = self.inner.clone();
        inner.rep_lines = Some(rep_lines.iter().map(|line| line.inner.clone()).collect());
        Self { inner }
    }

    /// Attach loan-level assets, returning a new pool.
    ///
    /// Loan-level ``PoolAsset`` records carry ~30 fields and stay in their
    /// serde dict shape; use :meth:`with_rep_lines` for the typed,
    /// aggregated path.
    ///
    /// Parameters
    /// ----------
    /// value : list[dict] | str
    ///     ``PoolAsset`` objects as a list of dicts or a JSON array string.
    ///
    /// Returns
    /// -------
    /// AssetPool
    ///     A new pool with ``assets`` set (the original is unchanged).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the ``PoolAsset`` list shape.
    #[pyo3(text_signature = "($self, value)")]
    fn assets(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Self> {
        let assets: Vec<PoolAsset> =
            crate::bindings::module_utils::py_to_serde(py, value, "assets")?;
        let mut inner = self.inner.clone();
        inner.assets = assets;
        Ok(Self { inner })
    }

    /// Deserialize from the JSON produced by ``to_json``.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     Strict JSON object with exactly the fields ``to_json`` writes.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed or has the wrong shape.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|err| crate::errors::serde_json_to_py(err, "invalid AssetPool JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize to the canonical JSON wire form.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(crate::errors::display_to_py)
    }

    /// Return every field as a plain ``dict`` (canonical serde shape).
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
    }

    /// Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Pool identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.to_string()
    }

    /// Deal classification (serde name).
    #[getter]
    fn deal_type(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.deal_type)
    }

    /// Base ISO-4217 currency code.
    #[getter]
    fn base_currency(&self) -> String {
        self.inner.base_currency.to_string()
    }

    /// Loan-level assets as a list of dicts (``PoolAsset`` serde shape).
    #[getter]
    fn asset_records<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.assets)
    }

    /// Representative lines, or ``None`` when the pool is modelled loan-level.
    #[getter]
    fn rep_lines(&self) -> Option<Vec<PyRepLine>> {
        self.inner.rep_lines.as_ref().map(|lines| {
            lines
                .iter()
                .map(|line| PyRepLine {
                    inner: line.clone(),
                })
                .collect()
        })
    }

    /// Cumulative defaults to date.
    #[getter]
    fn cumulative_defaults(&self) -> PyMoney {
        money_to_py(self.inner.cumulative_defaults)
    }

    /// Cumulative recoveries to date.
    #[getter]
    fn cumulative_recoveries(&self) -> PyMoney {
        money_to_py(self.inner.cumulative_recoveries)
    }

    /// Cumulative prepayments to date.
    #[getter]
    fn cumulative_prepayments(&self) -> PyMoney {
        money_to_py(self.inner.cumulative_prepayments)
    }

    /// Cumulative scheduled amortization to date.
    #[getter]
    fn cumulative_scheduled_amortization(&self) -> PyMoney {
        money_to_py(self.inner.cumulative_scheduled_amortization)
    }

    /// Collection account balance.
    #[getter]
    fn collection_account(&self) -> PyMoney {
        money_to_py(self.inner.collection_account)
    }

    /// Reserve account balance.
    #[getter]
    fn reserve_account(&self) -> PyMoney {
        money_to_py(self.inner.reserve_account)
    }

    /// Excess-spread account balance.
    #[getter]
    fn excess_spread_account(&self) -> PyMoney {
        money_to_py(self.inner.excess_spread_account)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "AssetPool(id='{}', deal_type='{}', base_currency='{}', assets={}, rep_lines={})",
            self.inner.id.as_str(),
            enum_to_py_string(&self.inner.deal_type).unwrap_or_default(),
            self.inner.base_currency,
            self.inner.assets.len(),
            self.inner
                .rep_lines
                .as_ref()
                .map(|lines| lines.len().to_string())
                .unwrap_or_else(|| "None".to_string()),
        )
    }
}
