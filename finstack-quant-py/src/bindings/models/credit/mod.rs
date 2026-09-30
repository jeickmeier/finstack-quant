//! Python bindings for structural credit model specifications.

mod lgd;
mod liability_management;
mod migration;
mod pd;
mod recovery_waterfall;
mod scoring;

use std::sync::Arc;

use crate::bindings::core::market_data::curves::helpers::parse_day_count;
use crate::bindings::core::market_data::curves::PyHazardCurve;
use crate::bindings::date_utils::py_to_date;
use crate::bindings::extract::extract_credit_rating;
use crate::bindings::pandas_utils::{
    dict_to_dataframe, serde_object_to_single_row_dataframe, serde_to_py, values_to_series,
};
use crate::errors::{core_to_py, serde_json_to_py, value_error};
use finstack_quant_models::credit::{
    moodys_warf_factor as rust_moodys_warf_factor, AssetDynamics, CreditState, CreditStateVariable,
    DynamicRecoverySpec, EndogenousHazardSpec, MertonBarrierType, MertonModel, RatingFactorTable,
    SimulatedPaths, ThresholdDirection, ToggleExerciseModel,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule};

/// Return the Moody's WARF factor for a canonical credit rating.
///
/// Parameters
/// ----------
/// rating : str | CreditRating
///     Exact-notch rating, either a ``core.types.CreditRating`` or a rating
///     string in S&P/Fitch (``"BBB-"``) or Moody's (``"Baa3"``) notation.
///
/// Returns
/// -------
/// float
///     Moody's ordinal weighted-average rating factor (``B`` -> ``2720.0``).
///
/// Raises
/// ------
/// ValueError
///     If the string is not a recognised rating, the embedded
///     credit-assumptions registry is invalid, or the rating has no factor in
///     the configured Moody's table.
/// TypeError
///     If ``rating`` is neither a string nor a ``CreditRating``.
#[pyfunction]
#[pyo3(text_signature = "(rating)")]
fn moodys_warf_factor(rating: &Bound<'_, PyAny>) -> PyResult<f64> {
    let rating = extract_credit_rating(rating)?;
    rust_moodys_warf_factor(rating).map_err(core_to_py)
}

/// Rating-factor table (Moody's WARF methodology) loaded from the embedded
/// credit-assumptions registry.
///
/// Use ``RatingFactorTable.moodys_standard()`` for the default table or
/// ``RatingFactorTable.from_registry_id(id)`` for a named methodology. Exact
/// rating notches map to factors via ``get_factor``; unrated or defaulted
/// names take ``default_factor``.
#[pyclass(
    name = "RatingFactorTable",
    module = "finstack_quant.models.credit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyRatingFactorTable {
    pub(crate) inner: RatingFactorTable,
}

#[pymethods]
impl PyRatingFactorTable {
    /// Load the embedded Moody's standard WARF table.
    ///
    /// Raises ``ValueError`` if the embedded registry is invalid or its
    /// configured default rating-factor table is missing.
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn moodys_standard() -> PyResult<Self> {
        RatingFactorTable::moodys_standard()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Load a named rating-factor table from the embedded registry.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Exact registry identifier of the methodology. An unknown id raises
    ///     ``KeyError`` and never falls back to the default table.
    #[staticmethod]
    #[pyo3(text_signature = "(id)")]
    fn from_registry_id(id: &str) -> PyResult<Self> {
        RatingFactorTable::from_registry_id(id)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Rating factor for an exact rating notch.
    ///
    /// Parameters
    /// ----------
    /// rating : str | CreditRating
    ///     Rating notch as a ``core.types.CreditRating`` or a rating string.
    ///
    /// Raises ``ValueError`` when the notch has no factor in this table or
    /// the string is not a recognised rating.
    #[pyo3(text_signature = "($self, rating)")]
    fn get_factor(&self, rating: &Bound<'_, PyAny>) -> PyResult<f64> {
        let rating = extract_credit_rating(rating)?;
        self.inner.get_factor(rating).map_err(core_to_py)
    }

    /// Agency the table is sourced from (e.g. ``"moodys"``).
    #[getter]
    fn agency(&self) -> String {
        self.inner.agency().to_string()
    }

    /// Methodology label recorded in the registry for this table.
    #[getter]
    fn methodology(&self) -> String {
        self.inner.methodology().to_string()
    }

    /// Factor assigned to unrated or defaulted names.
    #[getter]
    fn default_factor(&self) -> f64 {
        self.inner.default_factor()
    }

    /// Deserialize a rating-factor table from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid RatingFactorTable JSON"))?,
        })
    }

    /// Serialize this table to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "RatingFactorTable serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Identify this value in notebooks and logs.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("RatingFactorTable", &self.inner)
    }
}

/// Default-barrier monitoring convention for structural credit models.
///
/// ``MertonBarrierType.terminal()`` tests default only at maturity (Merton 1974);
/// ``MertonBarrierType.first_passage(growth)`` tests continuously against a barrier
/// growing at ``growth`` per year (Black-Cox 1976).
#[pyclass(
    name = "MertonBarrierType",
    module = "finstack_quant.models.credit",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct PyBarrierType {
    pub(crate) inner: MertonBarrierType,
}

impl PyBarrierType {
    pub(crate) fn from_inner(inner: MertonBarrierType) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyBarrierType {
    /// Classic Merton barrier tested only at maturity.
    #[staticmethod]
    fn terminal() -> Self {
        Self {
            inner: MertonBarrierType::Terminal,
        }
    }

    /// Black-Cox first-passage barrier with optional growth rate.
    ///
    /// # Arguments
    ///
    /// * `barrier_growth_rate` - Continuous growth rate of the default barrier
    ///   over time, as a decimal (e.g. ``0.02`` for 2% annual growth).
    #[staticmethod]
    fn first_passage(barrier_growth_rate: f64) -> Self {
        Self {
            inner: MertonBarrierType::FirstPassage {
                barrier_growth_rate,
            },
        }
    }

    /// Deserialize a barrier type from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid MertonBarrierType JSON"))?,
        })
    }

    /// Serialize this barrier type to compact JSON.
    #[allow(clippy::wrong_self_convention)]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "MertonBarrierType serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Identify this value in notebooks and logs (``MertonBarrierType.terminal()``,
    /// ``MertonBarrierType.first_passage(barrier_growth_rate=0.02)``).
    fn __repr__(&self) -> String {
        variant_repr("MertonBarrierType", &self.inner)
    }
}

/// Asset-return dynamics for structural credit models.
///
/// ``geometric_brownian()`` (lognormal diffusion), ``jump_diffusion(...)``
/// (Merton 1976 jumps) or ``credit_grades(...)`` (stochastic barrier).
#[pyclass(
    name = "AssetDynamics",
    module = "finstack_quant.models.credit",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct PyAssetDynamics {
    pub(crate) inner: AssetDynamics,
}

