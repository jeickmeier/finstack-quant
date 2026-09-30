//! Python bindings for `finstack_quant_models::credit::lgd`.
//!
//! Exposes:
//!
//! - Seniority-based Beta recovery distributions (Moody's / S&P calibrations).
//! - Workout (collateral-waterfall) LGD as a builder and a one-shot function.
//! - Downturn LGD adjustments (stressed approximation, regulatory floor).
//! - Exposure-at-default for term loans and revolvers.

use finstack_quant_models::credit::lgd::{
    self, BetaRecovery, CollateralPiece, CreditConversionFactor, DownturnLgd, EadCalculator,
    WorkoutCollateralType, WorkoutCosts, WorkoutLgd, WorkoutLgdBuilder, WorkoutLgdResult,
};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_rows_to_dataframe_with_schema,
};
use crate::errors::{core_to_py, serde_json_to_py, value_error};

/// Accepted `WorkoutCollateralType` strings, in canonical order.
const COLLATERAL_TYPES: &str =
    "cash, securities, receivables, inventory, equipment, real_estate, intellectual_property, other";

fn parse_collateral_type(value: &str) -> PyResult<WorkoutCollateralType> {
    value.parse::<WorkoutCollateralType>().map_err(core_to_py)
}

/// Beta-distributed recovery rate parameterised by mean and standard deviation.
///
/// ``alpha`` / ``beta_param`` are the moment-matched Beta shape parameters.
/// Obtain one from ``seniority_recovery_stats`` (agency calibration) or build
/// it directly from ``(mean, std_dev)`` decimals in (0, 1).
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
    /// Build a Beta recovery distribution from its first two moments.
    ///
    /// Parameters
    /// ----------
    /// mean : float
    ///     Mean recovery rate as a decimal in (0, 1).
    /// std_dev : float
    ///     Standard deviation; must satisfy ``std_dev**2 < mean * (1 - mean)``.
    ///
    /// Raises ``ValueError`` when the moments cannot parameterise a Beta
    /// distribution.
    #[new]
    #[pyo3(text_signature = "(mean, std_dev)")]
    fn new(mean: f64, std_dev: f64) -> PyResult<Self> {
        BetaRecovery::new(mean, std_dev)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Mean recovery rate (decimal).
    #[getter]
    fn mean(&self) -> f64 {
        self.inner.mean()
    }

    /// Standard deviation of the recovery rate.
    #[getter]
    fn std_dev(&self) -> f64 {
        self.inner.std_dev()
    }

    /// Beta shape parameter alpha.
    #[getter]
    fn alpha(&self) -> f64 {
        self.inner.alpha()
    }

    /// Beta shape parameter beta.
    #[getter]
    fn beta_param(&self) -> f64 {
        self.inner.beta_param()
    }

    /// Variance of the recovery rate (``std_dev**2``).
    #[getter]
    fn variance(&self) -> f64 {
        self.inner.variance()
    }

    /// Mode of the distribution, or ``None`` when a shape parameter is <= 1.
    #[getter]
    fn mode(&self) -> Option<f64> {
        self.inner.mode()
    }

    /// Expected loss given default, ``1 - mean``.
    #[getter]
    fn mean_lgd(&self) -> f64 {
        self.inner.mean_lgd()
    }

    /// Recovery rate at probability ``p``.
    ///
    /// Parameters
    /// ----------
    /// p : float
    ///     Probability in (0, 1).
    ///
    /// Raises ``ValueError`` when ``p`` is non-finite or outside (0, 1).
    #[pyo3(text_signature = "($self, p)")]
    fn quantile(&self, p: f64) -> PyResult<f64> {
        self.inner.quantile(p).map_err(core_to_py)
    }

    /// Draw ``n_samples`` recovery rates with a deterministic PCG64 RNG.
    ///
    /// Parameters
    /// ----------
    /// n_samples : int
    ///     Number of draws.
    /// seed : int
    ///     RNG seed; the same seed yields the same sequence.
    ///
    /// Raises ``ValueError`` when sampling fails.
    #[pyo3(text_signature = "($self, n_samples, seed)")]
    fn sample_seeded(&self, py: Python<'_>, n_samples: usize, seed: u64) -> PyResult<Vec<f64>> {
        py.detach(|| self.inner.sample_seeded(n_samples, seed))
            .map_err(core_to_py)
    }

    /// Deserialize from canonical JSON (shape parameters are re-validated).
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid BetaRecovery JSON"))?,
        })
    }

    /// Serialize to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "BetaRecovery serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Export as a single-row pandas ``DataFrame``
    /// (``mean``, ``std_dev``, ``alpha``, ``beta_param``).
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &self.inner,
            &["mean", "std_dev", "alpha", "beta_param"],
        )
    }

    /// Identify this value in notebooks and logs.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("BetaRecovery", &self.inner)
    }
}

