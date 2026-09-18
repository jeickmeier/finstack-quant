//! Typed CMBS and NPL collateral terms: `BalloonSpec`, `PrepaymentPenalty`,
//! `SpecialServicingSpec` and `LiquidationSpec`, the sub-specs a `PoolAsset`
//! carries for a commercial mortgage or a non-performing loan.

use pyo3::prelude::*;

use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::errors::core_to_py;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    BalloonSpec, LiquidationSpec, PenaltyStep, PrepaymentPenalty, SpecialServicingSpec,
};

/// ``(through, pct)`` pairs of a step-down schedule as Python dates.
type PenaltySteps<'py> = Vec<(Bound<'py, PyAny>, f64)>;

fn opt_date(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<finstack_quant_core::dates::Date>> {
    value.map(extract_date).transpose()
}

/// Balloon terms of a commercial mortgage at maturity.
///
/// At the loan's maturity a share ``extension_prob`` of the performing
/// balance is extended ``extension_months`` at ``extension_rate`` (the
/// original coupon when ``None``), a share ``loss_prob`` defaults into a
/// workout that recovers ``1 − severity_pct / 100`` after ``workout_months``,
/// and the rest pays as the balloon.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import BalloonSpec
/// >>> spec = BalloonSpec(0.3, 24, extension_rate=0.07, loss_prob=0.1, severity_pct=40.0, workout_months=18)
/// >>> spec.extension_prob, spec.workout_months
/// (0.3, 18)
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

sc_wire_methods!(PyBalloonSpec, BalloonSpec, "BalloonSpec");

#[pymethods]
impl PyBalloonSpec {
    /// Construct balloon terms.
    ///
    /// Parameters
    /// ----------
    /// extension_prob : float
    ///     Share of the performing balance extended at maturity, decimal in
    ///     ``[0, 1]``.
    /// extension_months : int
    ///     Length of the extension in months.
    /// extension_rate : float, optional
    ///     Annual coupon (decimal) on the extended balance; ``None`` keeps
    ///     the loan's coupon (spread and index for a floater).
    /// loss_prob : float, optional
    ///     Share of the performing balance that defaults into a workout at
    ///     maturity, decimal in ``[0, 1]``; ``0.0`` by default.
    /// severity_pct : float, optional
    ///     Loss severity on the workout share in percent (``40.0`` = 40%);
    ///     ``0.0`` by default.
    /// workout_months : int, optional
    ///     Months after maturity until the workout recovery is received;
    ///     ``0`` by default (the deal's recovery lag).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a share is outside ``[0, 1]``, the severity is outside
    ///     ``[0, 100]`` or the rate is not finite.
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

    /// Share of the performing balance extended at maturity (decimal).
    #[getter]
    fn extension_prob(&self) -> f64 {
        self.inner.extension_prob
    }

    /// Length of the extension in months.
    #[getter]
    fn extension_months(&self) -> u32 {
        self.inner.extension_months
    }

    /// Annual coupon on the extended balance (decimal), or ``None``.
    #[getter]
    fn extension_rate(&self) -> Option<f64> {
        self.inner.extension_rate
    }

    /// Share of the performing balance that defaults into a workout (decimal).
    #[getter]
    fn loss_prob(&self) -> f64 {
        self.inner.loss_prob
    }

    /// Loss severity on the workout share in percent.
    #[getter]
    fn severity_pct(&self) -> f64 {
        self.inner.severity_pct
    }

    /// Months after maturity until the workout recovery is received.
    #[getter]
    fn workout_months(&self) -> u32 {
        self.inner.workout_months
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

/// Prepayment protection on a commercial mortgage.
///
/// Built through one of the four constructors: :meth:`lockout` blocks
/// voluntary prepayment, :meth:`fixed` charges a percent of the prepaid
/// balance, :meth:`step_down` charges the step in force from a declining
/// schedule and :meth:`yield_maintenance` charges the coupon lost over the
/// remaining term. Premiums are collected by the trust as interest.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.valuations.instruments import PrepaymentPenalty
/// >>> PrepaymentPenalty.fixed(3.0, through=datetime.date(2026, 1, 1)).kind
/// 'fixed'
/// >>> PrepaymentPenalty.lockout(datetime.date(2025, 12, 31)).through
/// datetime.date(2025, 12, 31)
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

sc_wire_methods!(PyPrepaymentPenalty, PrepaymentPenalty, "PrepaymentPenalty");

impl PyPrepaymentPenalty {
    fn build(inner: PrepaymentPenalty) -> PyResult<Self> {
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }
}

#[pymethods]
impl PyPrepaymentPenalty {
    /// A lockout: no voluntary prepayment on or before ``through``.
    ///
    /// Parameters
    /// ----------
    /// through : datetime.date, optional
    ///     Last date of the lockout; ``None`` locks the loan out to maturity.
    ///
    /// Returns
    /// -------
    /// PrepaymentPenalty
    ///     The lockout.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``through`` is not a valid date.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import PrepaymentPenalty
    /// >>> PrepaymentPenalty.lockout().kind
    /// 'lockout'
    #[staticmethod]
    #[pyo3(signature = (through=None))]
    #[pyo3(text_signature = "(through=None)")]
    fn lockout(through: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        Self::build(PrepaymentPenalty::Lockout {
            through: opt_date(through)?,
        })
    }

    /// A fixed percent of the prepaid balance.
    ///
    /// Parameters
    /// ----------
    /// pct : float
    ///     Premium in percent of the prepaid balance (``3.0`` = 3%).
    /// through : datetime.date, optional
    ///     Last date the penalty applies; ``None`` applies it to maturity.
    ///
    /// Returns
    /// -------
    /// PrepaymentPenalty
    ///     The fixed penalty.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``pct`` is negative or ``through`` is not a valid date.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import PrepaymentPenalty
    /// >>> PrepaymentPenalty.fixed(3.0).pct
    /// 3.0
    #[staticmethod]
    #[pyo3(signature = (pct, through=None))]
    #[pyo3(text_signature = "(pct, through=None)")]
    fn fixed(pct: f64, through: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        Self::build(PrepaymentPenalty::Fixed {
            pct,
            through: opt_date(through)?,
        })
    }

    /// A declining schedule of percents (5-4-3-2-1).
    ///
    /// Parameters
    /// ----------
    /// schedule : list[tuple[datetime.date, float]]
    ///     ``(through, pct)`` steps in ascending date order; the first step
    ///     whose date is on or after the prepayment applies, nothing after
    ///     the last.
    ///
    /// Returns
    /// -------
    /// PrepaymentPenalty
    ///     The step-down penalty.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the schedule is empty, not ascending, or a percent is negative.
    ///
    /// Examples
    /// --------
    /// >>> import datetime
    /// >>> from finstack_quant.valuations.instruments import PrepaymentPenalty
    /// >>> steps = [(datetime.date(2025, 12, 31), 5.0), (datetime.date(2026, 12, 31), 3.0)]
    /// >>> len(PrepaymentPenalty.step_down(steps).schedule)
    /// 2
    #[staticmethod]
    #[pyo3(text_signature = "(schedule)")]
    fn step_down(schedule: Vec<(Bound<'_, PyAny>, f64)>) -> PyResult<Self> {
        let schedule = schedule
            .iter()
            .map(|(through, pct)| {
                Ok(PenaltyStep {
                    through: extract_date(through)?,
                    pct: *pct,
                })
            })
            .collect::<PyResult<Vec<_>>>()?;
        Self::build(PrepaymentPenalty::StepDown { schedule })
    }

    /// Yield maintenance: the coupon lost over the remaining term.
    ///
    /// Parameters
    /// ----------
    /// reinvestment_rate : float, optional
    ///     Annual reinvestment rate as a decimal; ``None`` uses the curve's
    ///     zero rate to maturity (then ``discount_curve_id`` is required).
    /// discount_curve_id : str, optional
    ///     Market-context discount curve the annual lost coupons are
    ///     discounted on; ``None`` leaves the premium undiscounted.
    /// floor_pct : float, optional
    ///     Minimum premium in percent of the prepaid balance (the common
    ///     ``1.0``); ``None`` for no floor.
    /// through : datetime.date, optional
    ///     Last date the penalty applies; ``None`` applies it to maturity.
    ///
    /// Returns
    /// -------
    /// PrepaymentPenalty
    ///     The yield-maintenance penalty.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If neither a rate nor a curve is given, a rate or floor is
    ///     negative, or ``through`` is not a valid date.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import PrepaymentPenalty
    /// >>> ym = PrepaymentPenalty.yield_maintenance(discount_curve_id="USD-OIS", floor_pct=1.0)
    /// >>> ym.kind, ym.floor_pct
    /// ('yield_maintenance', 1.0)
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

    /// ``"lockout"``, ``"fixed"``, ``"step_down"`` or ``"yield_maintenance"``.
    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            PrepaymentPenalty::Lockout { .. } => "lockout",
            PrepaymentPenalty::Fixed { .. } => "fixed",
            PrepaymentPenalty::StepDown { .. } => "step_down",
            PrepaymentPenalty::YieldMaintenance { .. } => "yield_maintenance",
        }
    }

    /// Last date the penalty or lockout applies as ``datetime.date``, or
    /// ``None`` (to maturity, or a step-down schedule).
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

    /// Fixed premium percent, or ``None`` for the other kinds.
    #[getter]
    fn pct(&self) -> Option<f64> {
        match self.inner {
            PrepaymentPenalty::Fixed { pct, .. } => Some(pct),
            _ => None,
        }
    }

    /// Step-down schedule as ``(through, pct)`` pairs, or ``None``.
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

    /// Yield-maintenance reinvestment rate (decimal), or ``None``.
    #[getter]
    fn reinvestment_rate(&self) -> Option<f64> {
        match self.inner {
            PrepaymentPenalty::YieldMaintenance {
                reinvestment_rate, ..
            } => reinvestment_rate,
            _ => None,
        }
    }

    /// Yield-maintenance discount curve identifier, or ``None``.
    #[getter]
    fn discount_curve_id(&self) -> Option<String> {
        match &self.inner {
            PrepaymentPenalty::YieldMaintenance {
                discount_curve_id, ..
            } => discount_curve_id.as_ref().map(|id| id.as_str().to_string()),
            _ => None,
        }
    }

    /// Yield-maintenance floor in percent of the prepaid balance, or ``None``.
    #[getter]
    fn floor_pct(&self) -> Option<f64> {
        match self.inner {
            PrepaymentPenalty::YieldMaintenance { floor_pct, .. } => floor_pct,
            _ => None,
        }
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!("PrepaymentPenalty(kind={:?})", self.kind())
    }
}

/// Special-servicing state of a commercial mortgage at closing.
///
/// The appraisal reduction (ASER) cuts the interest advanced on the loan to
/// ``1 − appraisal_reduction_pct / 100`` of its balance; the loan is
/// specially serviced from closing, so the deal's special servicer fee,
/// workout fee and liquidation fee apply to it.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import SpecialServicingSpec
/// >>> SpecialServicingSpec(40.0).appraisal_reduction_pct
/// 40.0
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

sc_wire_methods!(
    PySpecialServicingSpec,
    SpecialServicingSpec,
    "SpecialServicingSpec"
);

#[pymethods]
impl PySpecialServicingSpec {
    /// Construct the special-servicing state.
    ///
    /// Parameters
    /// ----------
    /// appraisal_reduction_pct : float, optional
    ///     Appraisal reduction in percent of the loan balance (``40.0`` =
    ///     40%); ``0.0`` by default (specially serviced, full advancing).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the percent is outside ``[0, 100]``.
    #[new]
    #[pyo3(signature = (appraisal_reduction_pct=0.0))]
    #[pyo3(text_signature = "(appraisal_reduction_pct=0.0)")]
    fn new(appraisal_reduction_pct: f64) -> PyResult<Self> {
        let inner = SpecialServicingSpec {
            appraisal_reduction_pct,
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Appraisal reduction in percent of the loan balance.
    #[getter]
    fn appraisal_reduction_pct(&self) -> f64 {
        self.inner.appraisal_reduction_pct
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "SpecialServicingSpec(appraisal_reduction_pct={})",
            self.inner.appraisal_reduction_pct
        )
    }
}

/// Resolution timeline of a non-performing or re-performing loan.
///
/// The loan pays nothing until ``months_to_resolution`` months from its
/// origination (acquisition date, else closing); then the share
/// ``1 − reperformance_prob`` liquidates at ``proceeds_pct − carry_cost_pct``
/// percent of its balance and the rest re-performs at ``modified_rate``.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import LiquidationSpec
/// >>> spec = LiquidationSpec(18, 65.0, carry_cost_pct=5.0, reperformance_prob=0.2, modified_rate=0.04)
/// >>> spec.months_to_resolution, spec.net_proceeds_fraction
/// (18, 0.6)
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

sc_wire_methods!(PyLiquidationSpec, LiquidationSpec, "LiquidationSpec");

#[pymethods]
impl PyLiquidationSpec {
    /// Construct the resolution terms.
    ///
    /// Parameters
    /// ----------
    /// months_to_resolution : int
    ///     Months from origination until the loan resolves.
    /// proceeds_pct : float
    ///     Gross liquidation proceeds in percent of the balance.
    /// carry_cost_pct : float, optional
    ///     Carry and disposition costs in percent of the balance, netted
    ///     from the proceeds; ``0.0`` by default.
    /// reperformance_prob : float, optional
    ///     Share of the balance that re-performs instead of liquidating,
    ///     decimal in ``[0, 1]``; ``0.0`` by default.
    /// modified_rate : float, optional
    ///     Annual coupon (decimal) of the re-performing share; ``None`` keeps
    ///     the loan's coupon.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a percent is outside ``[0, 100]``, the share is outside
    ///     ``[0, 1]`` or the rate is not finite.
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

    /// Months from origination until the loan resolves.
    #[getter]
    fn months_to_resolution(&self) -> u32 {
        self.inner.months_to_resolution
    }

    /// Gross liquidation proceeds in percent of the balance.
    #[getter]
    fn proceeds_pct(&self) -> f64 {
        self.inner.proceeds_pct
    }

    /// Carry and disposition costs in percent of the balance.
    #[getter]
    fn carry_cost_pct(&self) -> f64 {
        self.inner.carry_cost_pct
    }

    /// Share of the balance that re-performs (decimal).
    #[getter]
    fn reperformance_prob(&self) -> f64 {
        self.inner.reperformance_prob
    }

    /// Coupon of the re-performing share (decimal), or ``None``.
    #[getter]
    fn modified_rate(&self) -> Option<f64> {
        self.inner.modified_rate
    }

    /// Net liquidation proceeds as a fraction of the balance
    /// (``(proceeds_pct − carry_cost_pct) / 100``, floored at zero).
    #[getter]
    fn net_proceeds_fraction(&self) -> f64 {
        self.inner.net_proceeds_fraction()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "LiquidationSpec(months_to_resolution={}, proceeds_pct={}, reperformance_prob={})",
            self.inner.months_to_resolution, self.inner.proceeds_pct, self.inner.reperformance_prob
        )
    }
}
