//! Python bindings for the real-estate operating-statement templates.
//!
//! Wraps [`finstack_statements_analytics::templates::real_estate`] — property-level
//! rent rolls, NOI / NCF buildups, and the full operating-statement template.
//!
//! Period-id arguments on lease specs are accepted as strings (e.g.
//! ``"2025Q1"``) and parsed via `PeriodId::FromStr`. Each binding rebuilds a
//! Rust `ModelBuilder` from a serialized [`FinancialModelSpec`], applies the
//! template, then returns the resulting spec as JSON. `meta` and
//! `capital_structure` from the input spec are preserved on output.

use crate::bindings::extract::extract_model_ref;
use crate::errors::display_to_py;
use finstack_core::dates::PeriodId;
use finstack_statements::builder::{ModelBuilder, Ready};
use finstack_statements::types::{CapitalStructureSpec, FinancialModelSpec};
use finstack_statements_analytics::templates::real_estate as rust_re;
use indexmap::IndexMap;
use pyo3::prelude::*;
use serde_json::Value;

/// Rebuilt model builder with preserved metadata.
type RebuiltBuilder = (
    ModelBuilder<Ready>,
    IndexMap<String, Value>,
    Option<CapitalStructureSpec>,
);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_period(s: &str) -> PyResult<PeriodId> {
    s.parse().map_err(display_to_py)
}

fn rebuild_builder(spec: FinancialModelSpec) -> PyResult<RebuiltBuilder> {
    let meta = spec.meta.clone();
    let capital_structure = spec.capital_structure.clone();
    let id = spec.id.clone();
    let periods = spec.periods.clone();
    let nodes = spec.nodes;

    let mut builder = ModelBuilder::new(id)
        .periods_explicit(periods)
        .map_err(display_to_py)?;
    for (node_id, node_spec) in nodes {
        builder.insert_node(node_id, node_spec);
    }
    Ok((builder, meta, capital_structure))
}

fn finalize_builder(
    builder: ModelBuilder<finstack_statements::builder::Ready>,
    meta: indexmap::IndexMap<String, serde_json::Value>,
    capital_structure: Option<finstack_statements::types::CapitalStructureSpec>,
) -> PyResult<String> {
    let mut new_spec = builder.build().map_err(display_to_py)?;
    new_spec.meta = meta;
    new_spec.capital_structure = capital_structure;
    serde_json::to_string(&new_spec).map_err(display_to_py)
}

// ---------------------------------------------------------------------------
// SimpleLeaseSpec
// ---------------------------------------------------------------------------

/// Lightweight per-lease rent schedule.
///
/// Parameters
/// ----------
/// node_id : str
///     Node id to store this lease's rent revenue series.
/// start : str
///     First active period (e.g. ``"2025Q1"``).
/// base_rent : float
///     Base rent per model period at ``start``.
/// end : str | None
///     Last active period (inclusive). ``None`` means through model end.
/// growth_rate : float
///     Growth rate applied per model period from ``start``.
/// free_rent_periods : int
///     Free-rent periods from ``start``.
/// occupancy : float
///     Occupancy factor in ``[0, 1]``.
#[pyclass(
    name = "SimpleLeaseSpec",
    module = "finstack.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PySimpleLeaseSpec {
    pub(crate) inner: rust_re::SimpleLeaseSpec,
}

#[pymethods]
impl PySimpleLeaseSpec {
    #[new]
    #[pyo3(signature = (node_id, start, base_rent, end=None, growth_rate=0.0, free_rent_periods=0, occupancy=1.0))]
    fn new(
        node_id: &str,
        start: &str,
        base_rent: f64,
        end: Option<&str>,
        growth_rate: f64,
        free_rent_periods: u32,
        occupancy: f64,
    ) -> PyResult<Self> {
        let inner = rust_re::SimpleLeaseSpec {
            node_id: node_id.to_string(),
            start: parse_period(start)?,
            end: end.map(parse_period).transpose()?,
            base_rent,
            growth_rate,
            free_rent_periods,
            occupancy,
        };
        Ok(Self { inner })
    }

