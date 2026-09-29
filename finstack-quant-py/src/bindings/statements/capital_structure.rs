//! Python wrappers for capital-structure specs (waterfall + ECF sweep + PIK toggle)
//! and the evaluated capital-structure cashflows.
//!
//! Mirrors `finstack_quant_statements::capital_structure::{WaterfallSpec, EcfSweepSpec,
//! PikToggleSpec, PaymentClassSpec, PaymentPriority, CapitalStructureCashflows}`.
//! All classes support JSON round-trip via `from_json`/`to_json`.

use crate::bindings::core::money::PyMoney;
use crate::bindings::pandas_utils::{serde_rows_to_dataframe_with_schema, ColumnSchema};
use crate::errors::{serde_json_to_py, statements_to_py};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::capital_structure::{
    CapitalStructureCashflows, CashflowBreakdown, EcfSweepSpec, PaymentClassSpec, PaymentPriority,
    PikToggleSpec, WaterfallSpec,
};
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Parse the serde name of a [`PaymentPriority`] (e.g. `"fees"`, `"mandatory_prepayment"`).
fn parse_priority(s: &str) -> PyResult<PaymentPriority> {
    finstack_quant_core::wire::serde_parse(s).map_err(crate::errors::core_to_py)
}

/// Serde name of a [`PaymentPriority`]; identical to the `to_json` form.
fn priority_to_str(p: PaymentPriority) -> PyResult<String> {
    finstack_quant_core::wire::serde_label(&p).map_err(crate::errors::core_to_py)
}

/// Columns emitted by `CapitalStructureCashflows.to_dataframe` and
/// `to_totals_dataframe`.
const CASHFLOW_COLUMNS: [ColumnSchema<'static>; 5] = [
    ("instrument", "str"),
    ("period", "str"),
    ("flow_type", "str"),
    ("amount", "float64"),
    ("currency", "str"),
];

/// Excess Cash Flow (ECF) sweep specification.
///
/// Defines how to compute ECF and what fraction sweeps to debt paydown.
#[pyclass(
    name = "EcfSweepSpec",
    module = "finstack_quant.statements",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyEcfSweepSpec {
    pub(super) inner: EcfSweepSpec,
}

