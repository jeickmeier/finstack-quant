//! Python bindings for `finstack_quant_models::credit::liability_management`.

use crate::bindings::pandas_utils::serde_object_to_single_row_dataframe_with_schema;
use crate::errors::{core_to_py, serde_json_to_py};
use finstack_quant_models::credit::liability_management::{
    self as lm, ExchangeOfferAnalysis, ExchangeType, LeverageImpact, LmeAnalysis, LmeType,
    TENDER_RECOMMENDATION_HURDLE,
};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

/// Hold-versus-tender economics of a distressed exchange offer.
///
/// ``exchange_type`` is one of ``par_for_par``, ``discount``, ``uptier``,
/// ``downtier``. Tendering is recommended when ``tender_total`` exceeds
/// ``old_npv * TENDER_RECOMMENDATION_HURDLE``.
#[pyclass(
    name = "ExchangeOfferAnalysis",
    module = "finstack_quant.models.credit.liability_management",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyExchangeOfferAnalysis {
    inner: ExchangeOfferAnalysis,
}

#[pymethods]
impl PyExchangeOfferAnalysis {
    /// Which exchange structure this analyses.
    #[getter]
    fn exchange_type(&self) -> String {
        self.inner.exchange_type.as_str().to_string()
    }

    /// Net present value of the existing instrument.
    #[getter]
    fn old_npv(&self) -> f64 {
        self.inner.old_npv
    }

    /// Net present value of the instrument offered in exchange.
    #[getter]
    fn new_npv(&self) -> f64 {
        self.inner.new_npv
    }

    /// Fee paid to holders for consenting.
    #[getter]
    fn consent_fee(&self) -> f64 {
        self.inner.consent_fee
    }

    /// Value of any equity offered alongside the new instrument.
    #[getter]
    fn equity_sweetener_value(&self) -> f64 {
        self.inner.equity_sweetener_value
    }

    /// Total consideration offered per unit tendered.
    #[getter]
    fn tender_total(&self) -> f64 {
        self.inner.tender_total
    }

    /// `new_npv - old_npv`; positive means the holder gains.
    #[getter]
    fn delta_npv(&self) -> f64 {
        self.inner.delta_npv
    }

    /// Recovery rate at which tendering and holding are worth the same.
    #[getter]
    fn breakeven_recovery(&self) -> f64 {
        self.inner.breakeven_recovery
    }

    /// Whether `delta_npv` favours tendering.
    #[getter]
    fn tender_recommended(&self) -> bool {
        self.inner.tender_recommended
    }

    /// Deserialize from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid ExchangeOfferAnalysis JSON"))?,
        })
    }

    /// Serialize to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "ExchangeOfferAnalysis serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Export as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``exchange_type``, ``old_npv``, ``new_npv``, ``consent_fee``,
    /// ``equity_sweetener_value``, ``tender_total``, ``delta_npv``,
    /// ``breakeven_recovery``, ``tender_recommended``.
    ///
    /// One offer is one flat record, so a one-row frame is the right shape:
    /// ``pd.concat`` over several candidate offers gives a hold-versus-tender
    /// comparison table directly.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        // Built explicitly rather than serializing `inner` so `exchange_type`
        // lands as its canonical string label instead of the serde enum
        // representation.
        let row = serde_json::json!({
            "exchange_type": self.inner.exchange_type.as_str(),
            "old_npv": self.inner.old_npv,
            "new_npv": self.inner.new_npv,
            "consent_fee": self.inner.consent_fee,
            "equity_sweetener_value": self.inner.equity_sweetener_value,
            "tender_total": self.inner.tender_total,
            "delta_npv": self.inner.delta_npv,
            "breakeven_recovery": self.inner.breakeven_recovery,
            "tender_recommended": self.inner.tender_recommended,
        });
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &[
                "exchange_type",
                "old_npv",
                "new_npv",
                "consent_fee",
                "equity_sweetener_value",
                "tender_total",
                "delta_npv",
                "breakeven_recovery",
                "tender_recommended",
            ],
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "ExchangeOfferAnalysis(exchange_type='{}', tender_total={}, delta_npv={}, \
             tender_recommended={})",
            self.inner.exchange_type,
            self.inner.tender_total,
            self.inner.delta_npv,
            self.inner.tender_recommended
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Gross-leverage impact of a liability management exercise.
#[pyclass(
    name = "LeverageImpact",
    module = "finstack_quant.models.credit.liability_management",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyLeverageImpact {
    inner: LeverageImpact,
}

#[pymethods]
impl PyLeverageImpact {
    /// Total debt before the transaction.
    #[getter]
    fn pre_total_debt(&self) -> f64 {
        self.inner.pre_total_debt
    }

    /// Total debt after the transaction.
    #[getter]
    fn post_total_debt(&self) -> f64 {
        self.inner.post_total_debt
    }

    /// Leverage multiple before the transaction.
    #[getter]
    fn pre_leverage(&self) -> f64 {
        self.inner.pre_leverage
    }

