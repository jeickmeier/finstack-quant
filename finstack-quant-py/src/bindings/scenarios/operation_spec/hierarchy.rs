//! `HierarchyTarget` wrapper for hierarchy-targeted scenario operations.

use finstack_quant_core::market_data::hierarchy::{HierarchyTarget, TagFilter};
use pyo3::prelude::*;

/// Path into the market-data hierarchy, with an optional tag filter, that a
/// hierarchy-targeted operation resolves against the execution context's
/// ``MarketDataHierarchy``.
///
/// Parameters
/// ----------
/// path : list[str]
///     Hierarchy path from the root, e.g. ``["Credit", "US", "IG"]``. Every
///     curve in that subtree is targeted.
/// tag_filter : dict[str, Any] | None, default None
///     Canonical ``{"predicates": [...]}`` filter with equals, in or exists
///     predicates combined with AND semantics; omit to match the whole subtree.
///
/// Examples
/// --------
/// >>> from finstack_quant.scenarios import HierarchyTarget
/// >>> target = HierarchyTarget(["Credit", "US"], {"predicates": [{"equals": {"key": "sector", "value": "financials"}}]})
/// >>> target.path
/// ['Credit', 'US']
/// >>> HierarchyTarget.from_json(target.to_json()) == target
/// True
#[pyclass(
    name = "HierarchyTarget",
    module = "finstack_quant.scenarios",
    eq,
    frozen,
    from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyHierarchyTarget {
    pub(crate) inner: HierarchyTarget,
}

#[pymethods]
impl PyHierarchyTarget {
    #[new]
    #[pyo3(signature = (path, tag_filter=None))]
    fn new(
        py: Python<'_>,
        path: Vec<String>,
        tag_filter: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let tag_filter = tag_filter
            .map(|value| {
                let json: String = py
                    .import("json")?
                    .call_method1("dumps", (value,))?
                    .extract()?;
                serde_json::from_str::<TagFilter>(&json).map_err(crate::errors::display_to_py)
            })
            .transpose()?;
        Ok(Self {
            inner: HierarchyTarget { path, tag_filter },
        })
    }

    /// Hierarchy path from the root.
    #[getter]
    fn path(&self) -> Vec<String> {
        self.inner.path.clone()
    }

    /// Complete canonical tag filter, including equals, in and exists predicates.
    #[getter]
    fn tag_filter<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .tag_filter
            .as_ref()
            .map(|filter| crate::bindings::pandas_utils::serde_to_py(py, filter))
            .transpose()
    }

    /// Serialize to canonical JSON (``{"path": [...], "tag_filter": {...}}``).
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(crate::errors::display_to_py)
    }

    /// Deserialize from canonical JSON, including ``in`` / ``exists`` tag
    /// predicates accepted by the constructor.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON does not match the ``HierarchyTarget`` contract.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: HierarchyTarget = serde_json::from_str(json).map_err(|e| {
            crate::errors::value_error(format!("Invalid HierarchyTarget JSON: {e}"))
        })?;
        Ok(Self { inner })
    }

    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "HierarchyTarget(path={:?}, tag_filter={})",
            self.inner.path,
            match &self.inner.tag_filter {
                None => "None".to_string(),
                Some(filter) => format!("<{} predicates>", filter.predicates.len()),
            }
        )
    }
}