/// Return the historical Beta recovery distribution for a seniority class.
///
/// Parameters
/// ----------
/// seniority : str
///     One of ``1st_lien_secured``, ``2nd_lien_secured``, ``senior_secured``,
///     ``senior_unsecured``, ``subordinated``, ``junior_subordinated``.
/// rating_agency : str | None
///     ``"moodys"`` (canonical) or ``"sp"``. ``None`` selects the registry
///     default calibration (Moody's historical).
///
/// Returns a ``BetaRecovery``.
///
/// Raises ``ValueError`` when the seniority or agency is unknown, or the
/// selected calibration has no entry for the class.
#[pyfunction]
#[pyo3(signature = (seniority, rating_agency = None))]
#[pyo3(text_signature = "(seniority, rating_agency=None)")]
fn seniority_recovery_stats(
    seniority: &str,
    rating_agency: Option<&str>,
) -> PyResult<PyBetaRecovery> {
    match rating_agency {
        Some(agency) => lgd::seniority_recovery_stats(seniority, agency),
        None => lgd::seniority_recovery_stats_default(seniority),
    }
    .map(|inner| PyBetaRecovery { inner })
    .map_err(core_to_py)
}

/// Draw ``n_samples`` recovery rates from ``BetaRecovery(mean, std)``.
///
/// Thin twin of ``BetaRecovery(mean, std).sample_seeded(n_samples, seed)``.
///
/// Parameters
/// ----------
/// mean : float
///     Mean recovery rate in (0, 1).
/// std : float
///     Standard deviation; must satisfy ``std**2 < mean * (1 - mean)``.
/// n_samples : int
///     Number of draws to produce.
/// seed : int
///     RNG seed. The same seed yields the same sequence.
///
/// Raises ``ValueError`` when the moments are invalid.
#[pyfunction]
#[pyo3(text_signature = "(mean, std, n_samples, seed)")]
fn beta_recovery_sample(
    py: Python<'_>,
    mean: f64,
    std: f64,
    n_samples: usize,
    seed: u64,
) -> PyResult<Vec<f64>> {
    py.detach(|| lgd::beta_recovery_sample(mean, std, n_samples, seed))
        .map_err(core_to_py)
}

/// Recovery rate at quantile ``q`` of ``BetaRecovery(mean, std)``.
///
/// Thin twin of ``BetaRecovery(mean, std).quantile(q)``.
///
/// Parameters
/// ----------
/// mean : float
///     Mean recovery rate in (0, 1).
/// std : float
///     Standard deviation; must satisfy ``std**2 < mean * (1 - mean)``.
/// q : float
///     Probability in (0, 1).
///
/// Raises ``ValueError`` when the moments or ``q`` are invalid.
#[pyfunction]
#[pyo3(text_signature = "(mean, std, q)")]
fn beta_recovery_quantile(mean: f64, std: f64, q: f64) -> PyResult<f64> {
    lgd::beta_recovery_quantile(mean, std, q).map_err(core_to_py)
}

