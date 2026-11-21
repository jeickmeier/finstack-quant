//! Volatility surface, cube and FX delta-surface bindings.

use finstack_quant_core::market_data::surfaces::{
    FxDeltaVolSurface, SabrParameterData, VolCube, VolGridOpts, VolInterpolationMode, VolSurface,
    VolSurfaceAxis,
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

enum VolInput {
    Flat(Vec<f64>),
    Rows(Vec<Vec<f64>>),
}

/// Preserve nested rows until the canonical Rust constructor validates their shape.
fn extract_vol_grid(obj: &Bound<'_, PyAny>) -> PyResult<VolInput> {
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

/// Delta-quoted FX volatility surface (ATM, 25-delta RR/BF, optional 10-delta wings).
///
/// Uses forward delta (premium-unadjusted). Quotes are decimal vols
/// (``0.08`` is 8%); risk reversals are call vol minus put vol and butterflies
/// are average wing vol minus ATM. See Wystup (2006) and Clark (2011).
///
/// Example
/// -------
/// >>> from finstack_quant.core.market_data import FxDeltaVolSurface
/// >>> surface = FxDeltaVolSurface("EURUSD", "EUR/USD", [0.25, 1.0], [0.08, 0.09], [0.01, 0.015], [0.005, 0.007], delta_convention="forward_unadjusted", atm_convention="delta_neutral_straddle", butterfly_convention="smile")
/// >>> len(surface.expiries)
/// 2
#[pyclass(
    name = "FxDeltaVolSurface",
    module = "finstack_quant.core.market_data",
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
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique surface identifier.
    /// pair : str | tuple[Currency | str, Currency | str]
    ///     Currency pair (e.g. ``"EUR/USD"``, ``"EURUSD"``, or ``(Currency.EUR, Currency.USD)``).
    /// expiries : list[float]
    ///     Strictly increasing positive expiry times in years.
    /// atm_vols : list[float]
    ///     ATM delta-neutral straddle vols per expiry (decimal, positive).
    /// rr_25d : list[float]
    ///     25-delta risk reversal per expiry (decimal, call vol - put vol).
    /// bf_25d : list[float]
    ///     25-delta smile butterfly per expiry (decimal, wing average - ATM).
    /// rr_10d : list[float], optional
    ///     10-delta risk reversal per expiry; requires ``bf_10d``.
    /// bf_10d : list[float], optional
    ///     10-delta smile butterfly per expiry; requires ``rr_10d``.
    /// delta_convention : str
    ///     Required FX delta convention (``"forward_unadjusted"``).
    /// atm_convention : str
    ///     Required ATM convention (``"delta_neutral_straddle"``).
    /// butterfly_convention : str
    ///     Required butterfly convention (``"smile"``).
    /// expiry_clock : VolatilityClock, optional
    ///     Calendar/day-count clock for resolving expiry dates to year fractions.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``pair`` or conventions are invalid, only one of ``rr_10d`` / ``bf_10d``
    ///     is given, any vector is empty or mismatched in length, expiries are not
    ///     strictly increasing and positive, or a quote is non-finite.
    /// TypeError
    ///     If a required convention is omitted or an option is unknown.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import FxDeltaVolSurface
    /// >>> FxDeltaVolSurface("EURUSD", "EUR/USD", [1.0], [0.09], [0.015], [0.007], delta_convention="forward_unadjusted", atm_convention="delta_neutral_straddle", butterfly_convention="smile").atm_vols
    /// [0.09]
    #[new]
    #[pyo3(
        signature = (id, pair, expiries, atm_vols, rr_25d, bf_25d, **options),
        text_signature = "(id, pair, expiries, atm_vols, rr_25d, bf_25d, *, delta_convention, atm_convention, butterfly_convention, rr_10d=None, bf_10d=None, expiry_clock=None)"
    )]
    fn new(
        id: &str,
        pair: &Bound<'_, PyAny>,
        expiries: Vec<f64>,
        atm_vols: Vec<f64>,
        rr_25d: Vec<f64>,
        bf_25d: Vec<f64>,
        options: Option<&Bound<'_, pyo3::types::PyDict>>,
    ) -> PyResult<Self> {
        use finstack_quant_core::market_data::surfaces::FxDeltaVolSurfaceParams;
        use finstack_quant_core::money::fx::CurrencyPair;
        use finstack_quant_core::types::CurveId;

        let parsed_pair = if let Ok(s) = pair.extract::<String>() {
            let parsed: CurrencyPair = s.trim().parse().map_err(core_to_py)?;
            CurrencyPair::try_new(parsed.base, parsed.quote).map_err(core_to_py)?
        } else if let Ok(items) = pair.extract::<Vec<Bound<'_, PyAny>>>() {
            if items.len() != 2 {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "pair sequence must contain exactly 2 currencies (base, quote)",
                ));
            }
            let base = crate::bindings::core::currency::extract_currency(&items[0])?;
            let quote = crate::bindings::core::currency::extract_currency(&items[1])?;
            CurrencyPair::try_new(base, quote).map_err(core_to_py)?
        } else {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "pair must be a currency pair string like 'EUR/USD' or a 2-tuple (base, quote)",
            ));
        };

        let mut rr_10d: Option<Vec<f64>> = None;
        let mut bf_10d: Option<Vec<f64>> = None;
        let mut delta_convention = None;
        let mut atm_convention = None;
        let mut butterfly_convention = None;
        let mut expiry_clock: Option<finstack_quant_core::dates::VolatilityClock> = None;

        if let Some(opts) = options {
            for (key, value) in opts.iter() {
                let key_str: String = key.extract()?;
                match key_str.as_str() {
                    "rr_10d" => rr_10d = value.extract::<Option<Vec<f64>>>()?,
                    "bf_10d" => bf_10d = value.extract::<Option<Vec<f64>>>()?,
                    "delta_convention" => {
                        let raw: String = value.extract()?;
                        delta_convention = Some(raw.parse().map_err(core_to_py)?);
                    }
                    "atm_convention" => {
                        let raw: String = value.extract()?;
                        atm_convention = Some(raw.parse().map_err(core_to_py)?);
                    }
                    "butterfly_convention" => {
                        let raw: String = value.extract()?;
                        butterfly_convention = Some(raw.parse().map_err(core_to_py)?);
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
                            "FxDeltaVolSurface() got an unexpected keyword argument '{other}'"
                        )));
                    }
                }
            }
        }

        let missing = |key: &str| {
            pyo3::exceptions::PyTypeError::new_err(format!(
                "FxDeltaVolSurface() missing required keyword-only argument '{key}'"
            ))
        };
        let surface = FxDeltaVolSurface::from_params(FxDeltaVolSurfaceParams {
            id: CurveId::from(id),
            pair: parsed_pair,
            delta_convention: delta_convention.ok_or_else(|| missing("delta_convention"))?,
            atm_convention: atm_convention.ok_or_else(|| missing("atm_convention"))?,
            butterfly_convention: butterfly_convention
                .ok_or_else(|| missing("butterfly_convention"))?,
            expiries,
            atm_vols,
            rr_25d,
            bf_25d,
            rr_10d,
            bf_10d,
            expiry_clock,
        })
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

    /// ATM delta-neutral straddle vols per expiry (decimal).
    #[getter]
    fn atm_vols(&self) -> Vec<f64> {
        self.inner.atm_vols().to_vec()
    }

    /// 25-delta risk reversals per expiry (decimal).
    #[getter]
    fn rr_25d(&self) -> Vec<f64> {
        self.inner.rr_25d().to_vec()
    }

    /// 25-delta butterflies per expiry (decimal).
    #[getter]
    fn bf_25d(&self) -> Vec<f64> {
        self.inner.bf_25d().to_vec()
    }

    /// 10-delta risk reversals per expiry (decimal), or ``None``.
    #[getter]
    fn rr_10d(&self) -> Option<Vec<f64>> {
        self.inner.rr_10d().map(<[f64]>::to_vec)
    }

    /// 10-delta butterflies per expiry (decimal), or ``None``.
    #[getter]
    fn bf_10d(&self) -> Option<Vec<f64>> {
        self.inner.bf_10d().map(<[f64]>::to_vec)
    }

    /// Canonical currency pair in ``"BASE/QUOTE"`` notation.
    #[getter]
    fn pair(&self) -> String {
        self.inner.pair().to_string()
    }

    /// Delta quoting convention (e.g. ``"forward_unadjusted"``).
    #[getter]
    fn delta_convention(&self) -> String {
        self.inner.delta_convention().to_string()
    }

    /// ATM quoting convention (e.g. ``"delta_neutral_straddle"``).
    #[getter]
    fn atm_convention(&self) -> String {
        self.inner.atm_convention().to_string()
    }

    /// Butterfly quoting convention (e.g. ``"smile"``).
    #[getter]
    fn butterfly_convention(&self) -> String {
        self.inner.butterfly_convention().to_string()
    }

    /// Quote convention stored by this surface (``"black_lognormal"``).
    #[getter]
    fn quote_type(&self) -> String {
        self.inner.quote_type().to_string()
    }

    /// Calendar/day-count volatility clock attached to this surface, or ``None``.
    #[getter]
    fn expiry_clock(&self) -> Option<crate::bindings::core::dates::daycount::PyVolatilityClock> {
        self.inner
            .expiry_clock()
            .cloned()
            .map(crate::bindings::core::dates::daycount::PyVolatilityClock::from_inner)
    }

    fn __repr__(&self) -> String {
        format!(
            "FxDeltaVolSurface(id='{}', pair='{}', num_expiries={}, has_10d={})",
            self.inner.id().as_str(),
            self.inner.pair(),
            self.inner.expiries().len(),
            if self.inner.rr_10d().is_some() {
                "True"
            } else {
                "False"
            }
        )
    }
}

