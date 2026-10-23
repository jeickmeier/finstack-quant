//! Python bindings for `finstack_quant_models::credit::lgd`.
//!
//! Exposes:
//!
//! - Seniority-based Beta recovery distributions (Moody's / S&P calibrations).
//! - Workout (collateral-waterfall) LGD as a builder and a one-shot function.
//! - Downturn LGD adjustments (stressed approximation, regulatory floor).
//! - Exposure-at-default for term loans and revolvers.

use crate::bindings::macros::wire_methods;
use crate::bindings::pydoc::pydoc;
use finstack_quant_models::credit::lgd::{
    self, BetaRecovery, CollateralPiece, CreditConversionFactor, DownturnLgd, EadCalculator,
    WorkoutCollateralType, WorkoutCosts, WorkoutLgd, WorkoutLgdBuilder, WorkoutLgdResult,
};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_rows_to_dataframe_with_schema,
};
use crate::errors::{core_to_py, value_error};

#[doc = pydoc!("finstack_quant.models.credit.lgd", "BetaRecovery")]
#[pyclass(
    module = "finstack_quant.models.credit.lgd",
    name = "BetaRecovery",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyBetaRecovery {
    pub(crate) inner: BetaRecovery,
}

#[pymethods]
impl PyBetaRecovery {
    #[doc = pydoc!("finstack_quant.models.credit.lgd", "BetaRecovery.__init__")]
    #[new]
    #[pyo3(text_signature = "(mean, std_dev)")]
    fn new(mean: f64, std_dev: f64) -> PyResult<Self> {
        BetaRecovery::new(mean, std_dev)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "BetaRecovery.mean")]
    #[getter]
    fn mean(&self) -> f64 {
        self.inner.mean()
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "BetaRecovery.std_dev")]
    #[getter]
    fn std_dev(&self) -> f64 {
        self.inner.std_dev()
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "BetaRecovery.alpha")]
    #[getter]
    fn alpha(&self) -> f64 {
        self.inner.alpha()
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "BetaRecovery.beta_param")]
    #[getter]
    fn beta_param(&self) -> f64 {
        self.inner.beta_param()
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "BetaRecovery.variance")]
    #[getter]
    fn variance(&self) -> f64 {
        self.inner.variance()
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "BetaRecovery.mode")]
    #[getter]
    fn mode(&self) -> Option<f64> {
        self.inner.mode()
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "BetaRecovery.mean_lgd")]
    #[getter]
    fn mean_lgd(&self) -> f64 {
        self.inner.mean_lgd()
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "BetaRecovery.quantile")]
    #[pyo3(text_signature = "($self, p)")]
    fn quantile(&self, p: f64) -> PyResult<f64> {
        self.inner.quantile(p).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "BetaRecovery.sample_seeded")]
    #[pyo3(text_signature = "($self, n_samples, seed)")]
    fn sample_seeded(&self, py: Python<'_>, n_samples: usize, seed: u64) -> PyResult<Vec<f64>> {
        py.detach(|| self.inner.sample_seeded(n_samples, seed))
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "BetaRecovery.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &self.inner,
            &["mean", "std_dev", "alpha", "beta_param"],
        )
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "BetaRecovery.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("BetaRecovery", &self.inner)
    }
}

wire_methods!(
    PyBetaRecovery,
    BetaRecovery,
    "BetaRecovery",
    module = "finstack_quant.models.credit.lgd"
);

