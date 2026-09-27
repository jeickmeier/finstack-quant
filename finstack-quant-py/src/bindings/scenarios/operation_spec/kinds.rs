//! Supporting enum wrappers for scenario operations.
//!
//! Each wrapper exposes one classmethod per Rust variant, a constructor from
//! the canonical snake-case wire label, ``name`` / ``value`` accessors, and
//! value semantics (``==`` / ``hash``).

use finstack_quant_scenarios::spec::{Compounding, CurveKind, TenorMatchMode, TimeRollMode};
use pyo3::prelude::*;
use pyo3::types::PyType;

use super::helpers::{enum_to_label, label_to_enum};

macro_rules! scenario_enum {
    (
        $(#[$meta:meta])*
        $py_name:literal, $wrapper:ident, $inner:ident, $accepted:literal,
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
                    inner: label_to_enum::<$inner>($py_name, label, $accepted)?,
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
                enum_to_label(&self.inner)
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
    "CurveKind", PyCurveKind, CurveKind, "discount, forward, par_cds, inflation, commodity",
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
    "TenorMatchMode", PyTenorMatchMode, TenorMatchMode, "exact, interpolate",
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
    "TimeRollMode", PyTimeRollMode, TimeRollMode, "business_days, calendar_days, approximate",
    {
        /// Business-day-aware roll (respects calendars when provided).
        business_days => BusinessDays,
        /// Pure calendar-day arithmetic.
        calendar_days => CalendarDays,
        /// Approximate day-count mode (non-additive across successive rolls).
        approximate => Approximate,
    }
);

/// Accepted compounding labels, as parsed by core `Compounding::from_str`.
pub(super) const COMPOUNDING_LABELS: &str =
    "simple, continuous, annual, semi_annual, quarterly, monthly";

/// Parse a compounding label via the canonical core `FromStr`.
pub(super) fn parse_compounding(label: &str) -> PyResult<Compounding> {
    label.parse::<Compounding>().map_err(|_| {
        crate::errors::value_error(format!(
            "Unknown Compounding label {label:?}; expected one of: {COMPOUNDING_LABELS}"
        ))
    })
}

/// Compounding convention for rate-extraction operations.
///
/// Wraps the canonical core ``Compounding`` (the scenario wire form is
/// ``"continuous"``, ``"simple"``, ``"annual"`` or ``{"periodic": n}``).
///
/// Examples
/// --------
/// >>> from finstack_quant.scenarios import Compounding
/// >>> Compounding("annual") == Compounding.annual()
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
            inner: parse_compounding(label)?,
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

    /// Variant name, e.g. ``"SemiAnnual"``.
    #[getter]
    fn name(&self) -> String {
        compounding_name(self.inner)
    }

    /// Canonical label, e.g. ``"semi_annual"``.
    #[getter]
    fn value(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("Compounding.{}", compounding_name(self.inner))
    }
}

fn compounding_name(value: Compounding) -> String {
    match value {
        Compounding::Simple => "Simple".to_string(),
        Compounding::Continuous => "Continuous".to_string(),
        Compounding::Annual => "Annual".to_string(),
        Compounding::Periodic(n) => match n.get() {
            2 => "SemiAnnual".to_string(),
            4 => "Quarterly".to_string(),
            12 => "Monthly".to_string(),
            other => format!("Periodic({other})"),
        },
    }
}
