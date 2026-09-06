//! Typed covenant definition wrappers: `CovenantType`, `CovenantConsequence`,
//! `SpringingCondition`, `Covenant`, `ThresholdSchedule`, `CovenantWaiver` and
//! `CovenantSpec`.
//!
//! Every wrapper is a frozen value object: fluent `with_*` setters return a
//! modified copy, mirroring the consuming Rust builders.

use crate::bindings::core::dates::tenor::{extract_tenor, PyTenor};
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::{core_to_py, display_to_py, value_error};
use finstack_quant_covenants::{
    Covenant, CovenantConsequence, CovenantScope, CovenantSpec, CovenantType, CovenantWaiver,
    SpringingCondition, ThresholdSchedule, ThresholdTest,
};
use pyo3::prelude::*;

/// Parse a `"maximum"` / `"minimum"` test direction plus bound value.
fn parse_threshold_test(test: &str, value: f64) -> PyResult<ThresholdTest> {
    match test {
        "maximum" => Ok(ThresholdTest::Maximum(value)),
        "minimum" => Ok(ThresholdTest::Minimum(value)),
        other => Err(value_error(format!(
            "test must be \"maximum\" or \"minimum\", got {other:?}"
        ))),
    }
}

fn threshold_test_parts(test: ThresholdTest) -> (&'static str, f64) {
    match test {
        ThresholdTest::Maximum(v) => ("maximum", v),
        ThresholdTest::Minimum(v) => ("minimum", v),
    }
}

pub(crate) fn scope_name(scope: &CovenantScope) -> &'static str {
    match scope {
        CovenantScope::Maintenance => "maintenance",
        CovenantScope::Incurrence => "incurrence",
    }
}

pub(crate) fn parse_scope(scope: &str) -> PyResult<CovenantScope> {
    match scope {
        "maintenance" => Ok(CovenantScope::Maintenance),
        "incurrence" => Ok(CovenantScope::Incurrence),
        _ => Err(value_error("scope must be maintenance or incurrence")),
    }
}

/// Type of financial or operational covenant with its static threshold.
///
/// Build one with the classmethod matching the Rust variant, for example
/// ``CovenantType.max_debt_to_ebitda(4.5)`` (ratios are in turns: ``4.5``
/// means 4.5x) or ``CovenantType.custom("ltv", "maximum", 0.75)`` for a
/// caller-defined metric with a decimal bound. Amount covenants
/// (``max_capex``, ``min_liquidity``, ``basket``) carry no currency; keep the
/// metric and the threshold in the same reporting currency.
#[pyclass(
    name = "CovenantType",
    module = "finstack_quant.covenants",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyCovenantType {
    pub(crate) inner: CovenantType,
}

