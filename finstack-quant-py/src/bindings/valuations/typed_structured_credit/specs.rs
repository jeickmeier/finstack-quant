//! Typed CMBS and NPL collateral terms: `BalloonSpec`, `PrepaymentPenalty`,
//! `SpecialServicingSpec` and `LiquidationSpec`, the sub-specs a `PoolAsset`
//! carries for a commercial mortgage or a non-performing loan.

use pyo3::prelude::*;

use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    BalloonSpec, LiquidationSpec, PenaltyStep, PrepaymentPenalty, SpecialServicingSpec,
};

/// ``(through, pct)`` pairs of a step-down schedule as Python dates.
type PenaltySteps<'py> = Vec<(Bound<'py, PyAny>, f64)>;

fn opt_date(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<finstack_quant_core::dates::Date>> {
    value.map(py_to_date).transpose()
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "BalloonSpec")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "BalloonSpec",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyBalloonSpec {
    /// Inner canonical Rust balloon terms.
    pub(crate) inner: BalloonSpec,
}

crate::bindings::macros::wire_methods!(
    PyBalloonSpec,
    BalloonSpec,
    "BalloonSpec",
    module = "finstack_quant.valuations.instruments._typed_structured_credit.specs",
    to_dict
);

#[pymethods]
impl PyBalloonSpec {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "BalloonSpec.__init__")]
    #[new]
    #[pyo3(signature = (extension_prob, extension_months, *, extension_rate=None, loss_prob=0.0, severity_pct=0.0, workout_months=0))]
    #[pyo3(
        text_signature = "(extension_prob, extension_months, *, extension_rate=None, loss_prob=0.0, severity_pct=0.0, workout_months=0)"
    )]
    fn new(
        extension_prob: f64,
        extension_months: u32,
        extension_rate: Option<f64>,
        loss_prob: f64,
        severity_pct: f64,
        workout_months: u32,
    ) -> PyResult<Self> {
        let inner = BalloonSpec {
            extension_prob,
            extension_months,
            extension_rate,
            loss_prob,
            severity_pct,
            workout_months,
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "BalloonSpec(extension_prob={}, extension_months={}, loss_prob={}, severity_pct={})",
            self.inner.extension_prob,
            self.inner.extension_months,
            self.inner.loss_prob,
            self.inner.severity_pct
        )
    }
}

crate::bindings::macros::py_getters!(PyBalloonSpec |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "BalloonSpec.extension_prob")]
    extension_prob -> f64 = s.inner.extension_prob;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "BalloonSpec.extension_months")]
    extension_months -> u32 = s.inner.extension_months;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "BalloonSpec.extension_rate")]
    extension_rate -> Option<f64> = s.inner.extension_rate;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "BalloonSpec.loss_prob")]
    loss_prob -> f64 = s.inner.loss_prob;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "BalloonSpec.severity_pct")]
    severity_pct -> f64 = s.inner.severity_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "BalloonSpec.workout_months")]
    workout_months -> u32 = s.inner.workout_months;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "PrepaymentPenalty")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "PrepaymentPenalty",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPrepaymentPenalty {
    /// Inner canonical Rust penalty.
    pub(crate) inner: PrepaymentPenalty,
}

crate::bindings::macros::wire_methods!(
    PyPrepaymentPenalty,
    PrepaymentPenalty,
    "PrepaymentPenalty",
    module = "finstack_quant.valuations.instruments._typed_structured_credit.specs",
    to_dict
);

impl PyPrepaymentPenalty {
    fn build(inner: PrepaymentPenalty) -> PyResult<Self> {
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }
}

#[pymethods]
impl PyPrepaymentPenalty {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "PrepaymentPenalty.lockout")]
    #[staticmethod]
    #[pyo3(signature = (through=None))]
    #[pyo3(text_signature = "(through=None)")]
    fn lockout(through: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        Self::build(PrepaymentPenalty::Lockout {
            through: opt_date(through)?,
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "PrepaymentPenalty.fixed")]
    #[staticmethod]
    #[pyo3(signature = (pct, through=None))]
    #[pyo3(text_signature = "(pct, through=None)")]
    fn fixed(pct: f64, through: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        Self::build(PrepaymentPenalty::Fixed {
            pct,
            through: opt_date(through)?,
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "PrepaymentPenalty.step_down")]
    #[staticmethod]
    #[pyo3(text_signature = "(schedule)")]
    fn step_down(schedule: Vec<(Bound<'_, PyAny>, f64)>) -> PyResult<Self> {
        let schedule = schedule
            .iter()
            .map(|(through, pct)| {
                Ok(PenaltyStep {
                    through: py_to_date(through)?,
                    pct: *pct,
                })
            })
            .collect::<PyResult<Vec<_>>>()?;
        Self::build(PrepaymentPenalty::StepDown { schedule })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "PrepaymentPenalty.yield_maintenance")]
    #[staticmethod]
    #[pyo3(signature = (*, reinvestment_rate=None, discount_curve_id=None, floor_pct=None, through=None))]
    #[pyo3(
        text_signature = "(*, reinvestment_rate=None, discount_curve_id=None, floor_pct=None, through=None)"
    )]
    fn yield_maintenance(
        reinvestment_rate: Option<f64>,
        discount_curve_id: Option<&str>,
        floor_pct: Option<f64>,
        through: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Self::build(PrepaymentPenalty::YieldMaintenance {
            discount_curve_id: discount_curve_id.map(Into::into),
            reinvestment_rate,
            floor_pct,
            through: opt_date(through)?,
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "PrepaymentPenalty.kind")]
    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            PrepaymentPenalty::Lockout { .. } => "lockout",
            PrepaymentPenalty::Fixed { .. } => "fixed",
            PrepaymentPenalty::StepDown { .. } => "step_down",
            PrepaymentPenalty::YieldMaintenance { .. } => "yield_maintenance",
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "PrepaymentPenalty.through")]
    #[getter]
    fn through<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let through = match &self.inner {
            PrepaymentPenalty::Lockout { through }
            | PrepaymentPenalty::Fixed { through, .. }
            | PrepaymentPenalty::YieldMaintenance { through, .. } => *through,
            PrepaymentPenalty::StepDown { .. } => None,
        };
        through.map(|date| date_to_py(py, date)).transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "PrepaymentPenalty.schedule")]
    #[getter]
    fn schedule<'py>(&self, py: Python<'py>) -> PyResult<Option<PenaltySteps<'py>>> {
        match &self.inner {
            PrepaymentPenalty::StepDown { schedule } => schedule
                .iter()
                .map(|step| Ok((date_to_py(py, step.through)?, step.pct)))
                .collect::<PyResult<Vec<_>>>()
                .map(Some),
            _ => Ok(None),
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "PrepaymentPenalty.reinvestment_rate")]
    #[getter]
    fn reinvestment_rate(&self) -> Option<f64> {
        match self.inner {
            PrepaymentPenalty::YieldMaintenance {
                reinvestment_rate, ..
            } => reinvestment_rate,
            _ => None,
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "PrepaymentPenalty.discount_curve_id")]
    #[getter]
    fn discount_curve_id(&self) -> Option<String> {
        match &self.inner {
            PrepaymentPenalty::YieldMaintenance {
                discount_curve_id, ..
            } => discount_curve_id.as_ref().map(|id| id.as_str().to_string()),
            _ => None,
        }
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!("PrepaymentPenalty(kind={:?})", self.kind())
    }
}

