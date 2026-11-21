//! Python bindings for FRTB SBA and SA-CCR regulatory capital frameworks.
//!
//! Callers build sensitivity or trade containers with the ``add_*`` methods
//! (or bulk-load a long-format frame), then run an engine or the matching
//! free function to get a typed result with a per-component breakdown.

use super::frame::{
    opt_date, opt_str, records, req_bool, req_date, req_f64, req_str, sensitivity_rows,
};
use super::types::{PyNettingSetId, extract_netting_set_id};
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::parse_label;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::{py_getters, tabular_methods, wire_methods};
use crate::bindings::module_utils::parse_currency;
use crate::bindings::table_builders::{
    serde_object_to_single_row_arrow_table, serde_rows_to_arrow_table,
};
use crate::errors::core_to_py;
use finstack_quant_margin::regulatory::{
    frtb::{
        CorrelationScenario, DrcPosition, FrtbRiskClass, FrtbSbaEngine, FrtbSbaResult,
        FrtbSensitivities, frtb_sba_charge as frtb_sba_charge_rs,
    },
    sa_ccr::{
        EadResult, SaCcrAssetClass, SaCcrEngine, SaCcrNettingSetConfig, SaCcrOptionType,
        SaCcrSupervisoryCategory, SaCcrTrade, saccr_ead as saccr_ead_rs,
    },
};
use pyo3::prelude::*;
use std::collections::BTreeMap;

/// Render a value as its own canonical serde wire label.
///
/// Reading the serde representation rather than re-listing variants here means
/// a new variant on a `#[non_exhaustive]` enum cannot silently collapse into a
/// shared `"unknown"` key — which, in a breakdown map, would drop a capital
/// charge as soon as two new variants collided.
fn serde_label<T: serde::Serialize + std::fmt::Debug>(value: T) -> String {
    finstack_quant_core::wire::serde_label(&value).unwrap_or_else(|_| format!("{value:?}"))
}

