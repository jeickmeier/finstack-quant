//! Volatility surface Python wrapper and grid extraction helpers.

use finstack_quant_core::market_data::surfaces::{
    VolGridOpts, VolInterpolationMode, VolSurface, VolSurfaceAxis,
};
use finstack_quant_core::wire::{serde_label, serde_parse};
use pyo3::types::PyDict;

use std::sync::Arc;

use finstack_quant_core::market_data::surfaces::VolQuoteType;
use pyo3::prelude::*;

use super::helpers::columns_to_arrow_table;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::{tabular_methods, wire_methods};
use crate::errors::core_to_py;

pub(super) enum VolInput {
    Flat(Vec<f64>),
    Rows(Vec<Vec<f64>>),
}

/// Preserve nested rows until the canonical Rust constructor validates their shape.
pub(super) fn extract_vol_grid(obj: &Bound<'_, PyAny>) -> PyResult<VolInput> {
    if crate::bindings::dataframe_utils::is_dataframe_like(obj)?
        && let Some((flat, _nrows, ncols)) =
            crate::bindings::dataframe_utils::extract_2d_f64(obj, "vols")?
    {
        if ncols == 0 {
            return Ok(VolInput::Flat(Vec::new()));
        }
        let rows = flat.chunks_exact(ncols).map(<[f64]>::to_vec).collect();
        return Ok(VolInput::Rows(rows));
    }
    if let Ok(untyped) = obj.cast::<numpy::PyUntypedArray>() {
        use numpy::PyUntypedArrayMethods;
        let ndim = untyped.ndim();
        if ndim == 1 {
            return Ok(VolInput::Flat(
                crate::bindings::dataframe_utils::extract_f64_vec(obj, "vols")?,
            ));
        }
        if ndim == 2 {
            let casted = obj.call_method1("astype", ("float64",))?;
            let array = casted.extract::<numpy::PyReadonlyArray2<'_, f64>>()?;
            let ncols = array.shape()[1];
            let flat = crate::bindings::matrix_input::numpy_row_major(&array);
            let rows = if ncols == 0 {
                Vec::new()
            } else {
                flat.chunks_exact(ncols).map(<[f64]>::to_vec).collect()
            };
            return Ok(VolInput::Rows(rows));
        }
    }
    if let Ok(rows) = obj.extract::<Vec<Vec<f64>>>() {
        return Ok(VolInput::Rows(rows));
    }
    if let Ok(flat) = obj.extract::<Vec<f64>>() {
        return Ok(VolInput::Flat(flat));
    }
    if obj.hasattr("tolist")? {
        let listed = obj.call_method0("tolist")?;
        if let Ok(rows) = listed.extract::<Vec<Vec<f64>>>() {
            return Ok(VolInput::Rows(rows));
        }
        if let Ok(flat) = listed.extract::<Vec<f64>>() {
            return Ok(VolInput::Flat(flat));
        }
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "vols must be a flat row-major list of floats, a list of rows, a DataFrame, or a 2-D numpy array",
    ))
}

