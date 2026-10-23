//! Spec-type bindings for `finstack_quant_cashflows::builder`.

use crate::bindings::macros::wire_methods;
use crate::bindings::pydoc::pydoc;
use finstack_quant_cashflows::builder::{
    CouponType, FixedCouponSpec, FloatingCouponSpec, FloatingLegCompounding, FloatingRateFallback,
    FloatingRateSpec, OvernightIndexConstraintApplication, StepUpCouponSpec,
};
use pyo3::prelude::*;
use rust_decimal::Decimal;

use super::*;
use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::tenor::{extract_tenor, PyTenor};
use crate::bindings::core::money::decimal_to_py;
use crate::bindings::date_utils::date_to_py;
use crate::bindings::repr_support::{enum_repr_from_serde as enum_repr, repr_from_serde};
use crate::errors::core_to_py;

#[doc = pydoc!("finstack_quant.cashflows.builder", "CouponType")]
#[pyclass(
    name = "CouponType",
    module = "finstack_quant.cashflows.builder",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyCouponType {
    /// Inner coupon settlement type.
    pub(crate) inner: CouponType,
}

#[pymethods]
impl PyCouponType {
    /// 100% paid in cash.
    #[classattr]
    const CASH: PyCouponType = PyCouponType {
        inner: CouponType::Cash,
    };
    /// 100% capitalized into principal.
    #[classattr]
    const PIK: PyCouponType = PyCouponType {
        inner: CouponType::Pik,
    };

    #[doc = pydoc!("finstack_quant.cashflows.builder", "CouponType.split")]
    #[staticmethod]
    #[pyo3(text_signature = "(cash_fraction, pik_fraction)")]
    fn split(cash_fraction: &Bound<'_, PyAny>, pik_fraction: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: CouponType::Split {
                cash_fraction: decimal_from_any(cash_fraction)?,
                pik_fraction: decimal_from_any(pik_fraction)?,
            },
        })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "CouponType.cash_fraction")]
    #[getter]
    fn cash_fraction<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let value = match self.inner {
            CouponType::Cash => Decimal::ONE,
            CouponType::Pik => Decimal::ZERO,
            CouponType::Split { cash_fraction, .. } => cash_fraction,
        };
        decimal_to_py(py, value)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "CouponType.pik_fraction")]
    #[getter]
    fn pik_fraction<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let value = match self.inner {
            CouponType::Cash => Decimal::ZERO,
            CouponType::Pik => Decimal::ONE,
            CouponType::Split { pik_fraction, .. } => pik_fraction,
        };
        decimal_to_py(py, value)
    }

    /// Python-style representation.
    fn __repr__(&self) -> String {
        enum_repr("CouponType", &self.inner)
    }
}

#[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingLegCompounding")]
#[pyclass(
    name = "FloatingLegCompounding",
    module = "finstack_quant.cashflows.builder",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyFloatingLegCompounding {
    /// Inner compounding convention.
    pub(crate) inner: FloatingLegCompounding,
}