#[pymethods]
impl PyEcfSweepSpec {
    /// Construct an ECF sweep spec.
    ///
    /// Excess cash flow is built as
    /// ``ECF = EBITDA - taxes - capex - ΔWC - cash interest paid``, less any
    /// fees and scheduled principal that rank ahead of the prepayment
    /// priority. Every node reference is evaluated per period as a monetary
    /// amount in the model's reporting currency.
    ///
    /// Parameters
    /// ----------
    /// ebitda_node : str
    ///     Node reference or DSL formula for EBITDA.
    /// sweep_percentage : float
    ///     Fraction of ECF swept to debt paydown, as a **decimal fraction in
    ///     [0, 1]** (0.5 = 50%), not a percentage.
    /// taxes_node, capex_node, working_capital_node, cash_interest_node : str | None
    ///     Optional node references deducted from EBITDA to compute ECF. When
    ///     ``cash_interest_node`` is omitted the engine deducts contractual
    ///     cash interest from the period's debt-service magnitude instead.
    /// target_instrument_id : str | None
    ///     If set, sweep applies only to this debt instrument id; otherwise
    ///     it applies to all term loans.
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

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: EcfSweepSpec = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid EcfSweepSpec JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize EcfSweepSpec"))
    }

    /// Validate the sweep on its own (the Rust ``EcfSweepSpec::validate``).
    ///
    /// The waterfall-level rule that a positive sweep needs a prepayment
    /// priority is checked by :meth:`WaterfallSpec.validate`.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``sweep_percentage`` is outside ``[0.0, 1.0]`` or not finite.
    #[pyo3(text_signature = "($self)")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(statements_to_py)
    }

    /// Node reference or DSL formula supplying EBITDA, the ECF starting
    /// point.
    ///
    /// Returns
    /// -------
    /// str
    ///     A node id (``"ebitda"``) or an expression over nodes
    ///     (``"revenue - cogs - opex"``), evaluated per period as a monetary
    ///     amount.
    #[getter]
    fn ebitda_node(&self) -> &str {
        &self.inner.ebitda_node
    }

    /// Node deducted as cash taxes, or ``None``.
    #[getter]
    fn taxes_node(&self) -> Option<&str> {
        self.inner.taxes_node.as_deref()
    }

    /// Node deducted as capital expenditure, or ``None``.
    #[getter]
    fn capex_node(&self) -> Option<&str> {
        self.inner.capex_node.as_deref()
    }

    /// Node deducted as the working-capital movement, or ``None``.
    #[getter]
    fn working_capital_node(&self) -> Option<&str> {
        self.inner.working_capital_node.as_deref()
    }

    /// Node deducted as cash interest paid, or ``None`` (the engine then
    /// deducts contractual cash interest from the debt schedule).
    #[getter]
    fn cash_interest_node(&self) -> Option<&str> {
        self.inner.cash_interest_node.as_deref()
    }

    /// Fraction of excess cash flow swept to debt paydown.
    ///
    /// Returns
    /// -------
    /// float
    ///     A **decimal fraction in [0, 1]**, not a percentage — 0.5 means a
    ///     50% sweep. Values outside the unit interval are rejected by
    ///     :meth:`WaterfallSpec.validate`.
    #[getter]
    fn sweep_percentage(&self) -> f64 {
        self.inner.sweep_percentage
    }

    /// Debt instrument the sweep pays down, if the sweep is targeted.
    ///
    /// Returns
    /// -------
    /// str | None
    ///     Instrument id, or ``None`` when the sweep applies to all term
    ///     loans in the capital structure.
    #[getter]
    fn target_instrument_id(&self) -> Option<&str> {
        self.inner.target_instrument_id.as_deref()
    }

    /// Return the representation with the EBITDA source, sweep fraction and
    /// target instrument.
    fn __repr__(&self) -> String {
        format!(
            "EcfSweepSpec(ebitda_node={:?}, sweep_percentage={}, target_instrument_id={})",
            self.inner.ebitda_node,
            self.inner.sweep_percentage,
            self.inner
                .target_instrument_id
                .as_deref()
                .map_or_else(|| "None".to_string(), |id| format!("{id:?}"))
        )
    }
}

/// PIK toggle specification.
///
/// Controls when interest accrues as PIK (added to principal) vs. cash, based
/// on a liquidity metric threshold with optional hysteresis.
#[pyclass(
    name = "PikToggleSpec",
    module = "finstack_quant.statements",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
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
    ///     Node reference or DSL formula for the liquidity signal (a balance
    ///     such as ``"cash_balance"`` or a ratio such as
    ///     ``"ebitda / interest_expense"``).
    /// threshold : float
    ///     PIK triggers when ``metric < threshold``. Expressed in the same
    ///     units as ``liquidity_metric`` — currency amount for a balance,
    ///     unitless for a ratio.
    /// target_instrument_ids : list[str] | None
    ///     Instruments that switch to PIK when the toggle triggers. Must be a
    ///     non-empty list: instrument-level PIK capability is not modeled, so
    ///     ``None`` or an empty list is rejected by :meth:`validate` and
    ///     :meth:`WaterfallSpec.validate` (it does not mean "every
    ///     instrument").
    /// min_periods_in_pik : int
    ///     Hysteresis floor counted in **periods** on the model's own cadence
    ///     (not months): once triggered, PIK stays on for at least this many
    ///     periods. Default 0 lets PIK toggle every period.
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

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a PIK toggle spec from JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: PikToggleSpec = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid PikToggleSpec JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize this PIK toggle spec to JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize PikToggleSpec"))
    }

    /// Validate the toggle on its own (the Rust ``PikToggleSpec::validate``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``target_instrument_ids`` is ``None`` or empty.
    #[pyo3(text_signature = "($self)")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(statements_to_py)
    }

    /// Node reference or DSL formula producing the liquidity signal.
    ///
    /// Returns
    /// -------
    /// str
    ///     A node id (``"cash_balance"``) or an expression
    ///     (``"ebitda / interest_expense"``). Whether the value is monetary
    ///     or a unitless ratio depends on the expression — the threshold must
    ///     use the same units.
    #[getter]
    fn liquidity_metric(&self) -> &str {
        &self.inner.liquidity_metric
    }

    /// Level below which interest accrues as PIK instead of cash.
    ///
    /// Returns
    /// -------
    /// float
    ///     Threshold in the **same units as the liquidity metric** — a
    ///     currency amount when the metric is a balance, a unitless ratio
    ///     when it is a coverage ratio. PIK triggers when
    ///     ``metric < threshold``.
    #[getter]
    fn threshold(&self) -> f64 {
        self.inner.threshold
    }

    /// Instruments the toggle applies to; ``None`` (or an empty list) does
    /// not validate.
    #[getter]
    fn target_instrument_ids(&self) -> Option<Vec<String>> {
        self.inner.target_instrument_ids.clone()
    }

    /// Hysteresis floor: minimum time PIK stays on once triggered.
    ///
    /// Returns
    /// -------
    /// int
    ///     Count in **periods** on the model's own cadence (quarters for a
    ///     quarterly model), not months. ``0`` disables hysteresis, letting
    ///     PIK toggle every period.
    #[getter]
    fn min_periods_in_pik(&self) -> usize {
        self.inner.min_periods_in_pik
    }

    /// Return the representation with metric, threshold and hysteresis.
    fn __repr__(&self) -> String {
        format!(
            "PikToggleSpec(liquidity_metric={:?}, threshold={}, min_periods_in_pik={})",
            self.inner.liquidity_metric, self.inner.threshold, self.inner.min_periods_in_pik
        )
    }
}

