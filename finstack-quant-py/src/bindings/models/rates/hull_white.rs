//! Python bindings for `finstack_quant_models::rates::hull_white`.
//!
//! Exposes the product-independent Hull-White one-factor parameter set and
//! the closed-form scalar kernels (convexity adjustment, bond-price
//! volatility, zero-coupon bond options, caplet normal-vol proxy, cap/floor
//! pricing). Quote preparation and fitting live in `finstack_quant.calibration`.

use crate::bindings::core::market_data::curves::PyDiscountCurve;
use crate::errors::{core_to_py, serde_json_to_py};
use finstack_quant_core::math::piecewise::PiecewiseConstantCurve;
use finstack_quant_models::rates::hull_white::{self, HullWhiteCalibrationParams, HullWhiteParams};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule, PyType};

/// Hull-White one-factor parameters with a piecewise-constant short-rate
/// volatility schedule.
///
/// Parameters
/// ----------
/// kappa : float
///     Mean-reversion speed in inverse years; must be finite and positive.
/// sigma : float
///     Constant short-rate volatility in absolute rate units per
///     square-root year (``0.01`` = 100 bp/sqrt(yr)); must be finite and
///     positive. Use ``HullWhiteParams.piecewise`` for a term structure of
///     volatility.
///
/// Raises
/// ------
/// ValueError
///     If ``kappa`` or ``sigma`` is non-finite or not strictly positive.
#[pyclass(
    name = "HullWhiteParams",
    module = "finstack_quant.models.rates.hull_white",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyHullWhiteParams {
    pub(crate) inner: HullWhiteParams,
}

impl PyHullWhiteParams {
    pub(crate) fn from_inner(inner: HullWhiteParams) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyHullWhiteParams {
    #[new]
    #[pyo3(text_signature = "(kappa, sigma)")]
    fn new(kappa: f64, sigma: f64) -> PyResult<Self> {
        HullWhiteParams::constant(kappa, sigma)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Build parameters with a piecewise-constant volatility schedule.
    ///
    /// ``times`` are strictly increasing knot times in years starting at
    /// ``0.0``; ``values[i]`` is the volatility (absolute rate units per
    /// square-root year) applying from ``times[i]`` until the next knot
    /// (left-continuous, flat extrapolation after the last knot).
    ///
    /// Raises ``ValueError`` if the schedule is empty, ragged, does not
    /// start at ``0.0``, is not strictly increasing, holds a negative value,
    /// or ``kappa`` is invalid.
    #[classmethod]
    #[pyo3(text_signature = "(cls, kappa, times, values)")]
    fn piecewise(
        _cls: &Bound<'_, PyType>,
        kappa: f64,
        times: Vec<f64>,
        values: Vec<f64>,
    ) -> PyResult<Self> {
        let schedule = PiecewiseConstantCurve::new(times, values).map_err(core_to_py)?;
        HullWhiteParams::new(kappa, schedule)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Mean-reversion speed in inverse years.
    #[getter]
    fn kappa(&self) -> f64 {
        self.inner.kappa
    }

    /// Volatility knot times in years.
    #[getter]
    fn times(&self) -> Vec<f64> {
        self.inner.volatility.times().to_vec()
    }

    /// Volatility values per knot (absolute rate units per square-root year).
    #[getter]
    fn values(&self) -> Vec<f64> {
        self.inner.volatility.values().to_vec()
    }

    /// Short-rate volatility applying at model time ``t`` (years).
    #[pyo3(text_signature = "(self, t)")]
    fn sigma(&self, t: f64) -> f64 {
        self.inner.volatility.value_at(t)
    }

    /// Variance of the centered short-rate state at time ``t`` (years).
    ///
    /// Raises ``ValueError`` if the integration interval is invalid.
    #[pyo3(text_signature = "(self, t)")]
    fn state_variance(&self, t: f64) -> PyResult<f64> {
        self.inner.state_variance(t).map_err(core_to_py)
    }

    /// Covariance of the centered short-rate state at two times (years).
    ///
    /// Raises ``ValueError`` if the earlier-time variance cannot be evaluated.
    #[pyo3(text_signature = "(self, left_time, right_time)")]
    fn state_covariance(&self, left_time: f64, right_time: f64) -> PyResult<f64> {
        self.inner
            .state_covariance(left_time, right_time)
            .map_err(core_to_py)
    }

    /// Volatility of the ``maturity`` zero-coupon bond price over
    /// ``[t, expiry]`` under the volatility schedule.
    ///
    /// Raises ``ValueError`` if ``t <= expiry <= maturity`` is violated or a
    /// time is negative/non-finite.
    #[pyo3(text_signature = "(self, t, expiry, maturity)")]
    fn bond_vol(&self, t: f64, expiry: f64, maturity: f64) -> PyResult<f64> {
        hull_white::hw_bond_vol_with_model(&self.inner, t, expiry, maturity).map_err(core_to_py)
    }

    /// Serialize to the canonical JSON wire format.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "HullWhiteParams serialization failed"))
    }

    /// Deserialize from JSON produced by ``to_json``.
    ///
    /// Raises ``ValueError`` when the payload is malformed or fails validation.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|err| serde_json_to_py(err, "invalid HullWhiteParams JSON"))
    }

    /// Support ``pickle``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "HullWhiteParams(kappa={:?}, times={:?}, values={:?})",
            self.inner.kappa,
            self.inner.volatility.times(),
            self.inner.volatility.values()
        )
    }
}

