//! Python bindings for the real-estate operating-statement templates.
//!
//! Wraps [`finstack_quant_statements_analytics::templates::real_estate`] — property-level
//! rent rolls, NOI / NCF buildups, and the full operating-statement template.
//!
//! Period-id arguments on lease specs are accepted as strings (e.g.
//! ``"2025Q1"``) and parsed via `PeriodId::FromStr`. Each binding reconstructs
//! the canonical Rust builder, applies the template, and returns a typed model
//! while preserving metadata and capital structure.

use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe, serde_object_to_single_row_dataframe_with_schema,
};
use crate::bindings::statements::types::PyFinancialModelSpec;
use crate::errors::display_to_py;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements_analytics::templates::real_estate as rust_re;
use pyo3::prelude::*;

use super::templates_common::{extract_builder, finish_builder};

fn parse_period(s: &str) -> PyResult<PeriodId> {
    s.parse().map_err(crate::errors::core_to_py)
}

/// Rent step that resets the base rent starting at ``start`` (inclusive).
#[pyclass(
    name = "RentStepSpec",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyRentStepSpec {
    pub(crate) inner: rust_re::RentStepSpec,
}

#[pymethods]
impl PyRentStepSpec {
    #[new]
    fn new(start: &str, rent: f64) -> PyResult<Self> {
        Ok(Self {
            inner: rust_re::RentStepSpec {
                start: parse_period(start)?,
                rent,
            },
        })
    }

    /// Period (inclusive) this rent level takes effect, as a period-id string.
    #[getter]
    fn start(&self) -> String {
        self.inner.start.to_string()
    }

    /// Rent for one model period from ``start``, in model currency units.
    ///
    /// This replaces the prevailing rent level rather than adding to it, and
    /// restarts growth compounding from ``start``.
    #[getter]
    fn rent(&self) -> f64 {
        self.inner.rent
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
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

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_re::RentStepSpec = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("RentStepSpec", &self.inner)
    }
}

/// Free rent (concession) window that zeros out rent for ``periods`` starting at ``start``.
#[pyclass(
    name = "FreeRentWindowSpec",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyFreeRentWindowSpec {
    pub(crate) inner: rust_re::FreeRentWindowSpec,
}

#[pymethods]
impl PyFreeRentWindowSpec {
    #[new]
    fn new(start: &str, periods: u32) -> PyResult<Self> {
        Ok(Self {
            inner: rust_re::FreeRentWindowSpec {
                start: parse_period(start)?,
                periods,
            },
        })
    }

    /// Period (inclusive) free rent starts, as a period-id string.
    #[getter]
    fn start(&self) -> String {
        self.inner.start.to_string()
    }

    /// Length of the concession as a count of model periods.
    #[getter]
    fn periods(&self) -> u32 {
        self.inner.periods
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
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

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_re::FreeRentWindowSpec =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("FreeRentWindowSpec", &self.inner)
    }
}

/// Renewal specification modeled in expected-value terms.
#[pyclass(
    name = "RenewalSpec",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyRenewalSpec {
    pub(crate) inner: rust_re::RenewalSpec,
}

#[pymethods]
impl PyRenewalSpec {
    #[new]
    #[pyo3(signature = (term_periods, probability, downtime_periods=0, rent_factor=1.0, free_rent_periods=0))]
    fn new(
        term_periods: u32,
        probability: f64,
        downtime_periods: u32,
        rent_factor: f64,
        free_rent_periods: u32,
    ) -> Self {
        Self {
            inner: rust_re::RenewalSpec {
                downtime_periods,
                term_periods,
                probability,
                rent_factor,
                free_rent_periods,
            },
        }
    }

    /// Renewal term length as a count of model periods.
    #[getter]
    fn term_periods(&self) -> u32 {
        self.inner.term_periods
    }

    /// Probability of renewal as a decimal fraction in ``[0, 1]``.
    ///
    /// Renewal is modelled in expected-value terms, so this weights the
    /// renewal rent rather than selecting a branch.
    #[getter]
    fn probability(&self) -> f64 {
        self.inner.probability
    }

    /// Rent-free downtime after the initial term ends, as a count of model
    /// periods.
    #[getter]
    fn downtime_periods(&self) -> u32 {
        self.inner.downtime_periods
    }