#[pymethods]
impl PyFloatingLegCompounding {
    /// Simple term-rate projection (one forward per period; the default).
    #[classattr]
    const SIMPLE: PyFloatingLegCompounding = PyFloatingLegCompounding {
        inner: FloatingLegCompounding::Simple,
    };
    /// Arithmetic average of daily fixings weighted by accrual days.
    #[classattr]
    const SIMPLE_AVERAGE: PyFloatingLegCompounding = PyFloatingLegCompounding {
        inner: FloatingLegCompounding::SimpleAverage,
    };

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingLegCompounding.compounded_in_arrears")]
    #[staticmethod]
    #[pyo3(text_signature = "(lookback_days)")]
    fn compounded_in_arrears(lookback_days: u32) -> Self {
        Self {
            inner: FloatingLegCompounding::CompoundedInArrears { lookback_days },
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingLegCompounding.compounded_with_observation_shift")]
    #[staticmethod]
    #[pyo3(text_signature = "(shift_days)")]
    fn compounded_with_observation_shift(shift_days: u32) -> Self {
        Self {
            inner: FloatingLegCompounding::CompoundedWithObservationShift { shift_days },
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingLegCompounding.sofr_observation_shift")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn sofr_observation_shift() -> Self {
        Self {
            inner: FloatingLegCompounding::sofr_observation_shift(),
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingLegCompounding.sonia_observation_shift")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn sonia_observation_shift() -> Self {
        Self {
            inner: FloatingLegCompounding::sonia_observation_shift(),
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingLegCompounding.rate_cutoff")]
    #[staticmethod]
    #[pyo3(text_signature = "(cutoff_days)")]
    fn rate_cutoff(cutoff_days: u32) -> Self {
        Self {
            inner: FloatingLegCompounding::rate_cutoff(cutoff_days),
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingLegCompounding.is_overnight")]
    #[pyo3(text_signature = "(self)")]
    fn is_overnight(&self) -> bool {
        self.inner.is_overnight()
    }

    /// Python-style representation.
    fn __repr__(&self) -> String {
        enum_repr("FloatingLegCompounding", &self.inner)
    }
}

#[doc = pydoc!("finstack_quant.cashflows.builder", "OvernightIndexConstraintApplication")]
#[pyclass(
    name = "OvernightIndexConstraintApplication",
    module = "finstack_quant.cashflows.builder",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyOvernightIndexConstraintApplication {
    /// Inner constraint-application policy.
    pub(crate) inner: OvernightIndexConstraintApplication,
}

#[pymethods]
impl PyOvernightIndexConstraintApplication {
    /// Apply index floors/caps to each daily fixing before compounding (default).
    #[classattr]
    const DAILY: PyOvernightIndexConstraintApplication = PyOvernightIndexConstraintApplication {
        inner: OvernightIndexConstraintApplication::Daily,
    };
    /// Apply index floors/caps once to the compounded period index rate.
    #[classattr]
    const PERIOD: PyOvernightIndexConstraintApplication = PyOvernightIndexConstraintApplication {
        inner: OvernightIndexConstraintApplication::Period,
    };

    /// Python-style representation.
    fn __repr__(&self) -> String {
        enum_repr("OvernightIndexConstraintApplication", &self.inner)
    }
}

#[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateFallback")]
#[pyclass(
    name = "FloatingRateFallback",
    module = "finstack_quant.cashflows.builder",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PyFloatingRateFallback {
    /// Inner fallback policy.
    pub(crate) inner: FloatingRateFallback,
}

#[pymethods]
impl PyFloatingRateFallback {
    /// Fail when required observations need missing market data (default).
    #[classattr]
    const ERROR: Self = Self {
        inner: FloatingRateFallback::Error,
    };
    /// Use a zero index rate for eligible missing market data (explicit opt-in).
    #[classattr]
    const SPREAD_ONLY: Self = Self {
        inner: FloatingRateFallback::SpreadOnly,
    };

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateFallback.fixed_rate")]
    #[staticmethod]
    #[pyo3(text_signature = "(rate)")]
    fn fixed_rate(rate: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: FloatingRateFallback::FixedRate(decimal_from_any(rate)?),
        })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateFallback.rate")]
    #[getter]
    fn rate<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        match self.inner {
            FloatingRateFallback::FixedRate(rate) => decimal_to_py(py, rate).map(Some),
            _ => Ok(None),
        }
    }

    /// Python-style representation.
    fn __repr__(&self) -> String {
        enum_repr("FloatingRateFallback", &self.inner)
    }
}

#[doc = pydoc!("finstack_quant.cashflows.builder", "FixedCouponSpec")]
#[pyclass(
    name = "FixedCouponSpec",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyFixedCouponSpec {
    /// Inner fixed-coupon spec.
    pub(crate) inner: FixedCouponSpec,
}

wire_methods!(
    PyFixedCouponSpec,
    FixedCouponSpec,
    "FixedCouponSpec",
    module = "finstack_quant.cashflows.builder"
);

#[pymethods]
impl PyFixedCouponSpec {
    #[doc = pydoc!("finstack_quant.cashflows.builder", "FixedCouponSpec.__init__")]
    #[new]
    #[pyo3(
        signature = (rate, schedule, coupon_type=None),
        text_signature = "(rate, schedule, coupon_type=None)"
    )]
    fn new(
        rate: &Bound<'_, PyAny>,
        schedule: PyRef<'_, PyScheduleParams>,
        coupon_type: Option<PyRef<'_, PyCouponType>>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: FixedCouponSpec {
                coupon_type: coupon_type.map(|c| c.inner).unwrap_or_default(),
                rate: decimal_from_any(rate)?,
                schedule: schedule.inner.clone(),
            },
        })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FixedCouponSpec.rate")]
    #[getter]
    fn rate<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        decimal_to_py(py, self.inner.rate)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FixedCouponSpec.schedule")]
    #[getter]
    fn schedule(&self) -> PyScheduleParams {
        PyScheduleParams::from_inner(self.inner.schedule.clone())
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FixedCouponSpec.coupon_type")]
    #[getter]
    fn coupon_type(&self) -> PyCouponType {
        PyCouponType {
            inner: self.inner.coupon_type,
        }
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        repr_from_serde("FixedCouponSpec", &self.inner)
    }
}

#[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec")]
#[pyclass(
    name = "FloatingRateSpec",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyFloatingRateSpec {
    /// Inner floating-rate spec.
    pub(crate) inner: FloatingRateSpec,
}

wire_methods!(
    PyFloatingRateSpec,
    FloatingRateSpec,
    "FloatingRateSpec",
    module = "finstack_quant.cashflows.builder",
    validate
);

#[pymethods]
impl PyFloatingRateSpec {
    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec.__init__")]
    #[new]
    #[pyo3(
        signature = (forward_curve_id, spread_bp, reset_frequency, gearing=None, gearing_includes_spread=FloatingRateSpec::DEFAULT_GEARING_INCLUDES_SPREAD, index_floor_bp=None, all_in_floor_bp=None, all_in_cap_bp=None, index_cap_bp=None, overnight_index_constraints=None, index_tenor=None, reset_lag_days=FloatingRateSpec::DEFAULT_RESET_LAG_DAYS, fixing_calendar_id=None, compounding=None, overnight_basis=None, fallback=None),
        text_signature = "(forward_curve_id, spread_bp, reset_frequency, gearing=None, gearing_includes_spread=True, index_floor_bp=None, all_in_floor_bp=None, all_in_cap_bp=None, index_cap_bp=None, overnight_index_constraints=None, index_tenor=None, reset_lag_days=2, fixing_calendar_id=None, compounding=None, overnight_basis=None, fallback=None)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn new(
        forward_curve_id: &str,
        spread_bp: &Bound<'_, PyAny>,
        reset_frequency: &Bound<'_, PyAny>,
        gearing: Option<&Bound<'_, PyAny>>,
        gearing_includes_spread: bool,
        index_floor_bp: Option<&Bound<'_, PyAny>>,
        all_in_floor_bp: Option<&Bound<'_, PyAny>>,
        all_in_cap_bp: Option<&Bound<'_, PyAny>>,
        index_cap_bp: Option<&Bound<'_, PyAny>>,
        overnight_index_constraints: Option<PyRef<'_, PyOvernightIndexConstraintApplication>>,
        index_tenor: Option<&Bound<'_, PyAny>>,
        reset_lag_days: i32,
        fixing_calendar_id: Option<String>,
        compounding: Option<PyRef<'_, PyFloatingLegCompounding>>,
        overnight_basis: Option<PyRef<'_, PyDayCount>>,
        fallback: Option<PyRef<'_, PyFloatingRateFallback>>,
    ) -> PyResult<Self> {
        let inner = FloatingRateSpec {
            forward_curve_id: finstack_quant_core::types::CurveId::from(forward_curve_id),
            spread_bp: decimal_from_any(spread_bp)?,
            gearing: gearing.map_or(Ok(FloatingRateSpec::DEFAULT_GEARING), decimal_from_any)?,
            gearing_includes_spread,
            index_floor_bp: index_floor_bp.map(decimal_from_any).transpose()?,
            all_in_floor_bp: all_in_floor_bp.map(decimal_from_any).transpose()?,
            all_in_cap_bp: all_in_cap_bp.map(decimal_from_any).transpose()?,
            index_cap_bp: index_cap_bp.map(decimal_from_any).transpose()?,
            overnight_index_constraints: overnight_index_constraints
                .map(|c| c.inner)
                .unwrap_or_default(),
            reset_frequency: extract_tenor(reset_frequency)?,
            index_tenor: index_tenor.map(extract_tenor).transpose()?,
            reset_lag_days,
            fixing_calendar_id: fixing_calendar_id.map(Into::into),
            compounding: compounding.map(|m| m.inner),
            overnight_basis: overnight_basis.map(|d| d.inner),
            fallback: fallback.map(|f| f.inner.clone()).unwrap_or_default(),
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec.sofr")]
    #[staticmethod]
    #[pyo3(text_signature = "(spread_bp)")]
    fn sofr(spread_bp: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: FloatingRateSpec::sofr(decimal_from_any(spread_bp)?),
        })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec.sonia")]
    #[staticmethod]
    #[pyo3(text_signature = "(spread_bp)")]
    fn sonia(spread_bp: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: FloatingRateSpec::sonia(decimal_from_any(spread_bp)?),
        })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec.euribor_3m")]
    #[staticmethod]
    #[pyo3(text_signature = "(spread_bp)")]
    fn euribor_3m(spread_bp: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: FloatingRateSpec::euribor_3m(decimal_from_any(spread_bp)?),
        })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec.forward_curve_id")]
    #[getter]
    fn forward_curve_id(&self) -> String {
        self.inner.forward_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec.spread_bp")]
    #[getter]
    fn spread_bp<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        decimal_to_py(py, self.inner.spread_bp)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec.gearing")]
    #[getter]
    fn gearing<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        decimal_to_py(py, self.inner.gearing)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec.gearing_includes_spread")]
    #[getter]
    fn gearing_includes_spread(&self) -> bool {
        self.inner.gearing_includes_spread
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec.index_floor_bp")]
    #[getter]
    fn index_floor_bp<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        decimal_opt_to_py(py, self.inner.index_floor_bp)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec.all_in_floor_bp")]
    #[getter]
    fn all_in_floor_bp<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        decimal_opt_to_py(py, self.inner.all_in_floor_bp)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec.all_in_cap_bp")]
    #[getter]
    fn all_in_cap_bp<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        decimal_opt_to_py(py, self.inner.all_in_cap_bp)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec.index_cap_bp")]
    #[getter]
    fn index_cap_bp<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        decimal_opt_to_py(py, self.inner.index_cap_bp)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec.overnight_index_constraints")]
    #[getter]
    fn overnight_index_constraints(&self) -> PyOvernightIndexConstraintApplication {
        PyOvernightIndexConstraintApplication {
            inner: self.inner.overnight_index_constraints,
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec.reset_frequency")]
    #[getter]
    fn reset_frequency(&self) -> PyTenor {
        PyTenor::from_inner(self.inner.reset_frequency)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec.index_tenor")]
    #[getter]
    fn index_tenor(&self) -> Option<PyTenor> {
        self.inner.index_tenor.map(PyTenor::from_inner)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec.reset_lag_days")]
    #[getter]
    fn reset_lag_days(&self) -> i32 {
        self.inner.reset_lag_days
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec.fixing_calendar_id")]
    #[getter]
    fn fixing_calendar_id(&self) -> Option<String> {
        self.inner
            .fixing_calendar_id
            .as_ref()
            .map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec.compounding")]
    #[getter]
    fn compounding(&self) -> Option<PyFloatingLegCompounding> {
        self.inner
            .compounding
            .map(|inner| PyFloatingLegCompounding { inner })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec.overnight_basis")]
    #[getter]
    fn overnight_basis(&self) -> Option<PyDayCount> {
        self.inner.overnight_basis.map(PyDayCount::from_inner)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec.fallback")]
    #[getter]
    fn fallback(&self) -> PyFloatingRateFallback {
        PyFloatingRateFallback {
            inner: self.inner.fallback.clone(),
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingRateSpec.validate")]
    #[pyo3(text_signature = "(self)")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        repr_from_serde("FloatingRateSpec", &self.inner)
    }
}

#[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingCouponSpec")]
#[pyclass(
    name = "FloatingCouponSpec",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyFloatingCouponSpec {
    /// Inner floating-coupon spec.
    pub(crate) inner: FloatingCouponSpec,
}

wire_methods!(
    PyFloatingCouponSpec,
    FloatingCouponSpec,
    "FloatingCouponSpec",
    module = "finstack_quant.cashflows.builder"
);

#[pymethods]
impl PyFloatingCouponSpec {
    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingCouponSpec.__init__")]
    #[new]
    #[pyo3(
        signature = (rate_spec, schedule, coupon_type=None),
        text_signature = "(rate_spec, schedule, coupon_type=None)"
    )]
    fn new(
        rate_spec: PyRef<'_, PyFloatingRateSpec>,
        schedule: PyRef<'_, PyScheduleParams>,
        coupon_type: Option<PyRef<'_, PyCouponType>>,
    ) -> Self {
        Self {
            inner: FloatingCouponSpec {
                rate_spec: rate_spec.inner.clone(),
                coupon_type: coupon_type.map(|c| c.inner).unwrap_or_default(),
                schedule: schedule.inner.clone(),
            },
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingCouponSpec.rate_spec")]
    #[getter]
    fn rate_spec(&self) -> PyFloatingRateSpec {
        PyFloatingRateSpec {
            inner: self.inner.rate_spec.clone(),
        }
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingCouponSpec.schedule")]
    #[getter]
    fn schedule(&self) -> PyScheduleParams {
        PyScheduleParams::from_inner(self.inner.schedule.clone())
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "FloatingCouponSpec.coupon_type")]
    #[getter]
    fn coupon_type(&self) -> PyCouponType {
        PyCouponType {
            inner: self.inner.coupon_type,
        }
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        repr_from_serde("FloatingCouponSpec", &self.inner)
    }
}

#[doc = pydoc!("finstack_quant.cashflows.builder", "StepUpCouponSpec")]
#[pyclass(
    name = "StepUpCouponSpec",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyStepUpCouponSpec {
    /// Inner step-up coupon spec.
    pub(crate) inner: StepUpCouponSpec,
}

wire_methods!(
    PyStepUpCouponSpec,
    StepUpCouponSpec,
    "StepUpCouponSpec",
    module = "finstack_quant.cashflows.builder"
);

#[pymethods]
impl PyStepUpCouponSpec {
    #[doc = pydoc!("finstack_quant.cashflows.builder", "StepUpCouponSpec.__init__")]
    #[new]
    #[pyo3(
        signature = (initial_rate, step_schedule, schedule, coupon_type=None),
        text_signature = "(initial_rate, step_schedule, schedule, coupon_type=None)"
    )]
    fn new(
        initial_rate: &Bound<'_, PyAny>,
        step_schedule: Vec<(Bound<'_, PyAny>, Bound<'_, PyAny>)>,
        schedule: PyRef<'_, PyScheduleParams>,
        coupon_type: Option<PyRef<'_, PyCouponType>>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: StepUpCouponSpec {
                coupon_type: coupon_type.map(|c| c.inner).unwrap_or_default(),
                initial_rate: decimal_from_any(initial_rate)?,
                step_schedule: date_decimal_pairs(step_schedule)?,
                schedule: schedule.inner.clone(),
            },
        })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "StepUpCouponSpec.initial_rate")]
    #[getter]
    fn initial_rate<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        decimal_to_py(py, self.inner.initial_rate)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "StepUpCouponSpec.step_schedule")]
    #[getter]
    fn step_schedule<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)>> {
        self.inner
            .step_schedule
            .iter()
            .map(|(d, r)| Ok((date_to_py(py, *d)?, decimal_to_py(py, *r)?)))
            .collect()
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "StepUpCouponSpec.schedule")]
    #[getter]
    fn schedule(&self) -> PyScheduleParams {
        PyScheduleParams::from_inner(self.inner.schedule.clone())
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "StepUpCouponSpec.coupon_type")]
    #[getter]
    fn coupon_type(&self) -> PyCouponType {
        PyCouponType {
            inner: self.inner.coupon_type,
        }
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        repr_from_serde("StepUpCouponSpec", &self.inner)
    }
}