    #[getter]
    fn node_id(&self) -> &str {
        &self.inner.node_id
    }

    #[getter]
    fn start(&self) -> String {
        self.inner.start.to_string()
    }

    #[getter]
    fn end(&self) -> Option<String> {
        self.inner.end.map(|p| p.to_string())
    }

    #[getter]
    fn base_rent(&self) -> f64 {
        self.inner.base_rent
    }

    #[getter]
    fn growth_rate(&self) -> f64 {
        self.inner.growth_rate
    }

    #[getter]
    fn free_rent_periods(&self) -> u32 {
        self.inner.free_rent_periods
    }

    #[getter]
    fn occupancy(&self) -> f64 {
        self.inner.occupancy
    }

    /// Validate lease fields.
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(display_to_py)
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_re::SimpleLeaseSpec = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }
}

// ---------------------------------------------------------------------------
// RentStepSpec / FreeRentWindowSpec / RenewalSpec
// ---------------------------------------------------------------------------

/// Rent step that resets the base rent starting at ``start`` (inclusive).
#[pyclass(
    name = "RentStepSpec",
    module = "finstack.statements_analytics",
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

    #[getter]
    fn start(&self) -> String {
        self.inner.start.to_string()
    }

    #[getter]
    fn rent(&self) -> f64 {
        self.inner.rent
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_re::RentStepSpec = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }
}

/// Free rent (concession) window that zeros out rent for ``periods`` starting at ``start``.
#[pyclass(
    name = "FreeRentWindowSpec",
    module = "finstack.statements_analytics",
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

    #[getter]
    fn start(&self) -> String {
        self.inner.start.to_string()
    }

    #[getter]
    fn periods(&self) -> u32 {
        self.inner.periods
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_re::FreeRentWindowSpec =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }
}

/// Renewal specification modeled in expected-value terms.
#[pyclass(
    name = "RenewalSpec",
    module = "finstack.statements_analytics",
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

    #[getter]
    fn term_periods(&self) -> u32 {
        self.inner.term_periods
    }

    #[getter]
    fn probability(&self) -> f64 {
        self.inner.probability
    }

    #[getter]
    fn downtime_periods(&self) -> u32 {
        self.inner.downtime_periods
    }

    #[getter]
    fn rent_factor(&self) -> f64 {
        self.inner.rent_factor
    }

    #[getter]
    fn free_rent_periods(&self) -> u32 {
        self.inner.free_rent_periods
    }

    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(display_to_py)
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_re::RenewalSpec = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }
}

// ---------------------------------------------------------------------------
// LeaseGrowthConvention
// ---------------------------------------------------------------------------

/// Compounding convention for lease rent growth.
#[pyclass(
    name = "LeaseGrowthConvention",
    module = "finstack.statements_analytics",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyLeaseGrowthConvention {
    PerPeriod,
    AnnualEscalator,
}

#[pymethods]
impl PyLeaseGrowthConvention {
    /// Parse from a string identifier (``"per_period"`` or ``"annual_escalator"``).
    #[staticmethod]
    fn from_str(value: &str) -> PyResult<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "per_period" | "perperiod" => Ok(PyLeaseGrowthConvention::PerPeriod),
            "annual_escalator" | "annualescalator" => Ok(PyLeaseGrowthConvention::AnnualEscalator),
            other => Err(pyo3::exceptions::PyValueError::new_err(format!(
                "unknown lease growth convention '{}' (expected per_period / annual_escalator)",
                other
            ))),
        }
    }

    fn value(&self) -> &'static str {
        match self {
            PyLeaseGrowthConvention::PerPeriod => "per_period",
            PyLeaseGrowthConvention::AnnualEscalator => "annual_escalator",
        }
    }

    fn __repr__(&self) -> String {
        format!("LeaseGrowthConvention.{}", self.value())
    }
}

