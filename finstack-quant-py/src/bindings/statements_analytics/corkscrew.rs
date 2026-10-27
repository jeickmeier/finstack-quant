//! Python bindings for the corkscrew (roll-forward / articulation) extension.
//!
//! Wraps [`finstack_quant_statements_analytics::extensions`] types:
//!
//! - [`PyAccountType`] — asset / liability / equity classifier (serialized as the snake_case rust enum).
//! - [`PyCorkscrewAccount`] — single account definition (balance node + change nodes).
//! - [`PyCorkscrewConfig`] — extension configuration (accounts, tolerance, fail-on-error).
//! - [`PyCorkscrewExtension`] — execution entry point against a model + statement results.
//! - [`PyCorkscrewReport`] — validation report (status, message, structured data, warnings, errors).

use crate::bindings::extract::{extract_model_ref, extract_results_ref};
use crate::bindings::macros::{impl_repr_html_via_dataframe, wire_methods};
use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_rows_to_dataframe_with_schema,
    ColumnSchema,
};
use crate::bindings::pydoc::pydoc;
use crate::errors::display_to_py;
use finstack_quant_statements_analytics::extensions as rust_corkscrew;
use pyo3::prelude::*;

/// Column schema for [`PyCorkscrewReport::to_validations_dataframe`].
const VALIDATION_COLUMNS: [ColumnSchema<'static>; 5] = [
    ("account", "str"),
    ("type", "str"),
    ("periods_validated", "int64"),
    ("max_error", "float64"),
    ("is_valid", "bool"),
];

/// Account type label: ``"asset"``, ``"liability"``, or ``"equity"``.
#[pyclass(
    name = "AccountType",
    module = "finstack_quant.statements_analytics",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PyAccountType {
    Asset,
    Liability,
    Equity,
}

#[pymethods]
impl PyAccountType {
    /// Parse an exact snake_case identifier (``"asset"``, ``"liability"``, or ``"equity"``).
    #[staticmethod]
    fn from_str(value: &str) -> PyResult<Self> {
        finstack_quant_core::wire::serde_parse::<Self>(value).map_err(crate::errors::core_to_py)
    }

    /// String identifier used in JSON (``"asset"``, ``"liability"``, ``"equity"``).
    fn value(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_label(self).map_err(crate::errors::core_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "AccountType.{}",
            self.value().unwrap_or_else(|_| "?".to_string())
        )
    }
}

impl PyAccountType {
    fn to_rust(self) -> PyResult<rust_corkscrew::AccountType> {
        crate::bindings::statements_analytics::enum_convert(&self)
    }

    fn from_rust(value: rust_corkscrew::AccountType) -> PyResult<Self> {
        crate::bindings::statements_analytics::enum_convert(&value)
    }
}

/// Configuration for a single corkscrew account.
///
/// Parameters
/// ----------
/// node_id : str
///     Node id for the balance account.
/// account_type : AccountType
///     Classifier (asset, liability, equity).
/// changes : list[str]
///     Node ids representing increases (or signed net changes) added to
///     the balance.
/// decreases : list[str]
///     Node ids representing positive decreases (repayments, outflows,
///     disposals) subtracted from the balance. Default empty.
/// beginning_balance_node : str | None
///     Optional override node for the beginning balance.
#[pyclass(
    name = "CorkscrewAccount",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyCorkscrewAccount {
    pub(crate) inner: rust_corkscrew::CorkscrewAccount,
}