impl PyFxDeltaVolSurface {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let mut columns = vec![
            ("expiry", self.inner.expiries().to_vec()),
            ("atm_vol", self.inner.atm_vols().to_vec()),
            ("rr_25d", self.inner.rr_25d().to_vec()),
            ("bf_25d", self.inner.bf_25d().to_vec()),
        ];
        if let (Some(rr), Some(bf)) = (self.inner.rr_10d(), self.inner.bf_10d()) {
            columns.push(("rr_10d", rr.to_vec()));
            columns.push(("bf_10d", bf.to_vec()));
        }
        columns_to_arrow_table(&columns)
    }
}

tabular_methods!(PyFxDeltaVolSurface);
wire_methods!(
    PyFxDeltaVolSurface,
    FxDeltaVolSurface,
    "FxDeltaVolSurface",
    arc
);

/// Calibrated SABR parameters for one vol-cube node.
///
/// Example
/// -------
/// >>> from finstack_quant.core.market_data import SabrParameterData
/// >>> p = SabrParameterData(0.02, 0.5, 0.3, -0.2)
/// >>> (p.alpha, p.shift)
/// (0.02, None)
#[pyclass(
    name = "SabrParameterData",
    module = "finstack_quant.core.market_data",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PySabrParameterData {
    /// Inner Rust parameter node.
    pub(crate) inner: SabrParameterData,
}

