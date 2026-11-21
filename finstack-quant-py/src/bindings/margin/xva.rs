//! Python wrappers for XVA types (CVA/DVA/FVA configuration and results).

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::tabular_methods;
use crate::bindings::module_utils::parse_currency;
use crate::bindings::table_builders::{TableBuilder, serde_object_to_single_row_arrow_table};
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
    /// margin_funding_spread : MarginFundingSpread | None
    ///     Dated or flat funding spread applied to expected posted IM. Required
    ///     when ``im_profile`` is present.
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
        margin_funding_spread=None,
    ))]
    fn new(
        funding_spread_bp: f64,
        funding_benefit_bp: Option<f64>,
        im_profile: Option<&PyImProfile>,
        margin_funding_spread: Option<&PyMarginFundingSpread>,
    ) -> PyResult<Self> {
        let inner = xva::FundingConfig {
            funding_spread_bp,
            funding_benefit_bp,
            im_profile: im_profile.map(|p| p.inner.clone()),
            margin_funding_spread: margin_funding_spread.map(|spread| spread.inner.clone()),
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

    /// Elected funding spread for expected posted IM, if configured.
    #[getter]
    fn margin_funding_spread(&self) -> Option<PyMarginFundingSpread> {
        self.inner
            .margin_funding_spread
            .clone()
            .map(|inner| PyMarginFundingSpread { inner })
    }

    /// Effective funding benefit spread in basis points.
    fn effective_benefit_bp(&self) -> f64 {
        self.inner.effective_benefit_bp()
    }

    fn __repr__(&self) -> String {
        format!(
            "FundingConfig(funding_spread_bp={}, funding_benefit_bp={}, margin_funding_spread={:?}, im_profile={})",
            self.inner.funding_spread_bp,
            self.inner
                .funding_benefit_bp
                .map_or("None".to_string(), |v| v.to_string()),
            self.inner.margin_funding_spread,
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

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ExposureProfile")]
#[pyclass(
    name = "ExposureProfile",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyExposureProfile {
    pub(super) inner: xva::ExposureProfile,
}

impl PyExposureProfile {
    pub(crate) fn from_inner(inner: xva::ExposureProfile) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyExposureProfile {
    #[new]
    #[pyo3(signature = (**fields))]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ExposureProfile.__init__")]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let fields = fields.map_or_else(|| Ok(PyDict::new(py)), |fields| fields.copy())?;
        if let Some(diagnostics) = fields.get_item("diagnostics")?
            && let Ok(diagnostics) = diagnostics.cast::<PyExposureDiagnostics>()
        {
            fields.set_item("diagnostics", serde_to_py(py, &diagnostics.borrow().inner)?)?;
        }
        let inner =
            crate::bindings::module_utils::fields_from_py(py, Some(&fields), "ExposureProfile")?;
        Ok(Self::from_inner(inner))
    }

    #[staticmethod]
    #[pyo3(signature = (frame, *, as_of=None, currency=None, basis=None, diagnostics=None, time_col=None))]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ExposureProfile.from_dataframe")]
    fn from_dataframe(
        frame: &Bound<'_, PyAny>,
        as_of: Option<&Bound<'_, PyAny>>,
        currency: Option<&str>,
        basis: Option<&Bound<'_, PyAny>>,
        diagnostics: Option<&Bound<'_, PyAny>>,
        time_col: Option<&str>,
    ) -> PyResult<Self> {
        use crate::bindings::dataframe_utils::{
            is_dataframe_like, is_pandas_dataframe, table_column_to_pylist,
        };
        let py = frame.py();
        if !is_dataframe_like(frame)? && !frame.hasattr("attrs")? {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "expected a pandas DataFrame, Polars DataFrame, or Arrow table",
            ));
        }
        let fields = if frame.hasattr("attrs")? {
            let attributes = frame.getattr("attrs")?;
            if let Ok(metadata) = attributes.get_item("finstack_exposure") {
                metadata.cast::<PyDict>()?.copy()?
            } else {
                PyDict::new(py)
            }
        } else {
            PyDict::new(py)
        };
        if let Some(as_of) = as_of {
            fields.set_item(
                "as_of",
                crate::bindings::date_utils::extract_date_iso(as_of)?,
            )?;
        }
        if let Some(currency) = currency {
            fields.set_item("currency", currency)?;
        }
        if let Some(basis) = basis {
            fields.set_item("basis", basis)?;
        }
        if let Some(diagnostics) = diagnostics {
            fields.set_item("diagnostics", diagnostics)?;
        }
        if fields.get_item("as_of")?.is_none() || fields.get_item("currency")?.is_none() {
            return Err(crate::errors::value_error(
                "exposure DataFrame has no finstack_exposure metadata; pass as_of=... and currency=...",
            ));
        }
        let date_items = table_column_to_pylist(frame, "dates")?.ok_or_else(|| {
            crate::errors::value_error("exposure DataFrame has no 'dates' column")
        })?;
        let iso_dates: Vec<String> = date_items
            .iter()
            .map(crate::bindings::date_utils::extract_date_iso)
            .collect::<PyResult<_>>()?;
        fields.set_item("dates", iso_dates)?;
        for name in ["mtm_values", "epe", "ene"] {
            let col_items = table_column_to_pylist(frame, name)?.ok_or_else(|| {
                crate::errors::value_error(format!("exposure DataFrame has no '{name}' column"))
            })?;
            fields.set_item(name, col_items)?;
        }
        if let Some(col_name) = time_col {
            let time_items = table_column_to_pylist(frame, col_name)?.ok_or_else(|| {
                pyo3::exceptions::PyKeyError::new_err(format!(
                    "exposure DataFrame has no time_col {col_name:?}"
                ))
            })?;
            fields.set_item("times", time_items)?;
        } else if let Some(time_items) = table_column_to_pylist(frame, "times")? {
            fields.set_item("times", time_items)?;
        } else if is_pandas_dataframe(frame)? {
            fields.set_item("times", frame.getattr("index")?.call_method0("tolist")?)?;
        } else {
            return Err(crate::errors::value_error(
                "exposure DataFrame has no 'times' column; pass time_col=... or include a 'times' column",
            ));
        }
        let profile = Self::new(py, Some(&fields))?;
        profile.validate()?;
        Ok(profile)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ExposureProfile.as_of")]
    fn as_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.as_of)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ExposureProfile.currency")]
    fn currency(&self) -> String {
        self.inner.currency.to_string()
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ExposureProfile.basis")]
    fn basis<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.basis)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ExposureProfile.dates")]
    fn dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        self.inner
            .dates
            .iter()
            .map(|&date| crate::bindings::date_utils::date_to_py(py, date))
            .collect()
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ExposureProfile.diagnostics")]
    fn diagnostics(&self) -> PyExposureDiagnostics {
        PyExposureDiagnostics {
            inner: self.inner.diagnostics.clone(),
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

    #[staticmethod]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ExposureProfile.from_json")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: xva::ExposureProfile = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ExposureProfile.to_json")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ExposureProfile.validate")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ExposureProfile.times")]
    fn times(&self) -> Vec<f64> {
        self.inner.times.clone()
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ExposureProfile.mtm_values")]
    fn mtm_values(&self) -> Vec<f64> {
        self.inner.mtm_values.clone()
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ExposureProfile.epe")]
    fn epe(&self) -> Vec<f64> {
        self.inner.epe.clone()
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ExposureProfile.ene")]
    fn ene(&self) -> Vec<f64> {
        self.inner.ene.clone()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ExposureProfile.__len__")]
    fn __len__(&self) -> usize {
        self.inner.times.len()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ExposureProfile.to_pandas")]
    fn to_pandas<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let table_py = Bound::new(py, self.to_arrow_table()?)?;
        let frame = PyArrowTable::to_pandas(&table_py, py)?;
        let frame = frame.call_method1("set_index", ("times",))?;
        let metadata = PyDict::new(py);
        metadata.set_item("as_of", self.inner.as_of.to_string())?;
        metadata.set_item("currency", self.currency())?;
        metadata.set_item("basis", self.basis(py)?)?;
        metadata.set_item("diagnostics", serde_to_py(py, &self.inner.diagnostics)?)?;
        frame
            .getattr("attrs")?
            .set_item("finstack_exposure", metadata)?;
        Ok(frame)
    }

    fn __repr__(&self) -> String {
        format!(
            "ExposureProfile(as_of={}, currency={}, points={})",
            self.inner.as_of,
            self.inner.currency,
            self.inner.times.len()
        )
    }
}

impl PyExposureProfile {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .f64("times", self.inner.times.clone())?
            .date("dates", &self.inner.dates)?
            .f64("mtm_values", self.inner.mtm_values.clone())?
            .f64("epe", self.inner.epe.clone())?
            .f64("ene", self.inner.ene.clone())?
            .build()
    }
}

tabular_methods!(
    PyExposureProfile,
    "ExposureProfile",
    module = "finstack_quant.margin._xva",
    custom_pandas
);

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

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "XvaResult.mva_detail")]
    fn mva_detail(&self) -> Option<PyMvaResult> {
        self.inner
            .mva_detail
            .clone()
            .map(|inner| PyMvaResult { inner })
    }

    /// All-in adjustment = CVA - DVA + FVA + MVA.
    ///
    /// Uncomputed legs contribute zero. This is the quantity subtracted from
    /// the risk-free value of the netting set.
    #[getter]
    fn total_xva(&self) -> f64 {
        self.inner.total_xva
    }

    /// Maximum expected positive exposure across the profile, in reporting currency.
    #[getter]
    fn max_epe(&self) -> f64 {
        self.inner.max_epe
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

    /// Effective EPE profile as list of (time, value) tuples.
    #[getter]
    fn effective_epe_profile(&self) -> Vec<(f64, f64)> {
        self.inner.effective_epe_profile.clone()
    }

    /// Per-time-bucket terms of the CVA sum as a list of dicts with keys
    /// ``time``, ``discount_factor``, ``survival_probability``,
    /// ``marginal_default_probability``, ``exposure_mid``,
    /// ``discount_factor_mid``, ``survival_weight``, ``loss_given_default``
    /// and ``contribution``; the contributions sum to ``cva``.
    #[getter]
    fn cva_rows<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.cva_rows)
    }

    /// Per-time-bucket terms of the DVA sum (same keys as ``cva_rows``), or
    /// ``None`` when DVA was not computed.
    #[getter]
    fn dva_rows<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.dva_rows)
    }

    /// Per-time-bucket terms of the FVA sum as a list of dicts with keys
    /// ``time``, ``dt``, ``discount_factor``, ``epe_mid``, ``ene_mid``,
    /// ``discount_factor_mid``, ``joint_survival_mid``, ``funding_spread``,
    /// ``funding_benefit_spread`` and ``contribution``, or ``None`` when FVA
    /// was not computed.
    #[getter]
    fn fva_rows<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.fva_rows)
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
    /// ``max_epe``, ``effective_epe`` — all in the netting set's currency
    /// units, matching the getters of the same name. Uncomputed legs
    /// (``dva`` / ``fva`` / ``mva``) are ``NaN`` rather than absent, so the
    /// frame keeps its schema across netting sets.
    ///
    /// This is the default export; one row per result, so a portfolio of
    /// netting sets stacks with
    /// ``pd.concat([r.to_pandas() for r in results])``. The time-indexed
    /// exposure profiles are a separate table — see ``to_profiles_table``.
    /// Export exposure profiles as an :class:`~finstack_quant.core.table.ArrowTable`.
    ///
    /// Columns: ``times`` (year fractions), ``epe``, ``ene``, ``effective_epe``.
    fn to_profiles_table(&self) -> PyResult<PyArrowTable> {
        let (times, epe_vals): (Vec<f64>, Vec<f64>) =
            self.inner.epe_profile.iter().copied().unzip();
        let (_, ene_vals): (Vec<f64>, Vec<f64>) = self.inner.ene_profile.iter().copied().unzip();
        let (_, eff_epe_vals): (Vec<f64>, Vec<f64>) =
            self.inner.effective_epe_profile.iter().copied().unzip();

        TableBuilder::new()
            .f64("times", times)?
            .f64("epe", epe_vals)?
            .f64("ene", ene_vals)?
            .f64("effective_epe", eff_epe_vals)?
            .build()
    }

    fn __repr__(&self) -> String {
        fn opt(value: Option<f64>) -> String {
            value.map_or("None".to_string(), |v| format!("{v:.4}"))
        }
        format!(
            "XvaResult(cva={:.4}, dva={}, fva={}, mva={}, total_xva={:.4}, max_epe={:.2})",
            self.inner.cva,
            opt(self.inner.dva),
            opt(self.inner.fva),
            opt(self.inner.mva),
            self.inner.total_xva,
            self.inner.max_epe
        )
    }
}