#[pymethods]
impl PyCorkscrewAccount {
    #[new]
    #[pyo3(signature = (node_id, account_type, changes=Vec::new(), decreases=Vec::new(), beginning_balance_node=None))]
    fn new(
        node_id: &str,
        account_type: PyAccountType,
        changes: Vec<String>,
        decreases: Vec<String>,
        beginning_balance_node: Option<&str>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: rust_corkscrew::CorkscrewAccount {
                node_id: node_id.to_string(),
                account_type: account_type.to_rust()?,
                changes,
                decreases,
                beginning_balance_node: beginning_balance_node.map(str::to_string),
            },
        })
    }

    /// Node id of the balance account being rolled forward.
    #[getter]
    fn node_id(&self) -> &str {
        &self.inner.node_id
    }

    /// Balance-sheet classifier: asset, liability, or equity.
    #[getter]
    fn account_type(&self) -> PyResult<PyAccountType> {
        PyAccountType::from_rust(self.inner.account_type)
    }

    /// Node ids of the period increases (or signed net changes) added to
    /// the balance.
    ///
    /// Identity: ``expected = prev + Σ changes − Σ decreases`` (or
    /// ``beginning + Σ changes − Σ decreases``). Prefer
    /// :meth:`decreases` for positive outflows so roll-forward decrease
    /// nodes do not need to be negated.
    #[getter]
    fn changes(&self) -> Vec<String> {
        self.inner.changes.clone()
    }

    /// Node ids of positive decreases (repayments, outflows, disposals)
    /// subtracted from the balance.
    #[getter]
    fn decreases(&self) -> Vec<String> {
        self.inner.decreases.clone()
    }

    /// Node id overriding the beginning balance, or ``None`` to use the
    /// account's own prior-period closing balance.
    #[getter]
    fn beginning_balance_node(&self) -> Option<&str> {
        self.inner.beginning_balance_node.as_deref()
    }

    fn __repr__(&self) -> String {
        format!(
            "CorkscrewAccount(node_id='{}', account_type={:?}, changes={})",
            self.inner.node_id,
            self.inner.account_type,
            self.inner.changes.len()
        )
    }
}

wire_methods!(
    PyCorkscrewAccount,
    rust_corkscrew::CorkscrewAccount,
    "CorkscrewAccount"
);

/// Configuration for corkscrew (roll-forward) validation.
///
/// Parameters
/// ----------
/// accounts : list[CorkscrewAccount]
///     Balance accounts to validate.
/// tolerance : float
///     Absolute roll-forward tolerance (default ``0.01``).
/// fail_on_error : bool
///     If ``True``, treat any roll-forward violation as fatal.
#[pyclass(
    name = "CorkscrewConfig",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyCorkscrewConfig {
    pub(crate) inner: rust_corkscrew::CorkscrewConfig,
}

#[pymethods]
impl PyCorkscrewConfig {
    #[new]
    #[pyo3(signature = (accounts=Vec::new(), tolerance=0.01, fail_on_error=false))]
    fn new(accounts: Vec<PyCorkscrewAccount>, tolerance: f64, fail_on_error: bool) -> Self {
        Self {
            inner: rust_corkscrew::CorkscrewConfig {
                accounts: accounts.into_iter().map(|a| a.inner).collect(),
                tolerance,
                fail_on_error,
            },
        }
    }

    /// Balance accounts validated by this configuration, in configured order.
    #[getter]
    fn accounts(&self) -> Vec<PyCorkscrewAccount> {
        self.inner
            .accounts
            .iter()
            .cloned()
            .map(|inner| PyCorkscrewAccount { inner })
            .collect()
    }

    /// Absolute roll-forward tolerance, in the balance node's own units.
    ///
    /// A period is flagged when
    /// ``abs(closing - (opening + sum(changes) - sum(decreases))) > tolerance``.
    #[getter]
    fn tolerance(&self) -> f64 {
        self.inner.tolerance
    }

    /// When ``True``, any roll-forward violation is fatal (reported as an
    /// error) rather than a warning.
    #[getter]
    fn fail_on_error(&self) -> bool {
        self.inner.fail_on_error
    }

    fn __repr__(&self) -> String {
        format!(
            "CorkscrewConfig(accounts={}, tolerance={}, fail_on_error={})",
            self.inner.accounts.len(),
            self.inner.tolerance,
            self.inner.fail_on_error
        )
    }
}

wire_methods!(
    PyCorkscrewConfig,
    rust_corkscrew::CorkscrewConfig,
    "CorkscrewConfig"
);

/// Report produced by [`PyCorkscrewExtension.execute`].
#[pyclass(
    name = "CorkscrewReport",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyCorkscrewReport {
    pub(crate) inner: rust_corkscrew::CorkscrewReport,
}

#[pymethods]
impl PyCorkscrewReport {
    #[doc = pydoc!("finstack_quant.statements_analytics._corkscrew", "CorkscrewReport.status")]
    #[getter]
    fn status(&self) -> PyResult<String> {
        crate::bindings::extract::label(&self.inner.status)
    }

    /// Human-readable summary of the validation run.
    #[getter]
    fn message(&self) -> &str {
        &self.inner.message
    }