/// Low-rate, small-variance Hull-White approximation to the simple-deposit
/// futures-minus-forward rate bias over ``[t_settle, t_end]``.
///
/// Parameters
/// ----------
/// kappa : float
///     Finite non-negative mean-reversion speed in inverse years; zero uses
///     the Ho-Lee limit.
/// sigma : float
///     Finite non-negative short-rate volatility in absolute decimal rate
///     units per square-root year.
/// t_settle : float
///     Settlement/fixing time in years using the deposit day-count convention.
/// t_end : float
///     End of the accrual period on the same day-count basis; ``t_end -
///     t_settle`` is the deposit accrual fraction.
///
/// Returns
/// -------
/// float
///     Approximate futures-minus-forward bias in decimal rate units; ``0.0``
///     for non-positive fixing time or accrual. Non-finite inputs or negative
///     ``kappa``/``sigma`` return ``NaN`` before degenerate-period handling.
///     The Ho-Lee limit is ``0.5 * sigma**2 * t_settle * (2*t_end - t_settle)``.
#[pyfunction]
#[pyo3(text_signature = "(kappa, sigma, t_settle, t_end)")]
fn hw1f_convexity_adjustment(kappa: f64, sigma: f64, t_settle: f64, t_end: f64) -> f64 {
    hull_white::hw1f_convexity_adjustment(kappa, sigma, t_settle, t_end)
}

/// Lognormal volatility of the ``maturity`` zero-coupon bond price over
/// ``[t, expiry]`` for constant Hull-White parameters.
///
/// Parameters
/// ----------
/// kappa : float
///     Mean-reversion speed in inverse years.
/// sigma : float
///     Short-rate volatility in absolute rate units per square-root year.
/// t : float
///     Valuation time in years.
/// expiry : float
///     Option expiry in years (``>= t``).
/// maturity : float
///     Bond maturity in years (``>= expiry``).
///
/// Returns
/// -------
/// float
///     Total (not annualized) bond-price volatility ``B(expiry, maturity) * sigma * sqrt(var)``.
#[pyfunction]
#[pyo3(text_signature = "(kappa, sigma, t, expiry, maturity)")]
fn hw_bond_vol(kappa: f64, sigma: f64, t: f64, expiry: f64, maturity: f64) -> f64 {
    hull_white::hw_bond_vol(kappa, sigma, t, expiry, maturity)
}

/// Jamshidian closed-form price of a European option on a zero-coupon bond.
///
/// Parameters
/// ----------
/// p0_expiry : float
///     Discount factor to the option expiry.
/// p0_maturity : float
///     Discount factor to the bond maturity.
/// strike : float
///     Strike bond price (per unit face).
/// bond_vol : float
///     Total bond-price volatility from ``hw_bond_vol`` or
///     ``HullWhiteParams.bond_vol``.
/// is_call : bool
///     ``True`` for a call on the bond, ``False`` for a put.
///
/// Returns
/// -------
/// float
///     Present value per unit face, floored at zero.
#[pyfunction]
#[pyo3(text_signature = "(p0_expiry, p0_maturity, strike, bond_vol, is_call)")]
fn hw1f_zcb_option_price(
    p0_expiry: f64,
    p0_maturity: f64,
    strike: f64,
    bond_vol: f64,
    is_call: bool,
) -> f64 {
    hull_white::hw1f_zcb_option_price(p0_expiry, p0_maturity, strike, bond_vol, is_call)
}

