//! Volatility surface bindings.

use finstack_core::market_data::surfaces::{
    FxDeltaVolSurface, VolCube, VolGridOpts, VolInterpolationMode, VolSurface,
};
use finstack_core::market_data::term_structures::VolatilityIndexCurve;
use finstack_core::math::volatility::sabr::SabrParams;
use pyo3::types::PyDict;

use std::sync::Arc;

use pyo3::prelude::*;

use super::helpers::{
    parse_day_count, parse_extrapolation, parse_interp_style, parse_vol_interpolation_mode,
    parse_vol_surface_axis,
};
use crate::bindings::core::dates::utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;

// PyVolSurface
// ---------------------------------------------------------------------------

/// Two-dimensional implied volatility surface on an expiry x strike grid.
///
/// Wraps [`VolSurface`] from `finstack-core`.
#[pyclass(
    name = "VolSurface",
    module = "finstack.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyVolSurface {
    /// Shared Rust surface.
    pub(crate) inner: Arc<VolSurface>,
}

impl PyVolSurface {
    /// Build from an existing `Arc<VolSurface>`.
    pub(crate) fn from_inner(inner: Arc<VolSurface>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyVolSurface {
    /// Construct a vol surface from row-major grid data.
    #[new]
    #[pyo3(signature = (id, expiries, strikes, vols_row_major, secondary_axis="strike", interpolation_mode="vol"))]
    fn new(
        id: &str,
        expiries: Vec<f64>,
        strikes: Vec<f64>,
        vols_row_major: Vec<f64>,
        secondary_axis: &str,
        interpolation_mode: &str,
    ) -> PyResult<Self> {
        let axis = parse_vol_surface_axis(secondary_axis)?;
        let mode = parse_vol_interpolation_mode(interpolation_mode)?;
        let surface = VolSurface::from_grid_opts(
            id,
            &expiries,
            &strikes,
            &vols_row_major,
            VolGridOpts {
                secondary_axis: axis,
                interpolation_mode: mode,
            },
        )
        .map_err(core_to_py)?;

        Ok(Self {
            inner: Arc::new(surface),
        })
    }

    /// Interpolated surface value with explicit bounds checking.
    #[pyo3(text_signature = "(self, expiry, strike)")]
    fn value_checked(&self, expiry: f64, strike: f64) -> PyResult<f64> {
        self.inner.value_checked(expiry, strike).map_err(core_to_py)
    }

    /// Interpolated surface value with flat extrapolation at the grid edges.
    #[pyo3(text_signature = "(self, expiry, strike)")]
    fn value_clamped(&self, expiry: f64, strike: f64) -> f64 {
        self.inner.value_clamped(expiry, strike)
    }

    /// Surface identifier string.
    #[getter]
    fn id(&self) -> &str {
        self.inner.id().as_str()
    }

    /// Expiry axis in years.
    #[getter]
    fn expiries(&self) -> Vec<f64> {
        self.inner.expiries().to_vec()
    }

    /// Strike axis.
    #[getter]
    fn strikes(&self) -> Vec<f64> {
        self.inner.strikes().to_vec()
    }

    /// Secondary-axis semantic meaning.
    #[getter]
    fn secondary_axis(&self) -> String {
        self.inner.secondary_axis().to_string()
    }

    /// Interpolation contract used between grid points.
    #[getter]
    fn interpolation_mode(&self) -> String {
        match self.inner.interpolation_mode() {
            VolInterpolationMode::Vol => "vol".to_string(),
            VolInterpolationMode::TotalVariance => "total_variance".to_string(),
        }
    }

    /// Surface grid shape as `(n_expiries, n_strikes)`.
    #[getter]
    fn grid_shape(&self) -> (usize, usize) {
        self.inner.grid_shape()
    }

    fn __repr__(&self) -> String {
        format!("VolSurface(id={:?})", self.inner.id().as_str())
    }
}

// ---------------------------------------------------------------------------
// PyFxDeltaVolSurface
// ---------------------------------------------------------------------------

/// Delta-quoted FX volatility surface (ATM, 25-delta RR/BF, optional 10-delta wings).
///
/// Uses forward delta (premium-unadjusted). Converts to strikes via Garman-Kohlhagen
/// for strike-axis pricing. See Wystup (2006) and Clark (2011).
#[pyclass(
    name = "FxDeltaVolSurface",
    module = "finstack.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFxDeltaVolSurface {
    /// Shared Rust surface.
    pub(crate) inner: Arc<FxDeltaVolSurface>,
}

impl PyFxDeltaVolSurface {
    /// Build from an existing `Arc<FxDeltaVolSurface>`.
    pub(crate) fn from_inner(inner: Arc<FxDeltaVolSurface>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFxDeltaVolSurface {
    /// Construct an FX delta-quoted vol surface with 25-delta wings.
    ///
    /// Optional `rr_10d` and `bf_10d` add 10-delta wings for richer smile
    /// interpolation in the wings.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique surface identifier.
    /// expiries : list[float]
    ///     Strictly increasing positive expiry times in years.
    /// atm_vols : list[float]
    ///     ATM delta-neutral straddle vols per expiry (must be positive).
    /// rr_25d : list[float]
    ///     25-delta risk reversal per expiry (call vol - put vol).
    /// bf_25d : list[float]
    ///     25-delta butterfly per expiry (wing average - ATM).
    /// rr_10d : list[float], optional
    ///     10-delta risk reversal per expiry. If provided, `bf_10d` is required.
    /// bf_10d : list[float], optional
    ///     10-delta butterfly per expiry. If provided, `rr_10d` is required.
    #[new]
    #[pyo3(signature = (id, expiries, atm_vols, rr_25d, bf_25d, rr_10d=None, bf_10d=None))]
    fn new(
        id: &str,
        expiries: Vec<f64>,
        atm_vols: Vec<f64>,
        rr_25d: Vec<f64>,
        bf_25d: Vec<f64>,
        rr_10d: Option<Vec<f64>>,
        bf_10d: Option<Vec<f64>>,
    ) -> PyResult<Self> {
        let surface = match (rr_10d, bf_10d) {
            (Some(rr), Some(bf)) => {
                FxDeltaVolSurface::with_10d(id, expiries, atm_vols, rr_25d, bf_25d, rr, bf)
                    .map_err(core_to_py)?
            }
            (None, None) => FxDeltaVolSurface::new(id, expiries, atm_vols, rr_25d, bf_25d)
                .map_err(core_to_py)?,
            _ => {
                return Err(crate::errors::value_error(
                    "rr_10d and bf_10d must both be provided or both omitted",
                ));
            }
        };
        Ok(Self {
            inner: Arc::new(surface),
        })
    }

    /// Surface identifier string.
    #[getter]
    fn id(&self) -> &str {
        self.inner.id().as_str()
    }

    /// Expiry axis in years.
    #[getter]
    fn expiries(&self) -> Vec<f64> {
        self.inner.expiries().to_vec()
    }

    /// Number of expiry pillars.
    #[getter]
    fn num_expiries(&self) -> usize {
        self.inner.num_expiries()
    }

    /// Pillar vols at the given expiry index as ``(atm, put_25d_vol, call_25d_vol)``.
    ///
    /// Raises ``IndexError`` if ``expiry_idx`` is out of range.
    #[pyo3(text_signature = "(self, expiry_idx)")]
    fn pillar_vols(&self, expiry_idx: usize) -> PyResult<(f64, f64, f64)> {
        if expiry_idx >= self.inner.num_expiries() {
            return Err(pyo3::exceptions::PyIndexError::new_err(format!(
                "expiry_idx {} out of range (num_expiries={})",
                expiry_idx,
                self.inner.num_expiries()
            )));
        }
        Ok(self.inner.pillar_vols(expiry_idx))
    }

    /// Interpolated implied vol at the given ``(expiry, strike)`` for the
    /// supplied forward and rates.
    #[pyo3(text_signature = "(self, expiry, strike, forward, r_d, r_f)")]
    fn implied_vol(
        &self,
        expiry: f64,
        strike: f64,
        forward: f64,
        r_d: f64,
        r_f: f64,
    ) -> PyResult<f64> {
        self.inner
            .implied_vol(expiry, strike, forward, r_d, r_f)
            .map_err(core_to_py)
    }

    /// Materialize this delta-quoted surface as a strike-axis ``VolSurface``.
    ///
    /// The conversion uses Garman-Kohlhagen with the supplied ``spot``, ``r_d``
    /// (domestic continuously-compounded rate), and ``r_f`` (foreign rate).
    #[pyo3(text_signature = "(self, spot, r_d, r_f)")]
    fn to_vol_surface(&self, spot: f64, r_d: f64, r_f: f64) -> PyResult<PyVolSurface> {
        let surface = self
            .inner
            .to_vol_surface(spot, r_d, r_f)
            .map_err(core_to_py)?;
        Ok(PyVolSurface::from_inner(Arc::new(surface)))
    }

    /// Convert a forward delta to a strike using Garman-Kohlhagen
    /// (premium-unadjusted forward delta).
    #[staticmethod]
    #[pyo3(text_signature = "(delta, forward, vol, expiry, r_f)")]
    fn delta_to_strike(delta: f64, forward: f64, vol: f64, expiry: f64, r_f: f64) -> f64 {
        FxDeltaVolSurface::delta_to_strike(delta, forward, vol, expiry, r_f)
    }

    /// Convert a strike to forward delta (premium-unadjusted call delta).
    #[staticmethod]
    #[pyo3(text_signature = "(strike, forward, vol, expiry, r_f)")]
    fn strike_to_delta(strike: f64, forward: f64, vol: f64, expiry: f64, r_f: f64) -> f64 {
        FxDeltaVolSurface::strike_to_delta(strike, forward, vol, expiry, r_f)
    }

    fn __repr__(&self) -> String {
        format!(
            "FxDeltaVolSurface(id={:?}, num_expiries={})",
            self.inner.id().as_str(),
            self.inner.num_expiries()
        )
    }
}

// ---------------------------------------------------------------------------
// PyVolCube
// ---------------------------------------------------------------------------

/// SABR volatility cube on an expiry x tenor grid.
///
/// Wraps [`VolCube`] from `finstack-core`.
#[pyclass(
    name = "VolCube",
    module = "finstack.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyVolCube {
    /// Shared Rust cube.
    pub(crate) inner: Arc<VolCube>,
}

impl PyVolCube {
    /// Build from an existing `Arc<VolCube>`.
    pub(crate) fn from_inner(inner: Arc<VolCube>) -> Self {
        Self { inner }
    }
}

/// Parse a Python dict to [`SabrParams`].
///
/// Required keys: `"alpha"`, `"beta"`, `"rho"`, `"nu"`.
/// Optional key: `"shift"`.
fn parse_sabr_dict(dict: &Bound<'_, PyDict>, idx: usize) -> PyResult<SabrParams> {
    let get = |key: &str| -> PyResult<f64> {
        dict.get_item(key)?
            .ok_or_else(|| {
                crate::errors::value_error(format!(
                    "params_row_major[{idx}]: missing required key {key:?}"
                ))
            })?
            .extract::<f64>()
    };

    let alpha = get("alpha")?;
    let beta = get("beta")?;
    let rho = get("rho")?;
    let nu = get("nu")?;

    let mut params = SabrParams::new(alpha, beta, rho, nu).map_err(core_to_py)?;

    if let Some(shift_obj) = dict.get_item("shift")? {
        let shift: f64 = shift_obj.extract()?;
        params = params.with_shift(shift);
    }

    Ok(params)
}

#[pymethods]
impl PyVolCube {
    /// Construct a vol cube from row-major grid data.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique cube identifier.
    /// expiries : list[float]
    ///     Option expiry axis in years.
    /// tenors : list[float]
    ///     Underlying swap tenor axis in years.
    /// params_row_major : list[dict]
    ///     SABR parameter dicts with keys ``"alpha"``, ``"beta"``, ``"rho"``,
    ///     ``"nu"``, and optionally ``"shift"``.
    /// forwards_row_major : list[float]
    ///     Forward rates in row-major order.
    /// interpolation_mode : str, optional
    ///     Interpolation contract: ``"vol"`` or ``"total_variance"``
    ///     (default ``"vol"``).
    #[new]
    #[pyo3(signature = (id, expiries, tenors, params_row_major, forwards_row_major, interpolation_mode="vol"))]
    fn new(
        id: &str,
        expiries: Vec<f64>,
        tenors: Vec<f64>,
        params_row_major: Vec<Bound<'_, PyDict>>,
        forwards_row_major: Vec<f64>,
        interpolation_mode: &str,
    ) -> PyResult<Self> {
        let mode = parse_vol_interpolation_mode(interpolation_mode)?;

        let sabr_params: Vec<SabrParams> = params_row_major
            .iter()
            .enumerate()
            .map(|(i, d)| parse_sabr_dict(d, i))
            .collect::<PyResult<Vec<_>>>()?;

        let cube = VolCube::from_grid(id, &expiries, &tenors, &sabr_params, &forwards_row_major)
            .map_err(core_to_py)?
            .with_interpolation_mode(mode);

        Ok(Self {
            inner: Arc::new(cube),
        })
    }

