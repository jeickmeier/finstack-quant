//! SABR parameter node and 3D volatility cube Python wrappers.

use finstack_quant_core::market_data::surfaces::{
    SabrParameterData, VolCube, VolInterpolationMode,
};
use finstack_quant_core::wire::{serde_label, serde_parse};
use pyo3::types::PyDict;

use std::sync::Arc;

use pyo3::prelude::*;

use super::helpers::columns_to_arrow_table;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::{tabular_methods, wire_methods};
use crate::errors::core_to_py;

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
