//! SA-CCR counterparty credit risk EAD Python bindings.

use super::super::frame::{opt_date, opt_str, records, req_bool, req_date, req_f64, req_str};
use super::super::types::{PyNettingSetId, extract_netting_set_id};
use super::frtb::{py_bool, serde_label};
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::parse_label;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::{py_getters, tabular_methods, wire_methods};
use crate::bindings::table_builders::{
    serde_object_to_single_row_arrow_table, serde_rows_to_arrow_table,
};
use crate::errors::core_to_py;
use finstack_quant_margin::regulatory::sa_ccr::{
    EadResult, SaCcrAssetClass, SaCcrEngine, SaCcrNettingSetConfig, SaCcrOptionType,
    SaCcrSupervisoryCategory, SaCcrTrade, saccr_ead as saccr_ead_rs,
};
use pyo3::prelude::*;
use std::collections::BTreeMap;

/// A single derivative trade for SA-CCR EAD computation (BCBS 279).
///
/// Build with keyword arguments or ``from_json``; both validate the
/// direction / supervisory-delta / option-type coherence up front.
/// ``notional`` and ``mtm`` are in the netting set's reporting currency;
/// ``direction`` is ``+1.0`` (long) or ``-1.0`` (short);
/// ``supervisory_delta`` is in ``[-1, 1]`` (exactly ``±1`` for linear trades,
/// the signed option delta otherwise).
#[pyclass(
    name = "SaCcrTrade",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PySaCcrTrade {
    pub(crate) inner: SaCcrTrade,
}

wire_methods!(PySaCcrTrade, SaCcrTrade, "SaCcrTrade", validate);
py_getters!(PySaCcrTrade |s| {
    /// SA-CCR asset class label (``interest_rate``, ``foreign_exchange``,
    /// ``credit``, ``equity``, or ``commodity``).
    asset_class -> String = serde_label(s.inner.asset_class);
    /// Supervisory category wire label, or None for IR and FX.
    supervisory_category -> Option<String> = s.inner.supervisory_category.map(serde_label);
    /// Trade notional, in the netting set's reporting currency.
    notional -> f64 = s.inner.notional;
    /// ``+1.0`` for long, ``-1.0`` for short.
    direction -> f64 = s.inner.direction;
    /// Supervisory delta in ``[-1, 1]``.
    supervisory_delta -> f64 = s.inner.supervisory_delta;
    /// Current mark-to-market value, in the netting set's reporting currency.
    mtm -> f64 = s.inner.mtm;
    /// Whether the trade is an option.
    is_option -> bool = s.inner.is_option;
    /// Option type label (``call_long``, ``call_short``, ``put_long``,
    /// ``put_short``) or ``None`` for a linear trade.
    option_type -> Option<String> = s.inner.option_type.map(serde_label);
});

const TRADE_COLUMNS: &[(&str, &str)] = &[
    ("trade_id", "str"),
    ("asset_class", "str"),
    ("notional", "float64"),
    ("start_date", "str"),
    ("end_date", "str"),
    ("underlier", "str"),
    ("hedging_set", "str"),
    ("direction", "float64"),
    ("supervisory_delta", "float64"),
    ("mtm", "float64"),
    ("is_option", "bool"),
    ("option_type", "str"),
    ("supervisory_category", "str"),
    ("option_maturity_date", "str"),
];

impl PySaCcrTrade {
    fn row(&self) -> serde_json::Value {
        let t = &self.inner;
        serde_json::json!({
            "trade_id": t.trade_id,
            "asset_class": serde_label(t.asset_class),
            "notional": t.notional,
            "start_date": t.start_date.to_string(),
            "end_date": t.end_date.to_string(),
            "underlier": t.underlier,
            "hedging_set": t.hedging_set,
            "direction": t.direction,
            "supervisory_delta": t.supervisory_delta,
            "mtm": t.mtm,
            "is_option": t.is_option,
            "option_type": t.option_type.map(serde_label),
            "supervisory_category": t.supervisory_category.map(serde_label),
            "option_maturity_date": t.option_maturity_date.map(|d| d.to_string()),
        })
    }
}

