//! Fixed-expiry SABR cube data, with an explicit tenor axis.

use std::sync::Arc;

use finstack_quant_core::market_data::surfaces::VolCubeExpirySlice;
use finstack_quant_core::market_data::surfaces::VolQuoteType;
use pyo3::prelude::*;

use super::helpers::columns_to_dataframe;
use crate::bindings::macros::{impl_repr_html_via_dataframe, wire_methods};
use crate::errors::core_to_py;

/// Volatility grid at one fixed option expiry, indexed by tenor and strike.
///
/// The expiry remains separate from the underlying-tenor axis. Shifted Black
/// quotes retain one displacement per tenor, in the same units as the strikes.
#[pyclass(
    name = "VolCubeExpirySlice",
    module = "finstack_quant.core.market_data",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyVolCubeExpirySlice {
    pub(crate) inner: Arc<VolCubeExpirySlice>,
}

impl PyVolCubeExpirySlice {
    pub(crate) fn from_inner(inner: Arc<VolCubeExpirySlice>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyVolCubeExpirySlice {
    /// Construct a fixed-expiry tenor-by-strike grid through Rust validation.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Identifier for the materialized grid.
    /// expiry : float
    ///     Fixed option expiry in years, finite and positive.
    /// tenors : list[float]
    ///     Increasing positive underlying tenors in years.
    /// strikes : list[float]
    ///     Increasing strike coordinates in forward-rate units.
    /// vols : list[float]
    ///     Tenor-major flat grid; decimal Black or absolute normal volatilities.
    /// quote_type : str, optional
    ///     ``black_lognormal``, ``shifted_black_lognormal``, or ``normal``.
    /// displacements : list[float] | None, optional
    ///     Shifted-Black displacements in rate units, one per tenor; otherwise absent.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If axes, grid shape, quote convention, shifts, or values are invalid.
    #[new]
    #[pyo3(signature = (id, expiry, tenors, strikes, vols, *, quote_type="black_lognormal", displacements=None))]
    fn new(
        id: &str,
        expiry: f64,
        tenors: Vec<f64>,
        strikes: Vec<f64>,
        vols: Vec<f64>,
        quote_type: &str,
        displacements: Option<Vec<f64>>,
    ) -> PyResult<Self> {
        VolCubeExpirySlice::from_grid(
            id,
            expiry,
            &tenors,
            &strikes,
            &vols,
            quote_type.parse::<VolQuoteType>().map_err(core_to_py)?,
            displacements.as_deref(),
        )
        .map(|slice| Self::from_inner(Arc::new(slice)))
        .map_err(core_to_py)
    }

    /// Identifier of the materialized grid.
    #[getter]
    fn id(&self) -> &str {
        self.inner.id().as_str()
    }

    /// Fixed option expiry in years.
    #[getter]
    fn expiry(&self) -> f64 {
        self.inner.expiry()
    }

    /// Underlying-tenor axis in years.
    #[getter]
    fn tenors(&self) -> Vec<f64> {
        self.inner.tenors().to_vec()
    }

    /// Strike axis in forward-rate units.
    #[getter]
    fn strikes(&self) -> Vec<f64> {
        self.inner.strikes().to_vec()
    }

    /// Flat tenor-major volatility grid in the declared quote convention.
    #[getter]
    fn vols(&self) -> Vec<f64> {
        self.inner.vols().to_vec()
    }

    /// Quote convention: Black, shifted Black, or normal.
    #[getter]
    fn quote_type(&self) -> String {
        self.inner.quote_type().to_string()
    }

    /// Per-tenor displacement in rate units, absent for unshifted quotes.
    #[getter]
    fn displacements(&self) -> Option<Vec<f64>> {
        self.inner.displacements().map(<[f64]>::to_vec)
    }

    /// Grid dimensions as ``(number of tenors, number of strikes)``.
    #[getter]
    fn grid_shape(&self) -> (usize, usize) {
        self.inner.grid_shape()
    }

    /// Export ``expiry``, ``tenor``, ``strike``, and ``vol`` columns.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let (tenor, (strike, vol)): (Vec<f64>, (Vec<f64>, Vec<f64>)) = self
            .inner
            .grid_points()
            .map(|(tenor, strike, vol)| (tenor, (strike, vol)))
            .unzip();
        columns_to_dataframe(
            py,
            &[
                ("expiry", vec![self.inner.expiry(); vol.len()]),
                ("tenor", tenor),
                ("strike", strike),
                ("vol", vol),
            ],
        )
    }

    fn __repr__(&self) -> String {
        let (n_ten, n_strk) = self.inner.grid_shape();
        format!(
            "VolCubeExpirySlice(id='{}', expiry={}, grid_shape=({}, {}), quote_type='{}')",
            self.inner.id().as_str(),
            self.inner.expiry(),
            n_ten,
            n_strk,
            self.inner.quote_type()
        )
    }
}

wire_methods!(
    PyVolCubeExpirySlice,
    VolCubeExpirySlice,
    "VolCubeExpirySlice",
    arc
);
impl_repr_html_via_dataframe!(PyVolCubeExpirySlice);