impl PyXvaResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let row = serde_json::json!({
            "cva": self.inner.cva,
            "dva": self.inner.dva,
            "fva": self.inner.fva,
            "mva": self.inner.mva,
            "total_xva": self.inner.total_xva,
            "max_epe": self.inner.max_epe,
            "effective_epe": self.inner.effective_epe,
        });
        serde_object_to_single_row_arrow_table(
            &row,
            &[
                ("cva", "float64"),
                ("dva", "float64"),
                ("fva", "float64"),
                ("mva", "float64"),
                ("total_xva", "float64"),
                ("max_epe", "float64"),
                ("effective_epe", "float64"),
            ],
        )
    }
}

tabular_methods!(PyXvaResult);

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
        let inner = mva::ImDecayProfile::linear_to_maturity(maturity_years).map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// IM decays like sqrt of remaining time to ``maturity_years``.
    #[staticmethod]
    fn sqrt_time(maturity_years: f64) -> PyResult<Self> {
        let inner = mva::ImDecayProfile::sqrt_time(maturity_years).map_err(core_to_py)?;
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

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ImProjectionSpec")]
#[pyclass(
    name = "ImProjectionSpec",
    module = "finstack_quant.margin",
    frozen,
    skip_from_py_object
)]
struct PyImProjectionSpec {
    inner: mva::ImProjectionSpec,
}