/// Two-dimensional implied volatility surface on an expiry x secondary-coordinate grid.
///
/// Volatilities are decimal annualised standard deviations (``0.20`` is 20%)
/// stored row-major by expiry. The secondary axis is a strike, a tenor or a
/// moneyness coordinate depending on ``secondary_axis``.
///
/// Example
/// -------
/// >>> from finstack_quant.core.market_data import VolSurface
/// >>> surface = VolSurface(
/// ...     "EQ-VOL",
/// ...     [1.0, 2.0],
/// ...     [90.0, 100.0, 110.0],
/// ...     [[0.22, 0.20, 0.21], [0.23, 0.21, 0.22]],
/// ...     secondary_axis="strike",
/// ...     quote_type="black_lognormal",
/// ...     interpolation_mode="vol",
/// ... )
/// >>> surface.grid_shape
/// (2, 3)
/// >>> from finstack_quant.models.volatility import get_surface_vol
/// >>> round(get_surface_vol(surface, 1.5, 100.0), 4)
/// 0.205
#[pyclass(
    name = "VolSurface",
    module = "finstack_quant.core.market_data",
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
    /// Construct a vol surface from an expiry x secondary-coordinate grid.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique surface identifier.
    /// expiries : list[float]
    ///     Strictly increasing expiry times in years.
    /// secondary_values : list[float]
    ///     Strictly increasing secondary-axis coordinates (strikes, tenors or moneyness).
    /// vols : list[float] | list[list[float]] | numpy.ndarray
    ///     Volatilities as decimals, either flat row-major (``len(expiries) *
    ///     len(secondary_values)``) or as ``len(expiries)`` rows of
    ///     ``len(secondary_values)`` values.
    /// secondary_axis : str
    ///     ``"strike"`` or ``"tenor"``.
    /// quote_type : str
    ///     ``"black_lognormal"``, ``"shifted_black_lognormal"``, or ``"normal"``.
    /// interpolation_mode : str
    ///     ``"vol"`` (bilinear in vol) or ``"total_variance"``.
    /// displacements : list[float] | None, optional
    ///     Shifted-Black rate displacements, one per expiry; absent for other conventions.
    /// expiry_clock : VolatilityClock | None, optional
    ///     Calendar/day-count clock for resolving expiry dates to year fractions.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the grid size does not match the axes, an axis is not strictly
    ///     increasing, a vol is non-finite or negative, or a label is unknown.
    /// TypeError
    ///     If ``vols`` is not a list, nested list or array-like, or a required
    ///     convention keyword argument is missing or unknown.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import VolSurface
    /// >>> VolSurface(
    /// ...     "EQ-VOL",
    /// ...     [1.0],
    /// ...     [90.0, 100.0],
    /// ...     [0.22, 0.20],
    /// ...     secondary_axis="strike",
    /// ...     quote_type="black_lognormal",
    /// ...     interpolation_mode="vol",
    /// ... ).secondary_values
    /// [90.0, 100.0]
    #[new]
    #[pyo3(
        signature = (id, expiries, secondary_values, vols, **options),
        text_signature = "(id, expiries, secondary_values, vols, *, secondary_axis, quote_type, interpolation_mode, displacements=None, expiry_clock=None)"
    )]
    fn new(
        id: &str,
        expiries: Vec<f64>,
        secondary_values: Vec<f64>,
        vols: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let mut secondary_axis: Option<VolSurfaceAxis> = None;
        let mut quote_type: Option<VolQuoteType> = None;
        let mut interpolation_mode: Option<VolInterpolationMode> = None;
        let mut displacements: Option<Vec<f64>> = None;
        let mut expiry_clock: Option<finstack_quant_core::dates::VolatilityClock> = None;

        if let Some(opts) = options {
            for (key, value) in opts.iter() {
                let key_str: String = key.extract()?;
                match key_str.as_str() {
                    "secondary_axis" => {
                        let raw: String = value.extract()?;
                        secondary_axis = Some(serde_parse(&raw).map_err(core_to_py)?);
                    }
                    "quote_type" => {
                        let raw: String = value.extract()?;
                        quote_type = Some(raw.parse::<VolQuoteType>().map_err(core_to_py)?);
                    }
                    "interpolation_mode" => {
                        let raw: String = value.extract()?;
                        interpolation_mode = Some(serde_parse(&raw).map_err(core_to_py)?);
                    }
                    "displacements" => {
                        displacements = value.extract::<Option<Vec<f64>>>()?;
                    }
                    "expiry_clock" => {
                        expiry_clock = value
                            .extract::<Option<
                                PyRef<
                                    '_,
                                    crate::bindings::core::dates::daycount::PyVolatilityClock,
                                >,
                            >>()?
                            .map(|clock| clock.inner.clone());
                    }
                    other => {
                        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                            "VolSurface() got an unexpected keyword argument '{other}'"
                        )));
                    }
                }
            }
        }

        let secondary_axis = secondary_axis.ok_or_else(|| {
            pyo3::exceptions::PyTypeError::new_err(
                "VolSurface() missing required keyword-only argument: 'secondary_axis'",
            )
        })?;
        let quote_type = quote_type.ok_or_else(|| {
            pyo3::exceptions::PyTypeError::new_err(
                "VolSurface() missing required keyword-only argument: 'quote_type'",
            )
        })?;
        let interpolation_mode = interpolation_mode.ok_or_else(|| {
            pyo3::exceptions::PyTypeError::new_err(
                "VolSurface() missing required keyword-only argument: 'interpolation_mode'",
            )
        })?;

        let grid = extract_vol_grid(vols)?;
        let surface = match grid {
            VolInput::Flat(values) => VolSurface::from_grid(
                id,
                &expiries,
                &secondary_values,
                &values,
                VolGridOpts {
                    secondary_axis,
                    quote_type,
                    interpolation_mode,
                },
                displacements.as_deref(),
            )
            .map(|surface| match expiry_clock {
                Some(clock) => surface.with_expiry_clock(clock),
                None => surface,
            }),
            VolInput::Rows(rows) => {
                let mut builder = VolSurface::builder(id)
                    .expiries(&expiries)
                    .secondary_values(&secondary_values)
                    .secondary_axis(secondary_axis)
                    .interpolation_mode(interpolation_mode)
                    .quote_type(quote_type)
                    .rows(&rows);
                if let Some(shifts) = displacements.as_deref() {
                    builder = builder.displacements(shifts);
                }
                if let Some(clock) = expiry_clock {
                    builder = builder.expiry_clock(clock);
                }
                builder.build()
            }
        }
        .map_err(core_to_py)?;

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

    /// Secondary-axis coordinates (strikes, tenors or moneyness) of the stored grid.
    #[getter]
    fn secondary_values(&self) -> Vec<f64> {
        self.inner.secondary_values().to_vec()
    }

    /// Volatility grid (decimal) as ``len(expiries)`` rows of ``len(secondary_values)`` values.
    #[getter]
    fn vols(&self) -> Vec<Vec<f64>> {
        let n_secondary = self.inner.secondary_values().len().max(1);
        self.inner
            .vols()
            .chunks(n_secondary)
            .map(<[f64]>::to_vec)
            .collect()
    }

    /// Secondary-axis semantic meaning (``"strike"`` or ``"tenor"``).
    #[getter]
    fn secondary_axis(&self) -> String {
        self.inner.secondary_axis().to_string()
    }

    /// Stored quote convention: ``"black_lognormal"``, ``"shifted_black_lognormal"``, or ``"normal"``.
    #[getter]
    fn quote_type(&self) -> String {
        self.inner.quote_type().to_string()
    }

    /// Per-expiry shifted-Black displacements in rate units, or ``None`` for unshifted quotes.
    #[getter]
    fn displacements(&self) -> Option<Vec<f64>> {
        self.inner.displacements().map(<[f64]>::to_vec)
    }

    /// Calendar/day-count volatility clock attached to this surface, or ``None``.
    #[getter]
    fn expiry_clock(&self) -> Option<crate::bindings::core::dates::daycount::PyVolatilityClock> {
        self.inner
            .expiry_clock()
            .cloned()
            .map(crate::bindings::core::dates::daycount::PyVolatilityClock::from_inner)
    }

    /// Interpolation contract between grid points (``"vol"`` or ``"total_variance"``).
    #[getter]
    fn interpolation_mode(&self) -> PyResult<String> {
        serde_label(&self.inner.interpolation_mode()).map_err(core_to_py)
    }

    /// Surface grid shape as ``(n_expiries, n_secondary_values)``.
    #[getter]
    fn grid_shape(&self) -> (usize, usize) {
        self.inner.grid_shape()
    }

    fn __repr__(&self) -> String {
        let (n_exp, n_strk) = self.inner.grid_shape();
        format!(
            "VolSurface(id='{}', grid_shape=({}, {}), secondary_axis='{}', quote_type='{}')",
            self.inner.id().as_str(),
            n_exp,
            n_strk,
            self.inner.secondary_axis(),
            self.inner.quote_type()
        )
    }
}

impl PyVolSurface {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let (expiry, (secondary_value, vol)): (Vec<f64>, (Vec<f64>, Vec<f64>)) = self
            .inner
            .grid_points()
            .map(|(expiry, secondary_value, vol)| (expiry, (secondary_value, vol)))
            .unzip();
        columns_to_arrow_table(&[
            ("expiry", expiry),
            ("secondary_value", secondary_value),
            ("vol", vol),
        ])
    }
}

tabular_methods!(PyVolSurface);
wire_methods!(PyVolSurface, VolSurface, "VolSurface", arc);
