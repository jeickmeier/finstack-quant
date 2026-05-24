//! Python bindings for the corkscrew (roll-forward / articulation) extension.
//!
//! Wraps [`finstack_statements_analytics::extensions::corkscrew`] types:
//!
//! - [`PyAccountType`] — asset / liability / equity classifier (serialized as the snake_case rust enum).
//! - [`PyCorkscrewAccount`] — single account definition (balance node + change nodes).
//! - [`PyCorkscrewConfig`] — extension configuration (accounts, tolerance, fail-on-error).
//! - [`PyCorkscrewExtension`] — execution entry point against a model + statement results.
//! - [`PyCorkscrewReport`] — validation report (status, message, structured data, warnings, errors).

use crate::bindings::extract::{extract_model_ref, extract_results_ref};
use crate::errors::display_to_py;
use finstack_statements_analytics::extensions::corkscrew as rust_corkscrew;
use pyo3::prelude::*;

// ---------------------------------------------------------------------------
// AccountType
// ---------------------------------------------------------------------------

/// Account type label: ``"asset"``, ``"liability"``, or ``"equity"``.
#[pyclass(
    name = "AccountType",
    module = "finstack.statements_analytics",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyAccountType {
    Asset,
    Liability,
    Equity,
}

#[pymethods]
impl PyAccountType {
    /// Parse from a string identifier (``"asset"``, ``"liability"``, ``"equity"``; case-insensitive).
    #[staticmethod]
    fn from_str(value: &str) -> PyResult<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "asset" => Ok(PyAccountType::Asset),
            "liability" => Ok(PyAccountType::Liability),
            "equity" => Ok(PyAccountType::Equity),
            other => Err(crate::errors::value_error(format!(
                "unknown account type '{}' (expected asset / liability / equity)",
                other
            ))),
        }
    }

    /// String identifier used in JSON (``"asset"``, ``"liability"``, ``"equity"``).
    fn value(&self) -> &'static str {
        match self {
            PyAccountType::Asset => "asset",
            PyAccountType::Liability => "liability",
            PyAccountType::Equity => "equity",
        }
    }

    fn __repr__(&self) -> String {
        format!("AccountType.{}", self.value())
    }
}

impl PyAccountType {
    fn to_rust(self) -> rust_corkscrew::AccountType {
        match self {
            PyAccountType::Asset => rust_corkscrew::AccountType::Asset,
            PyAccountType::Liability => rust_corkscrew::AccountType::Liability,
            PyAccountType::Equity => rust_corkscrew::AccountType::Equity,
        }
    }

    fn from_rust(value: rust_corkscrew::AccountType) -> Self {
        match value {
            rust_corkscrew::AccountType::Asset => PyAccountType::Asset,
            rust_corkscrew::AccountType::Liability => PyAccountType::Liability,
            rust_corkscrew::AccountType::Equity => PyAccountType::Equity,
        }
    }
}

// ---------------------------------------------------------------------------
// CorkscrewAccount
// ---------------------------------------------------------------------------

/// Configuration for a single corkscrew account.
///
/// Parameters
/// ----------
/// node_id : str
///     Node id for the balance account.
/// account_type : AccountType
///     Classifier (asset, liability, equity).
/// changes : list[str]
///     Node ids representing changes (additions or subtractions) to the balance.
/// beginning_balance_node : str | None
///     Optional override node for the beginning balance.
#[pyclass(
    name = "CorkscrewAccount",
    module = "finstack.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyCorkscrewAccount {
    pub(crate) inner: rust_corkscrew::CorkscrewAccount,
}

#[pymethods]
impl PyCorkscrewAccount {
    #[new]
    #[pyo3(signature = (node_id, account_type, changes=Vec::new(), beginning_balance_node=None))]
    fn new(
        node_id: &str,
        account_type: PyAccountType,
        changes: Vec<String>,
        beginning_balance_node: Option<&str>,
    ) -> Self {
        Self {
            inner: rust_corkscrew::CorkscrewAccount {
                node_id: node_id.to_string(),
                account_type: account_type.to_rust(),
                changes,
                beginning_balance_node: beginning_balance_node.map(str::to_string),
            },
        }
    }

    #[getter]
    fn node_id(&self) -> &str {
        &self.inner.node_id
    }

    #[getter]
    fn account_type(&self) -> PyAccountType {
        PyAccountType::from_rust(self.inner.account_type)
    }

    #[getter]
    fn changes(&self) -> Vec<String> {
        self.inner.changes.clone()
    }