impl PyAssetDynamics {
    pub(crate) fn from_inner(inner: AssetDynamics) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyAssetDynamics {
    /// Standard geometric Brownian motion (lognormal diffusion).
    #[staticmethod]
    fn geometric_brownian() -> Self {
        Self {
            inner: AssetDynamics::GeometricBrownian,
        }
    }

    /// Merton jump-diffusion asset dynamics.
    ///
    /// # Arguments
    ///
    /// * `jump_intensity` - Poisson jump arrival intensity (jumps per year).
    /// * `jump_mean` - Mean log-jump size.
    /// * `jump_vol` - Volatility of log-jump size.
    #[staticmethod]
    fn jump_diffusion(jump_intensity: f64, jump_mean: f64, jump_vol: f64) -> Self {
        Self {
            inner: AssetDynamics::JumpDiffusion {
                jump_intensity,
                jump_mean,
                jump_vol,
            },
        }
    }

    /// CreditGrades stochastic-barrier dynamics.
    ///
    /// # Arguments
    ///
    /// * `barrier_uncertainty` - Log-barrier volatility ``lambda`` (lognormal
    ///   standard deviation of the default barrier).
    /// * `mean_recovery` - Mean recovery rate at default, as a decimal in
    ///   ``[0, 1]``.
    #[staticmethod]
    fn credit_grades(barrier_uncertainty: f64, mean_recovery: f64) -> Self {
        Self {
            inner: AssetDynamics::CreditGrades {
                barrier_uncertainty,
                mean_recovery,
            },
        }
    }

    /// Deserialize asset dynamics from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid AssetDynamics JSON"))?,
        })
    }

    /// Serialize these asset dynamics to compact JSON.
    #[allow(clippy::wrong_self_convention)]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "AssetDynamics serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Identify this value in notebooks and logs
    /// (``AssetDynamics.geometric_brownian()``,
    /// ``AssetDynamics.jump_diffusion(jump_intensity=..., ...)``).
    fn __repr__(&self) -> String {
        variant_repr("AssetDynamics", &self.inner)
    }
}

/// Merton-family structural credit model (Merton 1974, Black-Cox 1976,
/// CreditGrades) over firm asset value, volatility, and a debt barrier.
///
/// Construct directly from asset inputs, or calibrate with ``from_equity``
/// (KMV), ``from_cds_spread``, ``from_target_pd`` or ``credit_grades``.
/// Rates and volatilities are decimals (``0.05`` is 5%); spreads returned by
/// ``implied_spread`` / ``debt_spread`` / ``cds_par_spread`` are decimals,
/// not basis points.
///
/// Raises ``ValueError`` on invalid inputs and ``RuntimeError`` when a
/// calibration root-finder fails to converge.
#[pyclass(
    name = "MertonModel",
    module = "finstack_quant.models.credit",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyMertonModel {
    pub(crate) inner: MertonModel,
}

