//! Volatility surface, cube and FX delta-surface bindings.

use finstack_quant_core::market_data::surfaces::{
    FxDeltaVolSurface, SabrParameterData, VolCube, VolGridOpts, VolSurface,
};
use pyo3::types::PyDict;

use std::sync::Arc;

use pyo3::prelude::*;

use super::helpers::{
    columns_to_dataframe, impl_arc_serde_pymethods, impl_repr_html_via_dataframe,
    parse_vol_interpolation_mode, parse_vol_quote_type, parse_vol_surface_axis,
    vol_interpolation_mode_name,
};
use crate::errors::core_to_py;

/// Extract a row-major volatility grid from a flat list, a nested list of
/// rows, or any object exposing ``tolist()`` (e.g. a 2-D numpy array).
fn extract_vol_grid(obj: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
    if let Ok(rows) = obj.extract::<Vec<Vec<f64>>>() {
        return Ok(rows.into_iter().flatten().collect());
    }
    if let Ok(flat) = obj.extract::<Vec<f64>>() {
        return Ok(flat);
    }
    if obj.hasattr("tolist")? {
        let listed = obj.call_method0("tolist")?;
        if let Ok(rows) = listed.extract::<Vec<Vec<f64>>>() {
            return Ok(rows.into_iter().flatten().collect());
        }
        if let Ok(flat) = listed.extract::<Vec<f64>>() {
            return Ok(flat);
        }
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "vols must be a flat row-major list of floats, a list of rows, or a 2-D numpy array",
    ))
}

/// Two-dimensional implied volatility surface on an expiry x strike grid.
///
/// Volatilities are decimal annualised standard deviations (``0.20`` is 20%)
/// stored row-major by expiry. The secondary axis is a strike, a tenor or a
/// moneyness coordinate depending on ``secondary_axis``.
///
/// Example
/// -------
/// >>> from finstack_quant.core.market_data import VolSurface
/// >>> surface = VolSurface("EQ-VOL", [1.0, 2.0], [90.0, 100.0, 110.0], [[0.22, 0.20, 0.21], [0.23, 0.21, 0.22]])
/// >>> surface.grid_shape
/// (2, 3)
/// >>> round(surface.vol(1.5, 100.0), 4)
/// 0.205
#[pyclass(
    name = "VolSurface",
    module = "finstack_quant.core.market_data.curves",
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
    /// Construct a vol surface from an expiry x strike grid.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique surface identifier.
    /// expiries : list[float]
    ///     Strictly increasing expiry times in years.
    /// strikes : list[float]
    ///     Strictly increasing secondary-axis coordinates (strikes, tenors or moneyness).
    /// vols : list[float] | list[list[float]] | numpy.ndarray
    ///     Volatilities as decimals, either flat row-major (``len(expiries) *
    ///     len(strikes)``) or as ``len(expiries)`` rows of ``len(strikes)`` values.
    /// secondary_axis : str, optional
    ///     ``"strike"`` (default) or ``"tenor"``.
    /// interpolation_mode : str, optional
    ///     ``"vol"`` (default, bilinear in vol) or ``"total_variance"``.
    /// quote_type : str, optional
    ///     ``"black_lognormal"`` (default) or ``"normal"``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the grid size does not match the axes, an axis is not strictly
    ///     increasing, a vol is non-finite or negative, or a label is unknown.
    /// TypeError
    ///     If ``vols`` is not a list, nested list or array-like.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import VolSurface
    /// >>> VolSurface("EQ-VOL", [1.0], [90.0, 100.0], [0.22, 0.20]).strikes
    /// [90.0, 100.0]
    #[new]
    #[pyo3(signature = (id, expiries, strikes, vols, *, secondary_axis="strike", interpolation_mode="vol", quote_type="black_lognormal"))]
    fn new(
        id: &str,
        expiries: Vec<f64>,
        strikes: Vec<f64>,
        vols: &Bound<'_, PyAny>,
        secondary_axis: &str,
        interpolation_mode: &str,
        quote_type: &str,
    ) -> PyResult<Self> {
        let axis = parse_vol_surface_axis(secondary_axis)?;
        let mode = parse_vol_interpolation_mode(interpolation_mode)?;
        let quote = parse_vol_quote_type(quote_type)?;
        let grid = extract_vol_grid(vols)?;
        let surface = VolSurface::from_grid_opts(
            id,
            &expiries,
            &strikes,
            &grid,
            VolGridOpts {
                secondary_axis: axis,
                quote_type: quote,
                interpolation_mode: mode,
            },
        )
        .map_err(core_to_py)?;

        Ok(Self {
            inner: Arc::new(surface),
        })
    }

    /// Interpolated volatility (decimal) at an expiry / secondary-axis point.
    ///
    /// Bilinear in vol or in total variance according to ``interpolation_mode``.
    ///
    /// Parameters
    /// ----------
    /// expiry : float
    ///     Expiry in years; must lie within the ``expiries`` range.
    /// strike : float
    ///     Secondary-axis coordinate; must lie within the ``strikes`` range.
    ///
    /// Returns
    /// -------
    /// float
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a coordinate is non-finite or outside the grid.
    #[pyo3(text_signature = "(self, expiry, strike)")]
    fn vol(&self, expiry: f64, strike: f64) -> PyResult<f64> {
        finstack_quant_models::volatility::get_surface_vol(&self.inner, expiry, strike)
            .map_err(core_to_py)
    }

    /// Export the grid in long form with columns ``expiry``, ``strike`` and ``vol``.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///     One row per grid node, expiries outer and strikes inner.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let expiries = self.inner.expiries();
        let strikes = self.inner.strikes();
        let mut exp_col = Vec::with_capacity(expiries.len() * strikes.len());
        let mut strike_col = Vec::with_capacity(exp_col.capacity());
        for &e in expiries {
            for &k in strikes {
                exp_col.push(e);
                strike_col.push(k);
            }
        }
        columns_to_dataframe(
            py,
            &[
                ("expiry", exp_col),
                ("strike", strike_col),
                ("vol", self.inner.vols().to_vec()),
            ],
        )
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

    /// Secondary axis (strikes, tenors or moneyness) of the stored grid.
    #[getter]
    fn strikes(&self) -> Vec<f64> {
        self.inner.strikes().to_vec()
    }

    /// Volatility grid (decimal) as ``len(expiries)`` rows of ``len(strikes)`` values.
    #[getter]
    fn vols(&self) -> Vec<Vec<f64>> {
        let n_strikes = self.inner.strikes().len().max(1);
        self.inner
            .vols()
            .chunks(n_strikes)
            .map(<[f64]>::to_vec)
            .collect()
    }

    /// Secondary-axis semantic meaning (``"strike"`` or ``"tenor"``).
    #[getter]
    fn secondary_axis(&self) -> String {
        self.inner.secondary_axis().to_string()
    }

    /// Quoting convention of the stored volatilities (``"black_lognormal"`` or ``"normal"``).
    #[getter]
    fn quote_type(&self) -> String {
        self.inner.quote_type().to_string()
    }

    /// Interpolation contract between grid points (``"vol"`` or ``"total_variance"``).
    #[getter]
    fn interpolation_mode(&self) -> PyResult<String> {
        vol_interpolation_mode_name(self.inner.interpolation_mode())
    }

    /// Surface grid shape as ``(n_expiries, n_strikes)``.
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