/// One collateral piece in a workout waterfall.
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
    /// Build a collateral piece.
    ///
    /// Parameters
    /// ----------
    /// collateral_type : str
    ///     One of ``cash``, ``securities``, ``receivables``, ``inventory``,
    ///     ``equipment``, ``real_estate``, ``intellectual_property``, ``other``.
    /// book_value : float
    ///     Pre-haircut book value (non-negative), in the exposure's currency.
    /// haircut : float
    ///     Liquidation haircut as a decimal in [0, 1].
    ///
    /// Raises ``ValueError`` for an unknown type, a negative value, or a
    /// haircut outside [0, 1].
    #[new]
    #[pyo3(text_signature = "(collateral_type, book_value, haircut)")]
    fn new(collateral_type: &str, book_value: f64, haircut: f64) -> PyResult<Self> {
        let collateral_type = parse_collateral_type(collateral_type)?;
        CollateralPiece::new(collateral_type, book_value, haircut)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Canonical collateral-type label.
    #[getter]
    fn collateral_type(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_label(&self.inner.collateral_type).map_err(core_to_py)
    }

    /// Pre-haircut book value.
    #[getter]
    fn book_value(&self) -> f64 {
        self.inner.book_value
    }

    /// Liquidation haircut as a decimal in [0, 1].
    #[getter]
    fn haircut(&self) -> f64 {
        self.inner.haircut
    }

    /// ``book_value * (1 - haircut)``.
    #[getter]
    fn liquidation_value(&self) -> f64 {
        self.inner.liquidation_value()
    }

    /// Deserialize from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid CollateralPiece JSON"))?,
        })
    }

    /// Serialize to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "CollateralPiece serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Identify this value in notebooks and logs.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CollateralPiece", &self.inner)
    }
}

/// Direct and indirect workout cost rates as decimal fractions of EAD.
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
    /// Build a cost specification.
    ///
    /// Parameters
    /// ----------
    /// direct_cost_rate : float
    ///     Direct (legal, administrative) costs as a decimal fraction of EAD (>= 0).
    /// indirect_cost_rate : float
    ///     Indirect (opportunity) costs as a decimal fraction of EAD (>= 0).
    ///
    /// Raises ``ValueError`` for negative or non-finite rates.
    #[new]
    #[pyo3(text_signature = "(direct_cost_rate, indirect_cost_rate)")]
    fn new(direct_cost_rate: f64, indirect_cost_rate: f64) -> PyResult<Self> {
        WorkoutCosts::new(direct_cost_rate, indirect_cost_rate)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Zero workout costs.
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn zero() -> Self {
        Self {
            inner: WorkoutCosts::zero(),
        }
    }

    /// Registry-default workout costs.
    ///
    /// Raises ``ValueError`` if the embedded credit registry is invalid.
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn standard() -> PyResult<Self> {
        WorkoutCosts::standard()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Direct cost rate (decimal fraction of EAD).
    #[getter]
    fn direct_cost_rate(&self) -> f64 {
        self.inner.direct_cost_rate
    }

    /// Indirect cost rate (decimal fraction of EAD).
    #[getter]
    fn indirect_cost_rate(&self) -> f64 {
        self.inner.indirect_cost_rate
    }

    /// ``direct_cost_rate + indirect_cost_rate``.
    #[getter]
    fn total_rate(&self) -> f64 {
        self.inner.total_rate()
    }

    /// Deserialize from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid WorkoutCosts JSON"))?,
        })
    }

    /// Serialize to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "WorkoutCosts serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Identify this value in notebooks and logs.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("WorkoutCosts", &self.inner)
    }
}