/// Seniority class for intra-category waterfall allocation.
///
/// When attached to :class:`WaterfallSpec.payment_classes`, each category
/// walks class rank and allocates pro-rata inside a class before the next
/// class sees remaining cash.
#[pyclass(
    name = "PaymentClassSpec",
    module = "finstack_quant.statements",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyPaymentClassSpec {
    pub(super) inner: PaymentClassSpec,
}

#[pymethods]
impl PyPaymentClassSpec {
    /// Construct a payment class.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Class identifier (for example ``"1L"``). Must be unique within a
    ///     waterfall.
    /// rank : int
    ///     Seniority rank; ``0`` is most senior. Ranks must be unique.
    /// instrument_ids : list[str]
    ///     Debt instrument ids in this class. Each instrument may appear in
    ///     at most one class.
    #[new]
    #[pyo3(text_signature = "(id, rank, instrument_ids)")]
    fn new(id: String, rank: u32, instrument_ids: Vec<String>) -> Self {
        Self {
            inner: PaymentClassSpec {
                id,
                rank,
                instrument_ids,
            },
        }
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: PaymentClassSpec = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid PaymentClassSpec JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize PaymentClassSpec"))
    }

    /// Class identifier (for example ``"1L"``).
    ///
    /// Returns
    /// -------
    /// str
    ///     Unique class id within the enclosing waterfall.
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    /// Seniority rank; ``0`` is most senior.
    ///
    /// Returns
    /// -------
    /// int
    ///     Unique rank used to order classes when allocating a category.
    #[getter]
    fn rank(&self) -> u32 {
        self.inner.rank
    }

    /// Debt instrument ids that belong to this class.
    ///
    /// Returns
    /// -------
    /// list[str]
    ///     Instrument ids allocated together inside this class.
    #[getter]
    fn instrument_ids(&self) -> Vec<String> {
        self.inner.instrument_ids.clone()
    }

    /// Return the representation with id, rank, and instrument ids.
    fn __repr__(&self) -> String {
        format!(
            "PaymentClassSpec(id={:?}, rank={}, instrument_ids={:?})",
            self.inner.id, self.inner.rank, self.inner.instrument_ids
        )
    }
}