crate::bindings::macros::py_getters!(PyPrepaymentPenalty |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "PrepaymentPenalty.pct")]
    pct -> Option<f64> =
        match s.inner { PrepaymentPenalty::Fixed { pct, .. } => Some(pct), _ => None };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "PrepaymentPenalty.floor_pct")]
    floor_pct -> Option<f64> = match s.inner {
        PrepaymentPenalty::YieldMaintenance { floor_pct, .. } => floor_pct,
        _ => None,
    };
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "SpecialServicingSpec")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "SpecialServicingSpec",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PySpecialServicingSpec {
    /// Inner canonical Rust special-servicing state.
    pub(crate) inner: SpecialServicingSpec,
}

crate::bindings::macros::wire_methods!(
    PySpecialServicingSpec,
    SpecialServicingSpec,
    "SpecialServicingSpec",
    module = "finstack_quant.valuations.instruments._typed_structured_credit.specs",
    to_dict
);

#[pymethods]
impl PySpecialServicingSpec {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "SpecialServicingSpec.__init__")]
    #[new]
    #[pyo3(text_signature = "(appraisal_reduction_pct)")]
    fn new(appraisal_reduction_pct: f64) -> PyResult<Self> {
        let inner = SpecialServicingSpec {
            appraisal_reduction_pct,
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "SpecialServicingSpec(appraisal_reduction_pct={})",
            self.inner.appraisal_reduction_pct
        )
    }
}

crate::bindings::macros::py_getters!(PySpecialServicingSpec |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "SpecialServicingSpec.appraisal_reduction_pct")]
    appraisal_reduction_pct -> f64 = s.inner.appraisal_reduction_pct;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "LiquidationSpec")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "LiquidationSpec",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyLiquidationSpec {
    /// Inner canonical Rust resolution terms.
    pub(crate) inner: LiquidationSpec,
}

crate::bindings::macros::wire_methods!(
    PyLiquidationSpec,
    LiquidationSpec,
    "LiquidationSpec",
    module = "finstack_quant.valuations.instruments._typed_structured_credit.specs",
    to_dict
);

#[pymethods]
impl PyLiquidationSpec {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "LiquidationSpec.__init__")]
    #[new]
    #[pyo3(signature = (months_to_resolution, proceeds_pct, *, carry_cost_pct=0.0, reperformance_prob=0.0, modified_rate=None))]
    #[pyo3(
        text_signature = "(months_to_resolution, proceeds_pct, *, carry_cost_pct=0.0, reperformance_prob=0.0, modified_rate=None)"
    )]
    fn new(
        months_to_resolution: u32,
        proceeds_pct: f64,
        carry_cost_pct: f64,
        reperformance_prob: f64,
        modified_rate: Option<f64>,
    ) -> PyResult<Self> {
        let inner = LiquidationSpec {
            months_to_resolution,
            proceeds_pct,
            carry_cost_pct,
            reperformance_prob,
            modified_rate,
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "LiquidationSpec(months_to_resolution={}, proceeds_pct={}, reperformance_prob={})",
            self.inner.months_to_resolution, self.inner.proceeds_pct, self.inner.reperformance_prob
        )
    }
}

crate::bindings::macros::py_getters!(PyLiquidationSpec |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "LiquidationSpec.months_to_resolution")]
    months_to_resolution -> u32 = s.inner.months_to_resolution;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "LiquidationSpec.proceeds_pct")]
    proceeds_pct -> f64 = s.inner.proceeds_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "LiquidationSpec.carry_cost_pct")]
    carry_cost_pct -> f64 = s.inner.carry_cost_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "LiquidationSpec.reperformance_prob")]
    reperformance_prob -> f64 = s.inner.reperformance_prob;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "LiquidationSpec.modified_rate")]
    modified_rate -> Option<f64> = s.inner.modified_rate;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.specs", "LiquidationSpec.net_proceeds_fraction")]
    net_proceeds_fraction -> f64 = s.inner.net_proceeds_fraction();
});