#[pymethods]
impl PyImProjectionSpec {
    #[new]
    #[pyo3(signature = (*, as_of, currency, netting_set, scope_source, dates, decay))]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ImProjectionSpec.__init__")]
    fn new(
        as_of: &Bound<'_, PyAny>,
        currency: &str,
        netting_set: &super::types::PyNettingSetId,
        scope_source: String,
        dates: Vec<Bound<'_, PyAny>>,
        decay: &PyImDecayProfile,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: mva::ImProjectionSpec {
                as_of: py_to_date(as_of)?,
                currency: parse_currency(currency)?,
                netting_set: netting_set.inner.clone(),
                scope_source,
                dates: dates.iter().map(py_to_date).collect::<PyResult<Vec<_>>>()?,
                decay: decay.inner.clone(),
            },
        })
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "MarginFundingSpread")]
#[pyclass(
    name = "MarginFundingSpread",
    module = "finstack_quant.margin",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
struct PyMarginFundingSpread {
    inner: mva::MarginFundingSpread,
}

#[pymethods]
impl PyMarginFundingSpread {
    #[staticmethod]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "MarginFundingSpread.flat")]
    fn flat(spread_bp: f64) -> Self {
        Self {
            inner: mva::MarginFundingSpread::Flat { spread_bp },
        }
    }