    /// Implied volatility with bounds checking.
    #[pyo3(text_signature = "(self, expiry, tenor, strike)")]
    fn vol(&self, expiry: f64, tenor: f64, strike: f64) -> PyResult<f64> {
        self.inner.vol(expiry, tenor, strike).map_err(core_to_py)
    }

    /// Implied volatility with clamped extrapolation.
    #[pyo3(text_signature = "(self, expiry, tenor, strike)")]
    fn vol_clamped(&self, expiry: f64, tenor: f64, strike: f64) -> f64 {
        self.inner.vol_clamped(expiry, tenor, strike)
    }

    /// Materialize a tenor slice as a [`VolSurface`].
    #[pyo3(text_signature = "(self, tenor, strikes)")]
    fn materialize_tenor_slice(&self, tenor: f64, strikes: Vec<f64>) -> PyResult<PyVolSurface> {
        let surface = self
            .inner
            .materialize_tenor_slice(tenor, &strikes)
            .map_err(core_to_py)?;
        Ok(PyVolSurface::from_inner(Arc::new(surface)))
    }

    /// Materialize an expiry slice as a [`VolSurface`].
    #[pyo3(text_signature = "(self, expiry, strikes)")]
    fn materialize_expiry_slice(&self, expiry: f64, strikes: Vec<f64>) -> PyResult<PyVolSurface> {
        let surface = self
            .inner
            .materialize_expiry_slice(expiry, &strikes)
            .map_err(core_to_py)?;
        Ok(PyVolSurface::from_inner(Arc::new(surface)))
    }

