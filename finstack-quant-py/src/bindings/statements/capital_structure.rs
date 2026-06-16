//! Python wrappers for capital-structure specs (waterfall + ECF sweep + PIK toggle).
//!
//! Mirrors `finstack_quant_statements::capital_structure::{WaterfallSpec, EcfSweepSpec,
//! PikToggleSpec, PaymentPriority}`. All classes support JSON round-trip via
//! `from_json`/`to_json` and structured keyword-argument construction.

use crate::errors::display_to_py;
use finstack_quant_statements::capital_structure::{
    EcfSweepSpec, PaymentPriority, PikToggleSpec, WaterfallSpec,
};
use pyo3::prelude::*;

fn parse_priority(s: &str) -> PyResult<PaymentPriority> {
    match s {
        "fees" => Ok(PaymentPriority::Fees),
        "interest" => Ok(PaymentPriority::Interest),
        "amortization" => Ok(PaymentPriority::Amortization),
        "mandatory_prepayment" => Ok(PaymentPriority::MandatoryPrepayment),
        "voluntary_prepayment" => Ok(PaymentPriority::VoluntaryPrepayment),
        "sweep" => Ok(PaymentPriority::Sweep),
        "equity" => Ok(PaymentPriority::Equity),
        other => Err(crate::errors::value_error(format!(
            "unknown payment priority {other:?}; expected one of: fees, interest, amortization, mandatory_prepayment, voluntary_prepayment, sweep, equity"
        ))),
    }
}

fn priority_to_str(p: PaymentPriority) -> &'static str {
    match p {
        PaymentPriority::Fees => "fees",
        PaymentPriority::Interest => "interest",
        PaymentPriority::Amortization => "amortization",
        PaymentPriority::MandatoryPrepayment => "mandatory_prepayment",
        PaymentPriority::VoluntaryPrepayment => "voluntary_prepayment",
        PaymentPriority::Sweep => "sweep",
        PaymentPriority::Equity => "equity",
    }
}

// ---------------------------------------------------------------------------
// EcfSweepSpec
// ---------------------------------------------------------------------------

/// Excess Cash Flow (ECF) sweep specification.
///
/// Defines how to compute ECF and what fraction sweeps to debt paydown.
#[pyclass(
    name = "EcfSweepSpec",
    module = "finstack_quant.statements",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyEcfSweepSpec {
    pub(super) inner: EcfSweepSpec,
}

#[pymethods]
impl PyEcfSweepSpec {
    /// Construct an ECF sweep spec.
    ///
    /// Parameters
    /// ----------
    /// ebitda_node : str
    ///     Node reference or formula for EBITDA.
    /// sweep_percentage : float
    ///     Sweep percentage in [0, 1] (e.g. 0.5 for 50%).
    /// taxes_node, capex_node, working_capital_node, cash_interest_node : str | None
    ///     Optional node references deducted from EBITDA to compute ECF.
    /// target_instrument_id : str | None
    ///     If set, sweep applies only to this debt instrument id.
    #[new]
    #[pyo3(
        signature = (
            ebitda_node,
            sweep_percentage,
            taxes_node=None,
            capex_node=None,
            working_capital_node=None,
            cash_interest_node=None,
            target_instrument_id=None,
        ),
        text_signature = "(ebitda_node, sweep_percentage, taxes_node=None, capex_node=None, working_capital_node=None, cash_interest_node=None, target_instrument_id=None)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn new(
        ebitda_node: String,
        sweep_percentage: f64,
        taxes_node: Option<String>,
        capex_node: Option<String>,
        working_capital_node: Option<String>,
        cash_interest_node: Option<String>,
        target_instrument_id: Option<String>,
    ) -> Self {
        Self {
            inner: EcfSweepSpec {
                ebitda_node,
                taxes_node,
                capex_node,
                working_capital_node,
                cash_interest_node,
                sweep_percentage,
                target_instrument_id,
            },
        }
    }

    /// Deserialize from JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: EcfSweepSpec = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn ebitda_node(&self) -> &str {
        &self.inner.ebitda_node
    }

    #[getter]
    fn sweep_percentage(&self) -> f64 {
        self.inner.sweep_percentage
    }

    #[getter]
    fn target_instrument_id(&self) -> Option<&str> {
        self.inner.target_instrument_id.as_deref()
    }

    fn __repr__(&self) -> String {
        format!(
            "EcfSweepSpec(ebitda_node={:?}, sweep_percentage={}, target_instrument_id={:?})",
            self.inner.ebitda_node,
            self.inner.sweep_percentage,
            self.inner
                .target_instrument_id
                .as_deref()
                .unwrap_or("<all>")
        )
    }
}

// ---------------------------------------------------------------------------
// PikToggleSpec
// ---------------------------------------------------------------------------

/// PIK toggle specification.
///
/// Controls when interest accrues as PIK (added to principal) vs. cash, based
/// on a liquidity metric threshold with optional hysteresis.
#[pyclass(
    name = "PikToggleSpec",
    module = "finstack_quant.statements",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPikToggleSpec {
    pub(super) inner: PikToggleSpec,
}