#[doc = pydoc!("finstack_quant.models.credit.lgd", "seniority_recovery_stats")]
#[pyfunction]
#[pyo3(signature = (seniority, calibration_id))]
#[pyo3(text_signature = "(seniority, calibration_id)")]
fn seniority_recovery_stats(seniority: &str, calibration_id: &str) -> PyResult<PyBetaRecovery> {
    lgd::seniority_recovery_stats(seniority, calibration_id)
        .map(|inner| PyBetaRecovery { inner })
        .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.credit.lgd", "CollateralPiece")]
#[pyclass(
    module = "finstack_quant.models.credit.lgd",
    name = "CollateralPiece",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyCollateralPiece {
    pub(crate) inner: CollateralPiece,
}

#[pymethods]
impl PyCollateralPiece {
    #[doc = pydoc!("finstack_quant.models.credit.lgd", "CollateralPiece.__init__")]
    #[new]
    #[pyo3(text_signature = "(collateral_type, book_value, haircut)")]
    fn new(collateral_type: &str, book_value: f64, haircut: f64) -> PyResult<Self> {
        let collateral_type =
            finstack_quant_core::wire::serde_parse::<WorkoutCollateralType>(collateral_type)
                .map_err(core_to_py)?;
        CollateralPiece::new(collateral_type, book_value, haircut)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "CollateralPiece.collateral_type")]
    #[getter]
    fn collateral_type(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_label(&self.inner.collateral_type).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "CollateralPiece.book_value")]
    #[getter]
    fn book_value(&self) -> f64 {
        self.inner.book_value
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "CollateralPiece.haircut")]
    #[getter]
    fn haircut(&self) -> f64 {
        self.inner.haircut
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "CollateralPiece.liquidation_value")]
    #[getter]
    fn liquidation_value(&self) -> f64 {
        self.inner.liquidation_value()
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "CollateralPiece.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CollateralPiece", &self.inner)
    }
}

wire_methods!(
    PyCollateralPiece,
    CollateralPiece,
    "CollateralPiece",
    module = "finstack_quant.models.credit.lgd"
);

#[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutCosts")]
#[pyclass(
    module = "finstack_quant.models.credit.lgd",
    name = "WorkoutCosts",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyWorkoutCosts {
    pub(crate) inner: WorkoutCosts,
}