#[pymethods]
impl PyMertonModel {
    /// Construct a Merton structural credit model from firm asset inputs.
    ///
    /// Parameters
    /// ----------
    /// asset_value : float
    ///     Firm asset value ``V_0`` (positive, finite), in the issuer's
    ///     reporting currency.
    /// asset_vol : float
    ///     Annualized asset volatility as a decimal (``0.25`` is 25%).
    /// debt_barrier : float
    ///     Default barrier ``B``, typically total debt face, same units as
    ///     ``asset_value``.
    /// risk_free_rate : float
    ///     Continuously compounded risk-free rate as a decimal.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     When inputs are non-finite or out of range.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.models.credit import MertonModel
    /// >>> round(MertonModel(100.0, 0.25, 80.0, 0.05).default_probability(1.0), 6)
    /// 0.166629
    #[new]
    #[pyo3(text_signature = "(asset_value, asset_vol, debt_barrier, risk_free_rate)")]
    fn new(
        asset_value: f64,
        asset_vol: f64,
        debt_barrier: f64,
        risk_free_rate: f64,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: MertonModel::new(asset_value, asset_vol, debt_barrier, risk_free_rate)
                .map_err(core_to_py)?,
        })
    }

    /// KMV calibration: recover asset value and volatility from equity inputs.
    ///
    /// # Arguments
    ///
    /// * `equity_value` - Observed market equity value
    /// * `equity_vol` - Observed equity volatility as a decimal
    /// * `total_debt` - Face value of debt used as the default barrier
    /// * `risk_free_rate` - Continuously compounded risk-free rate as a decimal
    /// * `payout_rate` - Continuous dividend / payout yield on assets as a decimal
    /// * `maturity` - Calibration horizon in years
    #[staticmethod]
    fn from_equity(
        equity_value: f64,
        equity_vol: f64,
        total_debt: f64,
        risk_free_rate: f64,
        payout_rate: f64,
        maturity: f64,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: MertonModel::from_equity(
                equity_value,
                equity_vol,
                total_debt,
                risk_free_rate,
                payout_rate,
                maturity,
            )
            .map_err(core_to_py)?,
        })
    }

    /// CDS spread calibration: find asset volatility matching a target spread.
    ///
    /// # Arguments
    ///
    /// * `cds_spread_bp` - Target CDS par spread in basis points
    /// * `recovery` - Assumed recovery rate as a decimal in ``[0, 1]``
    /// * `total_debt` - Face value of debt
    /// * `risk_free_rate` - Continuously compounded risk-free rate as a decimal
    /// * `maturity` - Calibration horizon in years
    /// * `asset_value` - Assumed initial firm asset value
    /// * `payout_rate` - Continuous payout rate on assets as a decimal
    #[staticmethod]
    fn from_cds_spread(
        cds_spread_bp: f64,
        recovery: f64,
        total_debt: f64,
        risk_free_rate: f64,
        maturity: f64,
        asset_value: f64,
        payout_rate: f64,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: MertonModel::from_cds_spread(
                cds_spread_bp,
                recovery,
                total_debt,
                risk_free_rate,
                maturity,
                asset_value,
                payout_rate,
            )
            .map_err(core_to_py)?,
        })
    }

    /// Calibrate the debt barrier to match a target cumulative default probability.
    ///
    /// # Arguments
    ///
    /// * `asset_value` - Current firm asset value, strictly positive
    /// * `asset_vol` - Annualized asset volatility as a decimal, strictly
    ///   positive
    /// * `risk_free_rate` - Continuously compounded risk-free rate as a
    ///   decimal. Pass the expected physical asset return instead to
    ///   calibrate against a real-world default rate
    /// * `payout_rate` - Continuous payout rate on assets as a decimal; it
    ///   enters the calibration drift and is carried on the returned model
    /// * `target_pd` - Target cumulative default probability in ``(0, 1)``
    /// * `maturity` - Calibration horizon in years, strictly positive
    #[staticmethod]
    fn from_target_pd(
        asset_value: f64,
        asset_vol: f64,
        risk_free_rate: f64,
        payout_rate: f64,
        target_pd: f64,
        maturity: f64,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: MertonModel::from_target_pd(
                asset_value,
                asset_vol,
                risk_free_rate,
                payout_rate,
                target_pd,
                maturity,
            )
            .map_err(core_to_py)?,
        })
    }

    /// Moody's KMV default point: short-term debt plus half of long-term debt.
    ///
    /// # Arguments
    ///
    /// * `short_term_debt` - Liabilities due within one year, non-negative
    /// * `long_term_debt` - Liabilities maturing beyond one year,
    ///   non-negative; half of it enters the default point
    ///
    /// # Errors
    ///
    /// Raises ``ValueError`` when either input is negative or non-finite, or
    /// when the resulting default point is zero.
    #[staticmethod]
    fn kmv_default_point(short_term_debt: f64, long_term_debt: f64) -> PyResult<f64> {
        MertonModel::kmv_default_point(short_term_debt, long_term_debt).map_err(core_to_py)
    }

    /// Construct a Merton model with explicit barrier and dynamics specifications.
    ///
    /// # Arguments
    ///
    /// * `asset_value` - Current firm asset value
    /// * `asset_vol` - Asset volatility as a decimal
    /// * `debt_barrier` - Default barrier level
    /// * `risk_free_rate` - Continuously compounded risk-free rate as a decimal
    /// * `payout_rate` - Continuous payout rate on assets as a decimal
    /// * `barrier_type` - Terminal or first-passage barrier monitoring
    /// * `dynamics` - Asset return dynamics specification
    #[staticmethod]
    #[allow(clippy::too_many_arguments)]
    fn new_with_dynamics(
        asset_value: f64,
        asset_vol: f64,
        debt_barrier: f64,
        risk_free_rate: f64,
        payout_rate: f64,
        barrier_type: PyRef<'_, PyBarrierType>,
        dynamics: PyRef<'_, PyAssetDynamics>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: MertonModel::new_with_dynamics(
                asset_value,
                asset_vol,
                debt_barrier,
                risk_free_rate,
                payout_rate,
                barrier_type.inner,
                dynamics.inner,
            )
            .map_err(core_to_py)?,
        })
    }

    /// Build a CreditGrades-style structural model from equity inputs.
    ///
    /// Calibrates asset value and volatility implied by observed equity value
    /// and equity volatility under the O'Kane CreditGrades barrier specification.
    #[staticmethod]
    fn credit_grades(
        equity_value: f64,
        equity_vol: f64,
        total_debt: f64,
        risk_free_rate: f64,
        barrier_uncertainty: f64,
        mean_recovery: f64,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: MertonModel::credit_grades(
                equity_value,
                equity_vol,
                total_debt,
                risk_free_rate,
                barrier_uncertainty,
                mean_recovery,
            )
            .map_err(core_to_py)?,
        })
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

    /// Deserialize a structural credit model from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid MertonModel JSON"))?,
        })
    }

    /// Serialize this model to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "MertonModel serialization failed"))
    }

    /// Current firm asset value ``V_0``, in the issuer's reporting currency.
    #[getter]
    fn asset_value(&self) -> f64 {
        self.inner.asset_value()
    }

    /// Annualized asset volatility ``sigma_V`` as a decimal (``0.25`` is 25%).
    #[getter]
    fn asset_vol(&self) -> f64 {
        self.inner.asset_vol()
    }

    /// Default barrier ``B``, in the same currency as ``asset_value``.
    #[getter]
    fn debt_barrier(&self) -> f64 {
        self.inner.debt_barrier()
    }

    /// Continuously compounded risk-free rate ``r`` as a decimal.
    #[getter]
    fn risk_free_rate(&self) -> f64 {
        self.inner.risk_free_rate()
    }

    /// Continuous payout (dividend) rate ``q`` on assets, as a decimal.
    #[getter]
    fn payout_rate(&self) -> f64 {
        self.inner.payout_rate()
    }

    /// Barrier monitoring convention.
    #[getter]
    fn barrier_type(&self) -> PyBarrierType {
        PyBarrierType::from_inner(*self.inner.barrier_type())
    }

    /// Asset return dynamics specification.
    #[getter]
    fn dynamics(&self) -> PyAssetDynamics {
        PyAssetDynamics::from_inner(*self.inner.dynamics())
    }

    /// Export the model parameters as a single-row :class:`pandas.DataFrame`.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe(py, &self.inner)
    }

    /// Risk-neutral distance to default at `horizon` years.
    fn distance_to_default(&self, horizon: f64) -> f64 {
        self.inner.distance_to_default(horizon)
    }

    /// Physical-measure (Moody's KMV) distance to default at `horizon` years.
    ///
    /// # Arguments
    ///
    /// * `asset_drift` - Expected physical total return on the firm's assets
    ///   as a continuously compounded decimal, replacing the risk-free rate
    /// * `horizon` - Time horizon in years
    ///
    /// # Errors
    ///
    /// Raises ``ValueError`` when `asset_drift` is not finite or the model
    /// uses driftless CreditGrades dynamics.
    fn distance_to_default_with_drift(&self, asset_drift: f64, horizon: f64) -> PyResult<f64> {
        self.inner
            .distance_to_default_with_drift(asset_drift, horizon)
            .map_err(core_to_py)
    }

    /// Risk-neutral default probability over `horizon` years.
    fn default_probability(&self, horizon: f64) -> f64 {
        self.inner.default_probability(horizon)
    }

    /// Risk-neutral cumulative default probabilities over several horizons.
    ///
    /// A pandas convenience over :meth:`default_probability`: each horizon
    /// follows the same Rust rule, so a non-positive horizon gives ``0.0``.
    ///
    /// Parameters
    /// ----------
    /// horizons : list[float]
    ///     Horizons in years.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Float series named ``default_probability`` indexed by the horizon
    ///     labels (``str(horizon)``), in input order.
    #[pyo3(text_signature = "($self, horizons)")]
    fn default_probabilities<'py>(
        &self,
        py: Python<'py>,
        horizons: Vec<f64>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let labels: Vec<String> = horizons.iter().map(|h| h.to_string()).collect();
        let values: Vec<f64> = horizons
            .iter()
            .map(|h| self.inner.default_probability(*h))
            .collect();
        values_to_series(py, values, &labels, "default_probability")
    }

    /// Physical-measure default probability (theoretical EDF) over `horizon` years.
    ///
    /// # Arguments
    ///
    /// * `asset_drift` - Expected physical total return on the firm's assets
    ///   as a continuously compounded decimal, replacing the risk-free rate
    /// * `horizon` - Time horizon in years
    ///
    /// # Errors
    ///
    /// Raises ``ValueError`` when `asset_drift` is not finite or the model
    /// uses driftless CreditGrades dynamics.
    fn default_probability_with_drift(&self, asset_drift: f64, horizon: f64) -> PyResult<f64> {
        self.inner
            .default_probability_with_drift(asset_drift, horizon)
            .map_err(core_to_py)
    }

    /// Zero-coupon bond spread with exogenous `recovery` (decimal, not bp).
    ///
    /// # Errors
    ///
    /// Raises `ValueError` when `horizon` or `recovery` are invalid.
    fn implied_spread(&self, horizon: f64, recovery: f64) -> PyResult<f64> {
        self.inner
            .implied_spread(horizon, recovery)
            .map_err(core_to_py)
    }

    /// Merton (1974) endogenous debt spread at `horizon` years (decimal, not bp).
    ///
    /// # Errors
    ///
    /// Raises ``ValueError`` when `horizon` is not positive, the barrier type
    /// is not terminal, or the implied debt value is non-positive.
    fn debt_spread(&self, horizon: f64) -> PyResult<f64> {
        self.inner.debt_spread(horizon).map_err(core_to_py)
    }

    /// ISDA-style CDS par spread at `maturity` years (decimal, not bp).
    ///
    /// # Arguments
    ///
    /// * `maturity` - CDS maturity in years, strictly positive
    /// * `recovery` - Recovery rate as a decimal in ``[0, 1]``; must equal the
    ///   model's ``mean_recovery`` under CreditGrades dynamics
    ///
    /// # Errors
    ///
    /// Raises ``ValueError`` when the inputs are out of range or the implied
    /// survival curve cannot be bootstrapped.
    fn cds_par_spread(&self, maturity: f64, recovery: f64) -> PyResult<f64> {
        self.inner
            .cds_par_spread(maturity, recovery)
            .map_err(core_to_py)
    }

    /// Implied equity value and equity volatility at ``horizon`` years.
    ///
    /// # Arguments
    ///
    /// * `horizon` - Time horizon in years (must be positive and finite)
    ///
    /// # Errors
    ///
    /// Raises ``ValueError`` when the firm is economically in default or the
    /// inversion is numerically ill-conditioned.
    fn try_implied_equity(&self, horizon: f64) -> PyResult<(f64, f64)> {
        self.inner.try_implied_equity(horizon).map_err(core_to_py)
    }

    /// Bootstrap a piecewise-constant hazard curve from structural default probabilities.
    ///
    /// # Arguments
    ///
    /// * `id` - Curve identifier
    /// * `base_date` - Valuation date for the curve
    /// * `tenors` - Tenor grid in years (non-empty, strictly positive, distinct)
    /// * `recovery` - Recovery rate assumption as a decimal in ``[0, 1]``;
    ///   must equal the model's ``mean_recovery`` under CreditGrades dynamics
    /// * `day_count` - Day-count convention the curve uses to turn dates into
    ///   year fractions; pass the convention of the discount curve the hazard
    ///   curve will be paired with (required, as in Rust and WASM)
    #[pyo3(text_signature = "($self, id, base_date, tenors, recovery, day_count)")]
    fn to_hazard_curve(
        &self,
        id: &str,
        base_date: &Bound<'_, PyAny>,
        tenors: Vec<f64>,
        recovery: f64,
        day_count: &str,
    ) -> PyResult<PyHazardCurve> {
        let base_date = py_to_date(base_date)?;
        let day_count = parse_day_count(day_count)?;
        let curve = self
            .inner
            .to_hazard_curve(id, base_date, &tenors, recovery, day_count)
            .map_err(core_to_py)?;
        Ok(PyHazardCurve::from_inner(Arc::new(curve)))
    }

    /// Simulate asset value paths using Monte Carlo.
    ///
    /// # Arguments
    ///
    /// * `num_paths` - Number of paths to simulate
    /// * `num_steps` - Number of time steps per path (must be >= 1)
    /// * `horizon` - Simulation horizon in years (must be > 0)
    /// * `seed` - Seed for reproducible draws; the Rust
    ///   ``MertonModel::simulate_paths_seeded`` owns the generator (PCG64), so
    ///   equal seeds give equal paths in every host
    /// * `antithetic` - When ``True``, use antithetic variates for variance
    ///   reduction (required, as in Rust and WASM)
    #[pyo3(text_signature = "($self, num_paths, num_steps, horizon, seed, antithetic)")]
    fn simulate_paths(
        &self,
        num_paths: usize,
        num_steps: usize,
        horizon: f64,
        seed: u64,
        antithetic: bool,
    ) -> PyResult<PySimulatedPaths> {
        let paths = self
            .inner
            .simulate_paths_seeded(num_paths, num_steps, horizon, seed, antithetic)
            .map_err(core_to_py)?;
        Ok(PySimulatedPaths::from_inner(paths))
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("MertonModel", &self.inner)
    }
}

