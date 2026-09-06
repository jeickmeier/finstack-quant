//! Registry-backed schema access shared by every `finstack_quant.*.schema` namespace.
//!
//! The per-crate accessors elsewhere in these bindings return one named schema
//! each, which requires the caller to already know what exists. The three
//! functions generated here are the discovery surface instead: `index` lists
//! every contract the crate publishes, `get` fetches one by path, and
//! `validate` checks a payload and reports where it failed.
//!
//! Schemas are rendered from the crate's `ARTIFACTS` registry on demand rather
//! than embedded, so nothing can drift from the installed wheel and the
//! extension module carries no copy of the six megabytes of checked-in JSON.

/// Generate `index`, `get` and `validate` for one crate's schema namespace.
///
/// # Arguments
///
/// * `$registry` - Expression yielding the crate's `ARTIFACTS` slice.
/// * `$python_path` - Dotted Python path of the namespace, for `__module__`.
macro_rules! schema_registry_functions {
    ($registry:expr, $python_path:literal) => {
        /// List every JSON Schema this crate publishes.
        ///
        /// Returns
        /// -------
        /// str
        ///     Pretty-printed JSON with an ``artifacts`` array. Each row carries
        ///     ``path``, ``$id``, ``title``, ``summary``, ``bytes`` and ``kind``
        ///     (``input`` for documents you author, ``output`` for documents the
        ///     library emits, ``component`` for shared definitions).
        ///
        /// Raises
        /// ------
        /// ValueError
        ///     If an artifact cannot be rendered.
        #[pyfunction]
        #[pyo3(text_signature = "()")]
        fn index() -> PyResult<String> {
            let value = finstack_quant_core::schema::build_schema_index($registry)
                .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
            serde_json::to_string_pretty(&value).map_err(|error| {
                pyo3::exceptions::PyValueError::new_err(format!("serialize index: {error}"))
            })
        }

        /// Fetch one JSON Schema by path, ``$id``, or filename.
        ///
        /// Parameters
        /// ----------
        /// selector : str
        ///     A ``path`` or ``$id`` from :func:`index`, or just the trailing
        ///     filename.
        ///
        /// profile : str, optional
        ///     ``"canonical"`` (default) returns the published contract, which
        ///     is what :func:`validate` checks against. ``"llm"`` returns the
        ///     projection: self-contained, unit enums flattened, Rust prose
        ///     stripped, and shaped for structured-output subsets. The
        ///     projection is deliberately **not** a validator.
        ///
        /// Returns
        /// -------
        /// str
        ///     Pretty-printed JSON Schema text.
        ///
        /// Raises
        /// ------
        /// KeyError
        ///     If no artifact matches ``selector``.
        /// ValueError
        ///     If ``profile`` is unknown, or the artifact cannot be rendered.
        #[pyfunction]
        #[pyo3(signature = (selector, profile = "canonical"), text_signature = "(selector, profile='canonical')")]
        fn get(selector: &str, profile: &str) -> PyResult<String> {
            let artifact =
                finstack_quant_core::schema::find_schema_artifact($registry, selector)
                    .map_err(|error| pyo3::exceptions::PyKeyError::new_err(error.to_string()))?;
            let value =
                finstack_quant::schema::render_profile(artifact, profile)
                    .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
            serde_json::to_string_pretty(&value).map_err(|error| {
                pyo3::exceptions::PyValueError::new_err(format!("serialize schema: {error}"))
            })
        }

        /// Validate a JSON payload against one published schema.
        ///
        /// Parameters
        /// ----------
        /// selector : str
        ///     A ``path`` or ``$id`` from :func:`index`, or just the trailing
        ///     filename.
        /// payload : str
        ///     JSON text to check.
        ///
        /// Returns
        /// -------
        /// str
        ///     Pretty-printed JSON array of failures, each with ``pointer`` (a
        ///     JSON Pointer into the payload) and ``message``. An empty array
        ///     means the payload validates.
        ///
        /// Raises
        /// ------
        /// KeyError
        ///     If no artifact matches ``selector``.
        /// ValueError
        ///     If ``payload`` is not valid JSON, or the schema cannot be built.
        #[pyfunction]
        #[pyo3(text_signature = "(selector, payload)")]
        fn validate(selector: &str, payload: &str) -> PyResult<String> {
            let artifact =
                finstack_quant_core::schema::find_schema_artifact($registry, selector)
                    .map_err(|error| pyo3::exceptions::PyKeyError::new_err(error.to_string()))?;
            let parsed: serde_json::Value = serde_json::from_str(payload).map_err(|error| {
                pyo3::exceptions::PyValueError::new_err(format!("payload is not JSON: {error}"))
            })?;
            let failures =
                finstack_quant::schema::validate(artifact, &parsed)
                    .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
            serde_json::to_string_pretty(&finstack_quant::schema::failures_to_value(&failures)).map_err(|error| {
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
            for name in ["index", "get", "validate"] {
                m.getattr(name)?.setattr("__module__", $python_path)?;
            }
            Ok(())
        }
    };
}

pub(crate) use schema_registry_functions;