    /// Cube identifier string.
    #[getter]
    fn id(&self) -> &str {
        self.inner.id().as_str()
    }

    /// Option expiry axis in years.
    #[getter]
    fn expiries(&self) -> Vec<f64> {
        self.inner.expiries().to_vec()
    }

    /// Underlying swap tenor axis in years.
    #[getter]
    fn tenors(&self) -> Vec<f64> {
        self.inner.tenors().to_vec()
    }

    /// Grid shape as `(n_expiries, n_tenors)`.
    #[getter]
    fn grid_shape(&self) -> (usize, usize) {
        self.inner.grid_shape()
    }

    fn __repr__(&self) -> String {
        format!("VolCube(id={:?})", self.inner.id().as_str())
    }
}

// ---------------------------------------------------------------------------
// PyVolatilityIndexCurve
// ---------------------------------------------------------------------------

/// Volatility index forward curve (e.g. VIX term structure).
///
/// Wraps [`VolatilityIndexCurve`] from `finstack-core`.
#[pyclass(
    name = "VolatilityIndexCurve",
    module = "finstack.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyVolatilityIndexCurve {
    /// Shared Rust curve.
    pub(crate) inner: Arc<VolatilityIndexCurve>,
}

impl PyVolatilityIndexCurve {
    /// Build from an existing `Arc<VolatilityIndexCurve>`.
    pub(crate) fn from_inner(inner: Arc<VolatilityIndexCurve>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyVolatilityIndexCurve {
    /// Construct a volatility index curve from knot points.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier (e.g. ``"VIX"``).
    /// base_date : datetime.date
    ///     Valuation date.
    /// knots : list[tuple[float, float]]
    ///     ``(time_years, forward_level)`` pairs.
    /// extrapolation : str, optional
    ///     Extrapolation policy (default ``"flat_zero"``).
    /// interp : str, optional
    ///     Interpolation style (default ``"linear"``).
    /// day_count : str, optional
    ///     Day-count convention (default ``"act_365f"``).
    #[new]
    #[pyo3(signature = (id, base_date, knots, extrapolation="flat_zero", interp="linear", day_count="act_365f"))]
    fn new(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        knots: Vec<(f64, f64)>,
        extrapolation: &str,
        interp: &str,
        day_count: &str,
    ) -> PyResult<Self> {
        let base = py_to_date(base_date)?;
        let extrap = parse_extrapolation(extrapolation)?;
        let style = parse_interp_style(interp)?;
        let dc = parse_day_count(day_count)?;

        let curve = VolatilityIndexCurve::builder(id)
            .base_date(base)
            .day_count(dc)
            .knots(knots)
            .interp(style)
            .extrapolation(extrap)
            .build()
            .map_err(core_to_py)?;

        Ok(Self {
            inner: Arc::new(curve),
        })
    }

    /// Forward volatility index level at year fraction `t`.
    #[pyo3(text_signature = "(self, t)")]
    fn forward_level(&self, t: f64) -> f64 {
        self.inner.forward_level(t)
    }

    /// Curve identifier string.
    #[getter]
    fn id(&self) -> &str {
        self.inner.id().as_str()
    }

    /// Valuation base date.
    #[getter]
    fn base_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.base_date())
    }

    fn __repr__(&self) -> String {
        format!("VolatilityIndexCurve(id={:?})", self.inner.id().as_str())
    }
}

// ---------------------------------------------------------------------------