impl_arc_serde_pymethods!(PyVolSurface, VolSurface, "VolSurface");
impl_repr_html_via_dataframe!(PyVolSurface);

/// Delta-quoted FX volatility surface (ATM, 25-delta RR/BF, optional 10-delta wings).
///
/// Uses forward delta (premium-unadjusted). Quotes are decimal vols
/// (``0.08`` is 8%); risk reversals are call vol minus put vol and butterflies
/// are average wing vol minus ATM. See Wystup (2006) and Clark (2011).
///
/// Example
/// -------
/// >>> from finstack_quant.core.market_data import FxDeltaVolSurface
/// >>> surface = FxDeltaVolSurface("EURUSD", [0.25, 1.0], [0.08, 0.09], [0.01, 0.015], [0.005, 0.007])
/// >>> surface.num_expiries
/// 2
#[pyclass(
    name = "FxDeltaVolSurface",
    module = "finstack_quant.core.market_data.curves",
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
    /// expiries : list[float]
    ///     Strictly increasing positive expiry times in years.
    /// atm_vols : list[float]
    ///     ATM delta-neutral straddle vols per expiry (decimal, positive).
    /// rr_25d : list[float]
    ///     25-delta risk reversal per expiry (decimal, call vol - put vol).
    /// bf_25d : list[float]
    ///     25-delta butterfly per expiry (decimal, wing average - ATM).
    /// rr_10d : list[float], optional
    ///     10-delta risk reversal per expiry; requires ``bf_10d``.
    /// bf_10d : list[float], optional
    ///     10-delta butterfly per expiry; requires ``rr_10d``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If only one of ``rr_10d`` / ``bf_10d`` is given, any vector is empty
    ///     or mismatched in length, expiries are not strictly increasing and
    ///     positive, or a quote is non-finite (ATM vols must be positive).
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import FxDeltaVolSurface
    /// >>> FxDeltaVolSurface("EURUSD", [1.0], [0.09], [0.015], [0.007]).atm_vols
    /// [0.09]
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
        let surface =
            FxDeltaVolSurface::new(id, expiries, atm_vols, rr_25d, bf_25d, rr_10d, bf_10d)
                .map_err(core_to_py)?;
        Ok(Self {
            inner: Arc::new(surface),
        })
    }

    /// Export pillars as a pandas ``DataFrame``.
    ///
    /// Columns: ``expiry``, ``atm_vol``, ``rr_25d``, ``bf_25d`` and, when
    /// 10-delta wings are stored, ``rr_10d`` and ``bf_10d``.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
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
        columns_to_dataframe(py, &columns)
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

    fn __repr__(&self) -> String {
        format!(
            "FxDeltaVolSurface(id='{}', num_expiries={}, has_10d={})",
            self.inner.id().as_str(),
            self.inner.num_expiries(),
            if self.inner.rr_10d().is_some() {
                "True"
            } else {
                "False"
            }
        )
    }
}

