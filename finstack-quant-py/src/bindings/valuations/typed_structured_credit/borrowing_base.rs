//! Typed borrowing-base rules: `EligibilityRule`, `AdvanceRate`,
//! `ConcentrationLimit` and `BorrowingBaseRules`, the collateral-eligibility
//! terms behind a facility's or a deal's borrowing-base test.

use pyo3::prelude::*;

use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AdvanceRate, BorrowingBaseRules, ConcentrationLimit, ConcentrationScope, EligibilityRule,
};

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base", "EligibilityRule")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "EligibilityRule",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyEligibilityRule {
    /// Inner canonical Rust rule.
    pub(crate) inner: EligibilityRule,
}

crate::bindings::macros::wire_methods!(
    PyEligibilityRule,
    EligibilityRule,
    "EligibilityRule",
    module = "finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base",
    to_dict
);

#[pymethods]
impl PyEligibilityRule {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base", "EligibilityRule.__init__")]
    #[new]
    #[pyo3(signature = (*, exclude_defaulted=true, max_maturity=None, max_days_past_due=None, exclude_non_performing=true))]
    #[pyo3(
        text_signature = "(*, exclude_defaulted=True, max_maturity=None, max_days_past_due=None, exclude_non_performing=True)"
    )]
    fn new(
        exclude_defaulted: bool,
        max_maturity: Option<&Bound<'_, PyAny>>,
        max_days_past_due: Option<u32>,
        exclude_non_performing: bool,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: EligibilityRule {
                exclude_defaulted,
                max_maturity: max_maturity.map(py_to_date).transpose()?,
                max_days_past_due,
                exclude_non_performing,
            },
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base", "EligibilityRule.max_maturity")]
    #[getter]
    fn max_maturity<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .max_maturity
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "EligibilityRule(exclude_defaulted={}, max_days_past_due={:?}, exclude_non_performing={})",
            self.inner.exclude_defaulted, self.inner.max_days_past_due, self.inner.exclude_non_performing
        )
    }
}

crate::bindings::macros::py_getters!(PyEligibilityRule |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base", "EligibilityRule.exclude_defaulted")]
    exclude_defaulted -> bool = s.inner.exclude_defaulted;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base", "EligibilityRule.max_days_past_due")]
    max_days_past_due -> Option<u32> = s.inner.max_days_past_due;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base", "EligibilityRule.exclude_non_performing")]
    exclude_non_performing -> bool = s.inner.exclude_non_performing;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base", "AdvanceRate")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "AdvanceRate",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyAdvanceRate {
    /// Inner canonical Rust advance rate.
    pub(crate) inner: AdvanceRate,
}

crate::bindings::macros::wire_methods!(
    PyAdvanceRate,
    AdvanceRate,
    "AdvanceRate",
    module = "finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base",
    to_dict
);

#[pymethods]
impl PyAdvanceRate {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base", "AdvanceRate.__init__")]
    #[new]
    #[pyo3(signature = (asset_class, rate, *, eligibility=None))]
    #[pyo3(text_signature = "(asset_class, rate, *, eligibility=None)")]
    fn new(
        asset_class: &str,
        rate: f64,
        eligibility: Option<PyRef<'_, PyEligibilityRule>>,
    ) -> PyResult<Self> {
        let inner = AdvanceRate {
            asset_class: asset_class.to_string(),
            rate,
            eligibility: eligibility
                .map_or_else(EligibilityRule::default, |rule| rule.inner.clone()),
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "AdvanceRate(asset_class={:?}, rate={})",
            self.inner.asset_class, self.inner.rate
        )
    }
}

crate::bindings::macros::py_getters!(PyAdvanceRate |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base", "AdvanceRate.asset_class")]
    asset_class -> String = s.inner.asset_class.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base", "AdvanceRate.rate")]
    rate -> f64 = s.inner.rate;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base", "AdvanceRate.eligibility")]
    eligibility -> PyEligibilityRule = PyEligibilityRule { inner: s.inner.eligibility.clone() };
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base", "ConcentrationLimit")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "ConcentrationLimit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyConcentrationLimit {
    /// Inner canonical Rust limit.
    pub(crate) inner: ConcentrationLimit,
}

crate::bindings::macros::wire_methods!(
    PyConcentrationLimit,
    ConcentrationLimit,
    "ConcentrationLimit",
    module = "finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base",
    to_dict
);

#[pymethods]
impl PyConcentrationLimit {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base", "ConcentrationLimit.__init__")]
    #[new]
    #[pyo3(text_signature = "(scope, max_pct)")]
    fn new(scope: &str, max_pct: f64) -> PyResult<Self> {
        let inner = ConcentrationLimit {
            scope: crate::bindings::valuations::convert::enum_from_str::<ConcentrationScope>(
                scope, "scope",
            )?,
            max_pct,
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "ConcentrationLimit(scope={:?}, max_pct={})",
            self.scope().unwrap_or_default(),
            self.inner.max_pct
        )
    }
}

crate::bindings::macros::py_getters!(PyConcentrationLimit |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base", "ConcentrationLimit.scope")]
    scope -> PyResult<String> =
        crate::bindings::valuations::convert::enum_to_py_string(&s.inner.scope);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base", "ConcentrationLimit.max_pct")]
    max_pct -> f64 = s.inner.max_pct;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base", "BorrowingBaseRules")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "BorrowingBaseRules",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyBorrowingBaseRules {
    /// Inner canonical Rust rules.
    pub(crate) inner: BorrowingBaseRules,
}

crate::bindings::macros::wire_methods!(
    PyBorrowingBaseRules,
    BorrowingBaseRules,
    "BorrowingBaseRules",
    module = "finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base",
    to_dict
);

#[pymethods]
impl PyBorrowingBaseRules {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base", "BorrowingBaseRules.__init__")]
    #[new]
    #[pyo3(signature = (advance_rates, *, concentration_limits=None))]
    #[pyo3(text_signature = "(advance_rates, *, concentration_limits=None)")]
    fn new(
        advance_rates: Vec<PyRef<'_, PyAdvanceRate>>,
        concentration_limits: Option<Vec<PyRef<'_, PyConcentrationLimit>>>,
    ) -> PyResult<Self> {
        let inner = BorrowingBaseRules {
            advance_rates: advance_rates
                .iter()
                .map(|rate| rate.inner.clone())
                .collect(),
            concentration_limits: concentration_limits
                .unwrap_or_default()
                .iter()
                .map(|limit| limit.inner.clone())
                .collect(),
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base", "BorrowingBaseRules.advance_rates")]
    #[getter]
    fn advance_rates(&self) -> Vec<PyAdvanceRate> {
        self.inner
            .advance_rates
            .iter()
            .map(|rate| PyAdvanceRate {
                inner: rate.clone(),
            })
            .collect()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base", "BorrowingBaseRules.concentration_limits")]
    #[getter]
    fn concentration_limits(&self) -> Vec<PyConcentrationLimit> {
        self.inner
            .concentration_limits
            .iter()
            .map(|limit| PyConcentrationLimit {
                inner: limit.clone(),
            })
            .collect()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "BorrowingBaseRules(advance_rates={}, concentration_limits={})",
            self.inner.advance_rates.len(),
            self.inner.concentration_limits.len()
        )
    }
}