fn py_bool(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

/// FRTB sensitivity portfolio for the Sensitivity-Based Approach.
///
/// Build up delta/vega/curvature/DRC/RRAO inputs with the ``add_*`` methods
/// (or ``from_dataframe``), then pass to ``frtb_sba_charge`` or
/// ``FrtbSbaEngine.calculate``. Units: GIRR deltas are base-currency P&L per
/// **1 percentage point** of curve shift (``100 x DV01``); CSR deltas are
/// base-currency P&L per 1 basis point of spread; equity, commodity and FX
/// deltas are base-currency P&L per 1 percentage point of the underlying;
/// vegas are base-currency P&L as volatility-scaled vega (sigma times dV/dsigma); curvature
/// pairs are the up/down shocked P&L positions; DRC amounts are signed JTD
/// notionals before LGD; RRAO amounts are gross notionals.
#[pyclass(
    name = "FrtbSensitivities",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFrtbSensitivities {
    pub(super) inner: FrtbSensitivities,
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._regulatory", "DrcPosition")]
#[pyclass(
    name = "DrcPosition",
    module = "finstack_quant.margin",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyDrcPosition {
    inner: DrcPosition,
}

#[pymethods]
impl PyDrcPosition {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._regulatory", "DrcPosition.__init__")]
    #[new]
    #[pyo3(signature = (**fields))]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, pyo3::types::PyDict>>) -> PyResult<Self> {
        let inner: DrcPosition =
            crate::bindings::module_utils::fields_from_py(py, fields, "DRC position")?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._regulatory", "DrcPosition.to_dict")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }
}

impl PyFrtbSensitivities {
    pub(crate) fn from_inner(inner: FrtbSensitivities) -> Self {
        Self { inner }
    }
}

wire_methods!(PyFrtbSensitivities, FrtbSensitivities, "FrtbSensitivities");
py_getters!(PyFrtbSensitivities |s| {
    /// Base/reporting currency code.
    base_currency -> String = s.inner.base_currency.to_string();
});

macro_rules! frtb_adders {
    (
        ccy_amount: [ $( $(#[$ca_doc:meta])* $ca_fn:ident ),+ $(,)? ],
        delta5: [ $( $(#[$d5_doc:meta])* $d5_fn:ident($d5_id:ident) ),+ $(,)? ],
        vega4: [ $( $(#[$v4_doc:meta])* $v4_fn:ident($v4_id:ident) ),+ $(,)? ],
        cvr4: [ $( $(#[$c4_doc:meta])* $c4_fn:ident($c4_id:ident) ),+ $(,)? ],
        eq3: [ $( $(#[$e3_doc:meta])* $e3_fn:ident ),+ $(,)? ],
    ) => {
        #[pymethods]
        impl PyFrtbSensitivities {
            $(
                $(#[$ca_doc])*
                #[pyo3(signature = (currency, amount))]
                fn $ca_fn(&mut self, currency: &str, amount: f64) -> PyResult<()> {
                    let ccy = parse_currency(currency)?;
                    self.inner.$ca_fn(ccy, amount);
                    Ok(())
                }
            )+
            $(
                $(#[$d5_doc])*
                #[pyo3(signature = ($d5_id, bucket, tenor, basis, amount))]
                fn $d5_fn(&mut self, $d5_id: &str, bucket: u8, tenor: &str, basis: &str, amount: f64) {
                    self.inner.$d5_fn($d5_id, bucket, tenor, basis, amount);
                }
            )+
            $(
                $(#[$v4_doc])*
                #[pyo3(signature = ($v4_id, bucket, maturity, amount))]
                fn $v4_fn(&mut self, $v4_id: &str, bucket: u8, maturity: &str, amount: f64) {
                    self.inner.$v4_fn($v4_id, bucket, maturity, amount);
                }
            )+
            $(
                $(#[$c4_doc])*
                #[pyo3(signature = ($c4_id, bucket, cvr_up, cvr_down))]
                fn $c4_fn(&mut self, $c4_id: &str, bucket: u8, cvr_up: f64, cvr_down: f64) {
                    self.inner.$c4_fn($c4_id, bucket, cvr_up, cvr_down);
                }
            )+
            $(
                $(#[$e3_doc])*
                #[pyo3(signature = (underlier, bucket, amount))]
                fn $e3_fn(&mut self, underlier: &str, bucket: u8, amount: f64) {
                    self.inner.$e3_fn(underlier, bucket, amount);
                }
            )+
        }
    };
}

frtb_adders!(
    ccy_amount: [
        /// Add a GIRR inflation delta: ``amount`` is base-currency P&L per 1
        /// percentage point of inflation shift. Raises ``ValueError`` for an
        /// unknown currency.
        add_girr_inflation_delta,
        /// Add a GIRR cross-currency basis delta: ``amount`` is base-currency
        /// P&L per 1 percentage point of basis shift for ``currency``. Raises
        /// ``ValueError`` for an unknown currency.
        add_girr_xccy_basis_delta,
    ],
    delta5: [
        /// Add a CSR non-securitisation delta: ``amount`` is base-currency P&L
        /// per percentage-point spread move (100 times CS01); ``bucket`` is the 1-based CSR
        /// non-sec bucket (MAR21.51).
        add_csr_nonsec_delta(issuer),
        /// Add a CSR securitisation (CTP) delta: ``amount`` is base-currency
        /// P&L per percentage-point spread move (100 times CS01); ``bucket`` is the 1-based
        /// sec-CTP bucket (MAR21.59).
        add_csr_sec_ctp_delta(tranche),
        /// Add a CSR securitisation (non-CTP) delta: ``amount`` is base-currency
        /// P&L per percentage-point spread move (100 times CS01); ``bucket`` is the 1-based
        /// sec non-CTP bucket (MAR21.64).
        add_csr_sec_nonctp_delta(tranche),
        /// Add a commodity delta: ``amount`` is base-currency P&L per 1
        /// percentage point move in the commodity price at ``tenor``; ``bucket``
        /// is the 1-based commodity bucket (MAR21.82).
        add_commodity_delta(name),
    ],
    vega4: [
        /// Add a CSR non-securitisation vega: ``amount`` is base-currency P&L
        /// as volatility-scaled vega (sigma times dV/dsigma) at option ``maturity``.
        add_csr_nonsec_vega(issuer),
        /// Add a CSR securitisation (CTP) vega: ``amount`` is base-currency P&L
        /// as volatility-scaled vega (sigma times dV/dsigma) at option ``maturity``.
        add_csr_sec_ctp_vega(tranche),
        /// Add a CSR securitisation (non-CTP) vega: ``amount`` is base-currency
        /// P&L as volatility-scaled vega (sigma times dV/dsigma) at option ``maturity``.
        add_csr_sec_nonctp_vega(tranche),
        /// Add a commodity vega: ``amount`` is base-currency P&L per unit
        /// implied-volatility move at option ``maturity``.
        add_commodity_vega(name),
        /// Add an equity vega: ``amount`` is base-currency P&L per unit
        /// implied-volatility move at option ``maturity``.
        add_equity_vega(underlier),
    ],
    cvr4: [
        /// Add a CSR non-securitisation curvature pair (up / down shocked P&L
        /// positions in base currency).
        add_csr_nonsec_curvature(issuer),
        /// Add a CSR securitisation (CTP) curvature pair (up / down shocked P&L
        /// positions in base currency).
        add_csr_sec_ctp_curvature(tranche),
        /// Add a CSR securitisation (non-CTP) curvature pair (up / down shocked
        /// P&L positions in base currency).
        add_csr_sec_nonctp_curvature(tranche),
        /// Add a commodity curvature pair (up / down shocked P&L positions in
        /// base currency).
        add_commodity_curvature(name),
        /// Add an equity curvature pair (up / down shocked P&L positions in base
        /// currency).
        add_equity_curvature(underlier),
    ],
    eq3: [
        /// Add an equity delta: ``amount`` is base-currency P&L per 1 percentage
        /// point move in the underlier; ``bucket`` is the 1-based equity bucket
        /// (MAR21.72).
        add_equity_delta,
        /// Add equity repo-rate P&L per one percentage-point rate shift.
        add_equity_repo_delta,
    ],
);

#[pymethods]
impl PyFrtbSensitivities {
    /// Create an empty sensitivity container.
    ///
    /// ``base_currency`` is the reporting currency (e.g. ``"USD"``) and is
    /// required; raises ``ValueError`` for an unknown code.
    #[new]
    #[pyo3(signature = (base_currency))]
    fn new(base_currency: &str) -> PyResult<Self> {
        let ccy = parse_currency(base_currency)?;
        Ok(Self {
            inner: FrtbSensitivities::new(ccy),
        })
    }

    /// Bulk-load sensitivities from the long-format table ``to_pandas`` /
    /// ``to_arrow`` emits.
    ///
    /// Parameters
    /// ----------
    /// frame : pandas.DataFrame
    ///     Columns ``risk_class``, ``kind``, ``issuer``, ``bucket``,
    ///     ``tenor``, ``amount`` (plus the DRC columns ``sector``,
    ///     ``seniority``, ``maturity_years``,
    ///     ``pnl_adjustment`` on ``drc`` rows) encoded as ``to_pandas``
    ///     documents. ``curvature_up`` / ``curvature_down`` rows are
    ///     recombined into pairs; ``rrao`` rows carry ``exotic_notional`` /
    ///     ``other_notional``; each ``drc`` row is one default-risk position.
    /// base_currency : str
    ///     Reporting currency of every ``amount`` (the frame does not carry
    ///     it).
    ///
    /// Rows with the same key accumulate. Decoding is the Rust
    /// ``FrtbSensitivities::from_rows``, the inverse of ``to_pandas``.
    /// Raises ``ValueError`` for an unknown risk class or kind, a missing
    /// column or value, a bad bucket, currency or DRC label.
    #[staticmethod]
    #[pyo3(signature = (frame, base_currency))]
    fn from_dataframe(frame: &Bound<'_, PyAny>, base_currency: &str) -> PyResult<Self> {
        let inner =
            FrtbSensitivities::from_rows(parse_currency(base_currency)?, &sensitivity_rows(frame)?)
                .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Validate labels, buckets, identifiers and amounts without pricing.
    ///
    /// Raises ``ValueError`` naming the first invalid field when a tenor or
    /// bucket is unsupported, an identifier is empty, or a value is
    /// non-finite. The engines run this automatically.
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._regulatory", "FrtbSensitivities.add_girr_delta")]
    #[pyo3(signature = (currency, curve_id, tenor, amount))]
    fn add_girr_delta(
        &mut self,
        currency: &str,
        curve_id: &str,
        tenor: &str,
        amount: f64,
    ) -> PyResult<()> {
        self.inner
            .add_girr_delta(parse_currency(currency)?, curve_id, tenor, amount)
            .map_err(core_to_py)
    }

    /// Add an FX delta for the pair ``(ccy1, ccy2)``: ``amount`` is
    /// base-currency P&L per 1 percentage point move in the exchange rate.
    /// Raises ``ValueError`` for an unknown currency.
    #[pyo3(signature = (ccy1, ccy2, amount))]
    fn add_fx_delta(&mut self, ccy1: &str, ccy2: &str, amount: f64) -> PyResult<()> {
        let c1 = parse_currency(ccy1)?;
        let c2 = parse_currency(ccy2)?;
        self.inner.add_fx_delta(c1, c2, amount);
        Ok(())
    }

    /// Add a GIRR vega: ``amount`` is base-currency P&L per unit
    /// implied-volatility move for the ``option_maturity`` x
    /// ``underlying_tenor`` point. Raises ``ValueError`` for an unknown
    /// currency.
    #[pyo3(signature = (currency, option_maturity, underlying_tenor, amount))]
    fn add_girr_vega(
        &mut self,
        currency: &str,
        option_maturity: &str,
        underlying_tenor: &str,
        amount: f64,
    ) -> PyResult<()> {
        let ccy = parse_currency(currency)?;
        self.inner
            .add_girr_vega(ccy, option_maturity, underlying_tenor, amount);
        Ok(())
    }

    /// Add an FX vega: ``amount`` is base-currency P&L per unit
    /// implied-volatility move at option ``maturity``. Raises ``ValueError``
    /// for an unknown currency.
    #[pyo3(signature = (ccy1, ccy2, maturity, amount))]
    fn add_fx_vega(&mut self, ccy1: &str, ccy2: &str, maturity: &str, amount: f64) -> PyResult<()> {
        let c1 = parse_currency(ccy1)?;
        let c2 = parse_currency(ccy2)?;
        self.inner.add_fx_vega(c1, c2, maturity, amount);
        Ok(())
    }

    /// Add a GIRR curvature pair (up / down shocked P&L positions in base
    /// currency). Raises ``ValueError`` for an unknown currency.
    #[pyo3(signature = (currency, cvr_up, cvr_down))]
    fn add_girr_curvature(&mut self, currency: &str, cvr_up: f64, cvr_down: f64) -> PyResult<()> {
        let ccy = parse_currency(currency)?;
        self.inner.add_girr_curvature(ccy, cvr_up, cvr_down);
        Ok(())
    }

    /// Add an FX curvature pair (up / down shocked P&L positions in base
    /// currency). Raises ``ValueError`` for an unknown currency.
    #[pyo3(signature = (ccy1, ccy2, cvr_up, cvr_down))]
    fn add_fx_curvature(
        &mut self,
        ccy1: &str,
        ccy2: &str,
        cvr_up: f64,
        cvr_down: f64,
    ) -> PyResult<()> {
        let c1 = parse_currency(ccy1)?;
        let c2 = parse_currency(ccy2)?;
        self.inner.add_fx_curvature(c1, c2, cvr_up, cvr_down);
        Ok(())
    }

    /// Add a canonical non-securitisation DRC position.
    ///
    /// Parameters
    /// ----------
    /// position : DrcPosition or dict
    ///     Validated native position or Rust DrcPosition fields: issuer, signed jtd_amount in reporting
    ///     currency before LGD, rating_bucket (1..9), sector, seniority,
    ///     maturity_years (non-negative), and optional pnl_adjustment (default 0).
    ///     Equity seniority requires corporate sector. Securitisations are unsupported.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If fields are unknown, malformed, or fail Rust position validation.
    /// TypeError
    ///     If the position contains values that cannot be converted to JSON.
    #[pyo3(signature = (position))]
    fn add_drc_position(&mut self, position: &Bound<'_, PyAny>) -> PyResult<()> {
        let position: DrcPosition = if let Ok(typed) = position.cast::<PyDrcPosition>() {
            typed.borrow().inner.clone()
        } else {
            crate::bindings::module_utils::py_to_serde(position.py(), position, "position")?
        };
        position.validate().map_err(core_to_py)?;
        self.inner.add_drc_position(position);
        Ok(())
    }

    /// Add an RRAO (residual risk add-on) position: ``notional`` is the
    /// gross notional in base currency. ``is_exotic`` is required: ``True``
    /// for the 1% weight (exotic underlying), ``False`` for the 0.1% weight
    /// (other residual risk: gap, correlation, behavioural).
    #[pyo3(signature = (instrument_id, notional, is_exotic))]
    fn add_rrao_position(&mut self, instrument_id: &str, notional: f64, is_exotic: bool) {
        self.inner
            .add_rrao_position(instrument_id, notional, is_exotic);
    }

    /// Summarise the container by populated bucket counts.
    fn __repr__(&self) -> String {
        let s = &self.inner;
        let delta = s.girr_delta.len()
            + s.girr_inflation_delta.len()
            + s.girr_xccy_basis_delta.len()
            + s.csr_nonsec_delta.len()
            + s.csr_sec_ctp_delta.len()
            + s.csr_sec_nonctp_delta.len()
            + s.equity_delta.len()
            + s.commodity_delta.len()
            + s.fx_delta.len();
        let vega = s.girr_vega.len()
            + s.csr_nonsec_vega.len()
            + s.csr_sec_ctp_vega.len()
            + s.csr_sec_nonctp_vega.len()
            + s.equity_vega.len()
            + s.commodity_vega.len()
            + s.fx_vega.len();
        let curvature = s.girr_curvature.len()
            + s.csr_nonsec_curvature.len()
            + s.csr_sec_ctp_curvature.len()
            + s.csr_sec_nonctp_curvature.len()
            + s.equity_curvature.len()
            + s.commodity_curvature.len()
            + s.fx_curvature.len();
        format!(
            "FrtbSensitivities(base_currency={}, delta={delta}, vega={vega}, curvature={curvature}, drc={}, rrao={})",
            s.base_currency,
            s.drc_positions.len(),
            s.rrao_exotic_notionals.len(),
        )
    }
}

impl PyFrtbSensitivities {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let table = self.inner.to_table().map_err(core_to_py)?;
        PyArrowTable::from_envelope(&table)
    }
}

tabular_methods!(PyFrtbSensitivities);

/// FRTB SBA engine.
///
/// Evaluates delta, vega and curvature under each configured correlation
/// scenario, takes the maximum, then adds DRC and RRAO (BCBS d457).
#[pyclass(
    name = "FrtbSbaEngine",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
pub struct PyFrtbSbaEngine {
    inner: FrtbSbaEngine,
}

py_getters!(PyFrtbSbaEngine |s| {
    /// Correlation scenario labels evaluated, in configured order.
    scenarios -> Vec<String> = s
        .inner
        .scenarios()
        .iter()
        .map(|sc| serde_label(*sc))
        .collect();
    /// Risk-class labels included, in configured order.
    risk_classes -> Vec<String> = s
        .inner
        .risk_classes()
        .iter()
        .map(|rc| serde_label(*rc))
        .collect();
});

#[pymethods]
impl PyFrtbSbaEngine {
    /// Create an FRTB SBA engine.
    ///
    /// Parameters
    /// ----------
    /// scenarios : list[str] | None
    ///     Correlation scenarios to evaluate (``"low"``, ``"medium"``,
    ///     ``"high"``); the charge is the maximum across them. ``None``
    ///     evaluates all three (the regulatory default).
    /// risk_classes : list[str] | None
    ///     Risk classes whose delta/vega/curvature are included (``"girr"``,
    ///     ``"csr_non_sec"``, ``"csr_sec_ctp"``, ``"csr_sec_non_ctp"``,
    ///     ``"equity"``, ``"commodity"``, ``"fx"``); ``None`` includes all.
    ///
    /// Raises ``ValueError`` for an unknown label or an empty list.
    #[new]
    #[pyo3(signature = (scenarios = None, risk_classes = None))]
    fn new(scenarios: Option<Vec<String>>, risk_classes: Option<Vec<String>>) -> PyResult<Self> {
        let scenarios = scenarios
            .map(|labels| {
                labels
                    .iter()
                    .map(|s| parse_label::<CorrelationScenario>(s))
                    .collect::<PyResult<Vec<_>>>()
            })
            .transpose()?;
        let risk_classes = risk_classes
            .map(|labels| {
                labels
                    .iter()
                    .map(|s| parse_label::<FrtbRiskClass>(s))
                    .collect::<PyResult<Vec<_>>>()
            })
            .transpose()?;
        let inner = FrtbSbaEngine::with_selection(scenarios, risk_classes).map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Calculate the FRTB SBA charge for a sensitivity portfolio.
    ///
    /// Returns a ``FrtbSbaResult`` carrying the total charge, the
    /// per-risk-class delta/vega/curvature breakdown, DRC, RRAO, and the
    /// per-scenario charges with the binding scenario named. Raises
    /// ``ValueError`` if the sensitivities fail validation.
    fn calculate(
        &self,
        py: Python<'_>,
        sensitivities: &PyFrtbSensitivities,
    ) -> PyResult<PyFrtbSbaResult> {
        let result = py
            .detach(|| self.inner.calculate(&sensitivities.inner))
            .map_err(core_to_py)?;
        Ok(PyFrtbSbaResult::from_inner(result))
    }

    fn __repr__(&self) -> String {
        format!(
            "FrtbSbaEngine(scenarios={:?}, risk_classes={:?})",
            self.scenarios(),
            self.risk_classes()
        )
    }
}

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
    pub(super) inner: SaCcrTrade,
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
    pub(super) inner: SaCcrNettingSetConfig,
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

/// FRTB SBA capital-charge result (BCBS d457).
///
/// Returned by ``frtb_sba_charge`` and ``FrtbSbaEngine.calculate``.
/// Carries the headline charge, the per-risk-class delta/vega/curvature
/// breakdown, the default risk charge, the residual risk add-on, and the
/// charge under each correlation scenario together with the binding one.
#[pyclass(
    name = "FrtbSbaResult",
    module = "finstack_quant.margin",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFrtbSbaResult {
    inner: FrtbSbaResult,
}

impl PyFrtbSbaResult {
    fn from_inner(inner: FrtbSbaResult) -> Self {
        Self { inner }
    }

    fn labeled(map: &BTreeMap<FrtbRiskClass, f64>) -> BTreeMap<String, f64> {
        map.iter().map(|(rc, v)| (serde_label(*rc), *v)).collect()
    }
}

wire_methods!(PyFrtbSbaResult, FrtbSbaResult, "FrtbSbaResult");
py_getters!(PyFrtbSbaResult |s| {
    /// Total capital charge, in the sensitivity portfolio's base currency.
    total -> f64 = s.inner.total;
    /// Default Risk Charge (credit + equity jump-to-default).
    drc -> f64 = s.inner.drc;
    /// Residual Risk Add-On.
    rrao -> f64 = s.inner.rrao;
    /// Correlation scenario that produced the binding charge
    /// (``"low"``, ``"medium"``, or ``"high"``).
    binding_scenario -> String = serde_label(s.inner.binding_scenario);
    /// Delta risk charge keyed by risk-class wire label (e.g. ``"girr"``).
    delta_by_risk_class -> BTreeMap<String, f64> = Self::labeled(&s.inner.delta_by_risk_class);
    /// Vega risk charge keyed by risk-class wire label.
    vega_by_risk_class -> BTreeMap<String, f64> = Self::labeled(&s.inner.vega_by_risk_class);
    /// Curvature risk charge keyed by risk-class wire label.
    curvature_by_risk_class -> BTreeMap<String, f64> = Self::labeled(&s.inner.curvature_by_risk_class);
    /// Delta+vega+curvature charge under each evaluated correlation scenario.
    scenario_charges -> BTreeMap<String, f64> = s
        .inner
        .scenario_charges
        .iter()
        .map(|(sc, v)| (serde_label(*sc), *v))
        .collect();
});

#[pymethods]
impl PyFrtbSbaResult {
    /// Per-risk-class delta, vega and curvature charges under every evaluated
    /// correlation scenario, as
    /// ``{scenario: {"delta_by_risk_class": {...}, "vega_by_risk_class":
    /// {...}, "curvature_by_risk_class": {...}}}`` keyed by wire labels. Each
    /// scenario's maps sum to its ``scenario_charges`` entry.
    #[getter]
    fn scenario_breakdown<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.scenario_breakdown)
    }

    /// Policy metadata stamped by the computing layer as a dict: numeric
    /// mode, active rounding context, any applied FX policy and the
    /// parallel-execution flag.
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    /// Export the per-risk-class breakdown as a long-format
    /// :class:`~finstack_quant.core.table.ArrowTable`.
    ///
    /// Columns: ``component`` (``"delta"``, ``"vega"``, or ``"curvature"``),
    /// ``risk_class`` (wire label, e.g. ``"girr"``), and ``charge`` (float in
    /// the portfolio's base currency). Rows are emitted component-major and
    /// risk-class sorted so repeated runs are byte-identical.
    ///
    /// The components do not sum to ``total``: SBA aggregates them with
    /// prescribed correlations, and ``drc``/``rrao`` sit outside this table.
    #[pyo3(text_signature = "($self)")]
    fn to_breakdown_table(&self) -> PyResult<PyArrowTable> {
        const COLUMNS: &[(&str, &str)] = &[
            ("component", "str"),
            ("risk_class", "str"),
            ("charge", "float64"),
        ];
        let mut rows: Vec<serde_json::Value> = Vec::new();
        for (component, map) in [
            ("delta", &self.inner.delta_by_risk_class),
            ("vega", &self.inner.vega_by_risk_class),
            ("curvature", &self.inner.curvature_by_risk_class),
        ] {
            for (rc, charge) in map {
                rows.push(serde_json::json!({
                    "component": component,
                    "risk_class": serde_label(*rc),
                    "charge": charge,
                }));
            }
        }
        serde_rows_to_arrow_table(&rows, COLUMNS)
    }

    /// Export the per-scenario SBA charges as an :class:`~finstack_quant.core.table.ArrowTable`.
    ///
    /// Columns: ``scenario`` (``"low"``, ``"medium"``, ``"high"``),
    /// ``charge`` (delta+vega+curvature under that scenario, float in the
    /// portfolio's base currency) and ``binding`` (``True`` on the scenario
    /// that produced ``total``). One row per evaluated scenario in
    /// low/medium/high order.
    #[pyo3(text_signature = "($self)")]
    fn to_scenario_table(&self) -> PyResult<PyArrowTable> {
        const COLUMNS: &[(&str, &str)] = &[
            ("scenario", "str"),
            ("charge", "float64"),
            ("binding", "bool"),
        ];
        let rows: Vec<serde_json::Value> = self
            .inner
            .scenario_charges
            .iter()
            .map(|(scenario, charge)| {
                serde_json::json!({
                    "scenario": serde_label(*scenario),
                    "charge": charge,
                    "binding": *scenario == self.inner.binding_scenario,
                })
            })
            .collect();
        serde_rows_to_arrow_table(&rows, COLUMNS)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "FrtbSbaResult(total={:.2}, drc={:.2}, rrao={:.2}, binding_scenario={:?})",
            self.inner.total,
            self.inner.drc,
            self.inner.rrao,
            self.binding_scenario()
        )
    }
}

impl PyFrtbSbaResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let row = serde_json::json!({
            "total": self.inner.total,
            "drc": self.inner.drc,
            "rrao": self.inner.rrao,
            "binding_scenario": self.binding_scenario(),
        });
        serde_object_to_single_row_arrow_table(
            &row,
            &[
                ("total", "float64"),
                ("drc", "float64"),
                ("rrao", "float64"),
                ("binding_scenario", "str"),
            ],
        )
    }
}

tabular_methods!(PyFrtbSbaResult);

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

/// Compute the FRTB SBA capital charge.
///
/// Returns a ``FrtbSbaResult`` exposing ``total``, ``drc``, ``rrao``,
/// ``binding_scenario``, ``scenario_charges``, and the per-risk-class
/// ``delta_by_risk_class`` / ``vega_by_risk_class`` /
/// ``curvature_by_risk_class`` breakdowns, plus ``to_pandas`` / ``to_polars`` /
/// ``to_arrow`` and ``to_breakdown_table`` / ``to_scenario_table`` exits.
///
/// If ``correlation_scenario`` is provided (``"low"``, ``"medium"``, ``"high"``),
/// only that scenario is evaluated. Otherwise all three are run and the
/// max is taken per BCBS d457. Raises ``ValueError`` for an unknown scenario
/// label or sensitivities that fail validation.
#[pyfunction]
#[pyo3(signature = (sensitivities, correlation_scenario = None))]
pub fn frtb_sba_charge(
    py: Python<'_>,
    sensitivities: &PyFrtbSensitivities,
    correlation_scenario: Option<&str>,
) -> PyResult<PyFrtbSbaResult> {
    let scenario = correlation_scenario
        .map(parse_label::<CorrelationScenario>)
        .transpose()?;
    let result = py
        .detach(|| frtb_sba_charge_rs(&sensitivities.inner, scenario))
        .map_err(core_to_py)?;

    Ok(PyFrtbSbaResult::from_inner(result))
}

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

/// Register FRTB / SA-CCR classes and functions on the margin module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFrtbSensitivities>()?;
    m.add_class::<PyDrcPosition>()?;
    m.add_class::<PyFrtbSbaEngine>()?;
    m.add_class::<PyFrtbSbaResult>()?;
    m.add_class::<PyEadResult>()?;
    m.add_class::<PySaCcrTrade>()?;
    m.add_class::<PySaCcrNettingSetConfig>()?;
    m.add_class::<PySaCcrEngine>()?;
    m.add_function(pyo3::wrap_pyfunction!(frtb_sba_charge, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(saccr_ead, m)?)?;
    Ok(())
}