impl PyLeaseGrowthConvention {
    fn to_rust(self) -> rust_re::LeaseGrowthConvention {
        match self {
            PyLeaseGrowthConvention::PerPeriod => rust_re::LeaseGrowthConvention::PerPeriod,
            PyLeaseGrowthConvention::AnnualEscalator => {
                rust_re::LeaseGrowthConvention::AnnualEscalator
            }
        }
    }

    fn from_rust(value: rust_re::LeaseGrowthConvention) -> Self {
        match value {
            rust_re::LeaseGrowthConvention::PerPeriod => PyLeaseGrowthConvention::PerPeriod,
            rust_re::LeaseGrowthConvention::AnnualEscalator => {
                PyLeaseGrowthConvention::AnnualEscalator
            }
        }
    }
}

// ---------------------------------------------------------------------------
// LeaseSpec
// ---------------------------------------------------------------------------

/// Rich lease spec for rent-roll generation.
#[pyclass(
    name = "LeaseSpec",
    module = "finstack.statements_analytics",
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
        growth_convention=PyLeaseGrowthConvention::PerPeriod,
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
            growth_convention: growth_convention.to_rust(),
            rent_steps: rent_steps.into_iter().map(|s| s.inner).collect(),
            free_rent_periods,
            free_rent_windows: free_rent_windows.into_iter().map(|w| w.inner).collect(),
            occupancy,
            renewal: renewal.map(|r| r.inner),
        };
        Ok(Self { inner })
    }

    #[getter]
    fn node_id(&self) -> &str {
        &self.inner.node_id
    }

    #[getter]
    fn start(&self) -> String {
        self.inner.start.to_string()
    }

    #[getter]
    fn end(&self) -> Option<String> {
        self.inner.end.map(|p| p.to_string())
    }

    #[getter]
    fn base_rent(&self) -> f64 {
        self.inner.base_rent
    }

    #[getter]
    fn growth_rate(&self) -> f64 {
        self.inner.growth_rate
    }

    #[getter]
    fn growth_convention(&self) -> PyLeaseGrowthConvention {
        PyLeaseGrowthConvention::from_rust(self.inner.growth_convention)
    }

    #[getter]
    fn free_rent_periods(&self) -> u32 {
        self.inner.free_rent_periods
    }

    #[getter]
    fn occupancy(&self) -> f64 {
        self.inner.occupancy
    }

    #[getter]
    fn renewal(&self) -> Option<PyRenewalSpec> {
        self.inner
            .renewal
            .clone()
            .map(|inner| PyRenewalSpec { inner })
    }

    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(display_to_py)
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_re::LeaseSpec = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }
}

// ---------------------------------------------------------------------------
// RentRollOutputNodes
// ---------------------------------------------------------------------------

/// Standard aggregated output node ids for a rent roll.
#[pyclass(
    name = "RentRollOutputNodes",
    module = "finstack.statements_analytics",
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

    #[getter]
    fn rent_pgi_node(&self) -> &str {
        &self.inner.rent_pgi_node
    }

    #[getter]
    fn free_rent_node(&self) -> &str {
        &self.inner.free_rent_node
    }

    #[getter]
    fn vacancy_loss_node(&self) -> &str {
        &self.inner.vacancy_loss_node
    }

    #[getter]
    fn rent_effective_node(&self) -> &str {
        &self.inner.rent_effective_node
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_re::RentRollOutputNodes =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }
}

// ---------------------------------------------------------------------------
// ManagementFeeBase / ManagementFeeSpec
// ---------------------------------------------------------------------------

/// Basis for management fee calculation.
#[pyclass(
    name = "ManagementFeeBase",
    module = "finstack.statements_analytics",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyManagementFeeBase {
    Egi,
    EffectiveRent,
}