impl PyCovenantType {
    pub(crate) fn from_inner(inner: CovenantType) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCovenantType {
    /// Maximum debt-to-EBITDA ratio (gross leverage), ``threshold`` in turns.
    #[staticmethod]
    #[pyo3(text_signature = "(threshold)")]
    fn max_debt_to_ebitda(threshold: f64) -> Self {
        Self::from_inner(CovenantType::MaxDebtToEbitda { threshold })
    }

    /// Minimum interest coverage ratio (EBIT / interest), ``threshold`` in turns.
    #[staticmethod]
    #[pyo3(text_signature = "(threshold)")]
    fn min_interest_coverage(threshold: f64) -> Self {
        Self::from_inner(CovenantType::MinInterestCoverage { threshold })
    }

    /// Minimum fixed-charge coverage ratio, ``threshold`` in turns.
    #[staticmethod]
    #[pyo3(text_signature = "(threshold)")]
    fn min_fixed_charge_coverage(threshold: f64) -> Self {
        Self::from_inner(CovenantType::MinFixedChargeCoverage { threshold })
    }

    /// Maximum total leverage ratio, ``threshold`` in turns.
    #[staticmethod]
    #[pyo3(text_signature = "(threshold)")]
    fn max_total_leverage(threshold: f64) -> Self {
        Self::from_inner(CovenantType::MaxTotalLeverage { threshold })
    }

    /// Maximum senior leverage ratio, ``threshold`` in turns.
    #[staticmethod]
    #[pyo3(text_signature = "(threshold)")]
    fn max_senior_leverage(threshold: f64) -> Self {
        Self::from_inner(CovenantType::MaxSeniorLeverage { threshold })
    }

    /// Minimum asset coverage ratio, ``threshold`` in turns.
    #[staticmethod]
    #[pyo3(text_signature = "(threshold)")]
    fn min_asset_coverage(threshold: f64) -> Self {
        Self::from_inner(CovenantType::MinAssetCoverage { threshold })
    }

    /// Negative covenant (a prohibition described by ``restriction``); never
    /// tested numerically.
    #[staticmethod]
    #[pyo3(text_signature = "(restriction)")]
    fn negative(restriction: String) -> Self {
        Self::from_inner(CovenantType::Negative { restriction })
    }

    /// Affirmative covenant (a requirement described by ``requirement``);
    /// never tested numerically.
    #[staticmethod]
    #[pyo3(text_signature = "(requirement)")]
    fn affirmative(requirement: String) -> Self {
        Self::from_inner(CovenantType::Affirmative { requirement })
    }

    /// Custom covenant testing ``metric`` against ``value`` with ``test``
    /// ``"maximum"`` (pass when metric <= value) or ``"minimum"`` (pass when
    /// metric >= value).
    ///
    /// Raises ``ValueError`` when ``test`` is neither ``"maximum"`` nor
    /// ``"minimum"``.
    #[staticmethod]
    #[pyo3(text_signature = "(metric, test, value)")]
    fn custom(metric: String, test: &str, value: f64) -> PyResult<Self> {
        Ok(Self::from_inner(CovenantType::Custom {
            metric,
            test: parse_threshold_test(test, value)?,
        }))
    }

    /// Basket covenant: utilization of basket ``name`` must stay at or below
    /// ``limit`` (a reporting-currency amount).
    #[staticmethod]
    #[pyo3(text_signature = "(name, limit)")]
    fn basket(name: String, limit: f64) -> Self {
        Self::from_inner(CovenantType::Basket { name, limit })
    }

    /// Minimum debt service coverage ratio (EBITDA / debt service),
    /// ``threshold`` in turns.
    #[staticmethod]
    #[pyo3(text_signature = "(threshold)")]
    fn min_dscr(threshold: f64) -> Self {
        Self::from_inner(CovenantType::MinDscr { threshold })
    }

    /// Maximum net debt-to-EBITDA ratio (net of cash), ``threshold`` in turns.
    #[staticmethod]
    #[pyo3(text_signature = "(threshold)")]
    fn max_net_debt_to_ebitda(threshold: f64) -> Self {
        Self::from_inner(CovenantType::MaxNetDebtToEbitda { threshold })
    }

    /// Maximum capital expenditure, ``threshold`` as a reporting-currency amount.
    #[staticmethod]
    #[pyo3(text_signature = "(threshold)")]
    fn max_capex(threshold: f64) -> Self {
        Self::from_inner(CovenantType::MaxCapex { threshold })
    }

    /// Minimum liquidity (cash + available revolver), ``threshold`` as a
    /// reporting-currency amount.
    #[staticmethod]
    #[pyo3(text_signature = "(threshold)")]
    fn min_liquidity(threshold: f64) -> Self {
        Self::from_inner(CovenantType::MinLiquidity { threshold })
    }

    /// Deserialize from the externally-tagged JSON form, e.g.
    /// ``{"max_debt_to_ebitda": {"threshold": 4.5}}``.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(display_to_py)
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support ``pickle`` via the JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Stable variant identifier (``"max_debt_ebitda"``, ``"min_dscr"``,
    /// ``"custom"``, ...); thresholds are not part of it.
    #[getter]
    fn covenant_id(&self) -> &'static str {
        self.inner.covenant_id()
    }

    /// Static threshold or limit, or ``None`` for negative / affirmative
    /// covenants.
    #[getter]
    fn threshold(&self) -> Option<f64> {
        self.inner.threshold_value()
    }

    /// Inequality direction: ``"at_most"``, ``"at_least"``, or ``None`` for
    /// non-numeric covenants.
    #[getter]
    fn bound_kind(&self) -> Option<&'static str> {
        self.inner.bound_kind().map(|kind| match kind {
            finstack_quant_covenants::BoundKind::AtMost => "at_most",
            finstack_quant_covenants::BoundKind::AtLeast => "at_least",
        })
    }