impl PySabrParameterData {
    /// Wrap an existing Rust node.
    pub(crate) fn from_inner(inner: SabrParameterData) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PySabrParameterData {
    /// Construct a validated SABR parameter node.
    ///
    /// Parameters
    /// ----------
    /// alpha : float
    ///     Initial volatility level; strictly positive.
    /// beta : float
    ///     CEV exponent in ``[0, 1]``.
    /// nu : float
    ///     Volatility of volatility; nonnegative, with zero giving deterministic volatility.
    /// rho : float
    ///     Forward/volatility correlation in ``(-1, 1)``.
    /// shift : float, optional
    ///     Displacement added to forward and strike (decimal rate units, e.g. ``0.03``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If any value is non-finite or outside its range.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import SabrParameterData
    /// >>> SabrParameterData(0.02, 0.5, 0.3, -0.2, shift=0.03).shift
    /// 0.03
    #[new]
    #[pyo3(signature = (alpha, beta, nu, rho, shift=None))]
    fn new(alpha: f64, beta: f64, nu: f64, rho: f64, shift: Option<f64>) -> PyResult<Self> {
        SabrParameterData::new(alpha, beta, nu, rho, shift)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Initial volatility level (strictly positive).
    #[getter]
    fn alpha(&self) -> f64 {
        self.inner.alpha()
    }

    /// CEV exponent in ``[0, 1]``.
    #[getter]
    fn beta(&self) -> f64 {
        self.inner.beta()
    }

    /// Forward/volatility correlation in ``(-1, 1)``.
    #[getter]
    fn rho(&self) -> f64 {
        self.inner.rho()
    }

    /// Volatility of volatility (nonnegative; zero gives deterministic volatility).
    #[getter]
    fn nu(&self) -> f64 {
        self.inner.nu()
    }

    /// Displacement applied to forward and strike, or ``None``.
    #[getter]
    fn shift(&self) -> Option<f64> {
        self.inner.shift()
    }

    fn __repr__(&self) -> String {
        let shift = self
            .inner
            .shift()
            .map_or_else(|| "None".to_string(), |s| s.to_string());
        format!(
            "SabrParameterData(alpha={}, beta={}, rho={}, nu={}, shift={})",
            self.inner.alpha(),
            self.inner.beta(),
            self.inner.rho(),
            self.inner.nu(),
            shift
        )
    }
}

wire_methods!(PySabrParameterData, SabrParameterData, "SabrParameterData");

/// Parse one cube node: a ``SabrParameterData`` or a dict in the Rust
/// ``SabrParameterData`` wire shape (``alpha``, ``beta``, ``rho``, ``nu`` and
/// optional ``shift``).
///
/// The dict is converted to its JSON value and decoded by the Rust
/// `Deserialize` impl, which rejects missing and unknown keys and validates the
/// parameter ranges. JSON cannot carry a non-finite number, so one is rejected
/// at this boundary with its key named, as the WASM JSON input walker does.
fn extract_sabr_node(obj: &Bound<'_, PyAny>, idx: usize) -> PyResult<SabrParameterData> {
    if let Ok(typed) = obj.extract::<PyRef<'_, PySabrParameterData>>() {
        return Ok(typed.inner);
    }
    let dict = obj.cast::<PyDict>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(format!(
            "params_row_major[{idx}]: expected SabrParameterData or dict"
        ))
    })?;
    let mut node = serde_json::Map::with_capacity(dict.len());
    for (key, value) in dict.iter() {
        let key: String = key.extract()?;
        let value = if value.is_none() {
            serde_json::Value::Null
        } else {
            let number: f64 = value.extract()?;
            serde_json::Number::from_f64(number)
                .map(serde_json::Value::Number)
                .ok_or_else(|| {
                    crate::errors::value_error(format!(
                        "params_row_major[{idx}].{key}: expected a finite number, got {number}"
                    ))
                })?
        };
        node.insert(key, value);
    }
    serde_json::from_value::<SabrParameterData>(serde_json::Value::Object(node))
        .map_err(|e| crate::errors::value_error(format!("params_row_major[{idx}]: {e}")))
}