/// Net recovery, LGD, and recovery rate from a workout evaluation.
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
    /// Post-cost, post-discount recovery amount (floored at zero), in EAD units.
    #[getter]
    fn net_recovery(&self) -> f64 {
        self.inner.net_recovery
    }

    /// Loss given default as a decimal in [0, 1].
    #[getter]
    fn lgd(&self) -> f64 {
        self.inner.lgd
    }

    /// Recovery rate ``1 - lgd`` as a decimal in [0, 1].
    #[getter]
    fn recovery_rate(&self) -> f64 {
        self.inner.recovery_rate
    }

    /// Deserialize from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid WorkoutLgdResult JSON"))?,
        })
    }

    /// Serialize to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "WorkoutLgdResult serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Export as a single-row pandas ``DataFrame``
    /// (``net_recovery``, ``lgd``, ``recovery_rate``).
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &self.inner,
            &["net_recovery", "lgd", "recovery_rate"],
        )
    }

    /// Identify this value in notebooks and logs.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("WorkoutLgdResult", &self.inner)
    }
}

/// Workout (collateral-waterfall) LGD model.
///
/// ``net_recovery = (min(sum liquidation values, EAD) - costs * EAD) * DF``
/// and ``lgd = 1 - clamp(net_recovery / EAD, 0, 1)``, where ``DF`` discounts
/// over the workout horizon (Basel workout-LGD methodology). Build with
/// ``WorkoutLgd.builder()``.
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
    /// Start a fluent builder (the only construction entry point).
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn builder() -> PyWorkoutLgdBuilder {
        PyWorkoutLgdBuilder {
            inner: Some(WorkoutLgd::builder()),
        }
    }

    /// Evaluate net recovery, LGD, and recovery rate at ``ead``.
    ///
    /// Parameters
    /// ----------
    /// ead : float
    ///     Exposure at default (> 0), in the collateral's currency.
    ///
    /// Raises ``ValueError`` when ``ead`` is non-finite or non-positive.
    #[pyo3(text_signature = "($self, ead)")]
    fn evaluate(&self, ead: f64) -> PyResult<PyWorkoutLgdResult> {
        self.inner
            .evaluate(ead)
            .map(|inner| PyWorkoutLgdResult { inner })
            .map_err(core_to_py)
    }

    /// Loss given default at ``ead`` as a decimal in [0, 1].
    ///
    /// Raises ``ValueError`` when ``ead`` is non-finite or non-positive.
    #[pyo3(text_signature = "($self, ead)")]
    fn lgd(&self, ead: f64) -> PyResult<f64> {
        self.inner.lgd(ead).map_err(core_to_py)
    }

    /// Net recovery amount at ``ead``.
    ///
    /// Raises ``ValueError`` when ``ead`` is non-finite or non-positive.
    #[pyo3(text_signature = "($self, ead)")]
    fn net_recovery(&self, ead: f64) -> PyResult<f64> {
        self.inner.net_recovery(ead).map_err(core_to_py)
    }

    /// Recovery rate ``1 - lgd`` at ``ead``.
    ///
    /// Raises ``ValueError`` when ``ead`` is non-finite or non-positive.
    #[pyo3(text_signature = "($self, ead)")]
    fn recovery_rate(&self, ead: f64) -> PyResult<f64> {
        self.inner.recovery_rate(ead).map_err(core_to_py)
    }

    /// Ordered collateral waterfall, highest priority first.
    #[getter]
    fn collateral(&self) -> Vec<PyCollateralPiece> {
        self.inner
            .collateral()
            .iter()
            .map(|piece| PyCollateralPiece { inner: *piece })
            .collect()
    }

    /// Expected workout duration in years.
    #[getter]
    fn workout_years(&self) -> f64 {
        self.inner.workout_years()
    }

    /// Annual decimal discount rate over the workout horizon.
    #[getter]
    fn discount_rate(&self) -> f64 {
        self.inner.discount_rate()
    }

    /// Direct and indirect cost rates.
    #[getter]
    fn costs(&self) -> PyWorkoutCosts {
        PyWorkoutCosts {
            inner: *self.inner.costs(),
        }
    }

    /// Export the collateral waterfall as a pandas ``DataFrame``
    /// (``collateral_type``, ``book_value``, ``haircut``), one row per piece.
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

    /// Deserialize from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid WorkoutLgd JSON"))?,
        })
    }

    /// Serialize to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "WorkoutLgd serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Identify this value in notebooks and logs.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("WorkoutLgd", &self.inner)
    }
}