    /// Human-readable description, e.g. ``"Debt/EBITDA <= 4.50x"``.
    #[getter]
    fn description(&self) -> String {
        self.inner.to_string()
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        match self.inner.threshold_value() {
            Some(threshold) => format!(
                "CovenantType(covenant_id={:?}, threshold={threshold})",
                self.inner.covenant_id()
            ),
            None => format!(
                "CovenantType(covenant_id={:?}, description={:?})",
                self.inner.covenant_id(),
                self.inner.to_string()
            ),
        }
    }
}

/// Consequence applied when a covenant is breached and its cure period
/// elapses.
///
/// Build with the classmethod matching the Rust variant: ``default()``,
/// ``rate_increase(bp_increase)``, ``cash_sweep(sweep_percentage)``,
/// ``block_distributions()``, ``require_collateral(description)`` or
/// ``accelerate_maturity(new_maturity)``.
#[pyclass(
    name = "CovenantConsequence",
    module = "finstack_quant.covenants",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyCovenantConsequence {
    pub(crate) inner: CovenantConsequence,
}

impl PyCovenantConsequence {
    pub(crate) fn from_inner(inner: CovenantConsequence) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCovenantConsequence {
    /// Event of default.
    #[staticmethod]
    #[pyo3(name = "default")]
    fn event_of_default() -> Self {
        Self::from_inner(CovenantConsequence::Default)
    }

    /// Interest margin step-up of ``bp_increase`` basis points.
    #[staticmethod]
    #[pyo3(text_signature = "(bp_increase)")]
    fn rate_increase(bp_increase: f64) -> Self {
        Self::from_inner(CovenantConsequence::RateIncrease { bp_increase })
    }

    /// Mandatory sweep of ``sweep_percentage`` (decimal fraction, ``1.0`` =
    /// 100%) of excess cash flow.
    #[staticmethod]
    #[pyo3(text_signature = "(sweep_percentage)")]
    fn cash_sweep(sweep_percentage: f64) -> Self {
        Self::from_inner(CovenantConsequence::CashSweep { sweep_percentage })
    }

    /// Block distributions to equity holders.
    #[staticmethod]
    fn block_distributions() -> Self {
        Self::from_inner(CovenantConsequence::BlockDistributions)
    }

    /// Require additional collateral described by ``description``.
    #[staticmethod]
    #[pyo3(text_signature = "(description)")]
    fn require_collateral(description: String) -> Self {
        Self::from_inner(CovenantConsequence::RequireCollateral { description })
    }

    /// Accelerate the loan maturity to ``new_maturity`` (``datetime.date``,
    /// ``pandas.Timestamp`` or ISO-8601 string).
    #[staticmethod]
    #[pyo3(text_signature = "(new_maturity)")]
    fn accelerate_maturity(new_maturity: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self::from_inner(CovenantConsequence::AccelerateMaturity {
            new_maturity: extract_date(new_maturity)?,
        }))
    }

    /// Deserialize from JSON (``"default"``, ``{"rate_increase": {"bp_increase": 200.0}}``, ...).
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(display_to_py)
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support ``pickle`` via the JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Variant name in snake_case: ``"default"``, ``"rate_increase"``,
    /// ``"cash_sweep"``, ``"block_distributions"``, ``"require_collateral"``
    /// or ``"accelerate_maturity"``.
    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            CovenantConsequence::Default => "default",
            CovenantConsequence::RateIncrease { .. } => "rate_increase",
            CovenantConsequence::CashSweep { .. } => "cash_sweep",
            CovenantConsequence::BlockDistributions => "block_distributions",
            CovenantConsequence::RequireCollateral { .. } => "require_collateral",
            CovenantConsequence::AccelerateMaturity { .. } => "accelerate_maturity",
        }
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            CovenantConsequence::Default => "CovenantConsequence(kind=\"default\")".to_string(),
            CovenantConsequence::BlockDistributions => {
                "CovenantConsequence(kind=\"block_distributions\")".to_string()
            }
            CovenantConsequence::RateIncrease { bp_increase } => {
                format!("CovenantConsequence(kind=\"rate_increase\", bp_increase={bp_increase})")
            }
            CovenantConsequence::CashSweep { sweep_percentage } => format!(
                "CovenantConsequence(kind=\"cash_sweep\", sweep_percentage={sweep_percentage})"
            ),
            CovenantConsequence::RequireCollateral { description } => format!(
                "CovenantConsequence(kind=\"require_collateral\", description={description:?})"
            ),
            CovenantConsequence::AccelerateMaturity { new_maturity } => format!(
                "CovenantConsequence(kind=\"accelerate_maturity\", new_maturity=\"{new_maturity}\")"
            ),
        }
    }
}