/// Annualized normal (Bachelier) volatility of a forward rate implied by
/// constant Hull-White parameters.
///
/// Parameters
/// ----------
/// kappa : float
///     Mean-reversion speed in inverse years.
/// sigma : float
///     Short-rate volatility in absolute rate units per square-root year.
/// t_fix : float
///     Caplet fixing time in years (``> 0``).
/// accrual : float
///     Accrual period of the underlying rate in years (``> 0``).
///
/// Returns
/// -------
/// float
///     Normal vol in absolute rate units per square-root year; ``0.0`` when
///     any input is non-positive.
#[pyfunction]
#[pyo3(text_signature = "(kappa, sigma, t_fix, accrual)")]
fn hw1f_caplet_forward_rate_normal_vol(kappa: f64, sigma: f64, t_fix: f64, accrual: f64) -> f64 {
    hull_white::hw1f_caplet_forward_rate_normal_vol(kappa, sigma, t_fix, accrual)
}

/// Price a cap or floor as a sum of Hull-White caplets/floorlets.
///
/// Parameters
/// ----------
/// kappa : float
///     Mean-reversion speed in inverse years; finite and positive.
/// sigma : float
///     Short-rate volatility in absolute rate units per square-root year;
///     finite and positive.
/// periods : list[tuple[float, float, float]]
///     One ``(t_fix, t_pay, accrual)`` triple per caplet, all in years.
/// strike : float
///     Cap/floor strike as a decimal rate.
/// is_cap : bool
///     ``True`` prices a cap, ``False`` a floor.
/// discount_curve : DiscountCurve
///     Discounting curve queried at ``t_pay``.
/// forward_curve : DiscountCurve | None, default ``None``
///     Projection curve for the forward rates; ``None`` reuses
///     ``discount_curve`` (single-curve).
///
/// Returns
/// -------
/// float
///     Present value per unit notional (``NaN`` if a discount factor is
///     non-positive).
///
/// Raises
/// ------
/// ValueError
///     If ``kappa`` or ``sigma`` is non-finite or not strictly positive.
#[pyfunction]
#[pyo3(signature = (kappa, sigma, periods, strike, is_cap, discount_curve, forward_curve = None))]
#[pyo3(
    text_signature = "(kappa, sigma, periods, strike, is_cap, discount_curve, forward_curve=None)"
)]
#[allow(clippy::too_many_arguments)]
fn hw1f_cap_floor_price(
    kappa: f64,
    sigma: f64,
    periods: Vec<(f64, f64, f64)>,
    strike: f64,
    is_cap: bool,
    discount_curve: PyRef<'_, PyDiscountCurve>,
    forward_curve: Option<PyRef<'_, PyDiscountCurve>>,
) -> PyResult<f64> {
    let params = HullWhiteCalibrationParams::new(kappa, sigma).map_err(core_to_py)?;
    let discount = std::sync::Arc::clone(&discount_curve.inner);
    let forward = forward_curve.map_or_else(
        || std::sync::Arc::clone(&discount_curve.inner),
        |curve| std::sync::Arc::clone(&curve.inner),
    );
    let discount_df = |t: f64| discount.df(t);
    let forward_df = |t: f64| forward.df(t);
    Ok(hull_white::hw1f_cap_floor_price(
        params,
        &discount_df,
        &forward_df,
        &periods,
        strike,
        is_cap,
    ))
}

/// Build the `finstack_quant.models.rates.hull_white` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "hull_white")?;
    m.setattr(
        "__doc__",
        "Hull-White one-factor parameters and closed-form pricing kernels.",
    )?;
    m.add_class::<PyHullWhiteParams>()?;
    m.add_function(wrap_pyfunction!(hw1f_cap_floor_price, &m)?)?;
    m.add_function(wrap_pyfunction!(hw1f_caplet_forward_rate_normal_vol, &m)?)?;
    m.add_function(wrap_pyfunction!(hw1f_convexity_adjustment, &m)?)?;
    m.add_function(wrap_pyfunction!(hw1f_zcb_option_price, &m)?)?;
    m.add_function(wrap_pyfunction!(hw_bond_vol, &m)?)?;
    let all = PyList::new(
        py,
        [
            "HullWhiteParams",
            "hw1f_cap_floor_price",
            "hw1f_caplet_forward_rate_normal_vol",
            "hw1f_convexity_adjustment",
            "hw1f_zcb_option_price",
            "hw_bond_vol",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;
    Ok(())
}