/// Fluent builder for ``WorkoutLgd``; obtain via ``WorkoutLgd.builder()``.
///
/// Unset ``workout_years`` / ``discount_rate`` / ``costs`` fall back to the
/// embedded registry defaults at ``build()``.
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
    /// Append one collateral piece to the waterfall (highest priority first).
    ///
    /// Parameters
    /// ----------
    /// piece : CollateralPiece
    ///     Collateral to append.
    #[pyo3(text_signature = "($self, piece)")]
    fn collateral<'py>(
        mut slf: PyRefMut<'py, Self>,
        piece: PyCollateralPiece,
    ) -> PyResult<PyRefMut<'py, Self>> {
        Self::with_inner(&mut slf, |b| b.collateral(piece.inner))?;
        Ok(slf)
    }

    /// Append several collateral pieces in order.
    ///
    /// Parameters
    /// ----------
    /// pieces : list[CollateralPiece]
    ///     Collateral to append, highest priority first.
    #[pyo3(text_signature = "($self, pieces)")]
    fn collateral_pieces<'py>(
        mut slf: PyRefMut<'py, Self>,
        pieces: Vec<PyCollateralPiece>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let pieces: Vec<CollateralPiece> = pieces.into_iter().map(|p| p.inner).collect();
        Self::with_inner(&mut slf, |b| b.collateral_pieces(pieces))?;
        Ok(slf)
    }

    /// Set the expected workout duration in years (>= 0).
    #[pyo3(text_signature = "($self, years)")]
    fn workout_years(mut slf: PyRefMut<'_, Self>, years: f64) -> PyResult<PyRefMut<'_, Self>> {
        Self::with_inner(&mut slf, |b| b.workout_years(years))?;
        Ok(slf)
    }

    /// Set the annual decimal discount rate over the workout horizon (>= 0).
    #[pyo3(text_signature = "($self, rate)")]
    fn discount_rate(mut slf: PyRefMut<'_, Self>, rate: f64) -> PyResult<PyRefMut<'_, Self>> {
        Self::with_inner(&mut slf, |b| b.discount_rate(rate))?;
        Ok(slf)
    }

    /// Set the workout cost rates.
    #[pyo3(text_signature = "($self, costs)")]
    fn costs<'py>(
        mut slf: PyRefMut<'py, Self>,
        costs: PyWorkoutCosts,
    ) -> PyResult<PyRefMut<'py, Self>> {
        Self::with_inner(&mut slf, |b| b.costs(costs.inner))?;
        Ok(slf)
    }

    /// Validate and build the ``WorkoutLgd`` model; the builder is consumed.
    ///
    /// Raises ``ValueError`` when ``workout_years`` or ``discount_rate`` is
    /// negative or non-finite, or the builder was already consumed.
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

/// Compute workout net recovery, LGD, and recovery rate in one call.
///
/// One-shot twin of ``WorkoutLgd.builder()...build().evaluate(ead)``.
///
/// Parameters
/// ----------
/// ead : float
///     Exposure at default (> 0).
/// collateral : list[tuple[str, float, float]]
///     ``(collateral_type, book_value, haircut)`` triples; ``collateral_type``
///     is one of ``cash``, ``securities``, ``receivables``, ``inventory``,
///     ``equipment``, ``real_estate``, ``intellectual_property``, ``other``
///     and ``haircut`` is a decimal in [0, 1].
/// direct_cost_pct : float
///     Direct resolution costs as a decimal fraction of EAD (>= 0).
/// indirect_cost_pct : float
///     Indirect resolution costs as a decimal fraction of EAD (>= 0).
/// time_to_resolution_years : float
///     Expected workout duration in years (>= 0).
/// discount_rate : float
///     Annual decimal discount rate for the workout period (>= 0).
///
/// Returns a ``WorkoutLgdResult``.
///
/// Raises ``ValueError`` for an unknown collateral type or any invalid input.
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