    #[staticmethod]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "MarginFundingSpread.dated")]
    fn dated(knots: &Bound<'_, PyAny>) -> PyResult<Self> {
        let knots = super::frame::dated_pairs_from_series_or_list(knots)?;
        Ok(Self {
            inner: mva::MarginFundingSpread::Dated { knots },
        })
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
    #[new]
    #[pyo3(signature = (*, as_of, currency, netting_set, source_id, dates, im_values))]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ImProfile.__init__")]
    fn new(
        as_of: &Bound<'_, PyAny>,
        currency: &str,
        netting_set: &super::types::PyNettingSetId,
        source_id: String,
        dates: Vec<Bound<'_, PyAny>>,
        im_values: Vec<f64>,
    ) -> PyResult<Self> {
        let inner = mva::ImProfile {
            as_of: py_to_date(as_of)?,
            currency: parse_currency(currency)?,
            basis: mva::ImProjectionBasis::External {
                source_id,
                netting_set: netting_set.inner.clone(),
            },
            dates: dates.iter().map(py_to_date).collect::<PyResult<Vec<_>>>()?,
            im_values,
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
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
        inner.validate().map_err(core_to_py)?;
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

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ImProfile.as_of")]
    fn as_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.as_of)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ImProfile.currency")]
    fn currency(&self) -> String {
        self.inner.currency.to_string()
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ImProfile.basis")]
    fn basis<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.basis)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "ImProfile.dates")]
    fn dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        self.inner
            .dates
            .iter()
            .map(|&date| date_to_py(py, date))
            .collect()
    }

    /// Time points in years.
    #[getter]
    fn times(&self) -> Vec<f64> {
        self.inner.times()
    }

    /// Expected IM at each time point.
    #[getter]
    fn im_values(&self) -> Vec<f64> {
        self.inner.im_values.clone()
    }

    /// Number of time points.
    fn __len__(&self) -> usize {
        self.inner.dates.len()
    }

    /// Export as a pandas ``DataFrame`` with time (years) as index.
    ///
    /// Columns: ``date`` and ``im``.
    fn to_pandas<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let table_py = Bound::new(py, self.to_arrow_table()?)?;
        let frame = PyArrowTable::to_pandas(&table_py, py)?;
        frame.call_method1("set_index", ("times",))
    }

    fn __repr__(&self) -> String {
        format!("ImProfile(points={})", self.inner.dates.len())
    }
}

