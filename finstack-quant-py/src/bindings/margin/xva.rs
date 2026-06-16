//! Python wrappers for XVA types (CVA/DVA/FVA configuration and results).

use crate::bindings::pandas_utils::dict_to_dataframe;
use crate::errors::{core_to_py, display_to_py};
use finstack_quant_margin::xva::types as xva;
use pyo3::prelude::*;
use pyo3::types::PyDict;

// ---------------------------------------------------------------------------
// FundingConfig
// ---------------------------------------------------------------------------

/// Funding cost/benefit configuration for FVA calculation.
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
    /// funding_spread_bps : float
    ///     Funding spread in basis points.
    /// funding_benefit_bps : float | None
    ///     Funding benefit spread in bps. If ``None``, symmetric funding.
    #[new]
    #[pyo3(signature = (funding_spread_bps, funding_benefit_bps=None))]
    fn new(funding_spread_bps: f64, funding_benefit_bps: Option<f64>) -> Self {
        Self {
            inner: xva::FundingConfig {
                funding_spread_bps,
                funding_benefit_bps,
            },
        }
    }

    /// Funding spread in basis points.
    #[getter]
    fn funding_spread_bps(&self) -> f64 {
        self.inner.funding_spread_bps
    }

    /// Funding benefit spread in basis points (or None).
    #[getter]
    fn funding_benefit_bps(&self) -> Option<f64> {
        self.inner.funding_benefit_bps
    }

    /// Effective funding benefit spread in basis points.
    fn effective_benefit_bps(&self) -> f64 {
        self.inner.effective_benefit_bps()
    }

    fn __repr__(&self) -> String {
        format!(
            "FundingConfig(spread={:.1}bp, benefit={:?}bp)",
            self.inner.funding_spread_bps, self.inner.funding_benefit_bps,
        )
    }
}

// ---------------------------------------------------------------------------
// XvaConfig
// ---------------------------------------------------------------------------

/// XVA calculation configuration.
#[pyclass(
    name = "XvaConfig",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyXvaConfig {
    pub(super) inner: xva::XvaConfig,
}

#[pymethods]
impl PyXvaConfig {
    /// Create a default XVA configuration (quarterly grid to 30Y, 40% recovery).
    #[new]
    #[pyo3(signature = (time_grid=None, recovery_rate=None, own_recovery_rate=None, funding=None))]
    fn new(
        time_grid: Option<Vec<f64>>,
        recovery_rate: Option<f64>,
        own_recovery_rate: Option<f64>,
        funding: Option<&PyFundingConfig>,
    ) -> Self {
        let default = xva::XvaConfig::default();
        Self {
            inner: xva::XvaConfig {
                time_grid: time_grid.unwrap_or(default.time_grid),
                recovery_rate: recovery_rate.unwrap_or(default.recovery_rate),
                own_recovery_rate,
                funding: funding.map(|f| f.inner.clone()),
            },
        }
    }

    /// Deserialize from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: xva::XvaConfig = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(display_to_py)
    }

    /// Validate configuration parameters.
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Time grid for exposure simulation (years from today).
    #[getter]
    fn time_grid(&self) -> Vec<f64> {
        self.inner.time_grid.clone()
    }

    /// Recovery rate for counterparty default.
    #[getter]
    fn recovery_rate(&self) -> f64 {
        self.inner.recovery_rate
    }

    /// Recovery rate for own default (or None).
    #[getter]
    fn own_recovery_rate(&self) -> Option<f64> {
        self.inner.own_recovery_rate
    }

    fn __repr__(&self) -> String {
        format!(
            "XvaConfig(grid_points={}, recovery={:.0}%)",
            self.inner.time_grid.len(),
            self.inner.recovery_rate * 100.0
        )
    }
}

// ---------------------------------------------------------------------------
// ExposureDiagnostics
// ---------------------------------------------------------------------------

/// Diagnostics from exposure simulation.
#[pyclass(
    name = "ExposureDiagnostics",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyExposureDiagnostics {
    #[allow(dead_code)]
    pub(super) inner: xva::ExposureDiagnostics,
}

#[pymethods]
impl PyExposureDiagnostics {
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

// ---------------------------------------------------------------------------
// ExposureProfile
// ---------------------------------------------------------------------------

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
    /// Construct from vectors.
    #[new]
    fn new(times: Vec<f64>, mtm_values: Vec<f64>, epe: Vec<f64>, ene: Vec<f64>) -> Self {
        Self {
            inner: xva::ExposureProfile {
                times,
                mtm_values,
                epe,
                ene,
                diagnostics: None,
            },
        }
    }

    /// Deserialize from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: xva::ExposureProfile = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(display_to_py)
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
        format!("ExposureProfile(points={})", self.inner.times.len())
    }
}