    /// Non-fatal warnings, including roll-forward breaks reported when
    /// ``fail_on_error`` is ``False``.
    #[getter]
    fn warnings(&self) -> Vec<String> {
        self.inner.warnings.clone()
    }

    /// Roll-forward violations treated as fatal (``fail_on_error=True``) plus
    /// any structural failure. A non-empty list means ``status`` is
    /// ``"failed"``.
    #[getter]
    fn errors(&self) -> Vec<String> {
        self.inner.errors.clone()
    }

    /// Structured data payload as a dict (per-account validations).
    #[getter]
    fn data<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.data)
    }

    /// Return the structured data payload as a JSON string (the wire form of
    /// ``data``).
    fn data_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner.data).map_err(display_to_py)
    }

    #[doc = pydoc!("finstack_quant.statements_analytics._corkscrew", "CorkscrewReport.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let row = serde_json::json!({
            "status": self.status()?,
            "message": self.inner.message,
            "account_count": self.validations().len(),
            "warning_count": self.inner.warnings.len(),
            "error_count": self.inner.errors.len(),
        });
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &[
                "status",
                "message",
                "account_count",
                "warning_count",
                "error_count",
            ],
        )
    }

    /// Export the per-account roll-forward validations as a pandas
    /// ``DataFrame``.
    ///
    /// Columns: ``account``, ``type``, ``periods_validated``, ``max_error``,
    /// ``is_valid``. One row per validated account, in configured order; a
    /// report with no validations still carries the full column schema.
    ///
    /// ``type`` is the account classifier (``"asset"``, ``"liability"``,
    /// ``"equity"``), ``periods_validated`` is a count of model periods, and
    /// ``max_error`` is the largest absolute roll-forward break across those
    /// periods, in the balance node's own units. ``is_valid`` is ``False``
    /// when ``max_error`` exceeded the configured tolerance.
    fn to_validations_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(py, &self.validations(), &VALIDATION_COLUMNS)
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "CorkscrewReport(status='{}', warnings={}, errors={})",
            self.status()?,
            self.inner.warnings.len(),
            self.inner.errors.len()
        ))
    }
}

wire_methods!(
    PyCorkscrewReport,
    rust_corkscrew::CorkscrewReport,
    "CorkscrewReport"
);
impl_repr_html_via_dataframe!(PyCorkscrewReport);

impl PyCorkscrewReport {
    /// Per-account validation records from the structured `data` payload.
    ///
    /// An absent or non-array `validations` entry yields an empty slice, so
    /// the DataFrame builders degrade to a zero-row frame rather than raising.
    fn validations(&self) -> Vec<serde_json::Value> {
        self.inner
            .data
            .get("validations")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default()
    }
}

/// Corkscrew extension for balance-sheet roll-forward validation.
#[pyclass(
    name = "CorkscrewExtension",
    module = "finstack_quant.statements_analytics",
    skip_from_py_object
)]
pub struct PyCorkscrewExtension {
    pub(crate) inner: rust_corkscrew::CorkscrewExtension,
}

#[pymethods]
impl PyCorkscrewExtension {
    /// Construct an extension with its configuration.
    #[new]
    fn new(config: PyCorkscrewConfig) -> Self {
        Self {
            inner: rust_corkscrew::CorkscrewExtension::new(config.inner),
        }
    }

    /// Return the current configuration.
    fn config(&self) -> PyCorkscrewConfig {
        PyCorkscrewConfig {
            inner: self.inner.config().clone(),
        }
    }

    /// Run the corkscrew validation against a model and pre-computed statement results.
    fn execute(
        &self,
        model: &Bound<'_, PyAny>,
        results: &Bound<'_, PyAny>,
    ) -> PyResult<PyCorkscrewReport> {
        let model = extract_model_ref(model)?;
        let results = extract_results_ref(results)?;
        let inner = self
            .inner
            .execute(&model, &results)
            .map_err(crate::errors::statements_to_py)?;
        Ok(PyCorkscrewReport { inner })
    }
}

/// Register corkscrew types on the parent module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyAccountType>()?;
    m.add_class::<PyCorkscrewAccount>()?;
    m.add_class::<PyCorkscrewConfig>()?;
    m.add_class::<PyCorkscrewReport>()?;
    m.add_class::<PyCorkscrewExtension>()?;
    Ok(())
}