    /// Leverage multiple after the transaction.
    #[getter]
    fn post_leverage(&self) -> f64 {
        self.inner.post_leverage
    }

    /// `pre_leverage - post_leverage`; negative when the deal levers up.
    #[getter]
    fn leverage_reduction(&self) -> f64 {
        self.inner.leverage_reduction
    }

    /// Deserialize from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid LeverageImpact JSON"))?,
        })
    }

    /// Serialize to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "LeverageImpact serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Export as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``pre_total_debt``, ``post_total_debt``, ``pre_leverage``,
    /// ``post_leverage``, ``leverage_reduction``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &self.inner,
            &[
                "pre_total_debt",
                "post_total_debt",
                "pre_leverage",
                "post_leverage",
                "leverage_reduction",
            ],
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "LeverageImpact(pre_leverage={}, post_leverage={}, leverage_reduction={})",
            self.inner.pre_leverage, self.inner.post_leverage, self.inner.leverage_reduction
        )
    }
}

/// Issuer-side economics of a liability management exercise.
///
/// ``lme_type`` is one of ``open_market_repurchase``, ``tender_offer``,
/// ``amend_and_extend``, ``dropdown``. Percentages are decimal fractions.
#[pyclass(
    name = "LmeAnalysis",
    module = "finstack_quant.models.credit.liability_management",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyLmeAnalysis {
    inner: LmeAnalysis,
}

#[pymethods]
impl PyLmeAnalysis {
    /// Which liability-management exercise this analyses.
    #[getter]
    fn lme_type(&self) -> String {
        self.inner.lme_type.as_str().to_string()
    }

    /// Cash cost of the exercise, including fees.
    #[getter]
    fn cost(&self) -> f64 {
        self.inner.cost
    }

    /// Face amount retired by the exercise.
    #[getter]
    fn notional_reduction(&self) -> f64 {
        self.inner.notional_reduction
    }

    /// Value captured by retiring debt below par.
    #[getter]
    fn discount_capture(&self) -> f64 {
        self.inner.discount_capture
    }

    /// Discount captured, as a decimal fraction (``0.1`` = 10%), not ×100.
    #[getter]
    fn discount_capture_pct(&self) -> f64 {
        self.inner.discount_capture_pct
    }

    /// Impact on remaining holders, as a decimal fraction (``-0.05`` = −5%),
    /// not ×100.
    #[getter]
    fn remaining_holder_impact_pct(&self) -> f64 {
        self.inner.remaining_holder_impact_pct
    }

    /// Leverage before and after the exercise.
    #[getter]
    fn leverage_impact(&self) -> Option<PyLeverageImpact> {
        self.inner
            .leverage_impact
            .clone()
            .map(|inner| PyLeverageImpact { inner })
    }

    /// Deserialize from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid LmeAnalysis JSON"))?,
        })
    }

    /// Serialize to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "LmeAnalysis serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Export as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``lme_type``, ``cost``, ``notional_reduction``,
    /// ``discount_capture``, ``discount_capture_pct``,
    /// ``remaining_holder_impact_pct``, ``pre_total_debt``,
    /// ``post_total_debt``, ``pre_leverage``, ``post_leverage``,
    /// ``leverage_reduction``.
    ///
    /// One exercise is one flat record, so a one-row frame is the right shape:
    /// ``pd.concat`` over several structures gives a discount-capture
    /// comparison table directly.
    ///
    /// The five leverage columns come from :attr:`leverage_impact` and are
    /// flattened onto the same row rather than nested. They are ``None`` (and
    /// therefore ``object`` dtype) when no positive EBITDA was supplied;
    /// coerce with ``pd.to_numeric`` before aggregating a mixed set.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        // `LmeAnalysis` derives Serialize, but `leverage_impact` is a nested
        // struct that blind serde would drop into a single object-dtype cell,
        // so the row is flattened by hand.
        let leverage = self.inner.leverage_impact.as_ref();
        let row = serde_json::json!({
            "lme_type": self.inner.lme_type.as_str(),
            "cost": self.inner.cost,
            "notional_reduction": self.inner.notional_reduction,
            "discount_capture": self.inner.discount_capture,
            "discount_capture_pct": self.inner.discount_capture_pct,
            "remaining_holder_impact_pct": self.inner.remaining_holder_impact_pct,
            "pre_total_debt": leverage.map(|l| l.pre_total_debt),
            "post_total_debt": leverage.map(|l| l.post_total_debt),
            "pre_leverage": leverage.map(|l| l.pre_leverage),
            "post_leverage": leverage.map(|l| l.post_leverage),
            "leverage_reduction": leverage.map(|l| l.leverage_reduction),
        });
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &[
                "lme_type",
                "cost",
                "notional_reduction",
                "discount_capture",
                "discount_capture_pct",
                "remaining_holder_impact_pct",
                "pre_total_debt",
                "post_total_debt",
                "pre_leverage",
                "post_leverage",
                "leverage_reduction",
            ],
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "LmeAnalysis(lme_type='{}', cost={}, notional_reduction={}, discount_capture={})",
            self.inner.lme_type,
            self.inner.cost,
            self.inner.notional_reduction,
            self.inner.discount_capture
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Compare hold-versus-tender economics for a distressed exchange offer.
///
/// Parameters
/// ----------
/// old_pv : float
///     Present value of the existing claim if not tendered.
/// new_pv : float
///     Present value of the new instrument received on tendering, same units.
/// consent_fee : float
///     Cash consent / early-tender fee per unit tendered, same units; pass
///     ``0.0`` when the offer carries none.
/// equity_sweetener_value : float
///     Value of equity or warrants attached to the new instrument, same units;
///     pass ``0.0`` when there is none.
/// exchange_type : str
///     One of ``par_for_par``, ``discount``, ``uptier``, ``downtier``.
///
/// Every argument is required, as in Rust and WASM: the offer terms are
/// deal-specific, so there is no default structure or fee.
///
/// Returns an ``ExchangeOfferAnalysis``; tendering is recommended only when
/// ``tender_total > old_pv * TENDER_RECOMMENDATION_HURDLE``.
///
/// Raises ``ValueError`` for an unknown ``exchange_type`` or a negative /
/// non-finite monetary input.
#[pyfunction]
#[pyo3(text_signature = "(old_pv, new_pv, consent_fee, equity_sweetener_value, exchange_type)")]
fn analyze_exchange_offer(
    old_pv: f64,
    new_pv: f64,
    consent_fee: f64,
    equity_sweetener_value: f64,
    exchange_type: &str,
) -> PyResult<PyExchangeOfferAnalysis> {
    let exchange_type: ExchangeType = exchange_type.parse().map_err(core_to_py)?;
    lm::analyze_exchange_offer(
        old_pv,
        new_pv,
        consent_fee,
        equity_sweetener_value,
        exchange_type,
    )
    .map(|inner| PyExchangeOfferAnalysis { inner })
    .map_err(core_to_py)
}

