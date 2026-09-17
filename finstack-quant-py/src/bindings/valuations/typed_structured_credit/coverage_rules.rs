//! Typed collateral valuation rules for the coverage tests (`CoverageRules`).

use pyo3::prelude::*;

use crate::bindings::pandas_utils::serde_to_py;
use crate::errors::core_to_py;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::CoverageRules;

/// Collateral valuation rules for the OC tests: rating haircuts, the value
/// carried for defaulted collateral, the excess-CCC bucket and discount
/// obligations (CLO indenture conventions). Percentages are percent values
/// (``7.5`` = 7.5%); haircuts are decimal fractions.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import CoverageRules
/// >>> rules = CoverageRules.clo_standard()
/// >>> rules.ccc_bucket["threshold_pct"]
/// 7.5
/// >>> CoverageRules(rating_haircuts={"NR": 0.5}).rating_haircuts
/// {'NR': 0.5}
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CoverageRules",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCoverageRules {
    /// Inner canonical Rust rules.
    pub(crate) inner: CoverageRules,
}

sc_wire_methods!(PyCoverageRules, CoverageRules, "CoverageRules");

#[pymethods]
impl PyCoverageRules {
    /// Construct rules from their serde parts (every part optional).
    ///
    /// Parameters
    /// ----------
    /// rating_haircuts : dict[str, float] | str, optional
    ///     Haircut per rating as a decimal fraction (``{"CCC": 0.7, "NR":
    ///     0.5}``); performing collateral is carried at ``par × (1 −
    ///     haircut)``. Empty when omitted.
    /// defaulted_valuation : dict | str, optional
    ///     ``DefaultedValuation`` serde value: ``"recovery"`` (modeled
    ///     recovery, the default) or ``{"market_value": {"pct": 60.0}}``.
    /// ccc_bucket : dict | str, optional
    ///     ``CccBucketRule`` (``threshold_pct``, ``treatment``); ``None``
    ///     disables the bucket.
    /// discount_obligation : dict | str, optional
    ///     ``DiscountObligationRule`` (``price_threshold_pct``); ``None``
    ///     disables it.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a part does not match its serde shape or the rules fail
    ///     validation.
    #[new]
    #[pyo3(signature = (rating_haircuts=None, defaulted_valuation=None, ccc_bucket=None, discount_obligation=None))]
    #[pyo3(
        text_signature = "(rating_haircuts=None, defaulted_valuation=None, ccc_bucket=None, discount_obligation=None)"
    )]
    fn new(
        py: Python<'_>,
        rating_haircuts: Option<&Bound<'_, PyAny>>,
        defaulted_valuation: Option<&Bound<'_, PyAny>>,
        ccc_bucket: Option<&Bound<'_, PyAny>>,
        discount_obligation: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let mut inner = CoverageRules::default();
        if let Some(value) = rating_haircuts {
            inner.rating_haircuts =
                crate::bindings::module_utils::py_to_serde(py, value, "rating_haircuts")?;
        }
        if let Some(value) = defaulted_valuation {
            inner.defaulted_valuation =
                crate::bindings::module_utils::py_to_serde(py, value, "defaulted_valuation")?;
        }
        if let Some(value) = ccc_bucket {
            inner.ccc_bucket = Some(crate::bindings::module_utils::py_to_serde(
                py,
                value,
                "ccc_bucket",
            )?);
        }
        if let Some(value) = discount_obligation {
            inner.discount_obligation = Some(crate::bindings::module_utils::py_to_serde(
                py,
                value,
                "discount_obligation",
            )?);
        }
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Standard CLO rules (mirrors Rust ``CoverageRules::clo_standard``):
    /// CCC/Caa haircuts, defaulted collateral at recovery, a 7.5% CCC bucket
    /// at market value and discount obligations below 80.
    ///
    /// Returns
    /// -------
    /// CoverageRules
    ///     The standard rules.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import CoverageRules
    /// >>> CoverageRules.clo_standard().discount_obligation["price_threshold_pct"]
    /// 80.0
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn clo_standard() -> Self {
        Self {
            inner: CoverageRules::clo_standard(),
        }
    }

    /// Haircut per rating as a ``dict[str, float]`` of decimal fractions.
    #[getter]
    fn rating_haircuts<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.rating_haircuts)
    }

    /// ``DefaultedValuation`` serde value.
    #[getter]
    fn defaulted_valuation<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.defaulted_valuation)
    }

    /// ``CccBucketRule`` serde dict, or ``None``.
    #[getter]
    fn ccc_bucket<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .ccc_bucket
            .as_ref()
            .map(|rule| serde_to_py(py, rule))
            .transpose()
    }

    /// ``DiscountObligationRule`` serde dict, or ``None``.
    #[getter]
    fn discount_obligation<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .discount_obligation
            .as_ref()
            .map(|rule| serde_to_py(py, rule))
            .transpose()
    }

    /// Validate haircuts, percentages and thresholds.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a haircut is outside ``[0, 1]`` or a percent is out of range.
    #[pyo3(text_signature = "($self)")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CoverageRules(rating_haircuts={}, ccc_bucket={}, discount_obligation={})",
            self.inner.rating_haircuts.len(),
            self.inner.ccc_bucket.is_some(),
            self.inner.discount_obligation.is_some()
        )
    }
}