/// Monte Carlo asset-value paths from ``MertonModel.simulate_paths``.
///
/// ``asset_values`` is row-major: path ``p`` occupies indices
/// ``p * values_per_path .. (p + 1) * values_per_path`` where
/// ``values_per_path == num_steps + 1`` (the grid includes ``t = 0``).
#[pyclass(
    name = "SimulatedPaths",
    module = "finstack_quant.models.credit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PySimulatedPaths {
    pub(crate) inner: SimulatedPaths,
}

impl PySimulatedPaths {
    pub(crate) fn from_inner(inner: SimulatedPaths) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PySimulatedPaths {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize simulated paths from their canonical JSON form.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: SimulatedPaths = serde_json::from_str(json)
            .map_err(|err| serde_json_to_py(err, "invalid SimulatedPaths JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize to canonical JSON (``times``, ``asset_values``, ``num_paths``,
    /// ``num_steps``).
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "SimulatedPaths serialization failed"))
    }

    /// Time grid from 0 to the simulation horizon.
    #[getter]
    fn times(&self) -> Vec<f64> {
        self.inner.times.clone()
    }

    /// Asset values in row-major order.
    #[getter]
    fn asset_values(&self) -> Vec<f64> {
        self.inner.asset_values.clone()
    }

    /// Number of simulated paths.
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.num_paths
    }

    /// Number of time steps between grid points.
    #[getter]
    fn num_steps(&self) -> usize {
        self.inner.num_steps
    }

    /// Number of stored values per path (``num_steps + 1``, including ``t = 0``).
    #[getter]
    fn values_per_path(&self) -> usize {
        self.inner.values_per_path()
    }

    /// Export the paths as a long-format :class:`pandas.DataFrame`.
    ///
    /// Columns: ``path`` (int), ``time`` (float, years), ``asset_value``
    /// (float); one row per path per grid point, ordered by path then time.
    /// Pivot with ``df.pivot(index="time", columns="path", values="asset_value")``
    /// for a wide time-by-path frame.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let per_path = self.inner.values_per_path();
        let n = self.inner.num_paths * per_path;
        let mut path_col = Vec::with_capacity(n);
        let mut time_col = Vec::with_capacity(n);
        for p in 0..self.inner.num_paths {
            for t in 0..per_path {
                path_col.push(p as u64);
                time_col.push(self.inner.times.get(t).copied().unwrap_or(f64::NAN));
            }
        }
        let columns = PyDict::new(py);
        columns.set_item("path", path_col)?;
        columns.set_item("time", time_col)?;
        columns.set_item("asset_value", self.inner.asset_values.clone())?;
        dict_to_dataframe(py, &columns, None)
    }

    /// Return one asset value by path and time-grid index.
    ///
    /// # Arguments
    ///
    /// * `path_idx` - Zero-based path index
    /// * `time_idx` - Zero-based time-grid index (includes ``t = 0``)
    fn get(&self, path_idx: usize, time_idx: usize) -> Option<f64> {
        self.inner.get(path_idx, time_idx)
    }

    /// Return the contiguous asset-value row for one path.
    ///
    /// # Arguments
    ///
    /// * `path_idx` - Zero-based path index
    fn path(&self, path_idx: usize) -> Option<Vec<f64>> {
        self.inner.path(path_idx).map(<[f64]>::to_vec)
    }

    /// Materialize nested path storage as a list of path rows.
    fn to_nested(&self) -> Vec<Vec<f64>> {
        self.inner.to_nested()
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation; the path arrays are summarised
    /// by length.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("SimulatedPaths", &self.inner)
    }
}

