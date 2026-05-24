//! Python bindings for `finstack_core::market_data::term_structures` curve types.

use std::sync::Arc;

use finstack_core::dates::DayCount;
use finstack_core::market_data::surfaces::{
    VolCube, VolGridOpts, VolInterpolationMode, VolSurface, VolSurfaceAxis,
};
use finstack_core::market_data::term_structures::{
    BaseCorrelationCurve, CreditIndexData, DiscountCurve, ForwardCurve, HazardCurve,
    InflationCurve, PriceCurve, VolatilityIndexCurve,
};
use finstack_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use finstack_core::math::volatility::sabr::SabrParams;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule};

use crate::bindings::core::dates::utils::{date_to_py, py_to_date};

use crate::errors::core_to_py;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse a DayCount from a Python string like `"act_365f"`, `"act_360"`, etc.
fn parse_day_count(s: &str) -> PyResult<DayCount> {
    s.parse::<DayCount>()
        .map_err(|e| crate::errors::value_error(format!("Invalid day_count {s:?}: {e}")))
}

/// Parse an [`InterpStyle`] from a Python string.
fn parse_interp_style(s: &str) -> PyResult<InterpStyle> {
    s.parse::<InterpStyle>()
        .map_err(|e| crate::errors::value_error(format!("Invalid interp style {s:?}: {e}")))
}

/// Parse an [`ExtrapolationPolicy`] from a Python string.
fn parse_extrapolation(s: &str) -> PyResult<ExtrapolationPolicy> {
    s.parse::<ExtrapolationPolicy>()
        .map_err(|e| crate::errors::value_error(format!("Invalid extrapolation {s:?}: {e}")))
}

/// Parse a [`VolSurfaceAxis`] from a Python string.
fn parse_vol_surface_axis(s: &str) -> PyResult<VolSurfaceAxis> {
    match s {
        "strike" => Ok(VolSurfaceAxis::Strike),
        "tenor" => Ok(VolSurfaceAxis::Tenor),
        _ => Err(crate::errors::value_error(format!(
            "Invalid vol surface axis {s:?}: expected 'strike' or 'tenor'",
        ))),
    }
}

/// Parse a [`VolInterpolationMode`] from a Python string.
fn parse_vol_interpolation_mode(s: &str) -> PyResult<VolInterpolationMode> {
    match s {
        "vol" => Ok(VolInterpolationMode::Vol),
        "total_variance" => Ok(VolInterpolationMode::TotalVariance),
        _ => Err(crate::errors::value_error(format!(
            "Invalid vol interpolation mode {s:?}: expected 'vol' or 'total_variance'",
        ))),
    }
}

// ---------------------------------------------------------------------------
// PyDiscountCurve
// ---------------------------------------------------------------------------

/// Discount factor curve for present-value calculations.
///
/// Wraps [`DiscountCurve`] from `finstack-core`. Constructed via the builder
/// pattern using `(time, df)` knot pairs.
#[pyclass(
    name = "DiscountCurve",
    module = "finstack.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyDiscountCurve {
    /// Shared Rust curve.
    pub(crate) inner: Arc<DiscountCurve>,
}

