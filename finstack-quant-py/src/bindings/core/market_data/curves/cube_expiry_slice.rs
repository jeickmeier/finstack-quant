//! Fixed-expiry SABR cube data, with an explicit tenor axis.

use std::sync::Arc;

use finstack_quant_core::market_data::surfaces::VolCubeExpirySlice;
use pyo3::prelude::*;

use super::helpers::{columns_to_dataframe, impl_arc_serde_pymethods, parse_vol_quote_type};
use crate::errors::core_to_py;

/// Volatility grid at one fixed option expiry, indexed by tenor and strike.
///
/// The expiry remains separate from the underlying-tenor axis. Shifted Black
/// quotes retain one displacement per tenor, in the same units as the strikes.
#[pyclass(
    name = "VolCubeExpirySlice",
    module = "finstack_quant.core.market_data.curves",
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
            parse_vol_quote_type(quote_type)?,
            displacements.as_deref(),
        )
        .map(|slice| Self::from_inner(Arc::new(slice)))
        .map_err(core_to_py)
    }

    /// Identifier of the materialized grid.
    fn get_id(&self) -> &str {
        self.inner.get_id().as_str()
    }

    /// Fixed option expiry in years.
    fn get_expiry(&self) -> f64 {
        self.inner.get_expiry()
    }

    /// Underlying-tenor axis in years.
    fn get_tenors(&self) -> Vec<f64> {
        self.inner.get_tenors().to_vec()
    }

    /// Strike axis in forward-rate units.
    fn get_strikes(&self) -> Vec<f64> {
        self.inner.get_strikes().to_vec()
    }

    /// Flat tenor-major volatility grid in the declared quote convention.
    fn get_vols(&self) -> Vec<f64> {
        self.inner.get_vols().to_vec()
    }

    /// Quote convention: Black, shifted Black, or normal.
    fn get_quote_type(&self) -> String {
        self.inner.get_quote_type().to_string()
    }

    /// Per-tenor displacement in rate units, absent for unshifted quotes.
    fn get_displacements(&self) -> Option<Vec<f64>> {
        self.inner.get_displacements().map(<[f64]>::to_vec)
    }

    /// Grid dimensions as ``(number of tenors, number of strikes)``.
    fn get_grid_shape(&self) -> (usize, usize) {
        self.inner.get_grid_shape()
    }

    /// Export ``expiry``, ``tenor``, ``strike``, and ``vol`` columns.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut tenors = Vec::with_capacity(self.inner.get_vols().len());
        let mut strikes = Vec::with_capacity(self.inner.get_vols().len());
        for &tenor in self.inner.get_tenors() {
            for &strike in self.inner.get_strikes() {
                tenors.push(tenor);
                strikes.push(strike);
            }
        }
        columns_to_dataframe(
            py,
            &[
                (
                    "expiry",
                    vec![self.inner.get_expiry(); self.inner.get_vols().len()],
                ),
                ("tenor", tenors),
                ("strike", strikes),
                ("vol", self.inner.get_vols().to_vec()),
            ],
        )
    }
}

impl_arc_serde_pymethods!(
    PyVolCubeExpirySlice,
    VolCubeExpirySlice,
    "VolCubeExpirySlice"
);