    #[getter]
    fn beginning_balance_node(&self) -> Option<&str> {
        self.inner.beginning_balance_node.as_deref()
    }

    /// Round-trip via JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Build a corkscrew account from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_corkscrew::CorkscrewAccount =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
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

// ---------------------------------------------------------------------------
// CorkscrewConfig
// ---------------------------------------------------------------------------

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
    module = "finstack.statements_analytics",
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

    #[getter]
    fn accounts(&self) -> Vec<PyCorkscrewAccount> {
        self.inner
            .accounts
            .iter()
            .cloned()
            .map(|inner| PyCorkscrewAccount { inner })
            .collect()
    }

    #[getter]
    fn tolerance(&self) -> f64 {
        self.inner.tolerance
    }

    #[getter]
    fn fail_on_error(&self) -> bool {
        self.inner.fail_on_error
    }

    /// Serialize this config to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Build a config from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_corkscrew::CorkscrewConfig =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
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

// ---------------------------------------------------------------------------
// CorkscrewReport
// ---------------------------------------------------------------------------

/// Report produced by [`PyCorkscrewExtension.execute`].
#[pyclass(
    name = "CorkscrewReport",
    module = "finstack.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyCorkscrewReport {
    pub(crate) inner: rust_corkscrew::CorkscrewReport,
}

#[pymethods]
impl PyCorkscrewReport {
    #[getter]
    fn status(&self) -> String {
        match self.inner.status {
            rust_corkscrew::CorkscrewStatus::Success => "success".to_string(),
            rust_corkscrew::CorkscrewStatus::Failed => "failed".to_string(),
        }
    }

    #[getter]
    fn message(&self) -> &str {
        &self.inner.message
    }

    #[getter]
    fn warnings(&self) -> Vec<String> {
        self.inner.warnings.clone()
    }

    #[getter]
    fn errors(&self) -> Vec<String> {
        self.inner.errors.clone()
    }

    /// Return the structured data payload as a JSON string.
    fn data_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner.data).map_err(display_to_py)
    }

    /// Serialize the full report to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Build a report from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_corkscrew::CorkscrewReport =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "CorkscrewReport(status='{}', warnings={}, errors={})",
            match self.inner.status {
                rust_corkscrew::CorkscrewStatus::Success => "success",
                rust_corkscrew::CorkscrewStatus::Failed => "failed",
            },
            self.inner.warnings.len(),
            self.inner.errors.len()
        )
    }
}

// ---------------------------------------------------------------------------
// CorkscrewExtension
// ---------------------------------------------------------------------------

/// Corkscrew extension for balance-sheet roll-forward validation.
#[pyclass(
    name = "CorkscrewExtension",
    module = "finstack.statements_analytics",
    skip_from_py_object
)]
pub struct PyCorkscrewExtension {
    pub(crate) inner: rust_corkscrew::CorkscrewExtension,
}

#[pymethods]
impl PyCorkscrewExtension {
    /// Construct a new extension with no configuration.
    #[new]
    fn new() -> Self {
        Self {
            inner: rust_corkscrew::CorkscrewExtension::new(),
        }
    }

    /// Construct an extension preloaded with a configuration.
    #[staticmethod]
    fn with_config(config: PyCorkscrewConfig) -> Self {
        Self {
            inner: rust_corkscrew::CorkscrewExtension::with_config(config.inner),
        }
    }

    /// Replace the current configuration.
    fn set_config(&mut self, config: PyCorkscrewConfig) {
        self.inner.set_config(config.inner);
    }

    /// Return the current configuration, if any.
    fn config(&self) -> Option<PyCorkscrewConfig> {
        self.inner
            .config()
            .cloned()
            .map(|inner| PyCorkscrewConfig { inner })
    }

    /// Run the corkscrew validation against a model and pre-computed statement results.
    fn execute(
        &mut self,
        model: &Bound<'_, PyAny>,
        results: &Bound<'_, PyAny>,
    ) -> PyResult<PyCorkscrewReport> {
        let model = extract_model_ref(model)?;
        let results = extract_results_ref(results)?;
        let inner = self
            .inner
            .execute(&model, &results)
            .map_err(display_to_py)?;
        Ok(PyCorkscrewReport { inner })
    }
}

impl Default for PyCorkscrewExtension {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register corkscrew types on the parent module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyAccountType>()?;
    m.add_class::<PyCorkscrewAccount>()?;
    m.add_class::<PyCorkscrewConfig>()?;
    m.add_class::<PyCorkscrewReport>()?;
    m.add_class::<PyCorkscrewExtension>()?;
    Ok(())
}
