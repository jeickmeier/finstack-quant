//! Python wrappers for XVA types (CVA/DVA/FVA configuration and results).

use crate::bindings::pandas_utils::{
    dict_to_dataframe, serde_object_to_single_row_dataframe_with_schema, serde_to_py,
};
use crate::errors::{core_to_py, display_to_py};
use finstack_quant_margin::xva::types as xva;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use finstack_quant_margin::xva::cva;
use finstack_quant_margin::xva::mva;

use crate::bindings::core::market_data::curves::{PyDiscountCurve, PyHazardCurve};
use crate::bindings::margin::im::{PySimmCalculator, PySimmSensitivities};

/// Funding cost/benefit configuration for FVA and MVA calculation.
#[pyclass(
    name = "FundingConfig",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFundingConfig {
    pub(super) inner: xva::FundingConfig,
}

#[pymethods]
impl PyFundingConfig {
    /// Create a new funding configuration.
    ///
    /// Parameters
    /// ----------
    /// funding_spread_bp : float
    ///     Non-negative finite funding cost spread in basis points.
    /// funding_benefit_bp : float | None
    ///     Non-negative finite funding benefit spread in bp, no greater than
    ///     ``funding_spread_bp``. If ``None``, symmetric funding.
    /// im_profile : ImProfile | None
    ///     Valid expected initial-margin profile driving MVA. If ``None``, MVA
    ///     is not computed.
    /// margin_funding_spread_bp : float | None
    ///     Non-negative finite spread applied to posted IM. If ``None``, uses
    ///     ``funding_spread_bp``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a spread or the optional IM profile violates these constraints.
    #[new]
    #[pyo3(signature = (
        funding_spread_bp,
        funding_benefit_bp=None,
        im_profile=None,
        margin_funding_spread_bp=None,
    ))]
    fn new(
        funding_spread_bp: f64,
        funding_benefit_bp: Option<f64>,
        im_profile: Option<&PyImProfile>,
        margin_funding_spread_bp: Option<f64>,
    ) -> PyResult<Self> {
        let inner = xva::FundingConfig {
            funding_spread_bp,
            funding_benefit_bp,
            im_profile: im_profile.map(|p| p.inner.clone()),
            margin_funding_spread_bp,
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Support pickle through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from the JSON produced by ``to_json``; raises
    /// ``ValueError`` on malformed input or a config that fails validation.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: xva::FundingConfig = serde_json::from_str(json).map_err(display_to_py)?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Funding spread in basis points.
    #[getter]
    fn funding_spread_bp(&self) -> f64 {
        self.inner.funding_spread_bp
    }

    /// Funding benefit spread in basis points (or None).
    #[getter]
    fn funding_benefit_bp(&self) -> Option<f64> {
        self.inner.funding_benefit_bp
    }

    /// Expected IM profile driving MVA (or None).
    #[getter]
    fn im_profile(&self) -> Option<PyImProfile> {
        self.inner.im_profile.as_ref().map(|inner| PyImProfile {
            inner: inner.clone(),
        })
    }

    /// IM funding spread in basis points (or None).
    #[getter]
    fn margin_funding_spread_bp(&self) -> Option<f64> {
        self.inner.margin_funding_spread_bp
    }

    /// Effective funding benefit spread in basis points.
    fn effective_benefit_bp(&self) -> f64 {
        self.inner.effective_benefit_bp()
    }

    /// Effective IM funding spread in basis points (falls back to
    /// ``funding_spread_bp``).
    fn effective_margin_spread_bp(&self) -> f64 {
        self.inner.effective_margin_spread_bp()
    }

    fn __repr__(&self) -> String {
        format!(
            "FundingConfig(funding_spread_bp={}, funding_benefit_bp={}, margin_funding_spread_bp={}, im_profile={})",
            self.inner.funding_spread_bp,
            self.inner
                .funding_benefit_bp
                .map_or("None".to_string(), |v| v.to_string()),
            self.inner
                .margin_funding_spread_bp
                .map_or("None".to_string(), |v| v.to_string()),
            if self.inner.im_profile.is_some() {
                "ImProfile(...)"
            } else {
                "None"
            },
        )
    }
}

/// Diagnostics from exposure simulation.
///
/// Counters an exposure engine attaches to an ``ExposureProfile``: how many
/// market-roll and valuation failures occurred over how many time points.
#[pyclass(
    name = "ExposureDiagnostics",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyExposureDiagnostics {
    pub(super) inner: xva::ExposureDiagnostics,
}

#[pymethods]
impl PyExposureDiagnostics {
    /// Create a diagnostics record from its three counters (all default to
    /// zero).
    #[new]
    #[pyo3(signature = (market_roll_failures = 0, valuation_failures = 0, total_time_points = 0))]
    fn new(
        market_roll_failures: usize,
        valuation_failures: usize,
        total_time_points: usize,
    ) -> Self {
        Self {
            inner: xva::ExposureDiagnostics {
                market_roll_failures,
                valuation_failures,
                total_time_points,
            },
        }
    }

    /// Support pickle through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from the JSON produced by ``to_json``; raises
    /// ``ValueError`` on malformed input.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: xva::ExposureDiagnostics = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Number of market-roll failures.
    #[getter]
    fn market_roll_failures(&self) -> usize {
        self.inner.market_roll_failures
    }

    /// Total instrument valuation failures.
    #[getter]
    fn valuation_failures(&self) -> usize {
        self.inner.valuation_failures
    }

    /// Total time grid points evaluated.
    #[getter]
    fn total_time_points(&self) -> usize {
        self.inner.total_time_points
    }

    fn __repr__(&self) -> String {
        format!(
            "ExposureDiagnostics(market_roll_failures={}, valuation_failures={}, points={})",
            self.inner.market_roll_failures,
            self.inner.valuation_failures,
            self.inner.total_time_points
        )
    }
}

/// Exposure profile at each time grid point.
#[pyclass(
    name = "ExposureProfile",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyExposureProfile {
    pub(super) inner: xva::ExposureProfile,
}

#[pymethods]
impl PyExposureProfile {
    /// Construct from parallel vectors on a time grid.
    ///
    /// ``times`` are nonnegative, strictly increasing year fractions; a zero-time
    /// origin is allowed, and XVA holds exposure constant before the first point. ``mtm_values``,
    /// ``epe`` and ``ene`` are amounts in the netting set's currency at each
    /// time. ``diagnostics`` optionally attaches the engine's failure
    /// counters. Values are stored as given; ``validate()`` and the XVA
    /// entry points check consistency.
    #[new]
    #[pyo3(signature = (times, mtm_values, epe, ene, diagnostics = None))]
    fn new(
        times: Vec<f64>,
        mtm_values: Vec<f64>,
        epe: Vec<f64>,
        ene: Vec<f64>,
        diagnostics: Option<&PyExposureDiagnostics>,
    ) -> Self {
        Self {
            inner: xva::ExposureProfile {
                times,
                mtm_values,
                epe,
                ene,
                diagnostics: diagnostics.map(|d| d.inner.clone()),
            },
        }
    }

    /// Build a profile from the frame ``to_dataframe`` emits: columns
    /// ``mtm_values``, ``epe``, ``ene`` indexed by time in years.
    ///
    /// Raises ``ValueError`` if a column is missing or non-numeric, and
    /// ``TypeError`` when ``frame`` is not a pandas ``DataFrame``.
    #[staticmethod]
    fn from_dataframe(frame: &Bound<'_, PyAny>) -> PyResult<Self> {
        fn column(frame: &Bound<'_, PyAny>, name: &str) -> PyResult<Vec<f64>> {
            let column = frame.get_item(name).map_err(|_| {
                crate::errors::value_error(format!("from_dataframe: column '{name}' is missing"))
            })?;
            column
                .call_method0("tolist")?
                .extract::<Vec<f64>>()
                .map_err(|_| {
                    crate::errors::value_error(format!(
                        "from_dataframe: column '{name}' must be numeric"
                    ))
                })
        }
        let times: Vec<f64> = frame
            .getattr("index")?
            .call_method0("tolist")?
            .extract()
            .map_err(|_| {
                crate::errors::value_error("from_dataframe: index must hold times in years")
            })?;
        Ok(Self {
            inner: xva::ExposureProfile {
                times,
                mtm_values: column(frame, "mtm_values")?,
                epe: column(frame, "epe")?,
                ene: column(frame, "ene")?,
                diagnostics: None,
            },
        })
    }

    /// Engine diagnostics attached to the profile, or ``None``.
    #[getter]
    fn diagnostics(&self) -> Option<PyExposureDiagnostics> {
        self.inner
            .diagnostics
            .as_ref()
            .map(|inner| PyExposureDiagnostics {
                inner: inner.clone(),
            })
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: xva::ExposureProfile = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Validate internal consistency.
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Time points in years.
    #[getter]
    fn times(&self) -> Vec<f64> {
        self.inner.times.clone()
    }

    /// Portfolio MtM values at each time point.
    #[getter]
    fn mtm_values(&self) -> Vec<f64> {
        self.inner.mtm_values.clone()
    }

    /// Expected Positive Exposure at each time point.
    #[getter]
    fn epe(&self) -> Vec<f64> {
        self.inner.epe.clone()
    }

    /// Expected Negative Exposure at each time point.
    #[getter]
    fn ene(&self) -> Vec<f64> {
        self.inner.ene.clone()
    }

    /// Number of time points.
    fn __len__(&self) -> usize {
        self.inner.times.len()
    }

    /// Export as a pandas ``DataFrame`` with time (years) as index.
    ///
    /// Columns: ``mtm_values``, ``epe``, ``ene``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        data.set_item("mtm_values", &self.inner.mtm_values)?;
        data.set_item("epe", &self.inner.epe)?;
        data.set_item("ene", &self.inner.ene)?;
        let idx = self.inner.times.clone().into_pyobject(py)?.into_any();
        dict_to_dataframe(py, &data, Some(idx))
    }

    fn __repr__(&self) -> String {
        format!(
            "ExposureProfile(points={}, diagnostics={})",
            self.inner.times.len(),
            if self.inner.diagnostics.is_some() {
                "ExposureDiagnostics(...)"
            } else {
                "None"
            }
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Result of XVA calculations (CVA, DVA, FVA, MVA, exposure profiles).
#[pyclass(
    name = "XvaResult",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyXvaResult {
    pub(super) inner: xva::XvaResult,
}

#[pymethods]
impl PyXvaResult {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: xva::XvaResult = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Unilateral CVA (positive = cost).
    #[getter]
    fn cva(&self) -> f64 {
        self.inner.cva
    }

    /// DVA (own-default benefit, or None).
    #[getter]
    fn dva(&self) -> Option<f64> {
        self.inner.dva
    }

    /// FVA (net funding cost/benefit, or None).
    #[getter]
    fn fva(&self) -> Option<f64> {
        self.inner.fva
    }

    /// MVA (funding cost of posted initial margin, or None).
    #[getter]
    fn mva(&self) -> Option<f64> {
        self.inner.mva
    }

    /// All-in adjustment = CVA - DVA + FVA + MVA.
    ///
    /// Uncomputed legs contribute zero. This is the quantity subtracted from
    /// the risk-free value of the netting set.
    #[getter]
    fn total_xva(&self) -> f64 {
        self.inner.total_xva
    }

    /// Maximum PFE across the profile.
    #[getter]
    fn max_pfe(&self) -> f64 {
        self.inner.max_pfe
    }

    /// Effective EPE (time-weighted average, regulatory metric).
    #[getter]
    fn effective_epe(&self) -> f64 {
        self.inner.effective_epe
    }

    /// EPE profile as list of (time, value) tuples.
    #[getter]
    fn epe_profile(&self) -> Vec<(f64, f64)> {
        self.inner.epe_profile.clone()
    }

    /// ENE profile as list of (time, value) tuples.
    #[getter]
    fn ene_profile(&self) -> Vec<(f64, f64)> {
        self.inner.ene_profile.clone()
    }

    /// PFE profile as list of (time, value) tuples.
    #[getter]
    fn pfe_profile(&self) -> Vec<(f64, f64)> {
        self.inner.pfe_profile.clone()
    }

    /// Effective EPE profile as list of (time, value) tuples.
    #[getter]
    fn effective_epe_profile(&self) -> Vec<(f64, f64)> {
        self.inner.effective_epe_profile.clone()
    }

    /// Policy metadata stamped by the computing layer as a dict: numeric
    /// mode, active rounding context, any applied FX policy and the
    /// parallel-execution flag.
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    /// Export the XVA components as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``cva``, ``dva``, ``fva``, ``mva``, ``total_xva``,
    /// ``max_pfe``, ``effective_epe`` — all in the netting set's currency
    /// units, matching the getters of the same name. Uncomputed legs
    /// (``dva`` / ``fva`` / ``mva``) are ``NaN`` rather than absent, so the
    /// frame keeps its schema across netting sets.
    ///
    /// This is the default export; one row per result, so a portfolio of
    /// netting sets stacks with
    /// ``pd.concat([r.to_dataframe() for r in results])``. The time-indexed
    /// exposure profiles are a separate table — see ``to_profiles_dataframe``.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        // Built explicitly rather than from `XvaResult`'s serde, which also
        // carries the four profile vectors — a nested list per cell is not a
        // scalar summary row.
        let row = serde_json::json!({
            "cva": self.inner.cva,
            "dva": self.inner.dva,
            "fva": self.inner.fva,
            "mva": self.inner.mva,
            "total_xva": self.inner.total_xva,
            "max_pfe": self.inner.max_pfe,
            "effective_epe": self.inner.effective_epe,
        });
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &[
                "cva",
                "dva",
                "fva",
                "mva",
                "total_xva",
                "max_pfe",
                "effective_epe",
            ],
        )
    }

    /// Export exposure profiles as a pandas ``DataFrame``.
    ///
    /// Columns: ``epe``, ``ene``, ``pfe``, ``effective_epe`` — indexed by
    /// time in years.  Time values are taken from the EPE profile.
    fn to_profiles_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        let (times, epe_vals): (Vec<f64>, Vec<f64>) =
            self.inner.epe_profile.iter().copied().unzip();
        let (_, ene_vals): (Vec<f64>, Vec<f64>) = self.inner.ene_profile.iter().copied().unzip();
        let (_, pfe_vals): (Vec<f64>, Vec<f64>) = self.inner.pfe_profile.iter().copied().unzip();
        let (_, eff_epe_vals): (Vec<f64>, Vec<f64>) =
            self.inner.effective_epe_profile.iter().copied().unzip();

        data.set_item("epe", epe_vals)?;
        data.set_item("ene", ene_vals)?;
        data.set_item("pfe", pfe_vals)?;
        data.set_item("effective_epe", eff_epe_vals)?;
        let idx = times.into_pyobject(py)?.into_any();
        dict_to_dataframe(py, &data, Some(idx))
    }

    fn __repr__(&self) -> String {
        fn opt(value: Option<f64>) -> String {
            value.map_or("None".to_string(), |v| format!("{v:.4}"))
        }
        format!(
            "XvaResult(cva={:.4}, dva={}, fva={}, mva={}, total_xva={:.4}, max_pfe={:.2})",
            self.inner.cva,
            opt(self.inner.dva),
            opt(self.inner.fva),
            opt(self.inner.mva),
            self.inner.total_xva,
            self.inner.max_pfe
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Deterministic IM decay profile for MVA (Green 2015, ch. 10).
#[pyclass(
    name = "ImDecayProfile",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyImDecayProfile {
    pub(super) inner: mva::ImDecayProfile,
}

#[pymethods]
impl PyImDecayProfile {
    /// IM stays at today's level for the whole horizon.
    #[staticmethod]
    fn constant() -> Self {
        Self {
            inner: mva::ImDecayProfile::Constant,
        }
    }

    /// IM decays linearly to zero at ``maturity_years``.
    #[staticmethod]
    fn linear_to_maturity(maturity_years: f64) -> PyResult<Self> {
        let inner = mva::ImDecayProfile::LinearToMaturity { maturity_years };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// IM decays like sqrt of remaining time to ``maturity_years``.
    #[staticmethod]
    fn sqrt_time(maturity_years: f64) -> PyResult<Self> {
        let inner = mva::ImDecayProfile::SqrtTime { maturity_years };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Decay factor at time ``t`` (years); always in ``[0, 1]``.
    fn factor(&self, t: f64) -> f64 {
        self.inner.factor(t)
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: mva::ImDecayProfile = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            mva::ImDecayProfile::Constant => "ImDecayProfile(constant)".to_string(),
            mva::ImDecayProfile::LinearToMaturity { maturity_years } => {
                format!("ImDecayProfile(linear_to_maturity, maturity_years={maturity_years})")
            }
            mva::ImDecayProfile::SqrtTime { maturity_years } => {
                format!("ImDecayProfile(sqrt_time, maturity_years={maturity_years})")
            }
        }
    }
}

/// Expected initial-margin profile E[IM(t)] on a time grid.
#[pyclass(
    name = "ImProfile",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyImProfile {
    pub(super) inner: mva::ImProfile,
}

#[pymethods]
impl PyImProfile {
    /// Construct from time and IM vectors.
    ///
    /// Values are stored as given; call ``validate()`` explicitly or rely
    /// on downstream functions (``compute_mva``, ``im_profile_from_simm``)
    /// to reject an inconsistent profile.
    #[new]
    fn new(times: Vec<f64>, im_values: Vec<f64>) -> Self {
        Self {
            inner: mva::ImProfile { times, im_values },
        }
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: mva::ImProfile = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Validate internal consistency.
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Time points in years.
    #[getter]
    fn times(&self) -> Vec<f64> {
        self.inner.times.clone()
    }

    /// Expected IM at each time point.
    #[getter]
    fn im_values(&self) -> Vec<f64> {
        self.inner.im_values.clone()
    }

    /// Number of time points.
    fn __len__(&self) -> usize {
        self.inner.times.len()
    }

    /// Export as a pandas ``DataFrame`` with time (years) as index.
    ///
    /// Columns: ``im``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        data.set_item("im", &self.inner.im_values)?;
        let idx = self.inner.times.clone().into_pyobject(py)?.into_any();
        dict_to_dataframe(py, &data, Some(idx))
    }

    fn __repr__(&self) -> String {
        format!("ImProfile(points={})", self.inner.times.len())
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Result of an MVA computation.
#[pyclass(
    name = "MvaResult",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyMvaResult {
    pub(super) inner: mva::MvaResult,
}

#[pymethods]
impl PyMvaResult {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: mva::MvaResult = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// MVA (positive = lifetime funding cost of posting IM).
    #[getter]
    fn mva(&self) -> f64 {
        self.inner.mva
    }

    /// Time-weighted average IM over the profile horizon.
    #[getter]
    fn average_im(&self) -> f64 {
        self.inner.average_im
    }

    /// IM profile used, as ``(time, value)`` tuples.
    #[getter]
    fn im_profile(&self) -> Vec<(f64, f64)> {
        self.inner.im_profile.clone()
    }

    /// Export the IM profile as a pandas ``DataFrame`` (column ``im``,
    /// indexed by time in years).
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        let (times, im_vals): (Vec<f64>, Vec<f64>) = self.inner.im_profile.iter().copied().unzip();
        data.set_item("im", im_vals)?;
        let idx = times.into_pyobject(py)?.into_any();
        dict_to_dataframe(py, &data, Some(idx))
    }

    fn __repr__(&self) -> String {
        format!(
            "MvaResult(mva={:.4}, average_im={:.2})",
            self.inner.mva, self.inner.average_im
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Build a deterministic IM profile from SIMM sensitivities:
/// ``IM(t) = SIMM(sensitivities) * decay(t)``.
#[pyfunction]
#[pyo3(signature = (calculator, sensitivities, currency, decay, time_grid))]
fn im_profile_from_simm(
    calculator: &PySimmCalculator,
    sensitivities: &PySimmSensitivities,
    currency: &str,
    decay: &PyImDecayProfile,
    time_grid: Vec<f64>,
) -> PyResult<PyImProfile> {
    let ccy: finstack_quant_core::currency::Currency = currency.parse().map_err(display_to_py)?;
    let inner = mva::im_profile_from_simm(
        &calculator.inner,
        &sensitivities.inner,
        ccy,
        &decay.inner,
        &time_grid,
    )
    .map_err(core_to_py)?;
    Ok(PyImProfile { inner })
}

/// Compute MVA: ``∫ spread(t) · IM(t) · DF(t) · S(t) dt`` (trapezoid).
///
/// Parameters
/// ----------
/// im_profile : ImProfile
///     Expected IM profile.
/// funding_spread_curve : list[tuple[float, float]] | pandas.Series
///     ``(time_years, spread_bp)`` pairs, or a ``Series`` of spreads in bp
///     indexed by time in years; a single pair means a flat spread.
/// discount_curve : DiscountCurve
///     Risk-free discount curve.
/// survival_curve : HazardCurve | None
///     Optional bank (own) hazard curve; ``None`` means no survival weighting.
#[pyfunction]
#[pyo3(signature = (im_profile, funding_spread_curve, discount_curve, survival_curve=None))]
fn compute_mva(
    im_profile: &PyImProfile,
    funding_spread_curve: &Bound<'_, PyAny>,
    discount_curve: &PyDiscountCurve,
    survival_curve: Option<&PyHazardCurve>,
) -> PyResult<PyMvaResult> {
    let funding_spread_curve = super::frame::pairs_from_series_or_list(funding_spread_curve)?;
    let inner = mva::compute_mva(
        &im_profile.inner,
        &funding_spread_curve,
        &discount_curve.inner,
        survival_curve.map(|c| c.inner.as_ref()),
    )
    .map_err(core_to_py)?;
    Ok(PyMvaResult { inner })
}

/// Compute bilateral XVA: CVA, DVA, FVA, MVA, and the all-in adjustment.
///
/// All legs are weighted by joint (first-to-default) survival. MVA is computed
/// only when ``funding`` carries an ``im_profile``; that posted IM also reduces
/// ENE for bilateral DVA.
///
/// The result reports ``total_xva = CVA - DVA + FVA + MVA``. Uncomputed
/// optional legs contribute zero.
///
/// Parameters
/// ----------
/// exposure_profile : ExposureProfile
///     EPE/ENE profile from exposure simulation.
/// counterparty_hazard_curve : HazardCurve
///     Hazard curve for the counterparty's credit.
/// own_hazard_curve : HazardCurve
///     Hazard curve for the institution's own credit.
/// discount_curve : DiscountCurve
///     Risk-free discount curve.
/// counterparty_recovery_rate : float
///     Recovery rate on counterparty default, in ``[0, 1]``.
/// own_recovery_rate : float
///     Recovery rate on own default, in ``[0, 1]``.
/// funding : FundingConfig | None
///     Funding configuration driving FVA and, when it carries an
///     ``im_profile``, MVA. ``None`` computes credit legs only.
///
/// Returns
/// -------
/// XvaResult
///     Bilateral credit, funding, margin, and exposure results.
///
/// Raises
/// ------
/// ValueError
///     If a profile, recovery rate, funding input, or curve evaluation is
///     invalid, including mismatched IM and exposure horizons.
#[pyfunction]
#[pyo3(signature = (
    exposure_profile,
    counterparty_hazard_curve,
    own_hazard_curve,
    discount_curve,
    counterparty_recovery_rate,
    own_recovery_rate,
    funding=None,
))]
fn compute_bilateral_xva(
    exposure_profile: &PyExposureProfile,
    counterparty_hazard_curve: &PyHazardCurve,
    own_hazard_curve: &PyHazardCurve,
    discount_curve: &PyDiscountCurve,
    counterparty_recovery_rate: f64,
    own_recovery_rate: f64,
    funding: Option<&PyFundingConfig>,
) -> PyResult<PyXvaResult> {
    let inner = cva::compute_bilateral_xva(
        &exposure_profile.inner,
        counterparty_hazard_curve.inner.as_ref(),
        own_hazard_curve.inner.as_ref(),
        &discount_curve.inner,
        counterparty_recovery_rate,
        own_recovery_rate,
        funding.map(|f| &f.inner),
    )
    .map_err(core_to_py)?;
    Ok(PyXvaResult { inner })
}

/// Register XVA classes.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFundingConfig>()?;
    m.add_class::<PyExposureDiagnostics>()?;
    m.add_class::<PyExposureProfile>()?;
    m.add_class::<PyXvaResult>()?;
    m.add_class::<PyImDecayProfile>()?;
    m.add_class::<PyImProfile>()?;
    m.add_class::<PyMvaResult>()?;
    m.add_function(pyo3::wrap_pyfunction!(im_profile_from_simm, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(compute_mva, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(compute_bilateral_xva, m)?)?;
    Ok(())
}