#[pymethods]
impl PyManagementFeeBase {
    #[staticmethod]
    fn from_str(value: &str) -> PyResult<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "egi" => Ok(PyManagementFeeBase::Egi),
            "effective_rent" | "effectiverent" => Ok(PyManagementFeeBase::EffectiveRent),
            other => Err(pyo3::exceptions::PyValueError::new_err(format!(
                "unknown management fee base '{}' (expected egi / effective_rent)",
                other
            ))),
        }
    }

    fn value(&self) -> &'static str {
        match self {
            PyManagementFeeBase::Egi => "egi",
            PyManagementFeeBase::EffectiveRent => "effective_rent",
        }
    }

    fn __repr__(&self) -> String {
        format!("ManagementFeeBase.{}", self.value())
    }
}

impl PyManagementFeeBase {
    fn to_rust(self) -> rust_re::ManagementFeeBase {
        match self {
            PyManagementFeeBase::Egi => rust_re::ManagementFeeBase::Egi,
            PyManagementFeeBase::EffectiveRent => rust_re::ManagementFeeBase::EffectiveRent,
        }
    }

    fn from_rust(value: rust_re::ManagementFeeBase) -> Self {
        match value {
            rust_re::ManagementFeeBase::Egi => PyManagementFeeBase::Egi,
            rust_re::ManagementFeeBase::EffectiveRent => PyManagementFeeBase::EffectiveRent,
        }
    }
}

/// Management fee specification.
#[pyclass(
    name = "ManagementFeeSpec",
    module = "finstack.statements_analytics",
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
    fn new(rate: f64, base: PyManagementFeeBase) -> Self {
        Self {
            inner: rust_re::ManagementFeeSpec {
                rate,
                base: base.to_rust(),
            },
        }
    }

    #[getter]
    fn rate(&self) -> f64 {
        self.inner.rate
    }

    #[getter]
    fn base(&self) -> PyManagementFeeBase {
        PyManagementFeeBase::from_rust(self.inner.base)
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_re::ManagementFeeSpec =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }
}

// ---------------------------------------------------------------------------
// PropertyTemplateNodes
// ---------------------------------------------------------------------------

/// Standard node ids for the full property operating-statement template.
#[pyclass(
    name = "PropertyTemplateNodes",
    module = "finstack.statements_analytics",
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

    #[getter]
    fn rent_roll(&self) -> PyRentRollOutputNodes {
        PyRentRollOutputNodes {
            inner: self.inner.rent_roll.clone(),
        }
    }

    #[getter]
    fn other_income_total_node(&self) -> &str {
        &self.inner.other_income_total_node
    }

    #[getter]
    fn egi_node(&self) -> &str {
        &self.inner.egi_node
    }

    #[getter]
    fn management_fee_node(&self) -> &str {
        &self.inner.management_fee_node
    }

    #[getter]
    fn opex_total_node(&self) -> &str {
        &self.inner.opex_total_node
    }

    #[getter]
    fn noi_node(&self) -> &str {
        &self.inner.noi_node
    }

    #[getter]
    fn capex_total_node(&self) -> &str {
        &self.inner.capex_total_node
    }

    #[getter]
    fn ncf_node(&self) -> &str {
        &self.inner.ncf_node
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: rust_re::PropertyTemplateNodes =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }
}

// ---------------------------------------------------------------------------
// add_noi_buildup
// ---------------------------------------------------------------------------

/// Apply the NOI buildup template to a model spec.
#[pyfunction]
fn add_noi_buildup(
    model: &Bound<'_, PyAny>,
    total_revenue_node: &str,
    revenue_nodes: Vec<String>,
    total_expenses_node: &str,
    expense_nodes: Vec<String>,
    noi_node: &str,
) -> PyResult<String> {
    let spec = extract_model_ref(model)?.into_owned();
    let (builder, meta, capital_structure) = rebuild_builder(spec)?;
    let revenue_refs: Vec<&str> = revenue_nodes.iter().map(String::as_str).collect();
    let expense_refs: Vec<&str> = expense_nodes.iter().map(String::as_str).collect();
    let builder = rust_re::add_noi_buildup(
        builder,
        total_revenue_node,
        &revenue_refs,
        total_expenses_node,
        &expense_refs,
        noi_node,
    )
    .map_err(display_to_py)?;
    finalize_builder(builder, meta, capital_structure)
}