/// Activation condition for a springing covenant.
///
/// The covenant is tested only while ``metric_id`` satisfies the condition,
/// e.g. ``SpringingCondition("revolver_utilization", "minimum", 0.30)``
/// activates once utilization reaches 30%. While the condition is unmet the
/// covenant reports a pass with an explanatory ``details``.
#[pyclass(
    name = "SpringingCondition",
    module = "finstack_quant.covenants",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PySpringingCondition {
    pub(crate) inner: SpringingCondition,
}

impl PySpringingCondition {
    pub(crate) fn from_inner(inner: SpringingCondition) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PySpringingCondition {
    /// Create a springing condition on ``metric_id`` with ``test``
    /// ``"maximum"`` or ``"minimum"`` against ``value``.
    ///
    /// Raises ``ValueError`` when ``test`` is not ``"maximum"`` / ``"minimum"``.
    #[new]
    #[pyo3(text_signature = "(metric_id, test, value)")]
    fn new(metric_id: String, test: &str, value: f64) -> PyResult<Self> {
        Ok(Self::from_inner(SpringingCondition {
            metric_id: metric_id.into(),
            test: parse_threshold_test(test, value)?,
        }))
    }

    /// Deserialize from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(display_to_py)
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support ``pickle`` via the JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Metric that controls activation.
    #[getter]
    fn metric_id(&self) -> &str {
        self.inner.metric_id.as_str()
    }

    /// Test direction: ``"maximum"`` or ``"minimum"``.
    #[getter]
    fn test(&self) -> &'static str {
        threshold_test_parts(self.inner.test).0
    }

    /// Bound the activation metric is compared against.
    #[getter]
    fn value(&self) -> f64 {
        threshold_test_parts(self.inner.test).1
    }

    fn __repr__(&self) -> String {
        let (test, value) = threshold_test_parts(self.inner.test);
        format!(
            "SpringingCondition(metric_id={:?}, test={test:?}, value={value})",
            self.inner.metric_id.as_str()
        )
    }
}

/// Financial covenant with test frequency, cure period, consequences, scope
/// and optional springing condition.
///
/// ``label`` is the covenant's identity: reports, breaches and waivers key
/// off it, so two covenants of the same type must carry distinct labels.
/// ``test_frequency`` is descriptive metadata only; the engine tests whenever
/// you call ``evaluate``. Defaults: 30-day cure period, no consequences,
/// active, maintenance scope, no springing condition.
#[pyclass(
    name = "Covenant",
    module = "finstack_quant.covenants",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyCovenant {
    pub(crate) inner: Covenant,
}

