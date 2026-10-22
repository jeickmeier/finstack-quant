//! Supporting enum wrappers for scenario operations.
//!
//! Each wrapper exposes one classmethod per Rust variant, a constructor from
//! the canonical snake-case wire label, ``name`` / ``value`` accessors, and
//! value semantics (``==`` / ``hash``).

use finstack_quant_core::math::Compounding;
use finstack_quant_scenarios::{CurveKind, TenorMatchMode, TimeRollMode};
use pyo3::prelude::*;
use pyo3::types::PyType;

macro_rules! scenario_enum {
    (
        $(#[$meta:meta])*
        $py_name:literal, $wrapper:ident, $inner:ident,
        { $( $(#[$vmeta:meta])* $method:ident => $variant:ident ),+ $(,)? }
    ) => {
        $(#[$meta])*
        #[pyclass(
            name = $py_name,
            module = "finstack_quant.scenarios",
            eq,
            hash,
            frozen,
            from_py_object
        )]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub struct $wrapper {
            pub(crate) inner: $inner,
        }

        #[pymethods]
        impl $wrapper {
            #[new]
            fn new(label: &str) -> PyResult<Self> {
                Ok(Self {
                    inner: finstack_quant_core::wire::serde_parse::<$inner>(label).map_err(crate::errors::core_to_py)?,
                })
            }

            $(
                $(#[$vmeta])*
                #[classmethod]
                fn $method(_cls: &Bound<'_, PyType>) -> Self {
                    Self { inner: $inner::$variant }
                }
            )+

            /// Rust variant name, e.g. ``"Discount"``.
            #[getter]
            fn name(&self) -> String {
                format!("{:?}", self.inner)
            }

            /// Serialized wire label, e.g. ``"discount"`` or ``"par_cds"``.
            #[getter]
            fn value(&self) -> PyResult<String> {
                finstack_quant_core::wire::serde_label(&self.inner).map_err(crate::errors::core_to_py)
            }

            fn __repr__(&self) -> String {
                format!("{}.{:?}", $py_name, self.inner)
            }
        }
    };
}

scenario_enum!(
    /// Type of market curve targeted by a scenario operation.
    ///
    /// Construct from a wire label (``CurveKind("par_cds")``) or a classmethod
    /// (``CurveKind.par_cds()``); every ``OperationSpec`` constructor accepts
    /// either form.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.scenarios import CurveKind
    /// >>> CurveKind("par_cds") == CurveKind.par_cds()
    /// True
    /// >>> CurveKind.discount().value
    /// 'discount'
    "CurveKind", PyCurveKind, CurveKind,
    {
        /// Discount factor curve.
        discount => Discount,
        /// Forward rate curve.
        forward => Forward,
        /// Par CDS spread curve.
        par_cds => ParCDS,
        /// Inflation index curve.
        inflation => Inflation,
        /// Commodity forward (price) curve. Basis-point shocks on this kind are
        /// interpreted as percent of the forward, not additive bp.
        commodity => Commodity,
    }
);

scenario_enum!(
    /// Tenor-pillar alignment strategy for curve-node operations.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.scenarios import TenorMatchMode
    /// >>> TenorMatchMode("interpolate") == TenorMatchMode.interpolate()
    /// True
    "TenorMatchMode", PyTenorMatchMode, TenorMatchMode,
    {
        /// Match the exact pillar only (errors if missing).
        exact => Exact,
        /// Interpolate the bump across adjacent knots.
        interpolate => Interpolate,
    }
);

scenario_enum!(
    /// Calendar-vs-business-day semantics for time-roll operations.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.scenarios import TimeRollMode
    /// >>> TimeRollMode("calendar_days").value
    /// 'calendar_days'
    "TimeRollMode", PyTimeRollMode, TimeRollMode,
    {
        /// Business-day-aware roll (respects calendars when provided).
        business_days => BusinessDays,
        /// Pure calendar-day arithmetic.
        calendar_days => CalendarDays,
        /// Approximate day-count mode (non-additive across successive rolls).
        approximate => Approximate,
    }
);

/// Compounding convention for rate-extraction operations.
///
/// Wraps the canonical core ``Compounding`` (the scenario wire form is
/// ``"continuous"``, ``"simple"`` or ``{"periodic": n}``).
///
/// Examples
/// --------
/// >>> from finstack_quant.scenarios import Compounding
/// >>> Compounding("periodic(1)").value == "periodic(1)"
/// True
#[pyclass(
    name = "Compounding",
    module = "finstack_quant.scenarios",
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
    #[new]
    fn new(label: &str) -> PyResult<Self> {
        Ok(Self {
            inner: label.parse().map_err(crate::errors::core_to_py)?,
        })
    }

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

    /// Semi-annual compounding.
    #[classmethod]
    fn semi_annual(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: Compounding::SEMI_ANNUAL,
        }
    }

    /// Quarterly compounding.
    #[classmethod]
    fn quarterly(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: Compounding::QUARTERLY,
        }
    }

    /// Monthly compounding.
    #[classmethod]
    fn monthly(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: Compounding::MONTHLY,
        }
    }

    /// The Rust label, e.g. ``"semi_annual"`` (same as ``value``).
    #[getter]
    fn name(&self) -> String {
        self.inner.to_string()
    }

    /// Canonical label, e.g. ``"semi_annual"``.
    #[getter]
    fn value(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("Compounding.{}", self.inner)
    }
}
