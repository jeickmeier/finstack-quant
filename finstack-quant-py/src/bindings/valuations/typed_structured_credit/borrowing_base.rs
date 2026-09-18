//! Typed borrowing-base rules: `EligibilityRule`, `AdvanceRate`,
//! `ConcentrationLimit` and `BorrowingBaseRules`, the collateral-eligibility
//! terms behind a facility's or a deal's borrowing-base test.

use pyo3::prelude::*;

use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::errors::core_to_py;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AdvanceRate, BorrowingBaseRules, ConcentrationLimit, ConcentrationScope, EligibilityRule,
};

/// Which collateral an advance rate applies to.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.valuations.instruments import EligibilityRule
/// >>> rule = EligibilityRule(max_days_past_due=60, max_maturity=datetime.date(2030, 1, 1))
/// >>> rule.exclude_defaulted, rule.max_days_past_due
/// (True, 60)
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

sc_wire_methods!(PyEligibilityRule, EligibilityRule, "EligibilityRule");

#[pymethods]
impl PyEligibilityRule {
    /// Construct an eligibility rule.
    ///
    /// Parameters
    /// ----------
    /// exclude_defaulted : bool, optional
    ///     Exclude defaulted rows; ``True`` by default.
    /// max_maturity : datetime.date, optional
    ///     Exclude rows maturing after this date; ``None`` for no limit.
    /// max_days_past_due : int, optional
    ///     Exclude balances more than this many days delinquent (a
    ///     delinquency bucket of 30 days each); ``None`` for no limit.
    /// exclude_non_performing : bool, optional
    ///     Exclude unresolved non-performing loans; ``True`` by default.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``max_maturity`` is not a valid date.
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
                max_maturity: max_maturity.map(extract_date).transpose()?,
                max_days_past_due,
                exclude_non_performing,
            },
        })
    }

    /// Whether defaulted rows are excluded.
    #[getter]
    fn exclude_defaulted(&self) -> bool {
        self.inner.exclude_defaulted
    }

    /// Latest eligible maturity as ``datetime.date``, or ``None``.
    #[getter]
    fn max_maturity<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .max_maturity
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    /// Maximum days past due of an eligible balance, or ``None``.
    #[getter]
    fn max_days_past_due(&self) -> Option<u32> {
        self.inner.max_days_past_due
    }

    /// Whether unresolved non-performing loans are excluded.
    #[getter]
    fn exclude_non_performing(&self) -> bool {
        self.inner.exclude_non_performing
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "EligibilityRule(exclude_defaulted={}, max_days_past_due={:?}, exclude_non_performing={})",
            self.inner.exclude_defaulted, self.inner.max_days_past_due, self.inner.exclude_non_performing
        )
    }
}

/// Advance rate on one asset class of the collateral.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import AdvanceRate, EligibilityRule
/// >>> rate = AdvanceRate("commercial_mortgage", 0.8, eligibility=EligibilityRule(max_days_past_due=60))
/// >>> rate.asset_class, rate.rate
/// ('commercial_mortgage', 0.8)
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

sc_wire_methods!(PyAdvanceRate, AdvanceRate, "AdvanceRate");

#[pymethods]
impl PyAdvanceRate {
    /// Construct an advance rate.
    ///
    /// Parameters
    /// ----------
    /// asset_class : str
    ///     ``AssetType`` wire name the rate applies to (for example
    ///     ``"commercial_mortgage"`` or ``"leveraged_loan"``).
    /// rate : float
    ///     Advance rate as a decimal in ``[0, 1]`` (``0.8`` lends 80% of
    ///     eligible balance).
    /// eligibility : EligibilityRule, optional
    ///     Which rows of the class are eligible; the default rule excludes
    ///     defaulted and non-performing rows.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``rate`` is outside ``[0, 1]`` or not finite.
    #[new]
    #[pyo3(signature = (asset_class, rate, *, eligibility=None))]
    #[pyo3(text_signature = "(asset_class, rate, *, eligibility=None)")]
    fn new(
        asset_class: &str,
        rate: f64,
        eligibility: Option<PyRef<'_, PyEligibilityRule>>,
    ) -> PyResult<Self> {
        if !rate.is_finite() || !(0.0..=1.0).contains(&rate) {
            return Err(crate::errors::value_error(format!(
                "advance rate ({rate}) must be a decimal in [0, 1]"
            )));
        }
        Ok(Self {
            inner: AdvanceRate {
                asset_class: asset_class.to_string(),
                rate,
                eligibility: eligibility
                    .map_or_else(EligibilityRule::default, |rule| rule.inner.clone()),
            },
        })
    }