/// SABR volatility cube on an expiry x tenor grid.
///
/// Each node stores calibrated SABR parameters and the forward swap rate
/// (decimal) for that expiry/tenor pair, row-major by expiry.
///
/// Example
/// -------
/// >>> from finstack_quant.core.market_data import SabrParameterData, VolCube
/// >>> p = SabrParameterData(0.02, 0.5, 0.3, -0.2)
/// >>> cube = VolCube("USD-SWPT", [1.0, 2.0], [5.0], [p, p], [0.03, 0.032])
/// >>> cube.forward_at(1, 0)
/// 0.032
#[pyclass(
    name = "VolCube",
    module = "finstack_quant.core.market_data",
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

#[pymethods]
impl PyVolCube {
    /// Construct a vol cube from row-major grid data.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique cube identifier.
    /// expiries : list[float]
    ///     Option expiry axis in years, strictly increasing.
    /// tenors : list[float]
    ///     Underlying swap tenor axis in years, strictly increasing.
    /// params_row_major : list[SabrParameterData | dict]
    ///     ``len(expiries) * len(tenors)`` SABR nodes, row-major by expiry.
    ///     Dicts use keys ``"alpha"``, ``"beta"``, ``"rho"``, ``"nu"`` and
    ///     optionally ``"shift"``; missing or unknown keys are rejected.
    /// forwards_row_major : list[float]
    ///     Forward swap rates (decimal) in the same row-major order.
    /// interpolation_mode : str, optional
    ///     ``"vol"`` or ``"total_variance"``; ``None`` keeps the Rust
    ///     ``VolCube::from_grid`` default (``"vol"``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If grid sizes do not match the axes, a node fails SABR validation,
    ///     or a label is unknown.
    /// TypeError
    ///     If a node is neither ``SabrParameterData`` nor a dict.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import VolCube
    /// >>> node = {"alpha": 0.02, "beta": 0.5, "rho": -0.2, "nu": 0.3}
    /// >>> VolCube("USD-SWPT", [1.0], [5.0, 10.0], [node, node], [0.03, 0.035]).grid_shape
    /// (1, 2)
    #[new]
    #[pyo3(signature = (id, expiries, tenors, params_row_major, forwards_row_major, interpolation_mode=None))]
    fn new(
        id: &str,
        expiries: Vec<f64>,
        tenors: Vec<f64>,
        params_row_major: Vec<Bound<'_, PyAny>>,
        forwards_row_major: Vec<f64>,
        interpolation_mode: Option<&str>,
    ) -> PyResult<Self> {
        let mode: Option<VolInterpolationMode> = interpolation_mode
            .map(|m| serde_parse(m).map_err(core_to_py))
            .transpose()?;

        let sabr_params: Vec<SabrParameterData> = params_row_major
            .iter()
            .enumerate()
            .map(|(i, node)| extract_sabr_node(node, i))
            .collect::<PyResult<Vec<_>>>()?;

        let mut cube =
            VolCube::from_grid(id, &expiries, &tenors, &sabr_params, &forwards_row_major)
                .map_err(core_to_py)?;
        if let Some(mode) = mode {
            cube = cube.with_interpolation_mode(mode);
        }

        Ok(Self {
            inner: Arc::new(cube),
        })
    }

