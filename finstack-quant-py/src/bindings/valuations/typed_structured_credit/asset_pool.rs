use pyo3::prelude::*;

use crate::bindings::core::money::PyMoney;
use crate::bindings::valuations::convert::{
    currency_from_py, enum_to_py_string, money_from_py, money_to_py,
};
use crate::bindings::valuations::instruments::{PyBond, PyTermLoan};
use crate::bindings::valuations::typed_revolving_credit::PyRevolvingCredit;
use crate::errors::serde_json_to_py;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, CallExercisePolicy, DealType, InstrumentCollateral, InstrumentExerciseOverride,
    PutExercisePolicy, ReinvestmentPeriod, ReserveInterestDestination,
};

use super::super::instruments::enum_from_str;
use super::pool_asset::pool_assets_from_py;
use super::{PyPoolAsset, PyRepLine};

/// Parse an internally tagged policy/destination enum from a bare variant
/// name (``"first_call"``), a ``dict`` in the serde shape, or a JSON ``str``.
fn tagged_enum_from_py<T: serde::de::DeserializeOwned + Send>(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
    tag: &str,
    label: &str,
) -> PyResult<T> {
    if let Ok(text) = value.extract::<String>() {
        let trimmed = text.trim();
        let json = if trimmed.starts_with('{') {
            trimmed.to_string()
        } else {
            serde_json::json!({ tag: trimmed }).to_string()
        };
        return serde_json::from_str(&json)
            .map_err(|err| serde_json_to_py(err, &format!("invalid {label}")));
    }
    crate::bindings::module_utils::py_to_serde(py, value, label)
}

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
    /// Parameters
    /// ----------
    /// value : list[PoolAsset | dict] | str
    ///     Typed :class:`PoolAsset` rows, their serde dicts, or a JSON
    ///     array string (mixing typed rows and dicts is allowed).
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
    fn with_assets(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Self> {
        let assets = pool_assets_from_py(py, value)?;
        let mut inner = self.inner.clone();
        inner.assets = assets;
        Ok(Self { inner })
    }

    /// Attach real instruments as the collateral, returning a new pool.
    ///
    /// The pool then holds ``Bond``, ``TermLoan`` and ``RevolvingCredit``
    /// instruments instead of asset rows or representative lines; each
    /// instrument's own cashflow schedule drives the deal, defaults come from
    /// the instrument's credit curve when present, and collateral draws are
    /// funded from the reserve account (see :meth:`with_reserve`).
    ///
    /// Parameters
    /// ----------
    /// bonds : list[Bond], optional
    ///     Bonds in any form (fixed, floating, step-up, amortizing, callable).
    /// term_loans : list[TermLoan], optional
    ///     Term loans, including delayed-draw facilities.
    /// revolvers : list[RevolvingCredit], optional
    ///     Revolving facilities; stochastic ones simulate draws on the paths.
    /// call_exercise : str | dict, optional
    ///     Default issuer-call policy: ``"contractual"`` (never, the
    ///     default), ``"first_call"``, ``"worst"`` (yield-to-worst, needs a
    ///     quoted clean price) or ``{"policy": "refinancing_incentive",
    ///     "threshold_bp": 50.0}``.
    /// put_exercise : str, optional
    ///     Default holder-put policy: ``"never"`` (default), ``"first_put"``, or the
    ///     dict ``{"policy": "reinvestment_incentive", "threshold_bp": 50.0}`` (put
    ///     when the reinvestment rate exceeds the coupon by more than the threshold).
    /// overrides : list[dict], optional
    ///     Per-instrument overrides ``{"id": ..., "call": {...}, "put": {...}}``.
    ///
    /// Returns
    /// -------
    /// AssetPool
    ///     A new pool with ``instruments`` set (the original is unchanged).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a policy or override does not match its serde shape.
    /// TypeError
    ///     If a list element is not the expected typed instrument.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.core.currency import Currency
    /// >>> from finstack_quant.valuations.instruments import AssetPool, Bond, RevolvingCredit
    /// >>> pool = AssetPool("POOL-1", "clo", Currency("USD")).with_instruments(
    /// ...     bonds=[Bond.example()], revolvers=[RevolvingCredit.example()],
    /// ...     call_exercise="first_call",
    /// ... )
    /// >>> sorted(pool.instruments)
    /// ['bonds', 'call_exercise', 'overrides', 'put_exercise', 'revolvers', 'term_loans']
    #[pyo3(signature = (bonds=None, term_loans=None, revolvers=None, call_exercise=None, put_exercise=None, overrides=None))]
    #[pyo3(
        text_signature = "($self, bonds=None, term_loans=None, revolvers=None, call_exercise=None, put_exercise=None, overrides=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn with_instruments(
        &self,
        py: Python<'_>,
        bonds: Option<Vec<PyRef<'_, PyBond>>>,
        term_loans: Option<Vec<PyRef<'_, PyTermLoan>>>,
        revolvers: Option<Vec<PyRef<'_, PyRevolvingCredit>>>,
        call_exercise: Option<&Bound<'_, PyAny>>,
        put_exercise: Option<&Bound<'_, PyAny>>,
        overrides: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let call_exercise: CallExercisePolicy = match call_exercise {
            Some(value) => tagged_enum_from_py(py, value, "policy", "call_exercise")?,
            None => CallExercisePolicy::default(),
        };
        let put_exercise: PutExercisePolicy = match put_exercise {
            Some(value) => tagged_enum_from_py(py, value, "policy", "put_exercise")?,
            None => PutExercisePolicy::default(),
        };
        let overrides: Vec<InstrumentExerciseOverride> = match overrides {
            Some(value) => crate::bindings::module_utils::py_to_serde(py, value, "overrides")?,
            None => Vec::new(),
        };
        let collateral = InstrumentCollateral {
            bonds: bonds
                .unwrap_or_default()
                .iter()
                .map(|bond| bond.inner.clone())
                .collect(),
            term_loans: term_loans
                .unwrap_or_default()
                .iter()
                .map(|loan| loan.inner.clone())
                .collect(),
            revolvers: revolvers
                .unwrap_or_default()
                .iter()
                .map(|facility| facility.inner.clone())
                .collect(),
            call_exercise,
            put_exercise,
            overrides,
        };
        let mut inner = self.inner.clone();
        inner.instruments = Some(collateral);
        Ok(Self { inner })
    }

    /// Configure the reserve account, returning a new pool.
    ///
    /// The reserve funds collateral draws (revolver utilization increases,
    /// delayed draws and loan-equivalent draws at default), is replenished by
    /// revolver repayments up to ``reserve_target``, and earns
    /// ``reserve_account_rate`` routed per ``reserve_interest_destination``.
    ///
    /// Parameters
    /// ----------
    /// reserve_account : Money | float
    ///     Opening reserve balance; a bare number needs ``currency``.
    /// reserve_account_rate : float, default 0.0
    ///     Annual interest rate earned by the reserve, as a decimal
    ///     (simple ACT/360 on the opening balance each period).
    /// reserve_target : Money | float, optional
    ///     Balance revolver repayments replenish toward; ``None`` disables
    ///     replenishment.
    /// reserve_interest_destination : str | dict, optional
    ///     ``"waterfall"`` (default, interest proceeds), ``"retain"``
    ///     (capitalized into the reserve) or ``{"kind": "tranche",
    ///     "tranche_id": "EQ"}`` (paid directly to that tranche).
    /// currency : str, optional
    ///     ISO-4217 code applied when a bare number is passed.
    ///
    /// Returns
    /// -------
    /// AssetPool
    ///     A new pool with the reserve configured (the original is unchanged).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an amount is not finite, a bare number has no currency, or the
    ///     destination does not match its serde shape.
    /// TypeError
    ///     If an amount is neither ``Money`` nor a number.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.core.currency import Currency
    /// >>> from finstack_quant.core.money import Money
    /// >>> from finstack_quant.valuations.instruments import AssetPool
    /// >>> pool = AssetPool("POOL-1", "clo", Currency("USD")).with_reserve(
    /// ...     Money(5_000_000.0, Currency("USD")), reserve_account_rate=0.03,
    /// ...     reserve_interest_destination={"kind": "tranche", "tranche_id": "EQ"},
    /// ... )
    /// >>> (pool.reserve_account_rate, pool.reserve_interest_destination["kind"])
    /// (0.03, 'tranche')
    #[pyo3(signature = (reserve_account, reserve_account_rate=0.0, reserve_target=None, reserve_interest_destination=None, currency=None))]
    #[pyo3(
        text_signature = "($self, reserve_account, reserve_account_rate=0.0, reserve_target=None, reserve_interest_destination=None, currency=None)"
    )]
    fn with_reserve(
        &self,
        py: Python<'_>,
        reserve_account: &Bound<'_, PyAny>,
        reserve_account_rate: f64,
        reserve_target: Option<&Bound<'_, PyAny>>,
        reserve_interest_destination: Option<&Bound<'_, PyAny>>,
        currency: Option<&str>,
    ) -> PyResult<Self> {
        let mut inner = self.inner.clone();
        inner.reserve_account = money_from_py(reserve_account, currency, "reserve_account")?;
        inner.reserve_account_rate = reserve_account_rate;
        inner.reserve_target = reserve_target
            .map(|value| money_from_py(value, currency, "reserve_target"))
            .transpose()?;
        inner.reserve_interest_destination = match reserve_interest_destination {
            Some(value) => tagged_enum_from_py(py, value, "kind", "reserve_interest_destination")?,
            None => ReserveInterestDestination::default(),
        };
        Ok(Self { inner })
    }

    /// Configure the deal-level reinvestment period, returning a new pool.
    ///
    /// Principal proceeds collected while the period is active are recycled
    /// into collateral instead of repaying the notes; every note is held flat
    /// except those listed in ``amortizing_tranches``, which are paid down
    /// first. Instrument-collateral pools cannot reinvest.
    ///
    /// Parameters
    /// ----------
    /// value : dict[str, Any] | str
    ///     ``ReinvestmentPeriod`` in its serde shape, or that JSON as a
    ///     string: ISO ``end_date`` (inclusive), ``is_active``, ``criteria``
    ///     (``max_price`` percent of par, ``min_yield`` annual decimal current
    ///     yield, ``maintain_credit_quality``, ``maintain_wal``), optional
    ///     ``amortizing_tranches`` (note ids paid down inside the window) and
    ///     optional ``assumptions`` (``spread_bp``, ``price_pct``,
    ///     ``maturity_months``, ``index_id``, ``coupon_floor``) describing the
    ///     replacement collateral; omitted assumptions clone the surviving
    ///     pool pro rata.
    ///
    /// Returns
    /// -------
    /// AssetPool
    ///     A new pool with the reinvestment period set (the original is
    ///     unchanged).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the ``ReinvestmentPeriod`` serde shape.
    ///     Tranche ids, dates and assumption ranges are validated when the
    ///     deal is built or priced.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.core.currency import Currency
    /// >>> from finstack_quant.valuations.instruments import AssetPool
    /// >>> pool = AssetPool("POOL-1", "clo", Currency("USD")).with_reinvestment_period({
    /// ...     "end_date": "2028-01-01", "is_active": True,
    /// ...     "criteria": {"max_price": 100.0, "min_yield": 0.0,
    /// ...                  "maintain_credit_quality": True, "maintain_wal": True},
    /// ...     "amortizing_tranches": ["A"],
    /// ... })
    /// >>> pool.reinvestment_period["amortizing_tranches"]
    /// ['A']
    #[pyo3(text_signature = "($self, value)")]
    fn with_reinvestment_period(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Self> {
        let period: ReinvestmentPeriod = if let Ok(text) = value.extract::<String>() {
            serde_json::from_str(&text)
                .map_err(|err| serde_json_to_py(err, "invalid reinvestment_period"))?
        } else {
            crate::bindings::module_utils::py_to_serde(py, value, "reinvestment_period")?
        };
        let mut inner = self.inner.clone();
        inner.reinvestment_period = Some(period);
        Ok(Self { inner })
    }

    /// Set the pool's historical tallies and cash accounts, returning a new
    /// pool (seasoned-deal inputs).
    ///
    /// Parameters
    /// ----------
    /// cumulative_defaults : Money, optional
    ///     Defaulted par to date; unchanged when omitted.
    /// cumulative_recoveries : Money, optional
    ///     Recoveries received to date; unchanged when omitted.
    /// cumulative_prepayments : Money, optional
    ///     Prepayments received to date; unchanged when omitted.
    /// cumulative_scheduled_amortization : Money, optional
    ///     Scheduled principal received to date; unchanged when omitted.
    /// collection_account : Money, optional
    ///     Undistributed collections held at closing; unchanged when omitted.
    /// excess_spread_account : Money, optional
    ///     Trapped excess spread held at closing; unchanged when omitted.
    ///
    /// Returns
    /// -------
    /// AssetPool
    ///     A new pool with the supplied balances (the original is unchanged).
    #[pyo3(signature = (*, cumulative_defaults=None, cumulative_recoveries=None, cumulative_prepayments=None, cumulative_scheduled_amortization=None, collection_account=None, excess_spread_account=None))]
    #[pyo3(
        text_signature = "($self, *, cumulative_defaults=None, cumulative_recoveries=None, cumulative_prepayments=None, cumulative_scheduled_amortization=None, collection_account=None, excess_spread_account=None)"
    )]
    // PyO3 binding: one keyword per pool account field.
    #[allow(clippy::too_many_arguments)]
    fn with_accounts(
        &self,
        cumulative_defaults: Option<PyRef<'_, PyMoney>>,
        cumulative_recoveries: Option<PyRef<'_, PyMoney>>,
        cumulative_prepayments: Option<PyRef<'_, PyMoney>>,
        cumulative_scheduled_amortization: Option<PyRef<'_, PyMoney>>,
        collection_account: Option<PyRef<'_, PyMoney>>,
        excess_spread_account: Option<PyRef<'_, PyMoney>>,
    ) -> Self {
        let mut inner = self.inner.clone();
        if let Some(value) = cumulative_defaults {
            inner.cumulative_defaults = value.inner;
        }
        if let Some(value) = cumulative_recoveries {
            inner.cumulative_recoveries = value.inner;
        }
        if let Some(value) = cumulative_prepayments {
            inner.cumulative_prepayments = value.inner;
        }
        if let Some(value) = cumulative_scheduled_amortization {
            inner.cumulative_scheduled_amortization = value.inner;
        }
        if let Some(value) = collection_account {
            inner.collection_account = value.inner;
        }
        if let Some(value) = excess_spread_account {
            inner.excess_spread_account = value.inner;
        }
        Self { inner }
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

    /// Loan-level assets as typed :class:`PoolAsset` rows (empty when rep
    /// lines or instruments are used).
    #[getter]
    fn assets(&self) -> Vec<PyPoolAsset> {
        self.inner
            .assets
            .iter()
            .map(|asset| PyPoolAsset {
                inner: asset.clone(),
            })
            .collect()
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

    /// Annual interest rate earned by the reserve account, as a decimal.
    #[getter]
    fn reserve_account_rate(&self) -> f64 {
        self.inner.reserve_account_rate
    }

    /// Reserve balance revolver repayments replenish toward, or ``None``.
    #[getter]
    fn reserve_target(&self) -> Option<PyMoney> {
        self.inner.reserve_target.map(money_to_py)
    }

    /// Destination of the reserve interest as its serde ``dict``
    /// (``{"kind": "waterfall"}``, ``{"kind": "retain"}`` or
    /// ``{"kind": "tranche", "tranche_id": ...}``).
    #[getter]
    fn reserve_interest_destination<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.reserve_interest_destination)
    }

    /// Instrument collateral as its serde ``dict`` (``bonds``, ``term_loans``,
    /// ``revolvers``, ``call_exercise``, ``put_exercise``, ``overrides``), or
    /// ``None`` when the pool is modelled with asset rows or representative
    /// lines.
    #[getter]
    fn instruments<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .instruments
            .as_ref()
            .map(|collateral| crate::bindings::pandas_utils::serde_to_py(py, collateral))
            .transpose()
    }

    /// Reinvestment period as its serde ``dict``, or ``None``.
    #[getter]
    fn reinvestment_period<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .reinvestment_period
            .as_ref()
            .map(|period| crate::bindings::pandas_utils::serde_to_py(py, period))
            .transpose()
    }

    /// Excess-spread account balance.
    #[getter]
    fn excess_spread_account(&self) -> PyMoney {
        money_to_py(self.inner.excess_spread_account)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "AssetPool(id='{}', deal_type='{}', base_currency='{}', assets={}, rep_lines={}, instruments={})",
            self.inner.id.as_str(),
            enum_to_py_string(&self.inner.deal_type).unwrap_or_default(),
            self.inner.base_currency,
            self.inner.assets.len(),
            self.inner
                .rep_lines
                .as_ref()
                .map(|lines| lines.len().to_string())
                .unwrap_or_else(|| "None".to_string()),
            self.inner
                .instruments
                .as_ref()
                .map(|collateral| collateral.len().to_string())
                .unwrap_or_else(|| "None".to_string()),
        )
    }
}