#[pymethods]
impl PyWorkoutCosts {
    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutCosts.__init__")]
    #[new]
    #[pyo3(text_signature = "(direct_cost_rate, indirect_cost_rate)")]
    fn new(direct_cost_rate: f64, indirect_cost_rate: f64) -> PyResult<Self> {
        WorkoutCosts::new(direct_cost_rate, indirect_cost_rate)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutCosts.zero")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn zero() -> Self {
        Self {
            inner: WorkoutCosts::zero(),
        }
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutCosts.standard")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn standard() -> PyResult<Self> {
        WorkoutCosts::standard()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutCosts.direct_cost_rate")]
    #[getter]
    fn direct_cost_rate(&self) -> f64 {
        self.inner.direct_cost_rate
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutCosts.indirect_cost_rate")]
    #[getter]
    fn indirect_cost_rate(&self) -> f64 {
        self.inner.indirect_cost_rate
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutCosts.total_rate")]
    #[getter]
    fn total_rate(&self) -> f64 {
        self.inner.total_rate()
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutCosts.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("WorkoutCosts", &self.inner)
    }
}

wire_methods!(
    PyWorkoutCosts,
    WorkoutCosts,
    "WorkoutCosts",
    module = "finstack_quant.models.credit.lgd"
);

#[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgdResult")]
#[pyclass(
    module = "finstack_quant.models.credit.lgd",
    name = "WorkoutLgdResult",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyWorkoutLgdResult {
    pub(crate) inner: WorkoutLgdResult,
}

#[pymethods]
impl PyWorkoutLgdResult {
    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgdResult.net_recovery")]
    #[getter]
    fn net_recovery(&self) -> f64 {
        self.inner.net_recovery
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgdResult.lgd")]
    #[getter]
    fn lgd(&self) -> f64 {
        self.inner.lgd
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgdResult.recovery_rate")]
    #[getter]
    fn recovery_rate(&self) -> f64 {
        self.inner.recovery_rate
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgdResult.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &self.inner,
            &["net_recovery", "lgd", "recovery_rate"],
        )
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgdResult.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("WorkoutLgdResult", &self.inner)
    }
}

wire_methods!(
    PyWorkoutLgdResult,
    WorkoutLgdResult,
    "WorkoutLgdResult",
    module = "finstack_quant.models.credit.lgd"
);

#[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgd")]
#[pyclass(
    module = "finstack_quant.models.credit.lgd",
    name = "WorkoutLgd",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyWorkoutLgd {
    pub(crate) inner: WorkoutLgd,
}

#[pymethods]
impl PyWorkoutLgd {
    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgd.builder")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn builder() -> PyWorkoutLgdBuilder {
        PyWorkoutLgdBuilder {
            inner: Some(WorkoutLgd::builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgd.evaluate")]
    #[pyo3(text_signature = "($self, ead)")]
    fn evaluate(&self, ead: f64) -> PyResult<PyWorkoutLgdResult> {
        self.inner
            .evaluate(ead)
            .map(|inner| PyWorkoutLgdResult { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgd.net_recovery")]
    #[pyo3(text_signature = "($self, ead)")]
    fn net_recovery(&self, ead: f64) -> PyResult<f64> {
        self.inner.net_recovery(ead).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgd.collateral")]
    #[getter]
    fn collateral(&self) -> Vec<PyCollateralPiece> {
        self.inner
            .collateral()
            .iter()
            .map(|piece| PyCollateralPiece { inner: *piece })
            .collect()
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgd.workout_years")]
    #[getter]
    fn workout_years(&self) -> f64 {
        self.inner.workout_years()
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgd.discount_rate")]
    #[getter]
    fn discount_rate(&self) -> f64 {
        self.inner.discount_rate()
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgd.costs")]
    #[getter]
    fn costs(&self) -> PyWorkoutCosts {
        PyWorkoutCosts {
            inner: *self.inner.costs(),
        }
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgd.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(
            py,
            self.inner.collateral(),
            &[
                ("collateral_type", "str"),
                ("book_value", "float64"),
                ("haircut", "float64"),
            ],
        )
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgd.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("WorkoutLgd", &self.inner)
    }
}

wire_methods!(
    PyWorkoutLgd,
    WorkoutLgd,
    "WorkoutLgd",
    module = "finstack_quant.models.credit.lgd"
);

#[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgdBuilder")]
#[pyclass(
    module = "finstack_quant.models.credit.lgd",
    name = "WorkoutLgdBuilder",
    skip_from_py_object
)]
pub(crate) struct PyWorkoutLgdBuilder {
    inner: Option<WorkoutLgdBuilder>,
}

impl PyWorkoutLgdBuilder {
    fn with_inner(
        slf: &mut PyRefMut<'_, Self>,
        f: impl FnOnce(WorkoutLgdBuilder) -> WorkoutLgdBuilder,
    ) -> PyResult<()> {
        let builder = slf
            .inner
            .take()
            .ok_or_else(|| value_error("WorkoutLgdBuilder has already been consumed by build()"))?;
        slf.inner = Some(f(builder));
        Ok(())
    }
}

#[pymethods]
impl PyWorkoutLgdBuilder {
    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgdBuilder.collateral")]
    #[pyo3(text_signature = "($self, piece)")]
    fn collateral<'py>(
        mut slf: PyRefMut<'py, Self>,
        piece: PyCollateralPiece,
    ) -> PyResult<PyRefMut<'py, Self>> {
        Self::with_inner(&mut slf, |b| b.collateral(piece.inner))?;
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgdBuilder.collateral_pieces")]
    #[pyo3(text_signature = "($self, pieces)")]
    fn collateral_pieces<'py>(
        mut slf: PyRefMut<'py, Self>,
        pieces: Vec<PyCollateralPiece>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let pieces: Vec<CollateralPiece> = pieces.into_iter().map(|p| p.inner).collect();
        Self::with_inner(&mut slf, |b| b.collateral_pieces(pieces))?;
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgdBuilder.workout_years")]
    #[pyo3(text_signature = "($self, years)")]
    fn workout_years(mut slf: PyRefMut<'_, Self>, years: f64) -> PyResult<PyRefMut<'_, Self>> {
        Self::with_inner(&mut slf, |b| b.workout_years(years))?;
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgdBuilder.discount_rate")]
    #[pyo3(text_signature = "($self, rate)")]
    fn discount_rate(mut slf: PyRefMut<'_, Self>, rate: f64) -> PyResult<PyRefMut<'_, Self>> {
        Self::with_inner(&mut slf, |b| b.discount_rate(rate))?;
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgdBuilder.costs")]
    #[pyo3(text_signature = "($self, costs)")]
    fn costs<'py>(
        mut slf: PyRefMut<'py, Self>,
        costs: PyWorkoutCosts,
    ) -> PyResult<PyRefMut<'py, Self>> {
        Self::with_inner(&mut slf, |b| b.costs(costs.inner))?;
        Ok(slf)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "WorkoutLgdBuilder.build")]
    #[pyo3(text_signature = "($self)")]
    fn build(mut slf: PyRefMut<'_, Self>) -> PyResult<PyWorkoutLgd> {
        let builder = slf
            .inner
            .take()
            .ok_or_else(|| value_error("WorkoutLgdBuilder has already been consumed by build()"))?;
        builder
            .build()
            .map(|inner| PyWorkoutLgd { inner })
            .map_err(core_to_py)
    }
}

#[doc = pydoc!("finstack_quant.models.credit.lgd", "workout_lgd")]
#[pyfunction]
#[pyo3(
    text_signature = "(ead, collateral, direct_cost_pct, indirect_cost_pct, time_to_resolution_years, discount_rate)"
)]
fn workout_lgd(
    ead: f64,
    collateral: Vec<(String, f64, f64)>,
    direct_cost_pct: f64,
    indirect_cost_pct: f64,
    time_to_resolution_years: f64,
    discount_rate: f64,
) -> PyResult<PyWorkoutLgdResult> {
    lgd::workout_lgd(
        ead,
        collateral,
        direct_cost_pct,
        indirect_cost_pct,
        time_to_resolution_years,
        discount_rate,
    )
    .map(|inner| PyWorkoutLgdResult { inner })
    .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.credit.lgd", "DownturnLgd")]
#[pyclass(
    module = "finstack_quant.models.credit.lgd",
    name = "DownturnLgd",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyDownturnLgd {
    pub(crate) inner: DownturnLgd,
}

#[pymethods]
impl PyDownturnLgd {
    #[doc = pydoc!("finstack_quant.models.credit.lgd", "DownturnLgd.stressed")]
    #[staticmethod]
    #[pyo3(text_signature = "(asset_correlation, lgd_sensitivity, stress_quantile)")]
    fn stressed(
        asset_correlation: f64,
        lgd_sensitivity: f64,
        stress_quantile: f64,
    ) -> PyResult<Self> {
        DownturnLgd::stressed(asset_correlation, lgd_sensitivity, stress_quantile)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "DownturnLgd.regulatory_floor")]
    #[staticmethod]
    #[pyo3(text_signature = "(add_on, floor)")]
    fn regulatory_floor(add_on: f64, floor: f64) -> PyResult<Self> {
        DownturnLgd::regulatory_floor(add_on, floor)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "DownturnLgd.from_registry_id")]
    #[staticmethod]
    #[pyo3(text_signature = "(id)")]
    fn from_registry_id(id: &str) -> PyResult<Self> {
        DownturnLgd::from_registry_id(id)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "DownturnLgd.default_calibration")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn default_calibration() -> PyResult<Self> {
        DownturnLgd::default_calibration()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "DownturnLgd.adjust")]
    #[pyo3(text_signature = "($self, base_lgd)")]
    fn adjust(&self, base_lgd: f64) -> PyResult<f64> {
        self.inner.adjust(base_lgd).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "DownturnLgd.method")]
    #[getter]
    fn method(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_tag(self.inner.method()).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "DownturnLgd.params")]
    #[getter]
    fn params<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, self.inner.method())
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "DownturnLgd.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("DownturnLgd", &self.inner)
    }
}

wire_methods!(
    PyDownturnLgd,
    DownturnLgd,
    "DownturnLgd",
    module = "finstack_quant.models.credit.lgd",
    validate
);

#[doc = pydoc!("finstack_quant.models.credit.lgd", "EadCalculator")]
#[pyclass(
    module = "finstack_quant.models.credit.lgd",
    name = "EadCalculator",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyEadCalculator {
    pub(crate) inner: EadCalculator,
}

#[pymethods]
impl PyEadCalculator {
    #[doc = pydoc!("finstack_quant.models.credit.lgd", "EadCalculator.__init__")]
    #[new]
    #[pyo3(text_signature = "(drawn, undrawn, ccf)")]
    fn new(drawn: f64, undrawn: f64, ccf: f64) -> PyResult<Self> {
        let ccf = CreditConversionFactor::new(ccf).map_err(core_to_py)?;
        EadCalculator::new(drawn, undrawn, ccf)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "EadCalculator.term_loan")]
    #[staticmethod]
    #[pyo3(text_signature = "(drawn)")]
    fn term_loan(drawn: f64) -> PyResult<Self> {
        EadCalculator::term_loan(drawn)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "EadCalculator.revolver")]
    #[staticmethod]
    #[pyo3(text_signature = "(drawn, undrawn)")]
    fn revolver(drawn: f64, undrawn: f64) -> PyResult<Self> {
        EadCalculator::revolver(drawn, undrawn)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "EadCalculator.ead")]
    #[getter]
    fn ead(&self) -> f64 {
        self.inner.ead()
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "EadCalculator.utilization")]
    #[getter]
    fn utilization(&self) -> f64 {
        self.inner.utilization()
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "EadCalculator.total_commitment")]
    #[getter]
    fn total_commitment(&self) -> f64 {
        self.inner.total_commitment()
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "EadCalculator.leq_from_observed_ead")]
    #[pyo3(text_signature = "($self, observed_ead)")]
    fn leq_from_observed_ead(&self, observed_ead: f64) -> Option<f64> {
        self.inner.leq_from_observed_ead(observed_ead)
    }

    #[doc = pydoc!("finstack_quant.models.credit.lgd", "EadCalculator.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("EadCalculator", &self.inner)
    }
}