#[pymethods]
impl PySaCcrTrade {
    /// Create and validate a trade; every field is keyword-only.
    ///
    /// Parameters
    /// ----------
    /// trade_id : str
    ///     Unique trade identifier.
    /// asset_class : str
    ///     ``"interest_rate"``, ``"foreign_exchange"``, ``"credit"``,
    ///     ``"equity"`` or ``"commodity"``.
    /// notional : float
    ///     Adjusted notional in the reporting currency.
    /// start_date : datetime.date | str
    ///     Trade start date (forward-start trades start after ``as_of``).
    /// end_date : datetime.date | str
    ///     Trade end date / maturity.
    /// underlier : str
    ///     Underlier reference (currency pair, issuer, equity, commodity).
    /// hedging_set : str
    ///     Hedging-set identifier within the asset class.
    /// direction : float
    ///     ``+1.0`` long or ``-1.0`` short.
    /// supervisory_delta : float
    ///     ``±1`` for linear trades; the signed option delta in ``[-1, 1]``
    ///     otherwise, sign-consistent with ``option_type`` (BCBS 279 ¶112).
    /// mtm : float
    ///     Current mark-to-market in the reporting currency.
    /// is_option : bool
    ///     Whether the trade is an option (required, as in the wire form).
    /// option_type : str | None
    ///     ``"call_long"``, ``"call_short"``, ``"put_long"`` or
    ///     ``"put_short"``; required when ``is_option`` is ``True``.
    ///
    /// Raises ``ValueError`` when a field is missing or unknown, a label or
    /// date string is malformed, or the trade fails the BCBS 279 coherence
    /// checks; ``TypeError`` when a value is not JSON-encodable.
    #[new]
    #[pyo3(
        signature = (**fields),
        text_signature = "(*, trade_id, asset_class, notional, start_date, end_date, underlier, hedging_set, direction, supervisory_delta, mtm, is_option, option_type=None, supervisory_category=None, option_maturity_date=None)"
    )]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, pyo3::types::PyDict>>) -> PyResult<Self> {
        let inner: SaCcrTrade =
            crate::bindings::module_utils::fields_from_py(py, fields, "SaCcrTrade")?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Build one validated trade per row of a trade tape.
    ///
    /// ``frame`` must carry the columns ``to_pandas`` / ``to_arrow`` emits (``trade_id``,
    /// ``asset_class``, ``notional``, ``start_date``, ``end_date``,
    /// ``underlier``, ``hedging_set``, ``direction``, ``supervisory_delta``,
    /// ``mtm``, ``is_option``; ``option_type``, ``supervisory_category`` and
    /// ``option_maturity_date`` may be null). Dates may be ISO strings or
    /// date-like values.
    /// Returns a list of ``SaCcrTrade``; raises ``ValueError`` naming the
    /// first invalid row.
    #[staticmethod]
    fn from_dataframe(frame: &Bound<'_, PyAny>) -> PyResult<Vec<Self>> {
        records(frame)?
            .iter()
            .map(|row| {
                let inner = SaCcrTrade {
                    supervisory_category: opt_str(row, "supervisory_category")?
                        .map(|s| parse_label::<SaCcrSupervisoryCategory>(&s))
                        .transpose()?,
                    option_maturity_date: opt_date(row, "option_maturity_date")?,
                    trade_id: req_str(row, "trade_id")?,
                    asset_class: parse_label::<SaCcrAssetClass>(&req_str(row, "asset_class")?)?,
                    notional: req_f64(row, "notional")?,
                    start_date: req_date(row, "start_date")?,
                    end_date: req_date(row, "end_date")?,
                    underlier: req_str(row, "underlier")?,
                    hedging_set: req_str(row, "hedging_set")?,
                    direction: req_f64(row, "direction")?,
                    supervisory_delta: req_f64(row, "supervisory_delta")?,
                    mtm: req_f64(row, "mtm")?,
                    is_option: req_bool(row, "is_option")?,
                    option_type: opt_str(row, "option_type")?
                        .map(|label| parse_label::<SaCcrOptionType>(&label))
                        .transpose()?,
                };
                inner.validate().map_err(core_to_py)?;
                Ok(Self { inner })
            })
            .collect()
    }

    /// Trade identifier.
    #[getter]
    fn trade_id(&self) -> &str {
        &self.inner.trade_id
    }

    /// Option expiry as a date, distinct from the underlying end date.
    #[getter]
    fn option_maturity_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .option_maturity_date
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    /// Trade start date as ``datetime.date``.
    #[getter]
    fn start_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.start_date)
    }

    /// Trade end date / maturity as ``datetime.date``.
    #[getter]
    fn end_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.end_date)
    }

    /// Underlier reference.
    #[getter]
    fn underlier(&self) -> &str {
        &self.inner.underlier
    }

    /// Hedging-set identifier within the asset class.
    #[getter]
    fn hedging_set(&self) -> &str {
        &self.inner.hedging_set
    }

    fn __repr__(&self) -> String {
        format!(
            "SaCcrTrade(trade_id={:?}, asset_class={:?}, notional={:.2}, start_date={}, end_date={}, direction={}, supervisory_delta={}, mtm={:.2}, is_option={})",
            self.inner.trade_id,
            serde_label(self.inner.asset_class),
            self.inner.notional,
            self.inner.start_date,
            self.inner.end_date,
            self.inner.direction,
            self.inner.supervisory_delta,
            self.inner.mtm,
            py_bool(self.inner.is_option),
        )
    }
}