/// Waterfall specification for dynamic cash flow allocation.
///
/// Configures payment priority, optional ECF sweep, optional PIK toggle,
/// payment classes, and separate mandatory / voluntary prepay nodes.
/// Call `validate()` before passing to a model builder to surface configuration
/// errors (e.g. `Sweep` ordered after `Equity` when a sweep is configured).
#[pyclass(
    name = "WaterfallSpec",
    module = "finstack_quant.statements",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
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
    ///     Pre-waterfall cash pool node or formula. ``None`` uses ``"cash"``.
    ///     Do not deduct ``cs.interest_expense``, ``cs.principal_payment``, or
    ///     ``cs.fees`` here — the waterfall allocates those.
    /// ecf_sweep : EcfSweepSpec | None
    ///     Optional ECF sweep configuration.
    /// pik_toggle : PikToggleSpec | None
    ///     Optional PIK toggle configuration.
    /// payment_classes : list[PaymentClassSpec] | None
    ///     Intra-category seniority classes. ``None`` or empty is one implicit
    ///     class (pro-rata across all instruments).
    /// mandatory_prepay_node : str | None
    ///     Node or formula sizing the ``mandatory_prepayment`` rung. Required
    ///     when that priority is listed.
    /// voluntary_prepay_node : str | None
    ///     Node or formula sizing the ``voluntary_prepayment`` rung. Required
    ///     when that priority is listed.
    #[new]
    #[pyo3(
        signature = (
            priority_of_payments=None,
            available_cash_node=None,
            ecf_sweep=None,
            pik_toggle=None,
            payment_classes=None,
            mandatory_prepay_node=None,
            voluntary_prepay_node=None,
        ),
        text_signature = "(priority_of_payments=None, available_cash_node=None, ecf_sweep=None, pik_toggle=None, payment_classes=None, mandatory_prepay_node=None, voluntary_prepay_node=None)"
    )]
    fn new(
        priority_of_payments: Option<Vec<String>>,
        available_cash_node: Option<String>,
        ecf_sweep: Option<&PyEcfSweepSpec>,
        pik_toggle: Option<&PyPikToggleSpec>,
        payment_classes: Option<Vec<Bound<'_, PyPaymentClassSpec>>>,
        mandatory_prepay_node: Option<String>,
        voluntary_prepay_node: Option<String>,
    ) -> PyResult<Self> {
        let mut inner = WaterfallSpec::default();
        if let Some(priority) = priority_of_payments {
            inner.priority_of_payments = priority
                .into_iter()
                .map(|s| parse_priority(&s))
                .collect::<PyResult<Vec<_>>>()?;
        }
        if let Some(node) = available_cash_node {
            inner.available_cash_node = node;
        }
        inner.ecf_sweep = ecf_sweep.map(|p| p.inner.clone());
        inner.pik_toggle = pik_toggle.map(|p| p.inner.clone());
        if let Some(classes) = payment_classes {
            inner.payment_classes = classes.iter().map(|c| c.borrow().inner.clone()).collect();
        }
        inner.mandatory_prepay_node = mandatory_prepay_node;
        inner.voluntary_prepay_node = voluntary_prepay_node;
        Ok(Self { inner })
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: WaterfallSpec = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid WaterfallSpec JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize WaterfallSpec"))
    }

    /// Validate the spec against internal consistency rules.
    ///
    /// Raises `ValueError` if the configuration is economically inconsistent
    /// (e.g. `Sweep` ordered after `Equity` while a positive ECF sweep is set).
    #[pyo3(text_signature = "($self)")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(statements_to_py)
    }

    /// Payment priority order, highest priority first.
    ///
    /// Returns
    /// -------
    /// list[str]
    ///     Snake-case priority names in allocation order — cash is applied to
    ///     the first entry before any of the next. Allocation *within* a
    ///     category is pro-rata inside each payment class, walking class rank.
    ///     Empty ``payment_classes`` is one implicit class.
    #[getter]
    fn priority_of_payments(&self) -> PyResult<Vec<String>> {
        self.inner
            .priority_of_payments
            .iter()
            .copied()
            .map(priority_to_str)
            .collect()
    }

    /// Node reference or DSL formula for the cash pool the waterfall may
    /// spend.
    ///
    /// Returns
    /// -------
    /// str
    ///     A node id or expression evaluating to a monetary amount per
    ///     period. This is the **pre-waterfall** cash pool; do not deduct
    ///     ``cs`` debt-service tokens here.
    #[getter]
    fn available_cash_node(&self) -> &str {
        self.inner.available_cash_node.as_str()
    }

    /// Excess-cash-flow sweep configuration, or ``None``.
    #[getter]
    fn ecf_sweep(&self) -> Option<PyEcfSweepSpec> {
        self.inner
            .ecf_sweep
            .clone()
            .map(|inner| PyEcfSweepSpec { inner })
    }

    /// PIK toggle configuration, or ``None``.
    #[getter]
    fn pik_toggle(&self) -> Option<PyPikToggleSpec> {
        self.inner
            .pik_toggle
            .clone()
            .map(|inner| PyPikToggleSpec { inner })
    }

    /// Intra-category seniority classes, empty when one implicit class is used.
    ///
    /// Returns
    /// -------
    /// list[PaymentClassSpec]
    ///     Configured classes. Empty means one implicit class: pro-rata
    ///     across all instruments in each category.
    #[getter]
    fn payment_classes(&self) -> Vec<PyPaymentClassSpec> {
        self.inner
            .payment_classes
            .iter()
            .cloned()
            .map(|inner| PyPaymentClassSpec { inner })
            .collect()
    }

    /// Node or formula sizing the ``mandatory_prepayment`` rung, if set.
    ///
    /// Returns
    /// -------
    /// str | None
    ///     Node id or formula, or ``None`` when that rung is unused.
    #[getter]
    fn mandatory_prepay_node(&self) -> Option<&str> {
        self.inner.mandatory_prepay_node.as_deref()
    }

    /// Node or formula sizing the ``voluntary_prepayment`` rung, if set.
    ///
    /// Returns
    /// -------
    /// str | None
    ///     Node id or formula, or ``None`` when that rung is unused.
    #[getter]
    fn voluntary_prepay_node(&self) -> Option<&str> {
        self.inner.voluntary_prepay_node.as_deref()
    }

    /// Whether an excess-cash-flow sweep is configured.
    #[getter]
    fn has_ecf_sweep(&self) -> bool {
        self.inner.ecf_sweep.is_some()
    }

    /// Whether a PIK toggle is configured.
    #[getter]
    fn has_pik_toggle(&self) -> bool {
        self.inner.pik_toggle.is_some()
    }

    /// Return the representation with priority order and which optional
    /// mechanics are configured.
    fn __repr__(&self) -> String {
        format!(
            "WaterfallSpec(priority={:?}, ecf_sweep={}, pik_toggle={})",
            self.priority_of_payments().unwrap_or_default(),
            if self.inner.ecf_sweep.is_some() {
                "True"
            } else {
                "False"
            },
            if self.inner.pik_toggle.is_some() {
                "True"
            } else {
                "False"
            },
        )
    }
}