    /// Multiplier applied to the last contractual rent of the initial term
    /// (``1.05`` means renewal starts 5% above the prior rent level).
    #[getter]
    fn rent_factor(&self) -> f64 {
        self.inner.rent_factor
    }

    /// Number of model periods of free rent at renewal start.
    #[getter]
    fn free_rent_periods(&self) -> u32 {
        self.inner.free_rent_periods
    }

    /// Validate renewal fields.
    fn validate(&self) -> PyResult<()> {
        self.inner
            .validate()
            .map_err(crate::errors::statements_to_py)
    }

    /// Export the renewal spec as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``downtime_periods``, ``term_periods``, ``probability``,
    /// ``rent_factor``, ``free_rent_periods``.
    ///
    /// The three ``*_periods`` columns are counts of model periods;
    /// ``probability`` is a decimal fraction in ``[0, 1]`` and
    /// ``rent_factor`` is a multiplier on the prior rent level.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe(py, &self.inner)
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
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

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_re::RenewalSpec = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("RenewalSpec", &self.inner)
    }
}

/// Compounding convention for lease rent growth.
#[pyclass(
    name = "LeaseGrowthConvention",
    module = "finstack_quant.statements_analytics",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PyLeaseGrowthConvention {
    PerPeriod,
    AnnualEscalator,
}

#[pymethods]
impl PyLeaseGrowthConvention {
    /// Parse from a string identifier (``"per_period"`` or ``"annual_escalator"``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not one of the serde names.
    #[staticmethod]
    fn from_str(value: &str) -> PyResult<Self> {
        finstack_quant_core::wire::serde_parse::<Self>(value).map_err(crate::errors::core_to_py)
    }

    /// String identifier used in JSON (``"per_period"`` or ``"annual_escalator"``).
    fn value(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_label(self).map_err(crate::errors::core_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "LeaseGrowthConvention.{}",
            self.value().unwrap_or_else(|_| "?".to_string())
        )
    }
}

impl PyLeaseGrowthConvention {
    fn to_rust(self) -> PyResult<rust_re::LeaseGrowthConvention> {
        crate::bindings::statements_analytics::enum_convert(&self)
    }

    fn from_rust(value: rust_re::LeaseGrowthConvention) -> PyResult<Self> {
        crate::bindings::statements_analytics::enum_convert(&value)
    }
}

/// Rich lease spec for rent-roll generation.
#[pyclass(
    name = "LeaseSpec",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyLeaseSpec {
    pub(crate) inner: rust_re::LeaseSpec,
}