    /// SABR parameters at grid indices.
    ///
    /// Parameters
    /// ----------
    /// exp_idx : int
    ///     Zero-based expiry index.
    /// tenor_idx : int
    ///     Zero-based tenor index.
    ///
    /// Returns
    /// -------
    /// SabrParameterData
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an index is outside the grid (checked by Rust ``VolCube``).
    #[pyo3(text_signature = "(self, exp_idx, tenor_idx)")]
    fn params_at(&self, exp_idx: usize, tenor_idx: usize) -> PyResult<PySabrParameterData> {
        self.inner
            .params_at(exp_idx, tenor_idx)
            .map(|params| PySabrParameterData::from_inner(*params))
            .map_err(core_to_py)
    }

    /// Forward swap rate (decimal) at grid indices.
    ///
    /// Parameters
    /// ----------
    /// exp_idx : int
    ///     Zero-based expiry index.
    /// tenor_idx : int
    ///     Zero-based tenor index.
    ///
    /// Returns
    /// -------
    /// float
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an index is outside the grid (checked by Rust ``VolCube``).
    #[pyo3(text_signature = "(self, exp_idx, tenor_idx)")]
    fn forward_at(&self, exp_idx: usize, tenor_idx: usize) -> PyResult<f64> {
        self.inner
            .forward_at(exp_idx, tenor_idx)
            .map_err(core_to_py)
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

    /// SABR nodes in row-major (expiry outer, tenor inner) order.
    #[getter]
    fn params(&self) -> Vec<PySabrParameterData> {
        self.inner
            .params()
            .iter()
            .copied()
            .map(PySabrParameterData::from_inner)
            .collect()
    }

    /// Forward swap rates (decimal) in row-major order.
    #[getter]
    fn forwards(&self) -> Vec<f64> {
        self.inner.forwards().to_vec()
    }

    /// Grid shape as ``(n_expiries, n_tenors)``.
    #[getter]
    fn grid_shape(&self) -> (usize, usize) {
        self.inner.grid_shape()
    }

    /// Interpolation contract (``"vol"`` or ``"total_variance"``).
    #[getter]
    fn interpolation_mode(&self) -> PyResult<String> {
        serde_label(&self.inner.interpolation_mode()).map_err(core_to_py)
    }

    fn __repr__(&self) -> String {
        let (n_exp, n_ten) = self.inner.grid_shape();
        format!(
            "VolCube(id='{}', grid_shape=({}, {}))",
            self.inner.id().as_str(),
            n_exp,
            n_ten
        )
    }
}

impl PyVolCube {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        type Node<'a> = (f64, f64, &'a SabrParameterData, f64);
        let nodes: Vec<Node<'_>> = self.inner.nodes().collect();
        let column = |f: &dyn Fn(&Node<'_>) -> f64| -> Vec<f64> { nodes.iter().map(f).collect() };
        columns_to_arrow_table(&[
            ("expiry", column(&|n| n.0)),
            ("tenor", column(&|n| n.1)),
            ("alpha", column(&|n| n.2.alpha())),
            ("beta", column(&|n| n.2.beta())),
            ("rho", column(&|n| n.2.rho())),
            ("nu", column(&|n| n.2.nu())),
            ("shift", column(&|n| n.2.shift().unwrap_or(f64::NAN))),
            ("forward", column(&|n| n.3)),
        ])
    }
}

tabular_methods!(PyVolCube);
wire_methods!(PyVolCube, VolCube, "VolCube", arc);