/// Compute discount capture and leverage impact for an LME transaction.
///
/// Parameters
/// ----------
/// lme_type : str
///     One of ``open_market_repurchase``, ``tender_offer``,
///     ``amend_and_extend``, ``dropdown``.
/// notional : float
///     Outstanding face amount of the target instrument (> 0).
/// repurchase_price_pct : float
///     Price as a decimal fraction of par for repurchases and tenders
///     (``(0, 1.5]``), the extension fee for amend-and-extend (``[0, 0.1]``),
///     or the transferred-asset fraction for a dropdown (``[0, 1]``).
/// opt_acceptance_pct : float
///     Fraction of holders participating, in [0, 1]; required, as in Rust
///     and WASM, because participation drives cost and notional reduction.
/// ebitda : float | None, default None
///     EBITDA in the same units as ``notional``; a positive value adds the
///     ``leverage_impact`` block.
///
/// Returns an ``LmeAnalysis``.
///
/// Raises ``ValueError`` for an unknown ``lme_type`` or an out-of-range input.
#[pyfunction]
#[pyo3(signature = (lme_type, notional, repurchase_price_pct, opt_acceptance_pct, ebitda=None))]
#[pyo3(
    text_signature = "(lme_type, notional, repurchase_price_pct, opt_acceptance_pct, ebitda=None)"
)]
fn analyze_lme(
    lme_type: &str,
    notional: f64,
    repurchase_price_pct: f64,
    opt_acceptance_pct: f64,
    ebitda: Option<f64>,
) -> PyResult<PyLmeAnalysis> {
    let lme_type: LmeType = lme_type.parse().map_err(core_to_py)?;
    lm::analyze_lme(
        lme_type,
        notional,
        repurchase_price_pct,
        opt_acceptance_pct,
        ebitda,
    )
    .map(|inner| PyLmeAnalysis { inner })
    .map_err(core_to_py)
}

/// Build the `finstack_quant.models.credit.liability_management` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "liability_management")?;
    m.setattr(
        "__doc__",
        "Distressed-exchange hold-versus-tender economics and issuer LME analytics.",
    )?;

    m.add("TENDER_RECOMMENDATION_HURDLE", TENDER_RECOMMENDATION_HURDLE)?;
    m.add_class::<PyExchangeOfferAnalysis>()?;
    m.add_class::<PyLeverageImpact>()?;
    m.add_class::<PyLmeAnalysis>()?;
    m.add_function(wrap_pyfunction!(analyze_exchange_offer, &m)?)?;
    m.add_function(wrap_pyfunction!(analyze_lme, &m)?)?;

    let all = PyList::new(
        py,
        [
            "ExchangeOfferAnalysis",
            "LeverageImpact",
            "LmeAnalysis",
            "TENDER_RECOMMENDATION_HURDLE",
            "analyze_exchange_offer",
            "analyze_lme",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "liability_management",
        "finstack_quant.models.credit",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;
    Ok(())
}