/// Downturn LGD adjuster (stressed approximation or regulatory floor).
///
/// ``method`` is ``"stressed_approximation"`` or ``"regulatory_floor"``;
/// ``adjust(base_lgd)`` returns the downturn LGD clamped to [0, 1].
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
    /// Stressed approximation:
    /// ``LGD_base + lgd_sensitivity * sqrt(rho) * Phi^-1(q) * sqrt(LGD_base * (1 - LGD_base))``.
    ///
    /// Parameters
    /// ----------
    /// asset_correlation : float
    ///     Asset correlation rho in (0, 1). Basel: 0.12-0.24.
    /// lgd_sensitivity : float
    ///     LGD sensitivity to the systematic factor (>= 0). Typical: 0.3-0.5.
    /// stress_quantile : float
    ///     Downturn quantile in (0, 1), e.g. 0.999.
    ///
    /// Raises ``ValueError`` on out-of-range parameters.
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

    /// Regulatory floor: ``max(LGD_base + add_on, floor)``.
    ///
    /// Parameters
    /// ----------
    /// add_on : float
    ///     Flat add-on (>= 0). Typical: 0.05-0.10.
    /// floor : float
    ///     Absolute floor in [0, 1]. Typical: 0.10 secured / 0.25 unsecured.
    ///
    /// Raises ``ValueError`` on out-of-range parameters.
    #[staticmethod]
    #[pyo3(text_signature = "(add_on, floor)")]
    fn regulatory_floor(add_on: f64, floor: f64) -> PyResult<Self> {
        DownturnLgd::regulatory_floor(add_on, floor)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Load a regulatory-floor preset by id from the embedded credit registry.
    ///
    /// Raises ``KeyError`` for an unknown id and ``ValueError`` for an
    /// unsupported preset method.
    #[staticmethod]
    #[pyo3(text_signature = "(id)")]
    fn from_registry_id(id: &str) -> PyResult<Self> {
        DownturnLgd::from_registry_id(id)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Registry default secured-exposure floor (Basel).
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn basel_secured() -> PyResult<Self> {
        DownturnLgd::basel_secured()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Registry ``basel_unsecured`` floor preset.
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn basel_unsecured() -> PyResult<Self> {
        DownturnLgd::basel_unsecured()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Downturn LGD for ``base_lgd`` (decimal in [0, 1]), clamped to [0, 1].
    ///
    /// Raises ``ValueError`` when ``base_lgd`` is non-finite or outside [0, 1].
    #[pyo3(text_signature = "($self, base_lgd)")]
    fn adjust(&self, base_lgd: f64) -> PyResult<f64> {
        self.inner.adjust(base_lgd).map_err(core_to_py)
    }

    /// Canonical method name: ``"stressed_approximation"`` or ``"regulatory_floor"``.
    #[getter]
    fn method(&self) -> PyResult<String> {
        match serde_json::to_value(self.inner.method())
            .map_err(|err| serde_json_to_py(err, "DownturnMethod serialization failed"))?
        {
            serde_json::Value::Object(map) => map
                .keys()
                .next()
                .cloned()
                .ok_or_else(|| value_error("DownturnMethod has no tag")),
            serde_json::Value::String(tag) => Ok(tag),
            other => Err(value_error(format!(
                "unexpected DownturnMethod form {other}"
            ))),
        }
    }

    /// Method parameters as a mapping in canonical JSON form.
    #[getter]
    fn params<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, self.inner.method())
    }

    /// Deserialize from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: DownturnLgd = serde_json::from_str(json)
            .map_err(|err| serde_json_to_py(err, "invalid DownturnLgd JSON"))?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "DownturnLgd serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Identify this value in notebooks and logs.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("DownturnLgd", &self.inner)
    }
}