/// Notional-dependent recovery curve for PIK-accreting instruments.
///
/// Maps the current (accreted) notional to a recovery rate. ``kind`` names
/// the active model: ``"constant"``, ``"inverse_linear"``,
/// ``"inverse_power"``, ``"floored_inverse"`` or ``"linear_decline"`` (the
/// canonical serde tags, which are also the keys in ``to_json``).
///
/// All recovery inputs are decimals in ``[0, 1]``; notionals are in the
/// instrument's currency and must be strictly positive.
#[pyclass(
    name = "DynamicRecoverySpec",
    module = "finstack_quant.models.credit",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyDynamicRecoverySpec {
    pub(crate) inner: DynamicRecoverySpec,
}

#[pymethods]
impl PyDynamicRecoverySpec {
    /// Constant recovery irrespective of notional.
    ///
    /// Parameters
    /// ----------
    /// recovery : float
    ///     Recovery rate as a decimal in ``[0, 1]``.
    ///
    /// Raises ``ValueError`` when ``recovery`` is non-finite or outside ``[0, 1]``.
    #[staticmethod]
    #[pyo3(text_signature = "(recovery)")]
    fn constant(recovery: f64) -> PyResult<Self> {
        Ok(Self {
            inner: DynamicRecoverySpec::constant(recovery).map_err(core_to_py)?,
        })
    }

    /// Recovery inversely proportional to notional: ``R(N) = R_0 * N_0 / N``.
    ///
    /// Parameters
    /// ----------
    /// base_recovery : float
    ///     Reference recovery ``R_0`` as a decimal in ``[0, 1]``.
    /// base_notional : float
    ///     Reference notional ``N_0`` (strictly positive).
    ///
    /// Raises ``ValueError`` on non-finite or out-of-range inputs.
    #[staticmethod]
    #[pyo3(text_signature = "(base_recovery, base_notional)")]
    fn inverse_linear(base_recovery: f64, base_notional: f64) -> PyResult<Self> {
        Ok(Self {
            inner: DynamicRecoverySpec::inverse_linear(base_recovery, base_notional)
                .map_err(core_to_py)?,
        })
    }

    /// Recovery decaying as a power of notional: ``R(N) = R_0 * (N_0 / N)^exponent``.
    ///
    /// Parameters
    /// ----------
    /// base_recovery : float
    ///     Reference recovery ``R_0`` as a decimal in ``[0, 1]``.
    /// base_notional : float
    ///     Reference notional ``N_0`` (strictly positive).
    /// exponent : float
    ///     Non-negative decay exponent (``1.0`` reproduces ``inverse_linear``).
    ///
    /// Raises ``ValueError`` on non-finite or out-of-range inputs.
    #[staticmethod]
    #[pyo3(text_signature = "(base_recovery, base_notional, exponent)")]
    fn inverse_power(base_recovery: f64, base_notional: f64, exponent: f64) -> PyResult<Self> {
        Ok(Self {
            inner: DynamicRecoverySpec::inverse_power(base_recovery, base_notional, exponent)
                .map_err(core_to_py)?,
        })
    }

    /// Inverse-linear recovery floored at a minimum: ``max(R_0 * N_0 / N, floor)``.
    ///
    /// Parameters
    /// ----------
    /// base_recovery : float
    ///     Reference recovery ``R_0`` as a decimal in ``[0, 1]``.
    /// base_notional : float
    ///     Reference notional ``N_0`` (strictly positive).
    /// floor : float
    ///     Minimum recovery as a decimal in ``[0, base_recovery]``.
    ///
    /// Raises ``ValueError`` on non-finite or out-of-range inputs.
    #[staticmethod]
    #[pyo3(text_signature = "(base_recovery, base_notional, floor)")]
    fn floored_inverse(base_recovery: f64, base_notional: f64, floor: f64) -> PyResult<Self> {
        Ok(Self {
            inner: DynamicRecoverySpec::floored_inverse(base_recovery, base_notional, floor)
                .map_err(core_to_py)?,
        })
    }