impl PyCovenant {
    pub(crate) fn from_inner(inner: Covenant) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCovenant {
    /// Create a covenant of ``covenant_type`` tested every ``test_frequency``
    /// (a ``Tenor`` or string such as ``"3M"``) under instance ``label``.
    ///
    /// Raises ``ValueError`` when the tenor string cannot be parsed and
    /// ``TypeError`` when ``test_frequency`` is neither a ``Tenor`` nor a
    /// string.
    #[new]
    #[pyo3(text_signature = "(covenant_type, test_frequency, label)")]
    fn new(
        covenant_type: PyRef<'_, PyCovenantType>,
        test_frequency: &Bound<'_, PyAny>,
        label: String,
    ) -> PyResult<Self> {
        Ok(Self::from_inner(Covenant::new(
            covenant_type.inner.clone(),
            extract_tenor(test_frequency)?,
            label,
        )))
    }

    /// Return a copy with cure period ``days`` (``None`` removes the cure
    /// period so a breach is immediate).
    #[pyo3(signature = (days), text_signature = "(days)")]
    fn with_cure_period(&self, days: Option<i32>) -> Self {
        Self::from_inner(self.inner.clone().with_cure_period(days))
    }

    /// Return a copy with ``consequence`` appended to the breach consequences.
    #[pyo3(text_signature = "(consequence)")]
    fn with_consequence(&self, consequence: PyRef<'_, PyCovenantConsequence>) -> Self {
        Self::from_inner(
            self.inner
                .clone()
                .with_consequence(consequence.inner.clone()),
        )
    }

    /// Return a copy with ``scope`` ``"maintenance"`` (tested on a schedule)
    /// or ``"incurrence"`` (tested on specific actions).
    ///
    /// Raises ``ValueError`` for any other scope string.
    #[pyo3(text_signature = "(scope)")]
    fn with_scope(&self, scope: &str) -> PyResult<Self> {
        let scope = match scope {
            "maintenance" => CovenantScope::Maintenance,
            "incurrence" => CovenantScope::Incurrence,
            other => {
                return Err(value_error(format!(
                    "scope must be \"maintenance\" or \"incurrence\", got {other:?}"
                )))
            }
        };
        Ok(Self::from_inner(self.inner.clone().with_scope(scope)))
    }

    /// Return a copy activated only while ``condition`` is met.
    #[pyo3(text_signature = "(condition)")]
    fn with_springing_condition(&self, condition: PyRef<'_, PySpringingCondition>) -> Self {
        Self::from_inner(
            self.inner
                .clone()
                .with_springing_condition(condition.inner.clone()),
        )
    }

    /// Deserialize from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(display_to_py)
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support ``pickle`` via the JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Covenant type and static threshold.
    #[getter]
    fn covenant_type(&self) -> PyCovenantType {
        PyCovenantType::from_inner(self.inner.covenant_type.clone())
    }

    /// Descriptive test frequency.
    #[getter]
    fn test_frequency(&self) -> PyTenor {
        PyTenor::from_inner(self.inner.test_frequency)
    }

    /// Cure period in days, or ``None`` when a breach is immediate.
    #[getter]
    fn cure_period_days(&self) -> Option<i32> {
        self.inner.cure_period_days
    }

    /// Consequences applied after an uncured breach.
    #[getter]
    fn consequences(&self) -> Vec<PyCovenantConsequence> {
        self.inner
            .consequences
            .iter()
            .cloned()
            .map(PyCovenantConsequence::from_inner)
            .collect()
    }

    /// Whether the covenant is active (inactive covenants report a pass).
    #[getter]
    fn is_active(&self) -> bool {
        self.inner.is_active
    }

    /// ``"maintenance"`` or ``"incurrence"``.
    #[getter]
    fn scope(&self) -> &'static str {
        scope_name(&self.inner.scope)
    }

    /// Activation condition, or ``None`` for an always-on covenant.
    #[getter]
    fn springing_condition(&self) -> Option<PySpringingCondition> {
        self.inner
            .springing_condition
            .clone()
            .map(PySpringingCondition::from_inner)
    }

    /// Instance label: the key under which reports and breaches are returned.
    #[getter]
    fn label(&self) -> &str {
        &self.inner.label
    }

    /// Human-readable description of the covenant type.
    #[getter]
    fn description(&self) -> String {
        self.inner.description()
    }

    fn __repr__(&self) -> String {
        format!(
            "Covenant(label={:?}, covenant_type={:?}, test_frequency=\"{}\", cure_period_days={}, scope={:?}, is_active={})",
            self.inner.label,
            self.inner.description(),
            self.inner.test_frequency,
            self.inner
                .cure_period_days
                .map_or("None".to_string(), |d| d.to_string()),
            scope_name(&self.inner.scope),
            if self.inner.is_active { "True" } else { "False" },
        )
    }
}

/// Piecewise-constant threshold step-down schedule.
///
/// Each entry is ``(effective_date, threshold)``; the threshold in force on a
/// test date is the last entry effective on or before it. Attached to a
/// ``CovenantSpec`` it overrides the covenant's static threshold from the
/// first effective date onward.
#[pyclass(
    name = "ThresholdSchedule",
    module = "finstack_quant.covenants",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyThresholdSchedule {
    pub(crate) inner: ThresholdSchedule,
}