// ---------------------------------------------------------------------------
// XvaResult
// ---------------------------------------------------------------------------

/// Result of XVA calculations (CVA, DVA, FVA, exposure profiles).
#[pyclass(
    name = "XvaResult",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyXvaResult {
    #[allow(dead_code)]
    pub(super) inner: xva::XvaResult,
}

#[pymethods]
impl PyXvaResult {
    /// Deserialize from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: xva::XvaResult = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(display_to_py)
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

    /// Bilateral CVA = CVA - DVA (or None).
    #[getter]
    fn bilateral_cva(&self) -> Option<f64> {
        self.inner.bilateral_cva
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

    /// Export exposure profiles as a pandas ``DataFrame``.
    ///
    /// Columns: ``epe``, ``ene``, ``pfe``, ``effective_epe`` — indexed by
    /// time in years.  Time values are taken from the EPE profile.
    fn profiles_to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
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
        format!(
            "XvaResult(cva={:.4}, dva={:?}, fva={:?}, max_pfe={:.2})",
            self.inner.cva, self.inner.dva, self.inner.fva, self.inner.max_pfe
        )
    }
}

// ---------------------------------------------------------------------------
// CsaTerms (XVA)
// ---------------------------------------------------------------------------

/// Credit Support Annex terms for XVA collateralization.
#[pyclass(
    name = "CsaTerms",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCsaTerms {
    #[allow(dead_code)]
    pub(super) inner: xva::CsaTerms,
}

#[pymethods]
impl PyCsaTerms {
    /// Create new CSA terms.
    #[new]
    fn new(threshold: f64, mta: f64, mpor_days: u32, independent_amount: f64) -> Self {
        Self {
            inner: xva::CsaTerms {
                threshold,
                mta,
                mpor_days,
                independent_amount,
            },
        }
    }

    /// Threshold below which no collateral is required.
    #[getter]
    fn threshold(&self) -> f64 {
        self.inner.threshold
    }

    /// Minimum transfer amount.
    #[getter]
    fn mta(&self) -> f64 {
        self.inner.mta
    }

    /// Margin period of risk in calendar days.
    #[getter]
    fn mpor_days(&self) -> u32 {
        self.inner.mpor_days
    }

    /// Independent amount (initial margin).
    #[getter]
    fn independent_amount(&self) -> f64 {
        self.inner.independent_amount
    }

    fn __repr__(&self) -> String {
        format!(
            "CsaTerms(threshold={:.0}, mta={:.0}, mpor={}d, ia={:.0})",
            self.inner.threshold,
            self.inner.mta,
            self.inner.mpor_days,
            self.inner.independent_amount
        )
    }
}

// ---------------------------------------------------------------------------
// NettingSet (XVA)
// ---------------------------------------------------------------------------

/// XVA netting set: collection of trades under a single ISDA master agreement.
#[pyclass(
    name = "XvaNettingSet",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyXvaNettingSet {
    #[allow(dead_code)]
    pub(super) inner: xva::XvaNettingSet,
}

#[pymethods]
impl PyXvaNettingSet {
    /// Create a new XVA netting set.
    #[new]
    #[pyo3(signature = (id, counterparty_id, csa=None, reporting_currency=None))]
    fn new(
        id: &str,
        counterparty_id: &str,
        csa: Option<&PyCsaTerms>,
        reporting_currency: Option<&str>,
    ) -> PyResult<Self> {
        let ccy = match reporting_currency {
            Some(s) => {
                let c: finstack_quant_core::currency::Currency =
                    s.parse().map_err(display_to_py)?;
                Some(c)
            }
            None => None,
        };
        Ok(Self {
            inner: xva::XvaNettingSet {
                id: id.to_string(),
                counterparty_id: counterparty_id.to_string(),
                csa: csa.map(|c| c.inner.clone()),
                reporting_currency: ccy,
            },
        })
    }

    /// Netting set identifier.
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    /// Counterparty identifier.
    #[getter]
    fn counterparty_id(&self) -> &str {
        &self.inner.counterparty_id
    }

    /// Whether this netting set is collateralized.
    #[getter]
    fn is_collateralized(&self) -> bool {
        self.inner.csa.is_some()
    }

    fn __repr__(&self) -> String {
        format!(
            "XvaNettingSet(id={:?}, cpty={:?}, collateralized={})",
            self.inner.id,
            self.inner.counterparty_id,
            self.inner.csa.is_some()
        )
    }
}

/// Register XVA classes.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFundingConfig>()?;
    m.add_class::<PyXvaConfig>()?;
    m.add_class::<PyExposureDiagnostics>()?;
    m.add_class::<PyExposureProfile>()?;
    m.add_class::<PyXvaResult>()?;
    m.add_class::<PyCsaTerms>()?;
    m.add_class::<PyXvaNettingSet>()?;
    Ok(())
}