    /// Recovery declining linearly with accretion:
    /// ``R(N) = max(R_0 - slope * (N - N_0) / N_0, floor)``.
    ///
    /// Parameters
    /// ----------
    /// base_recovery : float
    ///     Reference recovery ``R_0`` as a decimal in ``[0, 1]``.
    /// base_notional : float
    ///     Reference notional ``N_0`` (strictly positive).
    /// slope : float
    ///     Non-negative recovery decline per unit of relative accretion.
    /// floor : float
    ///     Minimum recovery as a decimal in ``[0, base_recovery]``.
    ///
    /// Raises ``ValueError`` on non-finite or out-of-range inputs.
    #[staticmethod]
    #[pyo3(text_signature = "(base_recovery, base_notional, slope, floor)")]
    fn linear_decline(
        base_recovery: f64,
        base_notional: f64,
        slope: f64,
        floor: f64,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: DynamicRecoverySpec::linear_decline(base_recovery, base_notional, slope, floor)
                .map_err(core_to_py)?,
        })
    }

    /// Canonical name of the active recovery model.
    ///
    /// One of ``"constant"``, ``"inverse_linear"``, ``"inverse_power"``,
    /// ``"floored_inverse"``, ``"linear_decline"``.
    #[getter]
    fn kind(&self) -> PyResult<String> {
        model_tag(self.inner.model())
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

    /// Deserialize a recovery specification from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid DynamicRecoverySpec JSON"))?,
        })
    }

    /// Serialize this specification to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "DynamicRecoverySpec serialization failed"))
    }

    /// Recovery rate (decimal) at the given current notional.
    ///
    /// Parameters
    /// ----------
    /// notional : float
    ///     Current (accreted) notional in the instrument's currency.
    #[pyo3(text_signature = "($self, notional)")]
    fn recovery_at_notional(&self, notional: f64) -> f64 {
        self.inner.recovery_at_notional(notional)
    }

    /// Base (reference) recovery rate ``R_0`` as a decimal in ``[0, 1]``.
    #[getter]
    fn base_recovery(&self) -> f64 {
        self.inner.base_recovery()
    }

    /// Base (reference) notional ``N_0`` the recovery mapping is anchored to.
    #[getter]
    fn base_notional(&self) -> f64 {
        self.inner.base_notional()
    }

    /// Notional-to-recovery mapping, in canonical JSON form.
    ///
    /// ``"constant"`` / ``"inverse_linear"`` for the parameterless models, or a
    /// single-key mapping (``inverse_power``, ``floored_inverse``,
    /// ``linear_decline``) carrying that model's parameters.
    #[getter]
    fn model<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, self.inner.model())
    }

    /// Export the recovery specification as a single-row :class:`pandas.DataFrame`.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe(py, &self.inner)
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("DynamicRecoverySpec", &self.inner)
    }
}

/// Leverage-dependent hazard-rate feedback for PIK-accreting instruments.
///
/// Maps leverage (debt / assets) to an annualized hazard rate. ``kind``
/// names the active mapping: ``"power_law"``, ``"exponential"`` or
/// ``"tabular"`` (the canonical serde tags, also the keys in ``to_json``).
/// Hazard rates are annualized decimals; leverage is a ratio.
#[pyclass(
    name = "EndogenousHazardSpec",
    module = "finstack_quant.models.credit",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyEndogenousHazardSpec {
    pub(crate) inner: EndogenousHazardSpec,
}

#[pymethods]
impl PyEndogenousHazardSpec {
    /// Power-law feedback: ``lambda(L) = lambda_0 * (L / L_0)^exponent``.
    ///
    /// Parameters
    /// ----------
    /// base_hazard : float
    ///     Reference annualized hazard rate ``lambda_0`` (non-negative decimal).
    /// base_leverage : float
    ///     Reference leverage ``L_0`` (strictly positive ratio).
    /// exponent : float
    ///     Non-negative elasticity of hazard to leverage.
    ///
    /// Raises ``ValueError`` on non-finite or out-of-range inputs.
    #[staticmethod]
    #[pyo3(text_signature = "(base_hazard, base_leverage, exponent)")]
    fn power_law(base_hazard: f64, base_leverage: f64, exponent: f64) -> PyResult<Self> {
        Ok(Self {
            inner: EndogenousHazardSpec::power_law(base_hazard, base_leverage, exponent)
                .map_err(core_to_py)?,
        })
    }

    /// Exponential feedback: ``lambda(L) = lambda_0 * exp(sensitivity * (L - L_0))``.
    ///
    /// Parameters
    /// ----------
    /// base_hazard : float
    ///     Reference annualized hazard rate ``lambda_0`` (non-negative decimal).
    /// base_leverage : float
    ///     Reference leverage ``L_0`` (strictly positive ratio).
    /// sensitivity : float
    ///     Non-negative exponential sensitivity per unit of leverage.
    ///
    /// Raises ``ValueError`` on non-finite or out-of-range inputs.
    #[staticmethod]
    #[pyo3(text_signature = "(base_hazard, base_leverage, sensitivity)")]
    fn exponential(base_hazard: f64, base_leverage: f64, sensitivity: f64) -> PyResult<Self> {
        Ok(Self {
            inner: EndogenousHazardSpec::exponential(base_hazard, base_leverage, sensitivity)
                .map_err(core_to_py)?,
        })
    }

    /// Piecewise-linear tabular feedback through ``(leverage, hazard)`` points.
    ///
    /// Parameters
    /// ----------
    /// leverage_points : list[float]
    ///     Strictly increasing leverage ratios (at least two).
    /// hazard_points : list[float]
    ///     Non-negative annualized hazard rates, one per leverage point. The
    ///     base leverage and base hazard are taken from the first point.
    ///
    /// Raises ``ValueError`` when the lists differ in length, have fewer than
    /// two points, are not strictly increasing in leverage, or contain
    /// non-finite or negative values.
    #[staticmethod]
    #[pyo3(text_signature = "(leverage_points, hazard_points)")]
    fn tabular(leverage_points: Vec<f64>, hazard_points: Vec<f64>) -> PyResult<Self> {
        Ok(Self {
            inner: EndogenousHazardSpec::tabular(leverage_points, hazard_points)
                .map_err(core_to_py)?,
        })
    }

    /// Canonical name of the active leverage-to-hazard mapping.
    ///
    /// One of ``"power_law"``, ``"exponential"``, ``"tabular"``.
    #[getter]
    fn kind(&self) -> PyResult<String> {
        model_tag(self.inner.leverage_hazard_map())
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

    /// Deserialize a hazard specification from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid EndogenousHazardSpec JSON"))?,
        })
    }

    /// Serialize this specification to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "EndogenousHazardSpec serialization failed"))
    }

    /// Annualized hazard rate (decimal) at the given leverage ratio.
    ///
    /// Parameters
    /// ----------
    /// leverage : float
    ///     Leverage ratio (debt / assets), non-negative.
    #[pyo3(text_signature = "($self, leverage)")]
    fn hazard_at_leverage(&self, leverage: f64) -> f64 {
        self.inner.hazard_at_leverage(leverage)
    }

    /// Annualized hazard rate after PIK accretion, with leverage
    /// ``accreted_notional / asset_value``.
    ///
    /// Parameters
    /// ----------
    /// accreted_notional : float
    ///     Notional outstanding after PIK accrual, in the instrument's currency.
    /// asset_value : float
    ///     Firm asset value in the same currency (strictly positive).
    #[pyo3(text_signature = "($self, accreted_notional, asset_value)")]
    fn hazard_after_pik_accrual(&self, accreted_notional: f64, asset_value: f64) -> f64 {
        self.inner
            .hazard_after_pik_accrual(accreted_notional, asset_value)
    }

    /// Base (reference) hazard rate ``lambda_0``, annualized.
    #[getter]
    fn base_hazard_rate(&self) -> f64 {
        self.inner.base_hazard_rate()
    }

    /// Base (reference) leverage level ``L_0`` the hazard mapping is anchored to.
    #[getter]
    fn base_leverage(&self) -> f64 {
        self.inner.base_leverage()
    }

    /// Leverage-to-hazard mapping, in canonical JSON form.
    ///
    /// A single-key mapping (``power_law``, ``exponential``, ``tabular``)
    /// carrying that model's parameters.
    #[getter]
    fn leverage_hazard_map<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, self.inner.leverage_hazard_map())
    }

    /// Export the hazard specification as a single-row :class:`pandas.DataFrame`.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe(py, &self.inner)
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("EndogenousHazardSpec", &self.inner)
    }
}