/// Exposure-at-default calculator: ``EAD = drawn + undrawn * CCF``.
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
    /// Build a calculator with an explicit credit conversion factor.
    ///
    /// Parameters
    /// ----------
    /// drawn : float
    ///     Currently drawn amount (>= 0).
    /// undrawn : float
    ///     Undrawn commitment (>= 0).
    /// ccf : float
    ///     Credit conversion factor as a decimal in [0, 1].
    ///
    /// Raises ``ValueError`` for negative or non-finite amounts or a CCF
    /// outside [0, 1].
    #[new]
    #[pyo3(text_signature = "(drawn, undrawn, ccf)")]
    fn new(drawn: f64, undrawn: f64, ccf: f64) -> PyResult<Self> {
        let ccf = CreditConversionFactor::new(ccf).map_err(core_to_py)?;
        EadCalculator::new(drawn, undrawn, ccf)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Fully drawn term loan (no undrawn component, CCF 1.0).
    ///
    /// Raises ``ValueError`` when ``drawn`` is negative or non-finite.
    #[staticmethod]
    #[pyo3(text_signature = "(drawn)")]
    fn term_loan(drawn: f64) -> PyResult<Self> {
        EadCalculator::term_loan(drawn)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Revolver with the Basel IRB CCF of 0.75.
    ///
    /// Raises ``ValueError`` when an amount is negative or non-finite.
    #[staticmethod]
    #[pyo3(text_signature = "(drawn, undrawn)")]
    fn revolver(drawn: f64, undrawn: f64) -> PyResult<Self> {
        EadCalculator::revolver(drawn, undrawn)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// ``drawn + undrawn * ccf``.
    #[getter]
    fn ead(&self) -> f64 {
        self.inner.ead()
    }

    /// ``drawn / (drawn + undrawn)``, or 0.0 when there is no commitment.
    #[getter]
    fn utilization(&self) -> f64 {
        self.inner.utilization()
    }

    /// ``drawn + undrawn``.
    #[getter]
    fn total_commitment(&self) -> f64 {
        self.inner.total_commitment()
    }

    /// Loan-equivalent exposure implied by an observed EAD:
    /// ``(observed_ead - drawn) / undrawn``, or ``None`` with no undrawn amount.
    ///
    /// Parameters
    /// ----------
    /// observed_ead : float
    ///     Realised exposure at default in the facility's currency.
    #[pyo3(text_signature = "($self, observed_ead)")]
    fn leq_from_observed_ead(&self, observed_ead: f64) -> Option<f64> {
        self.inner.leq_from_observed_ead(observed_ead)
    }

    /// Deserialize from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid EadCalculator JSON"))?,
        })
    }

    /// Serialize to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "EadCalculator serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Identify this value in notebooks and logs.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("EadCalculator", &self.inner)
    }
}

/// Apply a stressed downturn adjustment to a base LGD.
///
/// Thin twin of ``DownturnLgd.stressed(...).adjust(base_lgd)``:
///
/// ```text
/// LGD_downturn = LGD_base + lgd_sensitivity * sqrt(rho) * Phi^-1(q)
///              * sqrt(LGD_base * (1 - LGD_base))
/// ```
///
/// (a mean-plus-multiple-of-Bernoulli-stdev approximation, not the
/// Frye-Jacobs 2012 model). The result is clamped to [0, 1].
///
/// Parameters
/// ----------
/// base_lgd : float
///     Through-the-cycle LGD in [0, 1].
/// asset_correlation : float
///     Asset correlation rho in (0, 1). Basel: 0.12-0.24.
/// lgd_sensitivity : float
///     LGD sensitivity to the systematic factor (>= 0). Typical: 0.3-0.5.
/// stress_quantile : float
///     Downturn quantile in (0, 1), e.g. 0.999.
///
/// Raises ``ValueError`` on out-of-range inputs.
#[pyfunction]
#[pyo3(text_signature = "(base_lgd, asset_correlation, lgd_sensitivity, stress_quantile)")]
fn downturn_lgd_stressed(
    base_lgd: f64,
    asset_correlation: f64,
    lgd_sensitivity: f64,
    stress_quantile: f64,
) -> PyResult<f64> {
    lgd::downturn_lgd_stressed(
        base_lgd,
        asset_correlation,
        lgd_sensitivity,
        stress_quantile,
    )
    .map_err(core_to_py)
}