#[pymethods]
impl PyLeaseSpec {
    #[new]
    #[pyo3(signature = (
        node_id,
        start,
        base_rent,
        end=None,
        growth_rate=0.0,
        growth_convention=PyLeaseGrowthConvention::AnnualEscalator,
        rent_steps=Vec::new(),
        free_rent_periods=0,
        free_rent_windows=Vec::new(),
        occupancy=1.0,
        renewal=None,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        node_id: &str,
        start: &str,
        base_rent: f64,
        end: Option<&str>,
        growth_rate: f64,
        growth_convention: PyLeaseGrowthConvention,
        rent_steps: Vec<PyRentStepSpec>,
        free_rent_periods: u32,
        free_rent_windows: Vec<PyFreeRentWindowSpec>,
        occupancy: f64,
        renewal: Option<PyRenewalSpec>,
    ) -> PyResult<Self> {
        let inner = rust_re::LeaseSpec {
            node_id: node_id.to_string(),
            start: parse_period(start)?,
            end: end.map(parse_period).transpose()?,
            base_rent,
            growth_rate,
            growth_convention: growth_convention.to_rust()?,
            rent_steps: rent_steps.into_iter().map(|s| s.inner).collect(),
            free_rent_periods,
            free_rent_windows: free_rent_windows.into_iter().map(|w| w.inner).collect(),
            occupancy,
            renewal: renewal.map(|r| r.inner),
        };
        Ok(Self { inner })
    }

    /// Base node id; per-lease detail nodes are derived from it
    /// (``{node_id}.pgi``, ``.free_rent``, ``.vacancy_loss``,
    /// ``.effective_rent``).
    #[getter]
    fn node_id(&self) -> &str {
        &self.inner.node_id
    }

    /// First period (inclusive) the lease is active, as a period-id string
    /// (e.g. ``"2025Q1"``).
    #[getter]
    fn start(&self) -> String {
        self.inner.start.to_string()
    }

    /// Last period (inclusive) of the initial term, or ``None`` to run
    /// through the model end (which also disables renewal modelling).
    #[getter]
    fn end(&self) -> Option<String> {
        self.inner.end.map(|p| p.to_string())
    }

    /// Base rent for one model period at ``start``, in model currency units.
    ///
    /// A quarterly model means rent per quarter, not per year.
    #[getter]
    fn base_rent(&self) -> f64 {
        self.inner.base_rent
    }

    /// Growth rate applied within a rent segment as a decimal fraction
    /// (``0.03`` = +3%), compounded per ``growth_convention``.
    #[getter]
    fn growth_rate(&self) -> f64 {
        self.inner.growth_rate
    }

    /// Compounding convention for ``growth_rate``: every model period
    /// (``per_period``) or once per lease-start anniversary
    /// (``annual_escalator``, the default Argus/NCREIF annual bump).
    #[getter]
    fn growth_convention(&self) -> PyResult<PyLeaseGrowthConvention> {
        PyLeaseGrowthConvention::from_rust(self.inner.growth_convention)
    }

    /// Number of model periods of free rent counted from ``start``, before
    /// any additional ``free_rent_windows``.
    #[getter]
    fn free_rent_periods(&self) -> u32 {
        self.inner.free_rent_periods
    }

    /// Occupancy factor in ``[0, 1]`` applied to non-free contractual rent.
    #[getter]
    fn occupancy(&self) -> f64 {
        self.inner.occupancy
    }

    /// Renewal modelling applied after ``end``, or ``None`` for no renewal.
    #[getter]
    fn renewal(&self) -> Option<PyRenewalSpec> {
        self.inner
            .renewal
            .clone()
            .map(|inner| PyRenewalSpec { inner })
    }

    /// Validate lease fields.
    fn validate(&self) -> PyResult<()> {
        self.inner
            .validate()
            .map_err(crate::errors::statements_to_py)
    }

    /// Export the lease spec as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``node_id``, ``start``, ``end``, ``base_rent``,
    /// ``growth_rate``, ``growth_convention``, ``free_rent_periods``,
    /// ``occupancy``, ``rent_step_count``, ``free_rent_window_count``,
    /// ``has_renewal``.
    ///
    /// ``start`` and ``end`` are period-id strings, ``base_rent`` is per
    /// model period, ``growth_rate`` and ``occupancy`` are decimal fractions,
    /// and ``free_rent_periods`` is a count of model periods. The nested
    /// collections are summarised as counts here — read ``renewal`` (and its
    /// own ``to_dataframe``) for the renewal terms.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let row = serde_json::json!({
            "node_id": self.inner.node_id,
            "start": self.inner.start.to_string(),
            "end": self.inner.end.map(|p| p.to_string()),
            "base_rent": self.inner.base_rent,
            "growth_rate": self.inner.growth_rate,
            "growth_convention": crate::bindings::statements_analytics::serde_variant_str(&self.inner.growth_convention),
            "free_rent_periods": self.inner.free_rent_periods,
            "occupancy": self.inner.occupancy,
            "rent_step_count": self.inner.rent_steps.len(),
            "free_rent_window_count": self.inner.free_rent_windows.len(),
            "has_renewal": self.inner.renewal.is_some(),
        });
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &[
                "node_id",
                "start",
                "end",
                "base_rent",
                "growth_rate",
                "growth_convention",
                "free_rent_periods",
                "occupancy",
                "rent_step_count",
                "free_rent_window_count",
                "has_renewal",
            ],
        )
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
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

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_re::LeaseSpec = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("LeaseSpec", &self.inner)
    }
}

/// Standard aggregated output node ids for a rent roll.
#[pyclass(
    name = "RentRollOutputNodes",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyRentRollOutputNodes {
    pub(crate) inner: rust_re::RentRollOutputNodes,
}