impl PySaCcrTrade {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        serde_object_to_single_row_arrow_table(&self.row(), TRADE_COLUMNS)
    }
}

tabular_methods!(PySaCcrTrade);

/// SA-CCR netting-set configuration.
///
/// Collateral terms that select the margined or unmargined RC/PFE formulas.
/// All amounts are in the netting set's reporting currency; ``mpor_days`` is
/// the margin period of risk in business days.
#[pyclass(
    name = "SaCcrNettingSetConfig",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PySaCcrNettingSetConfig {
    pub(crate) inner: SaCcrNettingSetConfig,
}

wire_methods!(
    PySaCcrNettingSetConfig,
    SaCcrNettingSetConfig,
    "SaCcrNettingSetConfig"
);
py_getters!(PySaCcrNettingSetConfig |s| {
    /// Netting-set key.
    netting_set_id -> PyNettingSetId = PyNettingSetId {
        inner: s.inner.netting_set_id.clone(),
    };
    /// Whether the netting set is margined (a CSA with an MPoR applies).
    is_margined -> bool = s.inner.is_margined;
    /// Net variation margin held against the netting set, excluding NICA, in its reporting currency.
    collateral -> f64 = s.inner.collateral;
    /// CSA threshold (TH) in the reporting currency.
    threshold -> f64 = s.inner.threshold;
    /// Minimum transfer amount in the reporting currency.
    mta -> f64 = s.inner.mta;
    /// Net independent collateral amount in the reporting currency.
    nica -> f64 = s.inner.nica;
    /// Margin period of risk in business days.
    mpor_days -> u32 = s.inner.mpor_days;
});

#[pymethods]
impl PySaCcrNettingSetConfig {
    /// Create an unmargined netting set configuration.
    ///
    /// Parameters
    /// ----------
    /// netting_set_id : NettingSetId
    ///     Bilateral (``NettingSetId.bilateral``) or cleared
    ///     (``NettingSetId.cleared``) netting-set key.
    /// collateral : float
    ///     Net variation margin held, excluding NICA (positive = bank holds
    ///     collateral).
    /// as_of : datetime.date | str
    ///     Valuation date for forward-start and remaining-maturity
    ///     calculations.
    ///
    /// Raises ``ValueError`` for a non-finite amount or a non-ISO date
    /// string; ``TypeError`` for a non-date-like ``as_of``.
    #[staticmethod]
    #[pyo3(signature = (netting_set_id, collateral, as_of))]
    fn unmargined(
        netting_set_id: &Bound<'_, PyAny>,
        collateral: f64,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let inner = SaCcrNettingSetConfig::unmargined(
            extract_netting_set_id(netting_set_id)?,
            collateral,
            py_to_date(as_of)?,
        );
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Create a margined netting set configuration.
    ///
    /// Parameters
    /// ----------
    /// netting_set_id : NettingSetId
    ///     Bilateral or cleared netting-set key.
    /// collateral : float
    ///     Net variation margin held, excluding NICA (positive = bank holds
    ///     collateral).
    /// threshold : float
    ///     CSA threshold (TH), non-negative.
    /// mta : float
    ///     Minimum transfer amount, non-negative.
    /// nica : float
    ///     Net independent collateral amount, signed.
    /// mpor_days : int
    ///     Margin period of risk in business days; must be positive
    ///     (10 bilateral, 5 cleared under BCBS 279).
    /// as_of : datetime.date | str
    ///     Valuation date for forward-start and remaining-maturity
    ///     calculations.
    ///
    /// Raises ``ValueError`` if an amount is non-finite, threshold or MTA is
    /// negative, ``mpor_days`` is below ten bilateral or five cleared business days, or the date string is not ISO 8601.
    #[staticmethod]
    #[pyo3(signature = (netting_set_id, collateral, threshold, mta, nica, mpor_days, as_of))]
    fn margined(
        netting_set_id: &Bound<'_, PyAny>,
        collateral: f64,
        threshold: f64,
        mta: f64,
        nica: f64,
        mpor_days: u32,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let inner = SaCcrNettingSetConfig::margined(
            extract_netting_set_id(netting_set_id)?,
            collateral,
            threshold,
            mta,
            nica,
            mpor_days,
            py_to_date(as_of)?,
        );
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Validate the collateral and margin-agreement terms; raises
    /// ``ValueError`` on a non-finite amount, negative threshold/MTA, or a
    /// margined set with zero MPOR.
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Valuation date as ``datetime.date``.
    #[getter]
    fn as_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.as_of)
    }