/// Snapshot of an obligor's credit state at a PIK toggle decision date.
///
/// Feeds ``ToggleExerciseModel.should_pik_with_uniform``. Hazard rates are
/// annualized decimals, leverage is a ratio, and the monetary fields share
/// the instrument's currency.
#[pyclass(
    name = "CreditState",
    module = "finstack_quant.models.credit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyCreditState {
    pub(crate) inner: CreditState,
}

#[pymethods]
impl PyCreditState {
    /// Build a credit-state snapshot.
    ///
    /// Parameters
    /// ----------
    /// hazard_rate : float, default 0.0
    ///     Annualized instantaneous default intensity (decimal).
    /// distance_to_default : float | None, default None
    ///     Distance to default in standard deviations, when available.
    /// leverage : float, default 0.0
    ///     Leverage ratio (debt / assets).
    /// accreted_notional : float, default 0.0
    ///     PIK-accreted notional outstanding.
    /// coupon_due : float, default 0.0
    ///     Cash coupon due at the decision date.
    /// asset_value : float | None, default None
    ///     Fair value of the firm's assets, when available.
    ///
    /// Raises ``ValueError`` if any supplied value is non-finite.
    #[new]
    #[pyo3(signature = (hazard_rate=0.0, distance_to_default=None, leverage=0.0, accreted_notional=0.0, coupon_due=0.0, asset_value=None))]
    #[pyo3(
        text_signature = "(hazard_rate=0.0, distance_to_default=None, leverage=0.0, accreted_notional=0.0, coupon_due=0.0, asset_value=None)"
    )]
    fn new(
        hazard_rate: f64,
        distance_to_default: Option<f64>,
        leverage: f64,
        accreted_notional: f64,
        coupon_due: f64,
        asset_value: Option<f64>,
    ) -> PyResult<Self> {
        CreditState::new(
            hazard_rate,
            distance_to_default,
            leverage,
            accreted_notional,
            coupon_due,
            asset_value,
        )
        .map(|inner| Self { inner })
        .map_err(core_to_py)
    }

    /// Serialize this snapshot to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "CreditState serialization failed"))
    }

    /// Annualized instantaneous default intensity at this observation.
    #[getter]
    fn hazard_rate(&self) -> f64 {
        self.inner.hazard_rate
    }

    /// Distance-to-default in standard deviations, or ``None`` when unavailable.
    #[getter]
    fn distance_to_default(&self) -> Option<f64> {
        self.inner.distance_to_default
    }

    /// Leverage ratio (debt / assets).
    #[getter]
    fn leverage(&self) -> f64 {
        self.inner.leverage
    }

    /// Accreted (PIK-augmented) notional outstanding.
    #[getter]
    fn accreted_notional(&self) -> f64 {
        self.inner.accreted_notional
    }

    /// Cash coupon amount due at this decision date.
    #[getter]
    fn coupon_due(&self) -> f64 {
        self.inner.coupon_due
    }

    /// Fair value of the firm's assets, or ``None`` when unavailable.
    #[getter]
    fn asset_value(&self) -> Option<f64> {
        self.inner.asset_value
    }

    /// Export the observed credit state as a single-row :class:`pandas.DataFrame`.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe(py, &self.inner)
    }

    /// Deserialize from JSON produced by `to_json`.
    ///
    /// Completes the wire round-trip, which is also what makes this type
    /// picklable (see `__reduce__`).
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: CreditState = serde_json::from_str(json)
            .map_err(|err| serde_json_to_py(err, "invalid CreditState JSON"))?;
        Ok(Self { inner })
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CreditState", &self.inner)
    }
}

/// PIK toggle exercise rule: threshold, stochastic (logistic) or nested
/// Monte Carlo optimal exercise.
///
/// ``kind`` is ``"threshold"``, ``"stochastic"`` or ``"optimal_exercise"``.
/// Credit-state variables are ``"hazard_rate"``, ``"distance_to_default"``
/// or ``"leverage"``; threshold directions are ``"above"`` or ``"below"``.
#[pyclass(
    name = "ToggleExerciseModel",
    module = "finstack_quant.models.credit",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyToggleExerciseModel {
    pub(crate) inner: ToggleExerciseModel,
}