/// Aggregated capital-structure cashflows from an evaluation.
///
/// Available as ``StatementResult.cs_cashflows`` after
/// ``Evaluator.evaluate_with_market`` on a model with debt instruments.
/// Holds per-instrument and total breakdowns (cash / PIK interest,
/// principal, fees, debt balance, accrued interest) per period, plus the
/// post-waterfall equity distribution. Amounts are positive debt-service
/// magnitudes in each instrument's own currency; totals are in the
/// reporting currency when one is configured.
#[pyclass(
    name = "CapitalStructureCashflows",
    module = "finstack_quant.statements",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCapitalStructureCashflows {
    pub(crate) inner: CapitalStructureCashflows,
}

/// Append one breakdown as long-format rows.
fn push_breakdown_rows(
    rows: &mut Vec<serde_json::Value>,
    instrument: &str,
    period: &PeriodId,
    breakdown: &CashflowBreakdown,
) {
    let mut push = |flow_type: &str, money: finstack_quant_core::money::Money| {
        rows.push(serde_json::json!({
            "instrument": instrument,
            "period": period.to_string(),
            "flow_type": flow_type,
            "amount": money.amount(),
            "currency": money.currency().to_string(),
        }));
    };
    push("interest_expense_cash", breakdown.interest_expense_cash);
    if let Some(income) = breakdown.interest_income_cash {
        push("interest_income_cash", income);
    }
    push("interest_expense_pik", breakdown.interest_expense_pik);
    push("principal_payment", breakdown.principal_payment);
    push("fees", breakdown.fees);
    push("debt_balance", breakdown.debt_balance);
    push("accrued_interest", breakdown.accrued_interest);
}

