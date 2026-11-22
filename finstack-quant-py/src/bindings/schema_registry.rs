//! Registry-backed schema access shared by every `finstack_quant.*.schema` namespace.
//!
//! The per-crate accessors elsewhere in these bindings return one named schema
//! each, which requires the caller to already know what exists. The three
//! functions generated here are the discovery surface instead: `index` lists
//! every contract the crate publishes, `get` fetches one by path, and
//! `validate` checks a payload and reports where it failed.
//!
//! Schemas are rendered from the crate's `ARTIFACTS` registry on demand and
//! cached in `finstack_quant::schema`, so nothing can drift from the installed
//! wheel and the extension module carries no copy of the checked-in JSON.

/// Map a Python `<domain>.schema` module path to its umbrella schema domain name.
pub(crate) fn domain_for_module(module: &str) -> Option<&'static str> {
    match module {
        "finstack_quant.analytics.schema" => Some("analytics"),
        "finstack_quant.attribution.schema" => Some("attribution"),
        "finstack_quant.calibration.schema" => Some("calibration"),
        "finstack_quant.cashflows.schema" => Some("cashflows"),
        "finstack_quant.core.schema" => Some("core"),
        "finstack_quant.margin.schema" => Some("margin"),
        "finstack_quant.models.factor.schema" => Some("factor_model"),
        "finstack_quant.portfolio.schema" => Some("portfolio"),
        "finstack_quant.scenarios.schema" => Some("scenarios"),
        "finstack_quant.statements.schema" => Some("statements"),
        "finstack_quant.valuations.schema" => Some("valuations"),
        _ => None,
    }
}

/// Generate `index`, `get` and `validate` for one crate's schema namespace.
///
/// # Arguments
///
/// * `$registry` - Expression yielding the crate's `ARTIFACTS` slice.
/// * `$module` - Public dotted path of the schema namespace
///   (`"finstack_quant.core.schema"`); the three docstrings come from its stub.
macro_rules! schema_registry_functions {
    ($registry:expr_2021, $module:literal) => {
        #[doc = $crate::bindings::pydoc::pydoc!($module, "index")]
        #[pyfunction]
        #[pyo3(text_signature = "()")]
        fn index() -> PyResult<String> {
            let value = match $crate::bindings::schema_registry::domain_for_module($module) {
                Some(domain) => finstack_quant::schema::index_for_domain(domain)
                    .map_err(crate::errors::core_to_py)?,
                None => finstack_quant_core::schema::build_schema_index($registry)
                    .map_err(crate::errors::core_to_py)?,
            };
            serde_json::to_string_pretty(&value).map_err(|error| {
                pyo3::exceptions::PyValueError::new_err(format!("serialize index: {error}"))
            })
        }

        #[doc = $crate::bindings::pydoc::pydoc!($module, "get")]
        #[pyfunction]
        #[pyo3(signature = (selector, profile = "canonical"), text_signature = "(selector, profile='canonical')")]
        fn get(selector: &str, profile: &str) -> PyResult<String> {
            let artifact =
                finstack_quant_core::schema::find_schema_artifact($registry, selector)
                    .map_err(crate::errors::core_to_py)?;
            let value = finstack_quant::schema::render_profile(artifact, profile)
                .map_err(crate::errors::core_to_py)?;
            serde_json::to_string_pretty(&value).map_err(|error| {
                pyo3::exceptions::PyValueError::new_err(format!("serialize schema: {error}"))
            })
        }

        #[doc = $crate::bindings::pydoc::pydoc!($module, "validate")]
        #[pyfunction]
        #[pyo3(text_signature = "(selector, payload)")]
        fn validate(selector: &str, payload: &str) -> PyResult<String> {
            let artifact =
                finstack_quant_core::schema::find_schema_artifact($registry, selector)
                    .map_err(crate::errors::core_to_py)?;
            let parsed: serde_json::Value = serde_json::from_str(payload).map_err(|error| {
                pyo3::exceptions::PyValueError::new_err(format!("payload is not JSON: {error}"))
            })?;
            let failures = finstack_quant::schema::validate(artifact, &parsed)
                .map_err(crate::errors::core_to_py)?;
            serde_json::to_string_pretty(&failures).map_err(|error| {
                pyo3::exceptions::PyValueError::new_err(format!("serialize report: {error}"))
            })
        }

        /// Add `index`, `get` and `validate` to a schema namespace module.
        ///
        /// # Errors
        ///
        /// Returns a `PyErr` if any function cannot be registered.
        fn add_registry_functions(m: &Bound<'_, pyo3::types::PyModule>) -> PyResult<()> {
            m.add_function(wrap_pyfunction!(index, m)?)?;
            m.add_function(wrap_pyfunction!(get, m)?)?;
            m.add_function(wrap_pyfunction!(validate, m)?)?;
            Ok(())
        }
    };
}

pub(crate) use schema_registry_functions;