    /// ``AssetType`` wire name the rate applies to.
    #[getter]
    fn asset_class(&self) -> String {
        self.inner.asset_class.clone()
    }

    /// Advance rate as a decimal.
    #[getter]
    fn rate(&self) -> f64 {
        self.inner.rate
    }

    /// Eligibility rule of the class.
    #[getter]
    fn eligibility(&self) -> PyEligibilityRule {
        PyEligibilityRule {
            inner: self.inner.eligibility.clone(),
        }
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "AdvanceRate(asset_class={:?}, rate={})",
            self.inner.asset_class, self.inner.rate
        )
    }
}

/// Cap on the eligible collateral one obligor, industry or asset class may
/// contribute; balance above the cap is excluded from the borrowing base.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import ConcentrationLimit
/// >>> ConcentrationLimit("obligor", 5.0).scope
/// 'obligor'
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

sc_wire_methods!(
    PyConcentrationLimit,
    ConcentrationLimit,
    "ConcentrationLimit"
);

#[pymethods]
impl PyConcentrationLimit {
    /// Construct a concentration limit.
    ///
    /// Parameters
    /// ----------
    /// scope : str
    ///     ``"obligor"`` (per ``obligor_id``), ``"industry"`` (per
    ///     ``industry``) or ``"asset_class"`` (per asset type).
    /// max_pct : float
    ///     Maximum share of the eligible collateral in percent (``20.0`` =
    ///     20%).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``scope`` is not one of the three names or ``max_pct`` is
    ///     outside ``[0, 100]``.
    #[new]
    #[pyo3(text_signature = "(scope, max_pct)")]
    fn new(scope: &str, max_pct: f64) -> PyResult<Self> {
        let scope = match scope {
            "obligor" => ConcentrationScope::Obligor,
            "industry" => ConcentrationScope::Industry,
            "asset_class" => ConcentrationScope::AssetClass,
            other => {
                return Err(crate::errors::value_error(format!(
                "concentration scope must be 'obligor', 'industry' or 'asset_class', got {other:?}"
            )))
            }
        };
        if !max_pct.is_finite() || !(0.0..=100.0).contains(&max_pct) {
            return Err(crate::errors::value_error(format!(
                "concentration max_pct ({max_pct}) must be a percent in [0, 100]"
            )));
        }
        Ok(Self {
            inner: ConcentrationLimit { scope, max_pct },
        })
    }

    /// ``"obligor"``, ``"industry"`` or ``"asset_class"``.
    #[getter]
    fn scope(&self) -> &'static str {
        match self.inner.scope {
            ConcentrationScope::Obligor => "obligor",
            ConcentrationScope::Industry => "industry",
            ConcentrationScope::AssetClass => "asset_class",
        }
    }

    /// Maximum share of the eligible collateral in percent.
    #[getter]
    fn max_pct(&self) -> f64 {
        self.inner.max_pct
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "ConcentrationLimit(scope={:?}, max_pct={})",
            self.scope(),
            self.inner.max_pct
        )
    }
}

/// Advance rates and concentration limits that size a borrowing base.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import AdvanceRate, BorrowingBaseRules, ConcentrationLimit
/// >>> rules = BorrowingBaseRules(
/// ...     [AdvanceRate("commercial_mortgage", 0.8)],
/// ...     concentration_limits=[ConcentrationLimit("obligor", 20.0)],
/// ... )
/// >>> len(rules.advance_rates), rules.concentration_limits[0].max_pct
/// (1, 20.0)
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

sc_wire_methods!(
    PyBorrowingBaseRules,
    BorrowingBaseRules,
    "BorrowingBaseRules"
);

#[pymethods]
impl PyBorrowingBaseRules {
    /// Construct borrowing-base rules.
    ///
    /// Parameters
    /// ----------
    /// advance_rates : list[AdvanceRate]
    ///     One advance rate per eligible asset class (at least one).
    /// concentration_limits : list[ConcentrationLimit], optional
    ///     Caps on obligor, industry or asset-class shares; none by default.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If no advance rate is given, a class is repeated or a rate or
    ///     limit is invalid.
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

    /// Advance rates by asset class.
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

    /// Concentration limits.
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
