//! FX delta-quoted volatility surface Python wrapper.

use finstack_quant_core::market_data::surfaces::FxDeltaVolSurface;

use std::sync::Arc;

use pyo3::prelude::*;

use super::helpers::columns_to_arrow_table;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::{tabular_methods, wire_methods};
use crate::errors::core_to_py;

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