/// Apply a regulatory floor downturn adjustment to a base LGD.
///
/// Thin twin of ``DownturnLgd.regulatory_floor(add_on, floor).adjust(base_lgd)``:
/// ``LGD_downturn = max(LGD_base + add_on, floor)`` clamped to [0, 1].
///
/// Parameters
/// ----------
/// base_lgd : float
///     Through-the-cycle LGD in [0, 1].
/// add_on : float
///     Flat add-on (>= 0). Typical: 0.05-0.10.
/// floor : float
///     Absolute floor in [0, 1]. Typical: 0.10 secured / 0.25 unsecured.
///
/// Raises ``ValueError`` on out-of-range inputs.
#[pyfunction]
#[pyo3(text_signature = "(base_lgd, add_on, floor)")]
fn downturn_lgd_regulatory_floor(base_lgd: f64, add_on: f64, floor: f64) -> PyResult<f64> {
    lgd::downturn_lgd_regulatory_floor(base_lgd, add_on, floor).map_err(core_to_py)
}

/// Exposure at default for a fully drawn term loan (``principal`` itself).
///
/// Parameters
/// ----------
/// principal : float
///     Drawn principal (>= 0).
///
/// Raises ``ValueError`` when ``principal`` is negative or non-finite.
#[pyfunction]
#[pyo3(text_signature = "(principal)")]
fn ead_term_loan(principal: f64) -> PyResult<f64> {
    lgd::ead_term_loan(principal).map_err(core_to_py)
}

/// Exposure at default for a revolving facility: ``drawn + undrawn * ccf``.
///
/// Parameters
/// ----------
/// drawn : float
///     Currently drawn amount (>= 0).
/// undrawn : float
///     Undrawn commitment (>= 0).
/// ccf : float
///     Credit conversion factor in [0, 1]. Basel IRB: 0.75.
///
/// Raises ``ValueError`` on negative amounts or a CCF outside [0, 1].
#[pyfunction]
#[pyo3(text_signature = "(drawn, undrawn, ccf)")]
fn ead_revolver(drawn: f64, undrawn: f64, ccf: f64) -> PyResult<f64> {
    lgd::ead_revolver(drawn, undrawn, ccf).map_err(core_to_py)
}

/// Build the `finstack_quant.models.credit.lgd` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "lgd")?;
    m.setattr(
        "__doc__",
        format!(
            "Loss-given-default modeling: seniority Beta recovery, workout LGD, downturn adjustments, EAD. \
             Collateral types: {COLLATERAL_TYPES}."
        ),
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
    m.add_function(wrap_pyfunction!(beta_recovery_sample, &m)?)?;
    m.add_function(wrap_pyfunction!(beta_recovery_quantile, &m)?)?;
    m.add_function(wrap_pyfunction!(workout_lgd, &m)?)?;
    m.add_function(wrap_pyfunction!(downturn_lgd_stressed, &m)?)?;
    m.add_function(wrap_pyfunction!(downturn_lgd_regulatory_floor, &m)?)?;
    m.add_function(wrap_pyfunction!(ead_term_loan, &m)?)?;
    m.add_function(wrap_pyfunction!(ead_revolver, &m)?)?;

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
            "beta_recovery_quantile",
            "beta_recovery_sample",
            "downturn_lgd_regulatory_floor",
            "downturn_lgd_stressed",
            "ead_revolver",
            "ead_term_loan",
            "seniority_recovery_stats",
            "workout_lgd",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "lgd",
        "finstack_quant.models.credit",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