#[pymethods]
impl PyRentRollOutputNodes {
    #[new]
    #[pyo3(signature = (
        rent_pgi_node="rent_pgi",
        free_rent_node="free_rent",
        vacancy_loss_node="vacancy_loss",
        rent_effective_node="rent_effective",
    ))]
    fn new(
        rent_pgi_node: &str,
        free_rent_node: &str,
        vacancy_loss_node: &str,
        rent_effective_node: &str,
    ) -> Self {
        Self {
            inner: rust_re::RentRollOutputNodes {
                rent_pgi_node: rent_pgi_node.to_string(),
                free_rent_node: free_rent_node.to_string(),
                vacancy_loss_node: vacancy_loss_node.to_string(),
                rent_effective_node: rent_effective_node.to_string(),
            },
        }
    }

    /// Node id holding total contractual rent (potential gross income) across
    /// all leases.
    #[getter]
    fn rent_pgi_node(&self) -> &str {
        &self.inner.rent_pgi_node
    }

    /// Node id holding total free-rent concessions.
    #[getter]
    fn free_rent_node(&self) -> &str {
        &self.inner.free_rent_node
    }

    /// Node id holding total vacancy loss, including occupancy and
    /// renewal-probability effects.
    #[getter]
    fn vacancy_loss_node(&self) -> &str {
        &self.inner.vacancy_loss_node
    }

    /// Node id holding total effective rent, the EGI rent component
    /// ``rent_pgi - free_rent - vacancy_loss``.
    #[getter]
    fn rent_effective_node(&self) -> &str {
        &self.inner.rent_effective_node
    }

    /// Export the node-id mapping as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``rent_pgi_node``, ``free_rent_node``, ``vacancy_loss_node``,
    /// ``rent_effective_node``. Every value is a statement node id, not a
    /// numeric amount.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe(py, &self.inner)
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
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

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_re::RentRollOutputNodes =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("RentRollOutputNodes", &self.inner)
    }
}

/// Basis for management fee calculation.
#[pyclass(
    name = "ManagementFeeBase",
    module = "finstack_quant.statements_analytics",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PyManagementFeeBase {
    Egi,
    EffectiveRent,
}

#[pymethods]
impl PyManagementFeeBase {
    /// Parse an exact snake_case identifier (``"egi"`` or ``"effective_rent"``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not one of the serde names.
    #[staticmethod]
    fn from_str(value: &str) -> PyResult<Self> {
        finstack_quant_core::wire::serde_parse::<Self>(value).map_err(crate::errors::core_to_py)
    }

    /// String identifier used in JSON (``"egi"`` or ``"effective_rent"``).
    fn value(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_label(self).map_err(crate::errors::core_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "ManagementFeeBase.{}",
            self.value().unwrap_or_else(|_| "?".to_string())
        )
    }
}

impl PyManagementFeeBase {
    fn to_rust(self) -> PyResult<rust_re::ManagementFeeBase> {
        crate::bindings::statements_analytics::enum_convert(&self)
    }

    fn from_rust(value: rust_re::ManagementFeeBase) -> PyResult<Self> {
        crate::bindings::statements_analytics::enum_convert(&value)
    }
}

/// Management fee specification.
#[pyclass(
    name = "ManagementFeeSpec",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyManagementFeeSpec {
    pub(crate) inner: rust_re::ManagementFeeSpec,
}

#[pymethods]
impl PyManagementFeeSpec {
    #[new]
    #[pyo3(signature = (rate, base=PyManagementFeeBase::Egi))]
    fn new(rate: f64, base: PyManagementFeeBase) -> PyResult<Self> {
        Ok(Self {
            inner: rust_re::ManagementFeeSpec {
                rate,
                base: base.to_rust()?,
            },
        })
    }

    /// Management fee rate as a decimal fraction (``0.03`` = 3%).
    #[getter]
    fn rate(&self) -> f64 {
        self.inner.rate
    }

    /// Basis the fee applies to: ``egi`` (effective gross income) or
    /// ``effective_rent`` (rent only, excluding other income).
    #[getter]
    fn base(&self) -> PyResult<PyManagementFeeBase> {
        PyManagementFeeBase::from_rust(self.inner.base)
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
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

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_re::ManagementFeeSpec =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ManagementFeeSpec", &self.inner)
    }
}

/// Standard node ids for the full property operating-statement template.
#[pyclass(
    name = "PropertyTemplateNodes",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyPropertyTemplateNodes {
    pub(crate) inner: rust_re::PropertyTemplateNodes,
}