impl PyDiscountCurve {
    /// Build from an existing `Arc<DiscountCurve>`.
    pub(crate) fn from_inner(inner: Arc<DiscountCurve>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyDiscountCurve {
    /// Construct a discount curve from knot points.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier (e.g. ``"USD-OIS"``).
    /// base_date : datetime.date
    ///     Valuation date.
    /// knots : list[tuple[float, float]]
    ///     ``(time_years, discount_factor)`` pairs.
    /// interp : str, optional
    ///     Interpolation style (default ``"monotone_convex"``).
    /// extrapolation : str, optional
    ///     Extrapolation policy (default ``"flat_forward"``).
    /// day_count : str, optional
    ///     Day-count convention. When omitted, Rust infers a market default from the curve ID.
    #[new]
    #[pyo3(signature = (id, base_date, knots, interp="monotone_convex", extrapolation="flat_forward", day_count=None))]
    fn new(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        knots: Vec<(f64, f64)>,
        interp: &str,
        extrapolation: &str,
        day_count: Option<&str>,
    ) -> PyResult<Self> {
        let base = py_to_date(base_date)?;
        let style = parse_interp_style(interp)?;
        let extrap = parse_extrapolation(extrapolation)?;

        let mut builder = DiscountCurve::builder(id)
            .base_date(base)
            .knots(knots)
            .interp(style)
            .extrapolation(extrap);
        if let Some(day_count) = day_count {
            builder = builder.day_count(parse_day_count(day_count)?);
        }

        let curve = builder.build().map_err(core_to_py)?;

        Ok(Self {
            inner: Arc::new(curve),
        })
    }

    /// Discount factor at year fraction `t`.
    #[pyo3(text_signature = "(self, t)")]
    fn df(&self, t: f64) -> f64 {
        self.inner.df(t)
    }

    /// Continuously-compounded zero rate at year fraction `t`.
    #[pyo3(text_signature = "(self, t)")]
    fn zero(&self, t: f64) -> f64 {
        self.inner.zero(t)
    }

    /// Continuously-compounded forward rate between `t1` and `t2`.
    #[pyo3(text_signature = "(self, t1, t2)")]
    fn forward_rate(&self, t1: f64, t2: f64) -> PyResult<f64> {
        self.inner.forward(t1, t2).map_err(core_to_py)
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
        format!("DiscountCurve(id={:?})", self.inner.id().as_str())
    }
}

// ---------------------------------------------------------------------------
// PyForwardCurve
// ---------------------------------------------------------------------------

/// Forward rate curve for a floating-rate index with a fixed tenor.
///
/// Wraps [`ForwardCurve`] from `finstack-core`.
#[pyclass(
    name = "ForwardCurve",
    module = "finstack.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyForwardCurve {
    /// Shared Rust curve.
    pub(crate) inner: Arc<ForwardCurve>,
}

impl PyForwardCurve {
    /// Build from an existing `Arc<ForwardCurve>`.
    pub(crate) fn from_inner(inner: Arc<ForwardCurve>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyForwardCurve {
    /// Construct a forward rate curve from knot points.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier (e.g. ``"USD-SOFR-3M"``).
    /// tenor : float
    ///     Index tenor in years (e.g. ``0.25`` for 3 months).
    /// knots : list[tuple[float, float]]
    ///     ``(time_years, forward_rate)`` pairs.
    /// base_date : datetime.date
    ///     Valuation date.
    /// day_count : str, optional
    ///     Day-count convention (default ``"act_360"``).
    /// interp : str, optional
    ///     Interpolation style (default ``"linear"``).
    /// extrapolation : str, optional
    ///     Extrapolation policy (default ``"flat_forward"``).
    #[new]
    #[pyo3(signature = (id, tenor, knots, base_date, day_count="act_360", interp="linear", extrapolation="flat_forward"))]
    fn new(
        id: &str,
        tenor: f64,
        knots: Vec<(f64, f64)>,
        base_date: &Bound<'_, PyAny>,
        day_count: &str,
        interp: &str,
        extrapolation: &str,
    ) -> PyResult<Self> {
        let base = py_to_date(base_date)?;
        let dc = parse_day_count(day_count)?;
        let style = parse_interp_style(interp)?;
        let extrap = parse_extrapolation(extrapolation)?;

        let curve = ForwardCurve::builder(id, tenor)
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

    /// Forward rate at year fraction `t`.
    #[pyo3(text_signature = "(self, t)")]
    fn rate(&self, t: f64) -> f64 {
        self.inner.rate(t)
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
        format!("ForwardCurve(id={:?})", self.inner.id().as_str())
    }
}

// ---------------------------------------------------------------------------
// PyHazardCurve
// ---------------------------------------------------------------------------

/// Credit hazard-rate curve for default probability modeling.
///
/// Wraps [`HazardCurve`] from `finstack-core`.
#[pyclass(
    name = "HazardCurve",
    module = "finstack.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyHazardCurve {
    /// Shared Rust curve.
    pub(crate) inner: Arc<HazardCurve>,
}

impl PyHazardCurve {
    /// Build from an existing `Arc<HazardCurve>`.
    pub(crate) fn from_inner(inner: Arc<HazardCurve>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyHazardCurve {
    /// Construct a hazard curve from knot points.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier (e.g. ``"ACME-HZD"``).
    /// base_date : datetime.date
    ///     Valuation date.
    /// knots : list[tuple[float, float]]
    ///     ``(time_years, hazard_rate)`` pairs.
    /// recovery_rate : float, optional
    ///     Recovery rate. Defaults to the credit assumptions registry value.
    /// day_count : str, optional
    ///     Day-count convention (default ``"act_365f"``).
    /// par_spreads : list[tuple[float, float]], optional
    ///     Market par-spread quotes in basis points used for rebootstrap risks.
    #[new]
    #[pyo3(signature = (id, base_date, knots, recovery_rate=None, day_count="act_365f", par_spreads=None))]
    fn new(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        knots: Vec<(f64, f64)>,
        recovery_rate: Option<f64>,
        day_count: &str,
        par_spreads: Option<Vec<(f64, f64)>>,
    ) -> PyResult<Self> {
        let base = py_to_date(base_date)?;
        let dc = parse_day_count(day_count)?;
        let recovery_rate = match recovery_rate {
            Some(r) => r,
            None => finstack_core::credit::registry::default_market_recovery_rate()
                .map_err(core_to_py)?,
        };

        let mut builder = HazardCurve::builder(id)
            .base_date(base)
            .recovery_rate(recovery_rate)
            .day_count(dc)
            .knots(knots);
        if let Some(points) = par_spreads {
            builder = builder.par_spreads(points);
        }
        let curve = builder.build().map_err(core_to_py)?;

        Ok(Self {
            inner: Arc::new(curve),
        })
    }

    /// Survival probability at year fraction `t`.
    #[pyo3(text_signature = "(self, t)")]
    fn survival(&self, t: f64) -> f64 {
        self.inner.sp(t)
    }

    /// Instantaneous hazard rate at year fraction `t`.
    #[pyo3(text_signature = "(self, t)")]
    fn hazard_rate(&self, t: f64) -> f64 {
        self.inner.hazard_rate(t)
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
        format!("HazardCurve(id={:?})", self.inner.id().as_str())
    }
}

// ---------------------------------------------------------------------------
// PyInflationCurve
// ---------------------------------------------------------------------------

/// CPI inflation curve for inflation-linked pricing and breakeven analysis.
///
/// Wraps [`InflationCurve`] from `finstack-core`.
#[pyclass(
    name = "InflationCurve",
    module = "finstack.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyInflationCurve {
    /// Shared Rust curve.
    pub(crate) inner: Arc<InflationCurve>,
}

impl PyInflationCurve {
    /// Build from an existing `Arc<InflationCurve>`.
    pub(crate) fn from_inner(inner: Arc<InflationCurve>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyInflationCurve {
    /// Construct an inflation curve from CPI knot points.
    #[new]
    #[pyo3(signature = (id, base_date, base_cpi, knots, day_count="act_365f", indexation_lag_months=3, interp="log_linear"))]
    fn new(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        base_cpi: f64,
        knots: Vec<(f64, f64)>,
        day_count: &str,
        indexation_lag_months: u32,
        interp: &str,
    ) -> PyResult<Self> {
        let base = py_to_date(base_date)?;
        let dc = parse_day_count(day_count)?;
        let style = parse_interp_style(interp)?;

        let curve = InflationCurve::builder(id)
            .base_date(base)
            .base_cpi(base_cpi)
            .day_count(dc)
            .indexation_lag_months(indexation_lag_months)
            .knots(knots)
            .interp(style)
            .build()
            .map_err(core_to_py)?;

        Ok(Self {
            inner: Arc::new(curve),
        })
    }

    /// CPI level at year fraction `t`, without indexation lag.
    #[pyo3(text_signature = "(self, t)")]
    fn cpi(&self, t: f64) -> f64 {
        self.inner.cpi(t)
    }

    /// CPI level at year fraction `t`, with configured indexation lag applied.
    #[pyo3(text_signature = "(self, t)")]
    fn cpi_with_lag(&self, t: f64) -> f64 {
        self.inner.cpi_with_lag(t)
    }

    /// Annualized inflation rate between `t1` and `t2` using CAGR.
    #[pyo3(text_signature = "(self, t1, t2)")]
    fn inflation_rate(&self, t1: f64, t2: f64) -> f64 {
        self.inner.inflation_rate(t1, t2)
    }

    /// Simple non-compounded inflation rate between `t1` and `t2`.
    #[pyo3(text_signature = "(self, t1, t2)")]
    fn inflation_rate_simple(&self, t1: f64, t2: f64) -> f64 {
        self.inner.inflation_rate_simple(t1, t2)
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

    /// Day-count convention used by this curve.
    #[getter]
    fn day_count(&self) -> String {
        self.inner.day_count().to_string()
    }

    /// Indexation lag in months.
    #[getter]
    fn indexation_lag_months(&self) -> u32 {
        self.inner.indexation_lag_months()
    }

    /// Base CPI level at `t = 0`.
    #[getter]
    fn base_cpi(&self) -> f64 {
        self.inner.base_cpi()
    }

    fn __repr__(&self) -> String {
        format!("InflationCurve(id={:?})", self.inner.id().as_str())
    }
}

// ---------------------------------------------------------------------------
// PyPriceCurve
// ---------------------------------------------------------------------------

/// Forward price curve for commodities and other price-based assets.
///
/// Wraps [`PriceCurve`] from `finstack-core`.
#[pyclass(
    name = "PriceCurve",
    module = "finstack.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPriceCurve {
    /// Shared Rust curve.
    pub(crate) inner: Arc<PriceCurve>,
}

impl PyPriceCurve {
    /// Build from an existing `Arc<PriceCurve>`.
    pub(crate) fn from_inner(inner: Arc<PriceCurve>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPriceCurve {
    /// Construct a price curve from knot points.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier (e.g. ``"WTI-FORWARD"``).
    /// base_date : datetime.date
    ///     Valuation date.
    /// knots : list[tuple[float, float]]
    ///     ``(time_years, forward_price)`` pairs.
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

        let curve = PriceCurve::builder(id)
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

    /// Forward price at year fraction `t`.
    #[pyo3(text_signature = "(self, t)")]
    fn price(&self, t: f64) -> f64 {
        self.inner.price(t)
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
        format!("PriceCurve(id={:?})", self.inner.id().as_str())
    }
}

// ---------------------------------------------------------------------------
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
// PyBaseCorrelationCurve
// ---------------------------------------------------------------------------

/// Base-correlation curve for synthetic credit index tranche pricing.
#[pyclass(
    name = "BaseCorrelationCurve",
    module = "finstack.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyBaseCorrelationCurve {
    /// Shared Rust curve.
    pub(crate) inner: Arc<BaseCorrelationCurve>,
}

#[pymethods]
impl PyBaseCorrelationCurve {
    /// Construct a base-correlation curve from `(detachment_pct, correlation)` knots.
    #[new]
    #[pyo3(signature = (id, knots))]
    fn new(id: &str, knots: Vec<(f64, f64)>) -> PyResult<Self> {
        let curve = BaseCorrelationCurve::builder(id)
            .knots(knots)
            .build()
            .map_err(core_to_py)?;
        Ok(Self {
            inner: Arc::new(curve),
        })
    }

    /// Interpolated base correlation at a detachment point.
    #[pyo3(text_signature = "(self, detachment_pct)")]
    fn correlation(&self, detachment_pct: f64) -> f64 {
        self.inner.correlation(detachment_pct)
    }

    /// Curve identifier string.
    #[getter]
    fn id(&self) -> &str {
        self.inner.id.as_str()
    }

    fn __repr__(&self) -> String {
        format!("BaseCorrelationCurve(id={:?})", self.inner.id.as_str())
    }
}

// ---------------------------------------------------------------------------
// PyCreditIndexData
// ---------------------------------------------------------------------------

/// Credit index data bundle for synthetic tranche pricing.
#[pyclass(
    name = "CreditIndexData",
    module = "finstack.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCreditIndexData {
    /// Rust credit index bundle.
    pub(crate) inner: CreditIndexData,
}

#[pymethods]
impl PyCreditIndexData {
    /// Construct homogeneous credit index data from index hazard and base correlation curves.
    #[new]
    #[pyo3(signature = (num_constituents, recovery_rate, index_credit_curve, base_correlation_curve))]
    fn new(
        num_constituents: u16,
        recovery_rate: f64,
        index_credit_curve: &PyHazardCurve,
        base_correlation_curve: &PyBaseCorrelationCurve,
    ) -> PyResult<Self> {
        let data = CreditIndexData::builder()
            .num_constituents(num_constituents)
            .recovery_rate(recovery_rate)
            .index_credit_curve(Arc::clone(&index_credit_curve.inner))
            .base_correlation_curve(Arc::clone(&base_correlation_curve.inner))
            .build()
            .map_err(core_to_py)?;
        Ok(Self { inner: data })
    }

    /// Number of constituents in the credit index.
    #[getter]
    fn num_constituents(&self) -> u16 {
        self.inner.num_constituents
    }

    /// Index recovery rate.
    #[getter]
    fn recovery_rate(&self) -> f64 {
        self.inner.recovery_rate
    }

    fn __repr__(&self) -> String {
        format!(
            "CreditIndexData(num_constituents={}, recovery_rate={})",
            self.inner.num_constituents, self.inner.recovery_rate
        )
    }
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

pub(super) const EXPORTS: &[&str] = &[
    "BaseCorrelationCurve",
    "CreditIndexData",
    "DiscountCurve",
    "ForwardCurve",
    "HazardCurve",
    "InflationCurve",
    "PriceCurve",
    "VolSurface",
    "VolCube",
    "VolatilityIndexCurve",
];

/// Register the `finstack.core.market_data.curves` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "curves")?;
    m.setattr(
        "__doc__",
        "Market-data bindings: discount, forward, hazard, inflation, price, vol surface, vol cube, and vol-index.",
    )?;

    m.add_class::<PyDiscountCurve>()?;
    m.add_class::<PyForwardCurve>()?;
    m.add_class::<PyHazardCurve>()?;
    m.add_class::<PyBaseCorrelationCurve>()?;
    m.add_class::<PyCreditIndexData>()?;
    m.add_class::<PyInflationCurve>()?;
    m.add_class::<PyPriceCurve>()?;
    m.add_class::<PyVolSurface>()?;
    m.add_class::<PyVolCube>()?;
    m.add_class::<PyVolatilityIndexCurve>()?;

    let all = PyList::new(py, EXPORTS)?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "curves",
        "finstack.core.market_data",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