impl_arc_serde_pymethods!(PyFxDeltaVolSurface, FxDeltaVolSurface, "FxDeltaVolSurface");
impl_repr_html_via_dataframe!(PyFxDeltaVolSurface);

/// Calibrated SABR parameters for one vol-cube node.
///
/// Example
/// -------
/// >>> from finstack_quant.core.market_data import SabrParameterData
/// >>> p = SabrParameterData(0.02, 0.5, -0.2, 0.3)
/// >>> (p.alpha, p.shift)
/// (0.02, None)
#[pyclass(
    name = "SabrParameterData",
    module = "finstack_quant.core.market_data.curves",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Copy, PartialEq)]
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
    /// rho : float
    ///     Forward/volatility correlation in ``(-1, 1)``.
    /// nu : float
    ///     Volatility of volatility; strictly positive.
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
    /// >>> SabrParameterData(0.02, 0.5, -0.2, 0.3, shift=0.03).shift
    /// 0.03
    #[new]
    #[pyo3(signature = (alpha, beta, rho, nu, shift=None))]
    fn new(alpha: f64, beta: f64, rho: f64, nu: f64, shift: Option<f64>) -> PyResult<Self> {
        SabrParameterData::new_with_shift(alpha, beta, rho, nu, shift)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Initial volatility level (strictly positive).
    #[getter]
    fn alpha(&self) -> f64 {
        self.inner.alpha
    }

    /// CEV exponent in ``[0, 1]``.
    #[getter]
    fn beta(&self) -> f64 {
        self.inner.beta
    }

    /// Forward/volatility correlation in ``(-1, 1)``.
    #[getter]
    fn rho(&self) -> f64 {
        self.inner.rho
    }

    /// Volatility of volatility (strictly positive).
    #[getter]
    fn nu(&self) -> f64 {
        self.inner.nu
    }

    /// Displacement applied to forward and strike, or ``None``.
    #[getter]
    fn shift(&self) -> Option<f64> {
        self.inner.shift
    }

    /// Serialize to canonical JSON (``{"alpha": ..., "beta": ..., "rho": ..., "nu": ..., "shift": ...}``).
    ///
    /// Returns
    /// -------
    /// str
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    // PyO3 `#[pymethods]` cannot take `self` by value.
    #[allow(clippy::wrong_self_convention)]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| {
            crate::errors::value_error(format!("failed to serialize SabrParameterData: {e}"))
        })
    }

    /// Deserialize from canonical JSON.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON produced by ``to_json``.
    ///
    /// Returns
    /// -------
    /// SabrParameterData
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed or the parameters fail validation.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str::<SabrParameterData>(json)
            .map(Self::from_inner)
            .map_err(|e| crate::errors::value_error(format!("invalid SabrParameterData JSON: {e}")))
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        let shift = self
            .inner
            .shift
            .map_or_else(|| "None".to_string(), |s| s.to_string());
        format!(
            "SabrParameterData(alpha={}, beta={}, rho={}, nu={}, shift={})",
            self.inner.alpha, self.inner.beta, self.inner.rho, self.inner.nu, shift
        )
    }
}

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
/// >>> p = SabrParameterData(0.02, 0.5, -0.2, 0.3)
/// >>> cube = VolCube("USD-SWPT", [1.0, 2.0], [5.0], [p, p], [0.03, 0.032])
/// >>> cube.forward_at(1, 0)
/// 0.032
#[pyclass(
    name = "VolCube",
    module = "finstack_quant.core.market_data.curves",
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
        let mode = interpolation_mode
            .map(parse_vol_interpolation_mode)
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

    /// Export nodes in long form.
    ///
    /// Columns: ``expiry``, ``tenor``, ``alpha``, ``beta``, ``rho``, ``nu``,
    /// ``shift`` (``NaN`` when absent) and ``forward``.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let expiries = self.inner.expiries();
        let tenors = self.inner.tenors();
        let params = self.inner.params();
        let n = params.len();
        let mut exp_col = Vec::with_capacity(n);
        let mut ten_col = Vec::with_capacity(n);
        for &e in expiries {
            for &t in tenors {
                exp_col.push(e);
                ten_col.push(t);
            }
        }
        columns_to_dataframe(
            py,
            &[
                ("expiry", exp_col),
                ("tenor", ten_col),
                ("alpha", params.iter().map(|p| p.alpha).collect()),
                ("beta", params.iter().map(|p| p.beta).collect()),
                ("rho", params.iter().map(|p| p.rho).collect()),
                ("nu", params.iter().map(|p| p.nu).collect()),
                (
                    "shift",
                    params.iter().map(|p| p.shift.unwrap_or(f64::NAN)).collect(),
                ),
                ("forward", self.inner.forwards().to_vec()),
            ],
        )
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
        vol_interpolation_mode_name(self.inner.interpolation_mode())
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

impl_arc_serde_pymethods!(PyVolCube, VolCube, "VolCube");
impl_repr_html_via_dataframe!(PyVolCube);