#[pymethods]
impl PyToggleExerciseModel {
    /// Deterministic rule: PIK when ``variable`` is ``direction`` the threshold.
    ///
    /// Parameters
    /// ----------
    /// variable : str
    ///     Credit-state variable observed: ``"hazard_rate"``,
    ///     ``"distance_to_default"`` or ``"leverage"``.
    /// threshold : float
    ///     Trigger level in the variable's own units (annualized decimal
    ///     hazard, standard deviations, or leverage ratio).
    /// direction : str
    ///     ``"above"`` to PIK when the variable exceeds the threshold,
    ///     ``"below"`` to PIK when it falls under it.
    ///
    /// Raises ``ValueError`` for an unknown variable or direction string, or
    /// a non-finite threshold.
    #[staticmethod]
    #[pyo3(text_signature = "(variable, threshold, direction)")]
    fn threshold(variable: &str, threshold: f64, direction: &str) -> PyResult<Self> {
        let variable = variable
            .parse::<CreditStateVariable>()
            .map_err(value_error)?;
        let direction = direction
            .parse::<ThresholdDirection>()
            .map_err(value_error)?;
        ToggleExerciseModel::threshold(variable, threshold, direction)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Stochastic rule: PIK with probability ``logistic(intercept + sensitivity * x)``.
    ///
    /// Parameters
    /// ----------
    /// variable : str
    ///     Credit-state variable ``x`` observed: ``"hazard_rate"``,
    ///     ``"distance_to_default"`` or ``"leverage"``.
    /// intercept : float
    ///     Logit intercept.
    /// sensitivity : float
    ///     Logit slope per unit of the variable.
    ///
    /// Raises ``ValueError`` for an unknown variable string or a non-finite
    /// intercept or sensitivity.
    #[staticmethod]
    #[pyo3(text_signature = "(variable, intercept, sensitivity)")]
    fn stochastic(variable: &str, intercept: f64, sensitivity: f64) -> PyResult<Self> {
        let variable = variable
            .parse::<CreditStateVariable>()
            .map_err(value_error)?;
        ToggleExerciseModel::stochastic(variable, intercept, sensitivity)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Whether the rule elects PIK for ``state`` given one uniform draw.
    ///
    /// Parameters
    /// ----------
    /// state : CreditState
    ///     Observed credit state at the decision date.
    /// u : float
    ///     Uniform draw in ``[0, 1)``; ignored by threshold rules and used as
    ///     the Bernoulli draw by stochastic rules. Optimal-exercise rules need
    ///     nested simulation and return ``False`` here.
    #[pyo3(text_signature = "($self, state, u)")]
    fn should_pik_with_uniform(&self, state: &PyCreditState, u: f64) -> bool {
        self.inner.should_pik_with_uniform(&state.inner, u)
    }

    /// Nested-Monte-Carlo optimal exercise rule.
    ///
    /// Parameters
    /// ----------
    /// nested_paths : int
    ///     Inner simulation paths per decision date.
    /// equity_discount_rate : float
    ///     Continuously compounded equity discount rate (decimal).
    /// asset_vol : float
    ///     Annualized asset volatility (decimal).
    /// risk_free_rate : float
    ///     Continuously compounded risk-free rate (decimal).
    /// horizon : float
    ///     Inner simulation horizon in years.
    ///
    /// Raises ``ValueError`` if ``nested_paths`` is zero, a rate is
    /// non-finite, ``asset_vol`` is negative or non-finite, or ``horizon`` is
    /// not finite and positive.
    #[staticmethod]
    #[pyo3(
        text_signature = "(nested_paths, equity_discount_rate, asset_vol, risk_free_rate, horizon)"
    )]
    fn optimal(
        nested_paths: usize,
        equity_discount_rate: f64,
        asset_vol: f64,
        risk_free_rate: f64,
        horizon: f64,
    ) -> PyResult<Self> {
        ToggleExerciseModel::optimal(
            nested_paths,
            equity_discount_rate,
            asset_vol,
            risk_free_rate,
            horizon,
        )
        .map(|inner| Self { inner })
        .map_err(core_to_py)
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

    /// Deserialize an exercise rule from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid ToggleExerciseModel JSON"))?,
        })
    }

    /// Serialize this rule to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "ToggleExerciseModel serialization failed"))
    }

    /// Which exercise rule this model carries.
    ///
    /// One of ``"threshold"``, ``"stochastic"`` or ``"optimal_exercise"`` —
    /// the canonical serde tag, so it also names the single key in the
    /// ``to_json`` payload.
    #[getter]
    fn kind(&self) -> &'static str {
        match &self.inner {
            ToggleExerciseModel::Threshold(_) => "threshold",
            ToggleExerciseModel::Stochastic(_) => "stochastic",
            ToggleExerciseModel::OptimalExercise(_) => "optimal_exercise",
        }
    }

    /// Parameters of the active rule, as a mapping in canonical JSON form.
    ///
    /// The keys depend on :attr:`kind`: ``state_variable`` / ``threshold`` /
    /// ``direction`` for a threshold rule, ``state_variable`` / ``intercept`` /
    /// ``sensitivity`` for a stochastic one, and the nested-Monte-Carlo
    /// settings for optimal exercise.
    #[getter]
    fn params<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match &self.inner {
            ToggleExerciseModel::Threshold(spec) => serde_to_py(py, spec),
            ToggleExerciseModel::Stochastic(spec) => serde_to_py(py, spec),
            ToggleExerciseModel::OptimalExercise(spec) => serde_to_py(py, spec),
        }
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ToggleExerciseModel", &self.inner)
    }
}

/// Canonical serde tag of an externally-tagged enum value (`"constant"`,
/// `{"inverse_power": {...}}` -> `"inverse_power"`).
fn model_tag<T: serde::Serialize>(value: &T) -> PyResult<String> {
    match serde_json::to_value(value)
        .map_err(|err| serde_json_to_py(err, "model tag serialization failed"))?
    {
        serde_json::Value::String(tag) => Ok(tag),
        serde_json::Value::Object(map) if map.len() == 1 => map
            .keys()
            .next()
            .cloned()
            .ok_or_else(|| value_error("model tag object is empty")),
        other => Err(value_error(format!(
            "expected an externally tagged enum value, got {other}"
        ))),
    }
}

/// Render an externally-tagged enum as its Python constructor call:
/// `"terminal"` -> `Name.terminal()`, `{"first_passage": {"k": v}}` ->
/// `Name.first_passage(k=v)`. Never fails; falls back to `Name(...)`.
fn variant_repr<T: serde::Serialize>(type_name: &str, value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(tag)) => format!("{type_name}.{tag}()"),
        Ok(serde_json::Value::Object(map)) if map.len() == 1 => {
            let Some((tag, fields)) = map.iter().next() else {
                return format!("{type_name}(...)");
            };
            let inner = crate::bindings::repr_support::repr_from_serde("", fields);
            format!("{type_name}.{tag}{inner}")
        }
        _ => format!("{type_name}(...)"),
    }
}

pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(py, "credit")?;
    module.setattr(
        "__doc__",
        "Product-independent credit models, scoring, migration, PD, LGD, recovery, and liability-management analytics.",
    )?;
    let qualified_name = crate::bindings::module_utils::set_submodule_package_by_package(
        parent,
        &module,
        "credit",
        "finstack_quant.models",
    )?;
    module.add_class::<PyAssetDynamics>()?;
    module.add_class::<PyBarrierType>()?;
    module.add_class::<PyMertonModel>()?;
    module.add_class::<PySimulatedPaths>()?;
    module.add_class::<PyDynamicRecoverySpec>()?;
    module.add_class::<PyEndogenousHazardSpec>()?;
    module.add_class::<PyCreditState>()?;
    module.add_class::<PyToggleExerciseModel>()?;
    module.add_class::<PyRatingFactorTable>()?;
    module.add_function(wrap_pyfunction!(moodys_warf_factor, &module)?)?;
    scoring::register(py, &module)?;
    pd::register(py, &module)?;
    lgd::register(py, &module)?;
    migration::register(py, &module)?;
    recovery_waterfall::register(py, &module)?;
    liability_management::register(py, &module)?;
    let all = PyList::new(
        py,
        [
            "AssetDynamics",
            "CreditState",
            "DynamicRecoverySpec",
            "EndogenousHazardSpec",
            "MertonBarrierType",
            "MertonModel",
            "RatingFactorTable",
            "SimulatedPaths",
            "ToggleExerciseModel",
            "lgd",
            "liability_management",
            "migration",
            "moodys_warf_factor",
            "pd",
            "recovery_waterfall",
            "scoring",
        ],
    )?;
    module.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule_at(py, parent, &module, &qualified_name)?;
    Ok(())
}