    fn __repr__(&self) -> String {
        format!(
            "SaCcrNettingSetConfig(netting_set_id={}, as_of={}, is_margined={}, collateral={:.2}, threshold={:.2}, mta={:.2}, nica={:.2}, mpor_days={})",
            self.inner.netting_set_id,
            self.inner.as_of,
            py_bool(self.inner.is_margined),
            self.inner.collateral,
            self.inner.threshold,
            self.inner.mta,
            self.inner.nica,
            self.inner.mpor_days,
        )
    }
}

/// SA-CCR engine (BCBS 279): ``EAD = alpha * (RC + PFE)``.
///
/// All monetary inputs must share one currency; the engine performs no
/// conversion.
#[pyclass(
    name = "SaCcrEngine",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
pub struct PySaCcrEngine {
    inner: SaCcrEngine,
}

py_getters!(PySaCcrEngine |s| {
    /// Alpha multiplier applied to ``RC + PFE``.
    alpha -> f64 = s.inner.alpha();
});

#[pymethods]
impl PySaCcrEngine {
    /// Create an SA-CCR engine; ``alpha`` overrides the regulatory 1.4
    /// (must be finite and at least 1.0, else ``ValueError``).
    #[new]
    #[pyo3(signature = (alpha = None))]
    fn new(alpha: Option<f64>) -> PyResult<Self> {
        let inner = match alpha {
            Some(a) => SaCcrEngine::with_alpha(a).map_err(core_to_py)?,
            None => SaCcrEngine::default(),
        };
        Ok(Self { inner })
    }

    /// Calculate SA-CCR EAD for a netting set and trade list.
    ///
    /// Returns an ``EadResult`` carrying EAD, RC, PFE, the multiplier, the
    /// aggregate and per-asset-class add-ons, alpha, and the maturity factor.
    /// Raises ``ValueError`` if the configuration or a trade fails validation.
    fn calculate_ead(
        &self,
        py: Python<'_>,
        trades: Vec<PyRef<'_, PySaCcrTrade>>,
        config: &PySaCcrNettingSetConfig,
    ) -> PyResult<PyEadResult> {
        let trade_vec: Vec<SaCcrTrade> = trades.iter().map(|t| t.inner.clone()).collect();
        let result = py
            .detach(|| self.inner.calculate_ead(&trade_vec, &config.inner))
            .map_err(core_to_py)?;
        Ok(PyEadResult::from_inner(result))
    }

    fn __repr__(&self) -> String {
        format!("SaCcrEngine(alpha={})", self.inner.alpha())
    }
}

/// SA-CCR Exposure at Default result (BCBS 279).
///
/// Returned by ``saccr_ead`` and ``SaCcrEngine.calculate_ead``.
/// ``ead == alpha * (rc + pfe)``; ``pfe == multiplier * add_on_aggregate``.
#[pyclass(
    name = "EadResult",
    module = "finstack_quant.margin",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyEadResult {
    inner: EadResult,
}

impl PyEadResult {
    fn from_inner(inner: EadResult) -> Self {
        Self { inner }
    }
}