#[pymethods]
impl PyPropertyTemplateNodes {
    #[new]
    #[pyo3(signature = (
        rent_roll=None,
        other_income_total_node="other_income_total",
        egi_node="egi",
        management_fee_node="management_fee",
        opex_total_node="opex_total",
        noi_node="noi",
        capex_total_node="capex_total",
        ncf_node="ncf",
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        rent_roll: Option<PyRentRollOutputNodes>,
        other_income_total_node: &str,
        egi_node: &str,
        management_fee_node: &str,
        opex_total_node: &str,
        noi_node: &str,
        capex_total_node: &str,
        ncf_node: &str,
    ) -> Self {
        Self {
            inner: rust_re::PropertyTemplateNodes {
                rent_roll: rent_roll.map(|r| r.inner).unwrap_or_default(),
                other_income_total_node: other_income_total_node.to_string(),
                egi_node: egi_node.to_string(),
                management_fee_node: management_fee_node.to_string(),
                opex_total_node: opex_total_node.to_string(),
                noi_node: noi_node.to_string(),
                capex_total_node: capex_total_node.to_string(),
                ncf_node: ncf_node.to_string(),
            },
        }
    }

    /// Rent-roll output node ids (PGI, free rent, vacancy loss, effective
    /// rent).
    #[getter]
    fn rent_roll(&self) -> PyRentRollOutputNodes {
        PyRentRollOutputNodes {
            inner: self.inner.rent_roll.clone(),
        }
    }

    /// Node id holding total other (non-rent) income.
    #[getter]
    fn other_income_total_node(&self) -> &str {
        &self.inner.other_income_total_node
    }

    /// Node id holding effective gross income (EGI).
    #[getter]
    fn egi_node(&self) -> &str {
        &self.inner.egi_node
    }

    /// Node id holding the management fee, when one is configured.
    #[getter]
    fn management_fee_node(&self) -> &str {
        &self.inner.management_fee_node
    }

    /// Node id holding total operating expenses, inclusive of the management
    /// fee when one is configured.
    #[getter]
    fn opex_total_node(&self) -> &str {
        &self.inner.opex_total_node
    }

    /// Node id holding net operating income (NOI).
    #[getter]
    fn noi_node(&self) -> &str {
        &self.inner.noi_node
    }

    /// Node id holding total capital expenditure.
    #[getter]
    fn capex_total_node(&self) -> &str {
        &self.inner.capex_total_node
    }

    /// Node id holding net cash flow, ``noi - capex_total``.
    #[getter]
    fn ncf_node(&self) -> &str {
        &self.inner.ncf_node
    }

    /// Export the node-id mapping as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``rent_pgi_node``, ``free_rent_node``, ``vacancy_loss_node``,
    /// ``rent_effective_node``, ``other_income_total_node``, ``egi_node``,
    /// ``management_fee_node``, ``opex_total_node``, ``noi_node``,
    /// ``capex_total_node``, ``ncf_node``.
    ///
    /// The four rent-roll node ids are flattened in rather than nested, so
    /// every value is a plain statement node id, not a numeric amount.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let row = serde_json::json!({
            "rent_pgi_node": self.inner.rent_roll.rent_pgi_node,
            "free_rent_node": self.inner.rent_roll.free_rent_node,
            "vacancy_loss_node": self.inner.rent_roll.vacancy_loss_node,
            "rent_effective_node": self.inner.rent_roll.rent_effective_node,
            "other_income_total_node": self.inner.other_income_total_node,
            "egi_node": self.inner.egi_node,
            "management_fee_node": self.inner.management_fee_node,
            "opex_total_node": self.inner.opex_total_node,
            "noi_node": self.inner.noi_node,
            "capex_total_node": self.inner.capex_total_node,
            "ncf_node": self.inner.ncf_node,
        });
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &[
                "rent_pgi_node",
                "free_rent_node",
                "vacancy_loss_node",
                "rent_effective_node",
                "other_income_total_node",
                "egi_node",
                "management_fee_node",
                "opex_total_node",
                "noi_node",
                "capex_total_node",
                "ncf_node",
            ],
        )
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
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

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_re::PropertyTemplateNodes =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("PropertyTemplateNodes", &self.inner)
    }
}

// add_noi_buildup