impl PyThresholdSchedule {
    pub(crate) fn from_inner(inner: ThresholdSchedule) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyThresholdSchedule {
    /// Create a schedule from ``entries`` of ``(date, threshold)`` pairs
    /// (dates as ``datetime.date``, ``pandas.Timestamp`` or ISO strings, in
    /// any order).
    ///
    /// Raises ``ValueError`` when a threshold is non-finite or two entries
    /// share an effective date.
    #[new]
    #[pyo3(text_signature = "(entries)")]
    fn new(entries: Vec<(Bound<'_, PyAny>, f64)>) -> PyResult<Self> {
        let entries = entries
            .iter()
            .map(|(date, value)| Ok((extract_date(date)?, *value)))
            .collect::<PyResult<Vec<_>>>()?;
        ThresholdSchedule::new(entries)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Deserialize from the JSON array form ``[["2026-01-01", 6.5], ...]``.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(display_to_py)
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support ``pickle`` via the JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Entries as ``(datetime.date, threshold)`` pairs in ascending date order.
    #[getter]
    fn entries<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, f64)>> {
        self.inner
            .entries()
            .iter()
            .map(|(date, value)| Ok((date_to_py(py, *date)?, *value)))
            .collect()
    }

    /// Threshold in force on ``test_date``, or ``None`` before the first
    /// effective date.
    #[pyo3(text_signature = "(test_date)")]
    fn threshold_for(&self, test_date: &Bound<'_, PyAny>) -> PyResult<Option<f64>> {
        Ok(self.inner.threshold_for(extract_date(test_date)?))
    }

    fn __len__(&self) -> usize {
        self.inner.entries().len()
    }

    fn __repr__(&self) -> String {
        let entries: Vec<String> = self
            .inner
            .entries()
            .iter()
            .map(|(date, value)| format!("(\"{date}\", {value})"))
            .collect();
        format!("ThresholdSchedule([{}])", entries.join(", "))
    }
}

/// Lender waiver or amendment for one covenant instance.
///
/// With ``amended_threshold=None`` the covenant is fully waived (reports a
/// pass) between ``effective_date`` and ``expiry_date``; with a threshold it
/// is an amendment and the covenant is tested against the amended value
/// instead. ``expiry_date=None`` makes the waiver permanent.
#[pyclass(
    name = "CovenantWaiver",
    module = "finstack_quant.covenants",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyCovenantWaiver {
    pub(crate) inner: CovenantWaiver,
}

impl PyCovenantWaiver {
    pub(crate) fn from_inner(inner: CovenantWaiver) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCovenantWaiver {
    /// Create a waiver for the covenant whose ``Covenant.label`` is
    /// ``covenant_id``.
    #[new]
    #[pyo3(
        signature = (covenant_id, effective_date, expiry_date=None, amended_threshold=None, description=String::new()),
        text_signature = "(covenant_id, effective_date, expiry_date=None, amended_threshold=None, description=\"\")"
    )]
    fn new(
        covenant_id: String,
        effective_date: &Bound<'_, PyAny>,
        expiry_date: Option<&Bound<'_, PyAny>>,
        amended_threshold: Option<f64>,
        description: String,
    ) -> PyResult<Self> {
        Ok(Self::from_inner(CovenantWaiver {
            covenant_id,
            effective_date: extract_date(effective_date)?,
            expiry_date: expiry_date.map(extract_date).transpose()?,
            amended_threshold,
            description,
        }))
    }

    /// Deserialize from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(display_to_py)
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support ``pickle`` via the JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Instance label of the waived covenant.
    #[getter]
    fn covenant_id(&self) -> &str {
        &self.inner.covenant_id
    }

    /// First date the waiver applies.
    #[getter]
    fn effective_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.effective_date)
    }

    /// Last date the waiver applies, or ``None`` for a permanent amendment.
    #[getter]
    fn expiry_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .expiry_date
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    /// Amended threshold, or ``None`` for a full waiver.
    #[getter]
    fn amended_threshold(&self) -> Option<f64> {
        self.inner.amended_threshold
    }

    /// Free-text description of the waiver terms.
    #[getter]
    fn description(&self) -> &str {
        &self.inner.description
    }

    fn __repr__(&self) -> String {
        repr_from_serde("CovenantWaiver", &self.inner)
    }
}