wire_methods!(
    PyEadCalculator,
    EadCalculator,
    "EadCalculator",
    module = "finstack_quant.models.credit.lgd"
);

/// Build the `finstack_quant.models.credit.lgd` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "lgd")?;
    m.setattr(
        "__doc__",
        pydoc!("finstack_quant.models.credit.lgd", "__module__"),
    )?;

    m.add_class::<PyBetaRecovery>()?;
    m.add_class::<PyCollateralPiece>()?;
    m.add_class::<PyDownturnLgd>()?;
    m.add_class::<PyEadCalculator>()?;
    m.add_class::<PyWorkoutCosts>()?;
    m.add_class::<PyWorkoutLgd>()?;
    m.add_class::<PyWorkoutLgdBuilder>()?;
    m.add_class::<PyWorkoutLgdResult>()?;
    m.add_function(wrap_pyfunction!(seniority_recovery_stats, &m)?)?;
    m.add_function(wrap_pyfunction!(workout_lgd, &m)?)?;

    let all = PyList::new(
        py,
        [
            "BetaRecovery",
            "CollateralPiece",
            "DownturnLgd",
            "EadCalculator",
            "WorkoutCosts",
            "WorkoutLgd",
            "WorkoutLgdBuilder",
            "WorkoutLgdResult",
            "seniority_recovery_stats",
            "workout_lgd",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;

    Ok(())
}