/// Apply the NOI buildup template to a model spec.
#[pyfunction]
fn add_noi_buildup(
    model: &Bound<'_, PyAny>,
    total_revenue_node: &str,
    revenue_nodes: Vec<String>,
    total_expenses_node: &str,
    expense_nodes: Vec<String>,
    noi_node: &str,
) -> PyResult<PyFinancialModelSpec> {
    let revenue_refs: Vec<&str> = revenue_nodes.iter().map(String::as_str).collect();
    let expense_refs: Vec<&str> = expense_nodes.iter().map(String::as_str).collect();
    let builder = rust_re::add_noi_buildup(
        extract_builder(model)?,
        total_revenue_node,
        &revenue_refs,
        total_expenses_node,
        &expense_refs,
        noi_node,
    )
    .map_err(crate::errors::statements_to_py)?;
    finish_builder(builder)
}

// add_ncf_buildup

/// Apply the NCF buildup template to a model spec.
#[pyfunction]
fn add_ncf_buildup(
    model: &Bound<'_, PyAny>,
    noi_node: &str,
    capex_nodes: Vec<String>,
    ncf_node: &str,
) -> PyResult<PyFinancialModelSpec> {
    let capex_refs: Vec<&str> = capex_nodes.iter().map(String::as_str).collect();
    let builder =
        rust_re::add_ncf_buildup(extract_builder(model)?, noi_node, &capex_refs, ncf_node)
            .map_err(crate::errors::statements_to_py)?;
    finish_builder(builder)
}

// add_rent_roll

/// Apply the rich rent-roll template to a model spec.
#[pyfunction]
#[pyo3(signature = (model, leases, nodes=None))]
fn add_rent_roll(
    model: &Bound<'_, PyAny>,
    leases: Vec<PyLeaseSpec>,
    nodes: Option<PyRentRollOutputNodes>,
) -> PyResult<PyFinancialModelSpec> {
    let lease_specs: Vec<rust_re::LeaseSpec> = leases.into_iter().map(|l| l.inner).collect();
    let nodes_inner = nodes.map(|n| n.inner).unwrap_or_default();
    let builder = rust_re::add_rent_roll(extract_builder(model)?, &lease_specs, &nodes_inner)
        .map_err(crate::errors::statements_to_py)?;
    finish_builder(builder)
}

// add_property_operating_statement

/// Apply the full property operating-statement template to a model spec.
#[pyfunction]
#[pyo3(signature = (
    model,
    leases,
    other_income_nodes=Vec::new(),
    opex_nodes=Vec::new(),
    capex_nodes=Vec::new(),
    management_fee=None,
    nodes=None,
))]
fn add_property_operating_statement(
    model: &Bound<'_, PyAny>,
    leases: Vec<PyLeaseSpec>,
    other_income_nodes: Vec<String>,
    opex_nodes: Vec<String>,
    capex_nodes: Vec<String>,
    management_fee: Option<PyManagementFeeSpec>,
    nodes: Option<PyPropertyTemplateNodes>,
) -> PyResult<PyFinancialModelSpec> {
    let lease_specs: Vec<rust_re::LeaseSpec> = leases.into_iter().map(|l| l.inner).collect();
    let other_refs: Vec<&str> = other_income_nodes.iter().map(String::as_str).collect();
    let opex_refs: Vec<&str> = opex_nodes.iter().map(String::as_str).collect();
    let capex_refs: Vec<&str> = capex_nodes.iter().map(String::as_str).collect();
    let nodes_inner = nodes.map(|n| n.inner).unwrap_or_default();
    let fee = management_fee.map(|f| f.inner);
    let builder = rust_re::add_property_operating_statement(
        extract_builder(model)?,
        &lease_specs,
        &other_refs,
        &opex_refs,
        &capex_refs,
        fee,
        &nodes_inner,
    )
    .map_err(crate::errors::statements_to_py)?;
    finish_builder(builder)
}

/// Register real-estate template types and functions on the parent module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyRentStepSpec>()?;
    m.add_class::<PyFreeRentWindowSpec>()?;
    m.add_class::<PyRenewalSpec>()?;
    m.add_class::<PyLeaseGrowthConvention>()?;
    m.add_class::<PyLeaseSpec>()?;
    m.add_class::<PyRentRollOutputNodes>()?;
    m.add_class::<PyManagementFeeBase>()?;
    m.add_class::<PyManagementFeeSpec>()?;
    m.add_class::<PyPropertyTemplateNodes>()?;
    m.add_function(pyo3::wrap_pyfunction!(add_noi_buildup, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(add_ncf_buildup, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(add_rent_roll, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(add_property_operating_statement, m)?)?;
    Ok(())
}
