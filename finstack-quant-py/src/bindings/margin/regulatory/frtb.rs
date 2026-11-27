//! FRTB Sensitivity-Based Approach (SBA) Python bindings.

use super::super::frame::sensitivity_rows;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::extract::parse_label;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::{py_getters, tabular_methods, wire_methods};
use crate::bindings::module_utils::parse_currency;
use crate::bindings::table_builders::{
    serde_object_to_single_row_arrow_table, serde_rows_to_arrow_table,
};
use crate::errors::core_to_py;
use finstack_quant_margin::regulatory::frtb::{
    CorrelationScenario, DrcPosition, FrtbRiskClass, FrtbSbaEngine, FrtbSbaResult,
    FrtbSensitivities,
};
use pyo3::prelude::*;
use std::collections::BTreeMap;

/// Render a value as its own canonical serde wire label.
///
/// Reading the serde representation rather than re-listing variants here means
/// a new variant on a `#[non_exhaustive]` enum cannot silently collapse into a
/// shared `"unknown"` key — which, in a breakdown map, would drop a capital
/// charge as soon as two new variants collided.
pub(super) fn serde_label<T: serde::Serialize + std::fmt::Debug>(value: T) -> String {
    finstack_quant_core::wire::serde_label(&value).unwrap_or_else(|_| format!("{value:?}"))
}

pub(super) fn py_bool(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

/// FRTB sensitivity portfolio for the Sensitivity-Based Approach.
///
/// Build up delta/vega/curvature/DRC/RRAO inputs with the ``add_*`` methods
/// (or ``from_dataframe``), then pass to ``FrtbSbaEngine.calculate``. Units: GIRR deltas are base-currency P&L per
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
    pub(crate) inner: FrtbSensitivities,
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._regulatory.frtb", "DrcPosition")]
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._regulatory.frtb", "DrcPosition.__init__")]
    #[new]
    #[pyo3(signature = (**fields))]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, pyo3::types::PyDict>>) -> PyResult<Self> {
        let inner: DrcPosition =
            crate::bindings::module_utils::fields_from_py(py, fields, "DRC position")?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._regulatory.frtb", "DrcPosition.to_dict")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._regulatory.frtb", "FrtbSensitivities.add_girr_delta")]
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

/// FRTB SBA capital-charge result (BCBS d457).
///
/// Returned by ``FrtbSbaEngine.calculate``.
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
