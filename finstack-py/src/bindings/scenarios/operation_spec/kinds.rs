//! Supporting enum wrappers for scenario operations.

use crate::errors::display_to_py;
use finstack_scenarios::spec::{
    Compounding, CurveKind, TenorMatchMode, TimeRollMode, VolSurfaceKind,
};
use pyo3::prelude::*;
use pyo3::types::PyType;

// ---------------------------------------------------------------------------
// CurveKind
// ---------------------------------------------------------------------------

/// Type of market curve targeted by a scenario operation.
///
/// Mirrors [`finstack_scenarios::CurveKind`]. Serde renames `forward` and
/// `par_cds` are preserved on the JSON wire format.
#[pyclass(
    name = "CurveKind",
    module = "finstack.scenarios",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyCurveKind {
    pub(crate) inner: CurveKind,
}

#[pymethods]
impl PyCurveKind {
    /// Discount factor curve.
    #[classmethod]
    fn discount(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: CurveKind::Discount,
        }
    }

    /// Forward rate curve.
    #[classmethod]
    fn forward(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: CurveKind::Forward,
        }
    }

    /// Par CDS spread curve.
    #[classmethod]
    fn par_cds(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: CurveKind::ParCDS,
        }
    }

    /// Inflation index curve.
    #[classmethod]
    fn inflation(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: CurveKind::Inflation,
        }
    }

    /// Commodity forward curve.
    #[classmethod]
    fn commodity(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: CurveKind::Commodity,
        }
    }

    /// Variant name, e.g. ``"Discount"``.
    #[getter]
    fn name(&self) -> String {
        format!("{:?}", self.inner)
    }

    /// Serialized wire value, e.g. ``"discount"`` or ``"par_cds"``.
    #[getter]
    fn value(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map(|s| s.trim_matches('"').to_string())
            .map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        format!("CurveKind.{:?}", self.inner)
    }
}

// ---------------------------------------------------------------------------
// VolSurfaceKind
// ---------------------------------------------------------------------------

/// Category of volatility surface targeted by a scenario operation.
#[pyclass(
    name = "VolSurfaceKind",
    module = "finstack.scenarios",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyVolSurfaceKind {
    pub(crate) inner: VolSurfaceKind,
}

#[pymethods]
impl PyVolSurfaceKind {
    /// Equity volatility surface.
    #[classmethod]
    fn equity(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: VolSurfaceKind::Equity,
        }
    }

    /// Credit volatility surface.
    #[classmethod]
    fn credit(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: VolSurfaceKind::Credit,
        }
    }

    /// Swaption volatility surface.
    #[classmethod]
    fn swaption(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: VolSurfaceKind::Swaption,
        }
    }

    #[getter]
    fn name(&self) -> String {
        format!("{:?}", self.inner)
    }

    #[getter]
    fn value(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map(|s| s.trim_matches('"').to_string())
            .map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        format!("VolSurfaceKind.{:?}", self.inner)
    }
}

// ---------------------------------------------------------------------------
// TenorMatchMode
// ---------------------------------------------------------------------------

/// Tenor-pillar alignment strategy for curve-node operations.
#[pyclass(
    name = "TenorMatchMode",
    module = "finstack.scenarios",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyTenorMatchMode {
    pub(crate) inner: TenorMatchMode,
}

#[pymethods]
impl PyTenorMatchMode {
    /// Match exact pillar only (errors if missing).
    #[classmethod]
    fn exact(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: TenorMatchMode::Exact,
        }
    }

    /// Interpolate the bump across adjacent knots.
    #[classmethod]
    fn interpolate(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: TenorMatchMode::Interpolate,
        }
    }

    #[getter]
    fn name(&self) -> String {
        format!("{:?}", self.inner)
    }

    #[getter]
    fn value(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map(|s| s.trim_matches('"').to_string())
            .map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        format!("TenorMatchMode.{:?}", self.inner)
    }
}

// ---------------------------------------------------------------------------
// TimeRollMode
// ---------------------------------------------------------------------------

/// Calendar-vs-business-day semantics for time-roll operations.
#[pyclass(
    name = "TimeRollMode",
    module = "finstack.scenarios",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyTimeRollMode {
    pub(crate) inner: TimeRollMode,
}

#[pymethods]
impl PyTimeRollMode {
    /// Business-day-aware roll (respects calendars when provided).
    #[classmethod]
    fn business_days(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: TimeRollMode::BusinessDays,
        }
    }

    /// Pure calendar-day arithmetic.
    #[classmethod]
    fn calendar_days(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: TimeRollMode::CalendarDays,
        }
    }

    /// Approximate day-count mode (see Rust docs for non-additivity caveats).
    #[classmethod]
    fn approximate(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: TimeRollMode::Approximate,
        }
    }

    #[getter]
    fn name(&self) -> String {
        format!("{:?}", self.inner)
    }

    #[getter]
    fn value(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map(|s| s.trim_matches('"').to_string())
            .map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        format!("TimeRollMode.{:?}", self.inner)
    }
}

// ---------------------------------------------------------------------------
// Compounding
// ---------------------------------------------------------------------------

/// Compounding convention for rate-extraction operations.
#[pyclass(
    name = "Compounding",
    module = "finstack.scenarios",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyCompounding {
    pub(crate) inner: Compounding,
}

#[pymethods]
impl PyCompounding {
    /// Simple interest (no compounding).
    #[classmethod]
    fn simple(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: Compounding::Simple,
        }
    }

    /// Continuous compounding (default).
    #[classmethod]
    fn continuous(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: Compounding::Continuous,
        }
    }

    /// Annual compounding.
    #[classmethod]
    fn annual(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: Compounding::Annual,
        }
    }

    /// Semi-annual compounding.
    #[classmethod]
    fn semi_annual(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: Compounding::SemiAnnual,
        }
    }

    /// Quarterly compounding.
    #[classmethod]
    fn quarterly(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: Compounding::Quarterly,
        }
    }

    /// Monthly compounding.
    #[classmethod]
    fn monthly(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: Compounding::Monthly,
        }
    }

    #[getter]
    fn name(&self) -> String {
        format!("{:?}", self.inner)
    }

    #[getter]
    fn value(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map(|s| s.trim_matches('"').to_string())
            .map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        format!("Compounding.{:?}", self.inner)
    }
}