wire_methods!(PyEadResult, EadResult, "EadResult");
py_getters!(PyEadResult |s| {
    /// Exposure at Default: ``alpha * (rc + pfe)``, in the netting set's
    /// reporting currency.
    ead -> f64 = s.inner.ead;
    /// Replacement cost component.
    rc -> f64 = s.inner.rc;
    /// Potential future exposure: ``multiplier * add_on_aggregate``.
    pfe -> f64 = s.inner.pfe;
    /// PFE multiplier, which recognises over-collateralization and negative
    /// mark-to-market (1.0 when neither applies, floored at 0.05).
    multiplier -> f64 = s.inner.multiplier;
    /// Aggregate add-on across asset classes, before the multiplier.
    add_on_aggregate -> f64 = s.inner.add_on_aggregate;
    /// Alpha multiplier (1.4 per BCBS 279 unless overridden on the engine).
    alpha -> f64 = s.inner.alpha;
    /// Maturity factor applied to the netting set.
    maturity_factor -> f64 = s.inner.maturity_factor;
    /// Add-on keyed by asset-class wire label (e.g. ``"interest_rate"``).
    add_on_by_asset_class -> BTreeMap<String, f64> = s
        .inner
        .add_on_by_asset_class
        .iter()
        .map(|(ac, v)| (serde_label(*ac), *v))
        .collect();
});

#[pymethods]
impl PyEadResult {
    /// Policy metadata stamped by the computing layer as a dict: numeric
    /// mode, active rounding context, any applied FX policy and the
    /// parallel-execution flag.
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    /// Export the per-asset-class add-on as an :class:`~finstack_quant.core.table.ArrowTable`.
    ///
    /// Columns: ``asset_class`` (wire label) and ``add_on`` (float in the
    /// netting set's reporting currency). One row per asset class present,
    /// sorted by ``asset_class`` so repeated runs are byte-identical. A netting
    /// set with no trades yields a zero-row table that still carries both
    /// columns with their real dtypes.
    #[pyo3(text_signature = "($self)")]
    fn to_add_on_table(&self) -> PyResult<PyArrowTable> {
        const COLUMNS: &[(&str, &str)] = &[("asset_class", "str"), ("add_on", "float64")];
        let rows: Vec<serde_json::Value> = self
            .inner
            .add_on_by_asset_class
            .iter()
            .map(|(ac, add_on)| {
                serde_json::json!({
                    "asset_class": serde_label(*ac),
                    "add_on": add_on,
                })
            })
            .collect();
        serde_rows_to_arrow_table(&rows, COLUMNS)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "EadResult(ead={:.2}, rc={:.2}, pfe={:.2}, alpha={:.2})",
            self.inner.ead, self.inner.rc, self.inner.pfe, self.inner.alpha
        )
    }
}

impl PyEadResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let row = serde_json::json!({
            "ead": self.inner.ead,
            "rc": self.inner.rc,
            "pfe": self.inner.pfe,
            "multiplier": self.inner.multiplier,
            "add_on_aggregate": self.inner.add_on_aggregate,
            "alpha": self.inner.alpha,
            "maturity_factor": self.inner.maturity_factor,
        });
        serde_object_to_single_row_arrow_table(
            &row,
            &[
                ("ead", "float64"),
                ("rc", "float64"),
                ("pfe", "float64"),
                ("multiplier", "float64"),
                ("add_on_aggregate", "float64"),
                ("alpha", "float64"),
                ("maturity_factor", "float64"),
            ],
        )
    }
}

tabular_methods!(PyEadResult);

/// Compute SA-CCR Exposure at Default for a netting set of trades.
///
/// Thin wrapper over ``SaCcrEngine.calculate_ead``: builds the engine with
/// the regulatory alpha of 1.4 (or ``alpha`` when supplied) and prices
/// ``trades`` under ``config``.
///
/// Parameters
/// ----------
/// trades : list[SaCcrTrade]
///     Derivative trades in the netting set (may be empty, giving zero EAD).
/// config : SaCcrNettingSetConfig
///     Netting-set collateral, threshold, MTA, NICA, MPoR and valuation date
///     (``SaCcrNettingSetConfig.unmargined`` / ``margined``).
/// alpha : float | None
///     Supervisory alpha override; must be finite and at least 1.0.
///
/// Returns an ``EadResult`` per BCBS 279. Raises ``ValueError`` if ``alpha``
/// is invalid or the configuration or a trade fails validation.
#[pyfunction]
#[pyo3(signature = (trades, config, alpha = None))]
pub fn saccr_ead(
    py: Python<'_>,
    trades: Vec<PyRef<'_, PySaCcrTrade>>,
    config: &PySaCcrNettingSetConfig,
    alpha: Option<f64>,
) -> PyResult<PyEadResult> {
    let trade_vec: Vec<SaCcrTrade> = trades.iter().map(|t| t.inner.clone()).collect();
    let result = py
        .detach(|| saccr_ead_rs(&trade_vec, &config.inner, alpha))
        .map_err(core_to_py)?;
    Ok(PyEadResult::from_inner(result))
}