impl PyImProfile {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .f64("times", self.inner.times())?
            .date("date", &self.inner.dates)?
            .f64("im", self.inner.im_values.clone())?
            .build()
    }
}

tabular_methods!(PyImProfile, custom_pandas);

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

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "MvaResult.im_profile")]
    fn im_profile(&self) -> PyImProfile {
        PyImProfile {
            inner: self.inner.im_profile.clone(),
        }
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "MvaResult.intervals")]
    fn intervals<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.intervals)
    }

    /// Export the observation profile with dates and IM as a pandas ``DataFrame`` indexed by time in years.
    fn to_pandas<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.im_profile().to_pandas(py)
    }

    fn __repr__(&self) -> String {
        format!(
            "MvaResult(mva={:.4}, average_im={:.2})",
            self.inner.mva, self.inner.average_im
        )
    }
}

impl PyMvaResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        self.im_profile().to_arrow_table()
    }
}

tabular_methods!(PyMvaResult, custom_pandas);

#[pyfunction]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "im_profile_from_simm")]
fn im_profile_from_simm(
    calculator: &PySimmCalculator,
    sensitivities: &PySimmSensitivities,
    spec: &PyImProjectionSpec,
) -> PyResult<PyImProfile> {
    let inner = mva::im_profile_from_simm(&calculator.inner, &sensitivities.inner, &spec.inner)
        .map_err(core_to_py)?;
    Ok(PyImProfile { inner })
}

#[pyfunction]
#[pyo3(signature = (im_profile, funding_spread, discount_curve, own_survival))]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.margin._xva", "compute_mva")]
fn compute_mva(
    im_profile: &PyImProfile,
    funding_spread: &PyMarginFundingSpread,
    discount_curve: &PyDiscountCurve,
    own_survival: Option<&PyHazardCurve>,
) -> PyResult<PyMvaResult> {
    let inner = mva::compute_mva(
        &im_profile.inner,
        &funding_spread.inner,
        &discount_curve.inner,
        own_survival.map(|curve| curve.inner.as_ref()),
    )
    .map_err(core_to_py)?;
    Ok(PyMvaResult { inner })
}

/// Compute bilateral XVA: CVA, DVA, FVA, MVA, and the all-in adjustment.
///
/// All legs are weighted by joint (first-to-default) survival. MVA is computed
/// only when ``funding`` carries an ``im_profile``. Bilateral DVA uses the
/// supplied ENE measured under the elected collateral and close-out terms.
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
    m.add_class::<PyImProjectionSpec>()?;
    m.add_class::<PyMarginFundingSpread>()?;
    m.add_class::<PyImProfile>()?;
    m.add_class::<PyMvaResult>()?;
    m.add_function(pyo3::wrap_pyfunction!(im_profile_from_simm, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(compute_mva, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(compute_bilateral_xva, m)?)?;
    Ok(())
}