#[pymethods]
impl PyPikToggleSpec {
    /// Construct a PIK toggle spec.
    ///
    /// Parameters
    /// ----------
    /// liquidity_metric : str
    ///     Node reference or formula for the liquidity signal.
    /// threshold : float
    ///     When the metric is below this value, PIK is triggered.
    /// target_instrument_ids : list[str] | None
    ///     If set, PIK toggles only these instruments.
    /// min_periods_in_pik : int
    ///     Minimum periods PIK stays active once triggered (hysteresis; default 0).
    #[new]
    #[pyo3(
        signature = (
            liquidity_metric,
            threshold,
            target_instrument_ids=None,
            min_periods_in_pik=0,
        ),
        text_signature = "(liquidity_metric, threshold, target_instrument_ids=None, min_periods_in_pik=0)"
    )]
    fn new(
        liquidity_metric: String,
        threshold: f64,
        target_instrument_ids: Option<Vec<String>>,
        min_periods_in_pik: usize,
    ) -> Self {
        Self {
            inner: PikToggleSpec {
                liquidity_metric,
                threshold,
                target_instrument_ids,
                min_periods_in_pik,
            },
        }
    }

    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: PikToggleSpec = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn liquidity_metric(&self) -> &str {
        &self.inner.liquidity_metric
    }

    #[getter]
    fn threshold(&self) -> f64 {
        self.inner.threshold
    }

    #[getter]
    fn min_periods_in_pik(&self) -> usize {
        self.inner.min_periods_in_pik
    }

    fn __repr__(&self) -> String {
        format!(
            "PikToggleSpec(liquidity_metric={:?}, threshold={}, min_periods_in_pik={})",
            self.inner.liquidity_metric, self.inner.threshold, self.inner.min_periods_in_pik
        )
    }
}

// ---------------------------------------------------------------------------
// WaterfallSpec
// ---------------------------------------------------------------------------

/// Waterfall specification for dynamic cash flow allocation.
///
/// Configures payment priority, optional ECF sweep, and optional PIK toggle.
/// Call `validate()` before passing to a model builder to surface configuration
/// errors (e.g. `Sweep` ordered after `Equity` when a sweep is configured).
#[pyclass(
    name = "WaterfallSpec",
    module = "finstack_quant.statements",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyWaterfallSpec {
    pub(super) inner: WaterfallSpec,
}

#[pymethods]
impl PyWaterfallSpec {
    /// Construct a waterfall spec.
    ///
    /// Parameters
    /// ----------
    /// priority_of_payments : list[str] | None
    ///     Priority order (strings: fees, interest, amortization, mandatory_prepayment,
    ///     voluntary_prepayment, sweep, equity). Defaults to the standard
    ///     [fees, interest, amortization, sweep, equity] order.
    /// available_cash_node : str | None
    ///     Optional formula/node reference for cash available to allocate.
    /// ecf_sweep : EcfSweepSpec | None
    ///     Optional ECF sweep configuration.
    /// pik_toggle : PikToggleSpec | None
    ///     Optional PIK toggle configuration.
    #[new]
    #[pyo3(
        signature = (
            priority_of_payments=None,
            available_cash_node=None,
            ecf_sweep=None,
            pik_toggle=None,
        ),
        text_signature = "(priority_of_payments=None, available_cash_node=None, ecf_sweep=None, pik_toggle=None)"
    )]
    fn new(
        priority_of_payments: Option<Vec<String>>,
        available_cash_node: Option<String>,
        ecf_sweep: Option<&PyEcfSweepSpec>,
        pik_toggle: Option<&PyPikToggleSpec>,
    ) -> PyResult<Self> {
        let mut inner = WaterfallSpec::default();
        if let Some(priority) = priority_of_payments {
            inner.priority_of_payments = priority
                .into_iter()
                .map(|s| parse_priority(&s))
                .collect::<PyResult<Vec<_>>>()?;
        }
        inner.available_cash_node = available_cash_node;
        inner.ecf_sweep = ecf_sweep.map(|p| p.inner.clone());
        inner.pik_toggle = pik_toggle.map(|p| p.inner.clone());
        Ok(Self { inner })
    }

    /// Deserialize from JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: WaterfallSpec = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Validate the spec against internal consistency rules.
    ///
    /// Raises `ValueError` if the configuration is economically inconsistent
    /// (e.g. `Sweep` ordered after `Equity` while a positive ECF sweep is set).
    #[pyo3(text_signature = "($self)")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(display_to_py)
    }

    /// Payment priority order as a list of strings.
    #[getter]
    fn priority_of_payments(&self) -> Vec<&'static str> {
        self.inner
            .priority_of_payments
            .iter()
            .copied()
            .map(priority_to_str)
            .collect()
    }

    #[getter]
    fn available_cash_node(&self) -> Option<&str> {
        self.inner.available_cash_node.as_deref()
    }

    #[getter]
    fn has_ecf_sweep(&self) -> bool {
        self.inner.ecf_sweep.is_some()
    }

    #[getter]
    fn has_pik_toggle(&self) -> bool {
        self.inner.pik_toggle.is_some()
    }

    fn __repr__(&self) -> String {
        format!(
            "WaterfallSpec(priority={:?}, ecf_sweep={}, pik_toggle={})",
            self.priority_of_payments(),
            self.inner.ecf_sweep.is_some(),
            self.inner.pik_toggle.is_some(),
        )
    }
}

/// Register capital-structure classes.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyEcfSweepSpec>()?;
    m.add_class::<PyPikToggleSpec>()?;
    m.add_class::<PyWaterfallSpec>()?;
    Ok(())
}