#[pymethods]
impl PyCapitalStructureCashflows {
    /// Support `pickle` via the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from canonical JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid CapitalStructureCashflows JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize to canonical JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize CapitalStructureCashflows"))
    }

    /// Instrument identifiers with cashflows, in capital-structure order.
    #[getter]
    fn instrument_ids(&self) -> Vec<String> {
        self.inner.by_instrument.keys().cloned().collect()
    }

    /// Period identifiers covered, in timeline order.
    #[getter]
    fn periods(&self) -> Vec<String> {
        if !self.inner.totals.is_empty() {
            return self.inner.totals.keys().map(ToString::to_string).collect();
        }
        let mut periods: Vec<PeriodId> = self
            .inner
            .by_instrument
            .values()
            .flat_map(|by_period| by_period.keys().copied())
            .collect();
        periods.sort();
        periods.dedup();
        periods.iter().map(ToString::to_string).collect()
    }

    /// ISO-4217 code of the reporting currency used for totals, or ``None``.
    #[getter]
    fn reporting_currency(&self) -> Option<String> {
        self.inner.reporting_currency.map(|c| c.to_string())
    }

    /// Total interest expense (cash + PIK) for one instrument and period.
    ///
    /// Returns
    /// -------
    /// float
    ///     Amount in the instrument's currency.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If the instrument or period is unknown.
    /// ValueError
    ///     If ``period`` is not a valid period id.
    #[pyo3(text_signature = "($self, instrument_id, period)")]
    fn get_interest(&self, instrument_id: &str, period: &str) -> PyResult<f64> {
        let pid = super::parse_period_id(period)?;
        self.inner
            .get_interest(instrument_id, &pid)
            .map_err(statements_to_py)
    }

    /// Cash interest expense for one instrument and period.
    #[pyo3(text_signature = "($self, instrument_id, period)")]
    fn get_interest_cash(&self, instrument_id: &str, period: &str) -> PyResult<f64> {
        let pid = super::parse_period_id(period)?;
        self.inner
            .get_interest_cash(instrument_id, &pid)
            .map_err(statements_to_py)
    }

    /// PIK (non-cash) interest accrued for one instrument and period.
    #[pyo3(text_signature = "($self, instrument_id, period)")]
    fn get_interest_pik(&self, instrument_id: &str, period: &str) -> PyResult<f64> {
        let pid = super::parse_period_id(period)?;
        self.inner
            .get_interest_pik(instrument_id, &pid)
            .map_err(statements_to_py)
    }

    /// Principal repaid for one instrument and period.
    #[pyo3(text_signature = "($self, instrument_id, period)")]
    fn get_principal(&self, instrument_id: &str, period: &str) -> PyResult<f64> {
        let pid = super::parse_period_id(period)?;
        self.inner
            .get_principal(instrument_id, &pid)
            .map_err(statements_to_py)
    }

    /// Outstanding debt balance at period end for one instrument.
    #[pyo3(text_signature = "($self, instrument_id, period)")]
    fn get_debt_balance(&self, instrument_id: &str, period: &str) -> PyResult<f64> {
        let pid = super::parse_period_id(period)?;
        self.inner
            .get_debt_balance(instrument_id, &pid)
            .map_err(statements_to_py)
    }

    /// Fees paid for one instrument and period.
    #[pyo3(text_signature = "($self, instrument_id, period)")]
    fn get_fees(&self, instrument_id: &str, period: &str) -> PyResult<f64> {
        let pid = super::parse_period_id(period)?;
        self.inner
            .get_fees(instrument_id, &pid)
            .map_err(statements_to_py)
    }

    /// Accrued, unpaid interest at period end for one instrument.
    #[pyo3(text_signature = "($self, instrument_id, period)")]
    fn get_accrued_interest(&self, instrument_id: &str, period: &str) -> PyResult<f64> {
        let pid = super::parse_period_id(period)?;
        self.inner
            .get_accrued_interest(instrument_id, &pid)
            .map_err(statements_to_py)
    }

    /// Total interest expense (cash + PIK) across instruments for a period,
    /// in the reporting currency.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If the period has no totals.
    #[pyo3(text_signature = "($self, period)")]
    fn get_total_interest(&self, period: &str) -> PyResult<f64> {
        let pid = super::parse_period_id(period)?;
        self.inner
            .get_total_interest(&pid)
            .map_err(statements_to_py)
    }

    /// Total principal repaid across instruments for a period.
    #[pyo3(text_signature = "($self, period)")]
    fn get_total_principal(&self, period: &str) -> PyResult<f64> {
        let pid = super::parse_period_id(period)?;
        self.inner
            .get_total_principal(&pid)
            .map_err(statements_to_py)
    }

    /// Total debt balance across instruments at period end.
    #[pyo3(text_signature = "($self, period)")]
    fn get_total_debt_balance(&self, period: &str) -> PyResult<f64> {
        let pid = super::parse_period_id(period)?;
        self.inner
            .get_total_debt_balance(&pid)
            .map_err(statements_to_py)
    }

    /// Total fees across instruments for a period.
    #[pyo3(text_signature = "($self, period)")]
    fn get_total_fees(&self, period: &str) -> PyResult<f64> {
        let pid = super::parse_period_id(period)?;
        self.inner.get_total_fees(&pid).map_err(statements_to_py)
    }

    /// Post-waterfall residual cash distributed to equity per period.
    ///
    /// Returns
    /// -------
    /// dict[str, Money]
    ///     Period id to ``Money``; empty unless a waterfall with
    ///     ``available_cash_node`` ran.
    #[getter]
    fn equity_distribution<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (period, money) in &self.inner.equity_distribution {
            dict.set_item(period.to_string(), PyMoney { inner: *money })?;
        }
        Ok(dict)
    }

    /// Export per-instrument cashflows as a long pandas ``DataFrame``.
    ///
    /// Columns: ``instrument``, ``period`` (period id string),
    /// ``flow_type`` (``interest_expense_cash``, ``interest_income_cash``
    /// when present, ``interest_expense_pik``, ``principal_payment``,
    /// ``fees``, ``debt_balance``, ``accrued_interest``), ``amount``
    /// (float64, positive debt-service magnitude) and ``currency`` (ISO
    /// code of that instrument). Rows follow instrument then period order.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut rows = Vec::new();
        for (instrument, by_period) in &self.inner.by_instrument {
            for (period, breakdown) in by_period {
                push_breakdown_rows(&mut rows, instrument, period, breakdown);
            }
        }
        serde_rows_to_dataframe_with_schema(py, &rows, &CASHFLOW_COLUMNS)
    }

    /// Export the cross-instrument totals as a long pandas ``DataFrame``.
    ///
    /// Same columns as :meth:`to_dataframe`; ``instrument`` is
    /// ``"__total__"`` and ``currency`` is the reporting currency. Empty
    /// when no reporting currency is configured.
    #[pyo3(text_signature = "($self)")]
    fn to_totals_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut rows = Vec::new();
        for (period, breakdown) in &self.inner.totals {
            push_breakdown_rows(&mut rows, "__total__", period, breakdown);
        }
        serde_rows_to_dataframe_with_schema(py, &rows, &CASHFLOW_COLUMNS)
    }

    /// Return ``CapitalStructureCashflows(instruments=2, periods=4)``.
    fn __repr__(&self) -> String {
        format!(
            "CapitalStructureCashflows(instruments={}, periods={}, reporting_currency={})",
            self.inner.by_instrument.len(),
            self.periods().len(),
            self.inner
                .reporting_currency
                .map_or_else(|| "None".to_string(), |c| format!("{:?}", c.to_string()))
        )
    }
}

/// Register capital-structure classes.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyEcfSweepSpec>()?;
    m.add_class::<PyPaymentClassSpec>()?;
    m.add_class::<PyPikToggleSpec>()?;
    m.add_class::<PyWaterfallSpec>()?;
    m.add_class::<PyCapitalStructureCashflows>()?;
    Ok(())
}