/// A covenant paired with the metric it is tested against and an optional
/// threshold step-down schedule.
///
/// ``metric_id`` names the key looked up in the metrics mapping passed to
/// ``CovenantEngine.evaluate``; when ``None`` the engine falls back to the
/// covenant type's conventional metric name (``debt_to_ebitda``, ``dscr``,
/// ...), or the ``metric`` of a custom covenant.
#[pyclass(
    name = "CovenantSpec",
    module = "finstack_quant.covenants",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyCovenantSpec {
    pub(crate) inner: CovenantSpec,
}

impl PyCovenantSpec {
    pub(crate) fn from_inner(inner: CovenantSpec) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCovenantSpec {
    /// Pair ``covenant`` with the ``metric_id`` it is evaluated on.
    #[new]
    #[pyo3(signature = (covenant, metric_id=None), text_signature = "(covenant, metric_id=None)")]
    fn new(covenant: PyRef<'_, PyCovenant>, metric_id: Option<String>) -> Self {
        let inner = match metric_id {
            Some(metric_id) => CovenantSpec::with_metric(covenant.inner.clone(), metric_id),
            None => CovenantSpec {
                covenant: covenant.inner.clone(),
                metric_id: None,
                denominator_metric_id: matches!(
                    covenant.inner.covenant_type,
                    CovenantType::MaxNetDebtToEbitda { .. }
                )
                .then(|| "ebitda".into()),
                threshold_schedule: None,
            },
        };
        Self::from_inner(inner)
    }

    /// Return a copy selecting the finite earnings denominator for a leverage ratio.
    ///
    /// ``metric_id`` identifies earnings for the same period as the ratio;
    /// non-positive earnings produce a breach with no meaningful headroom.
    #[pyo3(text_signature = "(metric_id)")]
    fn with_denominator_metric(&self, metric_id: String) -> Self {
        Self::from_inner(self.inner.clone().with_denominator_metric(metric_id))
    }

    /// Earnings denominator metric id; net-debt constructors default to ``ebitda``.
    #[getter]
    fn denominator_metric_id(&self) -> Option<&str> {
        self.inner
            .denominator_metric_id
            .as_ref()
            .map(|id| id.as_str())
    }

    /// Return a copy whose threshold follows ``schedule`` (step-downs) instead
    /// of the covenant's static threshold.
    #[pyo3(text_signature = "(schedule)")]
    fn with_threshold_schedule(&self, schedule: PyRef<'_, PyThresholdSchedule>) -> Self {
        Self::from_inner(
            self.inner
                .clone()
                .with_threshold_schedule(schedule.inner.clone()),
        )
    }

    /// Deserialize from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(display_to_py)
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support ``pickle`` via the JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// The covenant being evaluated.
    #[getter]
    fn covenant(&self) -> PyCovenant {
        PyCovenant::from_inner(self.inner.covenant.clone())
    }

    /// Metric key looked up at evaluation, or ``None`` for the type default.
    #[getter]
    fn metric_id(&self) -> Option<&str> {
        self.inner.metric_id.as_ref().map(|id| id.as_str())
    }

    /// Step-down schedule, or ``None`` when the static threshold applies.
    #[getter]
    fn threshold_schedule(&self) -> Option<PyThresholdSchedule> {
        self.inner
            .threshold_schedule
            .clone()
            .map(PyThresholdSchedule::from_inner)
    }

    fn __repr__(&self) -> String {
        format!(
            "CovenantSpec(label={:?}, covenant_type={:?}, metric_id={}, threshold_schedule={})",
            self.inner.covenant.label,
            self.inner.covenant.description(),
            self.inner
                .metric_id
                .as_ref()
                .map_or("None".to_string(), |id| format!("{:?}", id.as_str())),
            self.inner
                .threshold_schedule
                .as_ref()
                .map_or("None".to_string(), |s| format!(
                    "<{} entries>",
                    s.entries().len()
                )),
        )
    }
}