// ---------------------------------------------------------------------------
// add_ncf_buildup
// ---------------------------------------------------------------------------

/// Apply the NCF buildup template to a model spec.
#[pyfunction]
fn add_ncf_buildup(
    model: &Bound<'_, PyAny>,
    noi_node: &str,
    capex_nodes: Vec<String>,
    ncf_node: &str,
) -> PyResult<String> {
    let spec = extract_model_ref(model)?.into_owned();
    let (builder, meta, capital_structure) = rebuild_builder(spec)?;
    let capex_refs: Vec<&str> = capex_nodes.iter().map(String::as_str).collect();
    let builder = rust_re::add_ncf_buildup(builder, noi_node, &capex_refs, ncf_node)
        .map_err(display_to_py)?;
    finalize_builder(builder, meta, capital_structure)
}

// ---------------------------------------------------------------------------
// add_rent_roll
// ---------------------------------------------------------------------------

/// Apply the rich rent-roll template to a model spec.
#[pyfunction]
#[pyo3(signature = (model, leases, nodes=None))]
fn add_rent_roll(
    model: &Bound<'_, PyAny>,
    leases: Vec<PyLeaseSpec>,
    nodes: Option<PyRentRollOutputNodes>,
) -> PyResult<String> {
    let spec = extract_model_ref(model)?.into_owned();
    let (builder, meta, capital_structure) = rebuild_builder(spec)?;
    let lease_specs: Vec<rust_re::LeaseSpec> = leases.into_iter().map(|l| l.inner).collect();
    let nodes_inner = nodes.map(|n| n.inner).unwrap_or_default();
    let builder =
        rust_re::add_rent_roll(builder, &lease_specs, &nodes_inner).map_err(display_to_py)?;
    finalize_builder(builder, meta, capital_structure)
}

// ---------------------------------------------------------------------------
// add_rent_roll_rental_revenue
// ---------------------------------------------------------------------------

/// Apply the simple rent-roll rental revenue template to a model spec.
#[pyfunction]
fn add_rent_roll_rental_revenue(
    model: &Bound<'_, PyAny>,
    leases: Vec<PySimpleLeaseSpec>,
    total_rent_node: &str,
) -> PyResult<String> {
    let spec = extract_model_ref(model)?.into_owned();
    let (builder, meta, capital_structure) = rebuild_builder(spec)?;
    let lease_specs: Vec<rust_re::SimpleLeaseSpec> = leases.into_iter().map(|l| l.inner).collect();
    let builder = rust_re::add_rent_roll_rental_revenue(builder, &lease_specs, total_rent_node)
        .map_err(display_to_py)?;
    finalize_builder(builder, meta, capital_structure)
}

// ---------------------------------------------------------------------------
// add_property_operating_statement
// ---------------------------------------------------------------------------

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
) -> PyResult<String> {
    let spec = extract_model_ref(model)?.into_owned();
    let (builder, meta, capital_structure) = rebuild_builder(spec)?;
    let lease_specs: Vec<rust_re::LeaseSpec> = leases.into_iter().map(|l| l.inner).collect();
    let other_refs: Vec<&str> = other_income_nodes.iter().map(String::as_str).collect();
    let opex_refs: Vec<&str> = opex_nodes.iter().map(String::as_str).collect();
    let capex_refs: Vec<&str> = capex_nodes.iter().map(String::as_str).collect();
    let nodes_inner = nodes.map(|n| n.inner).unwrap_or_default();
    let fee = management_fee.map(|f| f.inner);
    let builder = rust_re::add_property_operating_statement(
        builder,
        &lease_specs,
        &other_refs,
        &opex_refs,
        &capex_refs,
        fee,
        &nodes_inner,
    )
    .map_err(display_to_py)?;
    finalize_builder(builder, meta, capital_structure)
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register real-estate template types and functions on the parent module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySimpleLeaseSpec>()?;
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
    m.add_function(pyo3::wrap_pyfunction!(add_rent_roll_rental_revenue, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(add_property_operating_statement, m)?)?;
    Ok(())
}
