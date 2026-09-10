//! Python bindings for the credit factor hierarchy.
//!
//! Exposes [`PyCreditFactorModel`], [`PyCreditCalibrator`], the free functions
//! [`decompose_levels`] and [`decompose_period`], and
//! [`PyFactorCovarianceForecast`] which wraps the vol-forecast engine from
//! `finstack-quant-models`.

use std::collections::BTreeMap;

use numpy::{PyArray2, PyArrayMethods, PyReadonlyArray2, PyUntypedArrayMethods};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};

use finstack_quant_core::types::IssuerId;
use finstack_quant_models::factor::credit::calibration::{
    CreditCalibrationConfig, CreditCalibrationInputs, GenericFactorSeries, HistoryPanel,
    IssuerTagPanel,
};
use finstack_quant_models::factor::credit::hierarchy::{
    CreditFactorModel, GenericFactorSpec, IssuerTags,
};
use finstack_quant_models::factor::{FactorCovarianceMatrix, FactorId, FactorModelConfig};

use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::module_utils::py_to_json_value;
use crate::bindings::pandas_utils::{
    dict_to_dataframe, labeled_values_to_series, serde_rows_to_dataframe_with_schema, serde_to_py,
    ColumnSchema,
};
use crate::bindings::pickle_support::reduce_via_json;
use crate::bindings::portfolio::factor_model::config::extract_vol_horizon;
use crate::errors::{core_to_py, decomposition_error_to_py, serde_json_to_py, value_error};

/// Column schema of `PyLevelsAtDate::to_dataframe`, kept so a level-free
/// snapshot still exports the documented columns.
const LEVEL_VALUE_COLUMNS: &[ColumnSchema<'static>] = &[
    ("date", "str"),
    ("level_index", "int64"),
    ("dimension", "str"),
    ("bucket", "str"),
    ("value", "float64"),
];

/// Column schema of `PyPeriodDecomposition::to_level_dataframe`, kept so a
/// level-free decomposition still exports the documented columns.
const LEVEL_DELTA_COLUMNS: &[ColumnSchema<'static>] = &[
    ("from_date", "str"),
    ("to_date", "str"),
    ("level_index", "int64"),
    ("dimension", "str"),
    ("bucket", "str"),
    ("delta", "float64"),
];

/// Column schema of `PyPeriodDecomposition::to_adder_dataframe`, kept so a
/// decomposition with no shared issuers still exports the documented columns.
const ADDER_DELTA_COLUMNS: &[ColumnSchema<'static>] = &[
    ("from_date", "str"),
    ("to_date", "str"),
    ("issuer_id", "str"),
    ("d_adder", "float64"),
];

/// Column schema of `PyCreditFactorModel::to_dataframe`, kept so a model with
/// no issuer rows still exports the documented columns.
const ISSUER_ROW_COLUMNS: &[ColumnSchema<'static>] = &[
    ("issuer_id", "str"),
    ("tags", "object"),
    ("mode", "str"),
    ("beta_pc", "float64"),
    ("beta_levels", "object"),
    ("adder_at_anchor", "float64"),
    ("adder_vol_annualized", "float64"),
    ("adder_vol_source", "str"),
    ("r_squared", "float64"),
    ("n_obs", "float64"),
    ("spread_duration", "float64"),
];

/// Display label for a hierarchy dimension, matching
/// `PyCreditFactorModel::level_names` so the two line up on a join.
fn dimension_label(
    dim: &finstack_quant_models::factor::credit::hierarchy::HierarchyDimension,
) -> String {
    use finstack_quant_models::factor::credit::hierarchy::HierarchyDimension;
    match dim {
        HierarchyDimension::Rating => "Rating".to_owned(),
        HierarchyDimension::Region => "Region".to_owned(),
        HierarchyDimension::Sector => "Sector".to_owned(),
        HierarchyDimension::Custom(name) => name.clone(),
        _ => "Unknown".to_owned(),
    }
}

/// Serde label (snake_case string) of a unit-variant enum.
fn label<T: serde::Serialize>(value: &T) -> PyResult<String> {
    finstack_quant_core::wire::serde_label(value).map_err(core_to_py)
}

/// Deserialize `obj` into `T`, accepting a JSON string, a mapping / list, or
/// any object exposing ``to_dict()`` (``pandas.Series`` / ``DataFrame``).
fn py_to_serde_any<'py, T: serde::de::DeserializeOwned>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
    label: &str,
) -> PyResult<T> {
    let value = if !obj.is_instance_of::<PyDict>()
        && obj.extract::<std::borrow::Cow<'_, str>>().is_err()
        && obj.hasattr("to_dict")?
    {
        let mapping = obj.call_method0("to_dict")?;
        py_to_json_value(py, &mapping, label)?
    } else {
        py_to_json_value(py, obj, label)?
    };
    serde_json::from_value(value).map_err(|e| serde_json_to_py(e, &format!("invalid {label}")))
}

/// Calibrated credit factor hierarchy artifact.
///
/// Produced by ``CreditCalibrator`` or loaded from JSON via ``from_json``.
/// All fields are read-only; mutations require re-calibrating.
///
/// Example:
///     >>> from finstack_quant.models.factor.credit import CreditCalibrator, CreditFactorModel
///     >>> config = {"hierarchy": {"levels": []}, "covariance_strategy": "diagonal",
///     ...           "bucket_weighting": "equal"}
///     >>> inputs = {"history_panel": {"dates": ["2024-01-01", "2024-02-01"],
///     ...                             "spreads": {"A": [0.010, 0.0101]}},
///     ...           "issuer_tags": {"tags": {"A": {}}},
///     ...           "generic_factor": {"spec": {"name": "G", "series_id": "G"},
///     ...                              "values": [0.010, 0.0101]},
///     ...           "as_of": "2024-02-01", "as_of_spreads": {"A": 0.0101},
///     ...           "idiosyncratic_overrides": {}}
///     >>> calibrated = CreditCalibrator(config).calibrate(inputs)
///     >>> CreditFactorModel.from_json(calibrated.to_json()).schema
///     'finstack_quant.credit_factor_model/1'
#[pyclass(
    name = "CreditFactorModel",
    module = "finstack_quant.models.factor.credit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyCreditFactorModel {
    pub(crate) inner: CreditFactorModel,
}

impl PyCreditFactorModel {
    pub(crate) fn from_inner(inner: CreditFactorModel) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCreditFactorModel {
    /// Support pickle through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a ``CreditFactorModel`` from JSON.
    ///
    /// Validates the required ``schema`` marker and all structural constraints.
    ///
    /// Args:
    ///     json: JSON string produced by ``to_json`` or the offline calibrator.
    ///
    /// Raises:
    ///     ValueError: If the JSON is malformed or fails validation.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: CreditFactorModel = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid CreditFactorModel JSON"))?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize this model to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "cannot serialize CreditFactorModel"))
    }

    /// Namespaced schema marker (``"finstack_quant.credit_factor_model/1"``).
    #[getter]
    fn schema(&self) -> &'static str {
        self.inner.schema.as_str()
    }

    /// Calibration anchor date (ISO 8601 string).
    #[getter]
    fn as_of(&self) -> String {
        self.inner.as_of.to_string()
    }

    /// History window consumed by calibration as ``(start, end)``
    /// ``datetime.date`` values (both inclusive).
    #[getter]
    fn calibration_window<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let start = date_to_py(py, self.inner.calibration_window.start)?;
        let end = date_to_py(py, self.inner.calibration_window.end)?;
        PyTuple::new(py, [start, end])
    }

    /// Issuer-beta policy used during calibration (serde label, e.g.
    /// ``"globally_off"``).
    #[getter]
    fn policy(&self) -> PyResult<String> {
        label(&self.inner.policy)
    }

    /// Panel observation frequency (``"daily"``, ``"monthly"`` or
    /// ``"quarterly"``) that fixed the annualization factor.
    #[getter]
    fn panel_frequency(&self) -> PyResult<String> {
        label(&self.inner.panel_frequency)
    }

    /// Bucket-mean weighting used at calibration (``"equal"`` or ``"dts"``).
    #[getter]
    fn bucket_weighting(&self) -> PyResult<String> {
        label(&self.inner.bucket_weighting)
    }

    /// Point-in-time factor-model configuration (factors, covariance,
    /// matching) embedded in the artifact.
    #[getter]
    fn config(&self) -> PyFactorModelConfig {
        PyFactorModelConfig::from_inner(self.inner.config.clone())
    }

    /// Point-in-time factor covariance matrix (``config.covariance``).
    #[getter]
    fn covariance(&self) -> PyFactorCovarianceMatrix {
        PyFactorCovarianceMatrix::from_inner(self.inner.config.covariance.clone())
    }

    /// Structured calibration diagnostics (``mode_counts``,
    /// ``bucket_sizes_per_level``, ``fold_ups``, ...) as a dict.
    #[getter]
    fn diagnostics<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.diagnostics)
    }

    /// Static factor correlation matrix ``rho`` as a dict with
    /// ``factor_ids`` and nested-list ``data``.
    #[getter]
    fn static_correlation<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.static_correlation)
    }

    /// Number of hierarchy levels (broadest → narrowest).
    #[getter]
    fn n_levels(&self) -> usize {
        self.inner.hierarchy.levels.len()
    }

    /// Number of issuer beta rows in the artifact.
    #[getter]
    fn n_issuers(&self) -> usize {
        self.inner.issuer_betas.len()
    }

    /// Number of factors in the model configuration.
    #[getter]
    fn n_factors(&self) -> usize {
        self.inner.config.factors.len()
    }

    /// Hierarchy level names as a list of strings.
    ///
    /// Returns:
    ///     List of dimension names (e.g. ``["Rating", "Region", "Sector"]``).
    fn level_names(&self) -> Vec<String> {
        self.inner
            .hierarchy
            .levels
            .iter()
            .map(dimension_label)
            .collect()
    }

    /// Issuer IDs present in the artifact.
    fn issuer_ids(&self) -> Vec<String> {
        self.inner
            .issuer_betas
            .iter()
            .map(|row| row.issuer_id.as_str().to_owned())
            .collect()
    }

    /// Factor IDs in the model configuration.
    fn factor_ids(&self) -> Vec<String> {
        self.inner
            .config
            .factors
            .iter()
            .map(|f| f.id.to_string())
            .collect()
    }

    /// Export the per-issuer beta rows as a pandas ``DataFrame``.
    ///
    /// One row per issuer, sorted by ``issuer_id``. Columns: ``issuer_id``,
    /// ``tags`` (dict of dimension key to bucket tag), ``mode``
    /// (``"issuer_beta"`` / ``"bucket_only"``), ``beta_pc``, ``beta_levels``
    /// (list aligned with ``level_names()``; ``0.0`` marks a folded level),
    /// ``adder_at_anchor`` (bp), ``adder_vol_annualized`` (bp per square-root year), ``adder_vol_source``,
    /// ``r_squared`` and ``n_obs`` (``NaN`` for bucket-only rows), and
    /// ``spread_duration`` (years).
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut rows: Vec<serde_json::Value> = Vec::with_capacity(self.inner.issuer_betas.len());
        for row in &self.inner.issuer_betas {
            let fit = row.fit_quality.as_ref();
            rows.push(serde_json::json!({
                "issuer_id": row.issuer_id.as_str(),
                "tags": row.tags,
                "mode": label(&row.mode)?,
                "beta_pc": row.betas.pc,
                "beta_levels": row.betas.levels,
                "adder_at_anchor": row.adder_at_anchor,
                "adder_vol_annualized": row.adder_vol_annualized,
                "adder_vol_source": label(&row.adder_vol_source)?,
                "r_squared": fit.map(|f| f.r_squared),
                "n_obs": fit.map(|f| f.n_obs),
                "spread_duration": row.spread_duration,
            }));
        }
        serde_rows_to_dataframe_with_schema(py, &rows, ISSUER_ROW_COLUMNS)
    }

    fn __repr__(&self) -> String {
        format!(
            "CreditFactorModel(as_of={:?}, n_levels={}, n_issuers={}, n_factors={})",
            self.inner.as_of.to_string(),
            self.inner.hierarchy.levels.len(),
            self.inner.issuer_betas.len(),
            self.inner.config.factors.len(),
        )
    }
}

/// Deterministic calibrator that produces a ``CreditFactorModel``.
///
/// The configuration is a ``CreditCalibrationConfig`` given as a dict or a
/// JSON string; every field has a default, so a partial dict such as
/// ``{"hierarchy": {"levels": ["rating"]}}`` is accepted. Pass ``None`` for
/// the all-defaults configuration.
///
/// Example:
///     >>> from finstack_quant.models.factor.credit import CreditCalibrator
///     >>> config = {"hierarchy": {"levels": []}, "covariance_strategy": "diagonal",
///     ...           "bucket_weighting": "equal"}
///     >>> inputs = {"history_panel": {"dates": ["2024-01-01", "2024-02-01"],
///     ...                             "spreads": {"A": [0.010, 0.0101]}},
///     ...           "issuer_tags": {"tags": {"A": {}}},
///     ...           "generic_factor": {"spec": {"name": "G", "series_id": "G"},
///     ...                              "values": [0.010, 0.0101]},
///     ...           "as_of": "2024-02-01", "as_of_spreads": {"A": 0.0101},
///     ...           "idiosyncratic_overrides": {}}
///     >>> CreditCalibrator(config).calibrate(inputs).n_issuers
///     1
#[pyclass(
    name = "CreditCalibrator",
    module = "finstack_quant.models.factor.credit",
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyCreditCalibrator {
    inner: finstack_quant_models::factor::credit::calibration::CreditCalibrator,
}

/// Parse an optional calibration config from a dict / JSON string.
fn extract_calibration_config(
    py: Python<'_>,
    config: Option<&Bound<'_, PyAny>>,
) -> PyResult<CreditCalibrationConfig> {
    match config {
        None => Ok(CreditCalibrationConfig::default()),
        Some(obj) if obj.is_none() => Ok(CreditCalibrationConfig::default()),
        Some(obj) => py_to_serde_any(py, obj, "CreditCalibrationConfig"),
    }
}

/// Build a `CreditCalibrationInputs` from pandas objects.
fn inputs_from_dataframe(
    py: Python<'_>,
    spreads: &Bound<'_, PyAny>,
    tags: &Bound<'_, PyAny>,
    generic: &Bound<'_, PyAny>,
    as_of: Option<&Bound<'_, PyAny>>,
    spread_durations: Option<&Bound<'_, PyAny>>,
) -> PyResult<CreditCalibrationInputs> {
    let index = spreads.getattr("index")?;
    let dates = index
        .try_iter()?
        .map(|d| extract_date(&d?))
        .collect::<PyResult<Vec<_>>>()?;
    if dates.is_empty() {
        return Err(value_error("spreads DataFrame has no rows"));
    }
    let columns = spreads.getattr("columns")?;
    let issuers = columns
        .try_iter()?
        .map(|c| c?.str()?.extract::<String>())
        .collect::<PyResult<Vec<_>>>()?;

    let mut panel: BTreeMap<IssuerId, Vec<Option<f64>>> = BTreeMap::new();
    for issuer in &issuers {
        let series = spreads.get_item(issuer.as_str())?.call_method0("tolist")?;
        let values: Vec<Option<f64>> = series
            .try_iter()?
            .map(|v| {
                let v = v?;
                if v.is_none() {
                    return Ok(None);
                }
                let f: f64 = v.extract()?;
                Ok(if f.is_nan() { None } else { Some(f) })
            })
            .collect::<PyResult<_>>()?;
        if values.len() != dates.len() {
            return Err(value_error(format!(
                "spreads column {issuer:?} has {} values for {} dates",
                values.len(),
                dates.len()
            )));
        }
        panel.insert(IssuerId::new(issuer.clone()), values);
    }

    let as_of = match as_of {
        Some(obj) if !obj.is_none() => extract_date(obj)?,
        _ => dates[dates.len() - 1],
    };
    let as_of_index = dates.iter().position(|d| *d == as_of).ok_or_else(|| {
        value_error(format!(
            "as_of {as_of} is not one of the spreads DataFrame index dates"
        ))
    })?;
    let mut as_of_spreads: BTreeMap<IssuerId, f64> = BTreeMap::new();
    for (issuer, values) in &panel {
        if let Some(v) = values[as_of_index] {
            as_of_spreads.insert(issuer.clone(), v);
        }
    }

    let tag_map: BTreeMap<IssuerId, IssuerTags> = {
        let obj = if tags.hasattr("to_dict")? && !tags.is_instance_of::<PyDict>() {
            let kwargs = PyDict::new(py);
            kwargs.set_item("orient", "index")?;
            tags.call_method("to_dict", (), Some(&kwargs))?
        } else {
            tags.clone()
        };
        let value = py_to_json_value(py, &obj, "issuer tags")?;
        serde_json::from_value(value).map_err(|e| serde_json_to_py(e, "invalid issuer tags"))?
    };

    let (name, values): (String, Vec<f64>) = if generic.hasattr("tolist")? {
        let name = generic
            .getattr("name")
            .ok()
            .filter(|n| !n.is_none())
            .map(|n| n.str().and_then(|s| s.extract::<String>()))
            .transpose()?
            .unwrap_or_else(|| "generic".to_owned());
        (name, generic.call_method0("tolist")?.extract()?)
    } else {
        ("generic".to_owned(), generic.extract()?)
    };
    if values.len() != dates.len() {
        return Err(value_error(format!(
            "generic series has {} values for {} dates",
            values.len(),
            dates.len()
        )));
    }

    let spread_durations: BTreeMap<IssuerId, f64> = match spread_durations {
        Some(obj) if !obj.is_none() => py_to_serde_any(py, obj, "spread_durations")?,
        _ => BTreeMap::new(),
    };

    Ok(CreditCalibrationInputs {
        history_panel: HistoryPanel {
            dates,
            spreads: panel,
        },
        issuer_tags: IssuerTagPanel { tags: tag_map },
        generic_factor: GenericFactorSeries {
            spec: GenericFactorSpec {
                name: name.clone(),
                series_id: name,
            },
            values,
        },
        as_of,
        as_of_spreads,
        idiosyncratic_overrides: BTreeMap::new(),
        spread_durations,
    })
}

#[pymethods]
impl PyCreditCalibrator {
    /// Construct a calibrator from a ``CreditCalibrationConfig``.
    ///
    /// Args:
    ///     config: Dict or JSON string of a ``CreditCalibrationConfig``; omitted
    ///         fields take their defaults. ``None`` selects the all-defaults
    ///         configuration (``policy="globally_off"``, empty hierarchy,
    ///         ``covariance_strategy="full_sample_repaired"``,
    ///         ``bucket_weighting="dts"``, monthly panel).
    ///
    /// Raises:
    ///     ValueError: If ``config`` is not a valid ``CreditCalibrationConfig``
    ///         (unknown field, bad enum label).
    #[new]
    #[pyo3(signature = (config = None))]
    fn new(py: Python<'_>, config: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let config = extract_calibration_config(py, config)?;
        Ok(Self {
            inner: finstack_quant_models::factor::credit::calibration::CreditCalibrator::new(
                config,
            ),
        })
    }

    /// Run the full calibration pipeline and return a ``CreditFactorModel``.
    ///
    /// Args:
    ///     inputs: Dict or JSON string of a ``CreditCalibrationInputs`` object
    ///         (``history_panel`` with decimal spreads, ``issuer_tags``,
    ///         ``generic_factor``, ``as_of``, ``as_of_spreads``, optional
    ///         ``idiosyncratic_overrides`` / ``spread_durations``). Overrides are decimal
    ///         spread volatility per square-root year (``0.001`` = 10 bp/√year).
    ///
    /// Returns:
    ///     Calibrated ``CreditFactorModel`` artifact.
    ///
    /// Raises:
    ///     ValueError: If ``inputs`` is structurally invalid or calibration
    ///         rejects the panel.
    fn calibrate(
        &self,
        py: Python<'_>,
        inputs: &Bound<'_, PyAny>,
    ) -> PyResult<PyCreditFactorModel> {
        let inputs: CreditCalibrationInputs =
            py_to_serde_any(py, inputs, "CreditCalibrationInputs")?;
        let model = py
            .detach(|| self.inner.calibrate(inputs))
            .map_err(core_to_py)?;
        Ok(PyCreditFactorModel::from_inner(model))
    }

    /// Calibrate straight from pandas objects.
    ///
    /// Builds the ``CreditCalibrationInputs`` from the frames (pure
    /// conversion) and runs ``calibrate`` under ``config``.
    ///
    /// Args:
    ///     spreads: ``pandas.DataFrame`` of decimal spreads (``0.01`` = 100 bp)
    ///         with a date index (sorted, regular grid) and one column per
    ///         issuer; ``NaN`` marks a gap.
    ///     tags: Issuer tags — a mapping ``{issuer: {dimension_key: tag}}`` or
    ///         a ``pandas.DataFrame`` indexed by issuer with one column per
    ///         hierarchy dimension (``"rating"``, ``"region"``, ...).
    ///     generic: Generic (PC) factor series aligned with ``spreads.index`` —
    ///         a ``pandas.Series`` (its ``name`` becomes the factor name) or a
    ///         list of decimal values.
    ///     as_of: Anchor date (date-like or ISO string); defaults to the last
    ///         index date.
    ///     spread_durations: Optional ``{issuer: years}`` mapping or
    ///         ``pandas.Series``; required when ``bucket_weighting="dts"``.
    ///     config: ``CreditCalibrationConfig`` dict / JSON string / ``None``
    ///         (see ``CreditCalibrator.__init__``).
    ///
    /// Returns:
    ///     Calibrated ``CreditFactorModel``.
    ///
    /// Raises:
    ///     ValueError: If the frames are misaligned, ``as_of`` is not an index
    ///         date, or calibration rejects the inputs.
    #[staticmethod]
    #[pyo3(signature = (spreads, tags, generic, as_of = None, spread_durations = None, config = None))]
    fn from_dataframe(
        py: Python<'_>,
        spreads: &Bound<'_, PyAny>,
        tags: &Bound<'_, PyAny>,
        generic: &Bound<'_, PyAny>,
        as_of: Option<&Bound<'_, PyAny>>,
        spread_durations: Option<&Bound<'_, PyAny>>,
        config: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyCreditFactorModel> {
        let config = extract_calibration_config(py, config)?;
        let inputs = inputs_from_dataframe(py, spreads, tags, generic, as_of, spread_durations)?;
        let calibrator =
            finstack_quant_models::factor::credit::calibration::CreditCalibrator::new(config);
        let model = py
            .detach(|| calibrator.calibrate(inputs))
            .map_err(core_to_py)?;
        Ok(PyCreditFactorModel::from_inner(model))
    }

    /// The calibration configuration as a dict (canonical serde fields).
    #[getter]
    fn config<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, self.inner.config())
    }

    fn __repr__(&self) -> String {
        let config = self.inner.config();
        format!(
            "CreditCalibrator(policy={:?}, n_levels={}, vol_model={:?}, covariance_strategy={:?}, bucket_weighting={:?})",
            label(&config.policy).unwrap_or_default(),
            config.hierarchy.levels.len(),
            label(&config.vol_model).unwrap_or_default(),
            label(&config.covariance_strategy).unwrap_or_default(),
            label(&config.bucket_weighting).unwrap_or_default(),
        )
    }
}

/// Snapshot of all hierarchy-level factor values at a single date.
///
/// Produced by ``decompose_levels``. Carry this into ``decompose_period`` to
/// compute period-over-period changes. Values are in basis points.
///
/// Example:
///     >>> from finstack_quant.models.factor.credit import CreditCalibrator, decompose_levels
///     >>> config = {"hierarchy": {"levels": []}, "covariance_strategy": "diagonal",
///     ...           "bucket_weighting": "equal"}
///     >>> inputs = {"history_panel": {"dates": ["2024-01-01", "2024-02-01"],
///     ...                             "spreads": {"A": [0.010, 0.0101]}},
///     ...           "issuer_tags": {"tags": {"A": {}}},
///     ...           "generic_factor": {"spec": {"name": "G", "series_id": "G"},
///     ...                              "values": [0.010, 0.0101]},
///     ...           "as_of": "2024-02-01", "as_of_spreads": {"A": 0.0101},
///     ...           "idiosyncratic_overrides": {}}
///     >>> model = CreditCalibrator(config).calibrate(inputs)
///     >>> levels = decompose_levels(model, {"A": 0.0105}, 0.010, "2024-03-01")
///     >>> (levels.date, levels.generic, levels.adder())
///     ('2024-03-01', 100.0, {'A': 5.0})
#[pyclass(
    name = "LevelsAtDate",
    module = "finstack_quant.models.factor.credit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyLevelsAtDate {
    inner: finstack_quant_models::factor::credit::decomposition::LevelsAtDate,
}

impl PyLevelsAtDate {
    fn from_inner(
        inner: finstack_quant_models::factor::credit::decomposition::LevelsAtDate,
    ) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyLevelsAtDate {
    /// Deserialize a factor-level snapshot from canonical JSON.
    ///
    /// Raises:
    ///     ValueError: If the JSON is malformed or fails validation.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_models::factor::credit::decomposition::LevelsAtDate =
            serde_json::from_str(json)
                .map_err(|e| serde_json_to_py(e, "invalid LevelsAtDate JSON"))?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize the snapshot to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        self.inner.validate().map_err(core_to_py)?;
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "cannot serialize LevelsAtDate"))
    }

    /// Support pickle through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    /// Observation date (ISO 8601 string).
    #[getter]
    fn date(&self) -> String {
        self.inner.date.to_string()
    }

    /// Generic (PC) factor value at this date, in basis points.
    #[getter]
    fn generic(&self) -> f64 {
        self.inner.generic
    }

    /// Number of hierarchy levels.
    #[getter]
    fn n_levels(&self) -> usize {
        self.inner.by_level.len()
    }

    /// Bucket values for a given level index as a dict ``{bucket_path: value}``.
    ///
    /// Args:
    ///     level_index: Zero-based hierarchy level index.
    ///
    /// Raises:
    ///     ValueError: If ``level_index`` is out of range.
    fn level_values<'py>(
        &self,
        py: Python<'py>,
        level_index: usize,
    ) -> PyResult<Bound<'py, PyDict>> {
        let lev = self.inner.by_level.get(level_index).ok_or_else(|| {
            value_error(format!(
                "level_index {} out of range (n_levels={})",
                level_index,
                self.inner.by_level.len()
            ))
        })?;
        let d = PyDict::new(py);
        for (k, v) in &lev.values {
            d.set_item(k, v)?;
        }
        Ok(d)
    }

    /// Per-issuer residual adder after peeling all levels, as a dict
    /// ``{issuer_id: adder_bp}``.
    fn adder<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new(py);
        for (issuer, val) in &self.inner.adder {
            d.set_item(issuer.as_str(), val)?;
        }
        Ok(d)
    }

    /// Export the per-level bucket values as a pandas ``DataFrame``.
    ///
    /// Columns: ``date``, ``level_index``, ``dimension``, ``bucket``,
    /// ``value``. One row per (level, bucket) pair, ordered by ``level_index``
    /// then ``bucket``. A snapshot from a hierarchy with no levels yields a
    /// zero-row frame that still carries the columns. The scalar ``generic``
    /// factor and the per-issuer residuals are not levels; read them from the
    /// ``generic`` getter and ``to_series`` respectively.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let date = self.inner.date.to_string();
        let mut rows: Vec<serde_json::Value> = Vec::new();
        for level in &self.inner.by_level {
            let dimension = dimension_label(&level.dimension);
            for (bucket, value) in &level.values {
                rows.push(serde_json::json!({
                    "date": date,
                    "level_index": level.level_index,
                    "dimension": dimension,
                    "bucket": bucket,
                    "value": value,
                }));
            }
        }
        serde_rows_to_dataframe_with_schema(py, &rows, LEVEL_VALUE_COLUMNS)
    }

    /// Export the per-issuer residual adders as a pandas ``Series`` named
    /// ``adder`` and indexed by issuer ID (sorted), in basis points.
    #[pyo3(text_signature = "($self)")]
    fn to_series<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let labels: Vec<String> = self
            .inner
            .adder
            .keys()
            .map(|issuer| issuer.as_str().to_owned())
            .collect();
        let values: Vec<f64> = self.inner.adder.values().copied().collect();
        labeled_values_to_series(py, &labels, values, "adder")
    }

    fn __repr__(&self) -> String {
        format!(
            "LevelsAtDate(date={:?}, generic={:.4}, n_levels={}, n_issuers={})",
            self.inner.date.to_string(),
            self.inner.generic,
            self.inner.by_level.len(),
            self.inner.adder.len(),
        )
    }
}

/// Component-wise difference between two ``LevelsAtDate`` snapshots.
///
/// Produced by ``decompose_period``. Satisfies the linear reconciliation
/// invariant ``dS_i = beta_i^PC * d_generic + sum_k beta_i^k * dL_k + d_adder_i``
/// for every issuer present in both snapshots.
///
/// Example:
///     >>> from finstack_quant.models.factor.credit import CreditCalibrator, decompose_levels, decompose_period
///     >>> config = {"hierarchy": {"levels": []}, "covariance_strategy": "diagonal",
///     ...           "bucket_weighting": "equal"}
///     >>> inputs = {"history_panel": {"dates": ["2024-01-01", "2024-02-01"],
///     ...                             "spreads": {"A": [0.010, 0.0101]}},
///     ...           "issuer_tags": {"tags": {"A": {}}},
///     ...           "generic_factor": {"spec": {"name": "G", "series_id": "G"},
///     ...                              "values": [0.010, 0.0101]},
///     ...           "as_of": "2024-02-01", "as_of_spreads": {"A": 0.0101},
///     ...           "idiosyncratic_overrides": {}}
///     >>> model = CreditCalibrator(config).calibrate(inputs)
///     >>> start = decompose_levels(model, {"A": 0.0105}, 0.010, "2024-03-01")
///     >>> end = decompose_levels(model, {"A": 0.01065}, 0.01015, "2024-03-02")
///     >>> period = decompose_period(start, end)
///     >>> (period.from_date, period.to_date, period.d_generic)
///     ('2024-03-01', '2024-03-02', 1.5)
#[pyclass(
    name = "PeriodDecomposition",
    module = "finstack_quant.models.factor.credit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyPeriodDecomposition {
    inner: finstack_quant_models::factor::credit::decomposition::PeriodDecomposition,
}

impl PyPeriodDecomposition {
    fn from_inner(
        inner: finstack_quant_models::factor::credit::decomposition::PeriodDecomposition,
    ) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPeriodDecomposition {
    /// Deserialize a period decomposition from canonical JSON.
    ///
    /// Raises:
    ///     ValueError: If the JSON is malformed or fails validation.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_models::factor::credit::decomposition::PeriodDecomposition =
            serde_json::from_str(json)
                .map_err(|e| serde_json_to_py(e, "invalid PeriodDecomposition JSON"))?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize the decomposition to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        self.inner.validate().map_err(core_to_py)?;
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "cannot serialize PeriodDecomposition"))
    }

    /// Support pickle through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    /// Earlier snapshot date (ISO 8601).
    #[getter(from_date)]
    fn get_from_date(&self) -> String {
        self.inner.from.to_string()
    }

    /// Later snapshot date (ISO 8601).
    #[getter]
    fn to_date(&self) -> String {
        self.inner.to.to_string()
    }

    /// Change in the generic (PC) factor value, in basis points.
    #[getter]
    fn d_generic(&self) -> f64 {
        self.inner.d_generic
    }

    /// Number of hierarchy levels.
    #[getter]
    fn n_levels(&self) -> usize {
        self.inner.by_level.len()
    }

    /// Bucket value deltas for a given level index as a dict.
    ///
    /// Args:
    ///     level_index: Zero-based hierarchy level index.
    ///
    /// Raises:
    ///     ValueError: If ``level_index`` is out of range.
    fn level_deltas<'py>(
        &self,
        py: Python<'py>,
        level_index: usize,
    ) -> PyResult<Bound<'py, PyDict>> {
        let lev = self.inner.by_level.get(level_index).ok_or_else(|| {
            value_error(format!(
                "level_index {} out of range (n_levels={})",
                level_index,
                self.inner.by_level.len()
            ))
        })?;
        let d = PyDict::new(py);
        for (k, v) in &lev.deltas {
            d.set_item(k, v)?;
        }
        Ok(d)
    }

    /// Per-issuer adder deltas as a dict ``{issuer_id: d_adder_bp}``.
    fn d_adder<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new(py);
        for (issuer, val) in &self.inner.d_adder {
            d.set_item(issuer.as_str(), val)?;
        }
        Ok(d)
    }

    /// Export the per-level bucket deltas as a pandas ``DataFrame``
    /// (identical to ``to_level_dataframe``).
    ///
    /// Columns: ``from_date``, ``to_date``, ``level_index``, ``dimension``,
    /// ``bucket``, ``delta``. The per-issuer adder deltas are a separate
    /// table; see ``to_adder_dataframe``.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.to_level_dataframe(py)
    }

    /// Export the per-level bucket deltas as a pandas ``DataFrame``.
    ///
    /// Columns: ``from_date``, ``to_date``, ``level_index``, ``dimension``,
    /// ``bucket``, ``delta``. One row per (level, bucket) pair, ordered by
    /// ``level_index`` then ``bucket``; a level-free decomposition yields a
    /// zero-row frame that still carries the columns.
    fn to_level_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let from_date = self.inner.from.to_string();
        let to_date = self.inner.to.to_string();
        let mut rows: Vec<serde_json::Value> = Vec::new();
        for level in &self.inner.by_level {
            let dimension = dimension_label(&level.dimension);
            for (bucket, delta) in &level.deltas {
                rows.push(serde_json::json!({
                    "from_date": from_date,
                    "to_date": to_date,
                    "level_index": level.level_index,
                    "dimension": dimension,
                    "bucket": bucket,
                    "delta": delta,
                }));
            }
        }
        serde_rows_to_dataframe_with_schema(py, &rows, LEVEL_DELTA_COLUMNS)
    }

    /// Export the per-issuer adder deltas as a pandas ``DataFrame``.
    ///
    /// Columns: ``from_date``, ``to_date``, ``issuer_id``, ``d_adder``. One
    /// row per issuer, ordered by ``issuer_id``.
    fn to_adder_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let from_date = self.inner.from.to_string();
        let to_date = self.inner.to.to_string();
        let rows: Vec<serde_json::Value> = self
            .inner
            .d_adder
            .iter()
            .map(|(issuer, delta)| {
                serde_json::json!({
                    "from_date": from_date,
                    "to_date": to_date,
                    "issuer_id": issuer.as_str(),
                    "d_adder": delta,
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, ADDER_DELTA_COLUMNS)
    }

    fn __repr__(&self) -> String {
        format!(
            "PeriodDecomposition(from_date={:?}, to_date={:?}, d_generic={:.4}, n_levels={})",
            self.inner.from.to_string(),
            self.inner.to.to_string(),
            self.inner.d_generic,
            self.inner.by_level.len(),
        )
    }
}

/// Decompose observed issuer spreads at a point in time into per-level factor
/// values and per-issuer residual adders.
///
/// Args:
///     model: Calibrated ``CreditFactorModel`` artifact.
///     observed_spreads: Mapping from issuer ID to observed **decimal** spread
///         (``0.01`` = 100 bp) — a dict, a ``pandas.Series`` indexed by
///         issuer, or a JSON string of the same object.
///     observed_generic: Generic (PC) factor value at ``as_of``, decimal.
///     as_of: Valuation date, either a date-like object (``datetime.date``,
///         ``pandas.Timestamp``) or an ISO 8601 string.
///     runtime_tags: Optional ``{issuer_id: {dim_key: tag_value}}`` mapping
///         (dict, ``pandas.DataFrame`` indexed by issuer, or JSON string) for
///         issuers not present in the model.
///
/// Returns:
///     ``LevelsAtDate`` snapshot with generic value, per-level bucket values
///     and per-issuer residual adders, all in basis points.
///
/// Raises:
///     KeyError: If an issuer has no model row and no ``runtime_tags`` entry.
///     ValueError: If a spread is not a finite decimal in ``(-0.5, 2.0)``, an
///         issuer is missing a required hierarchy tag, or a DTS weight cannot
///         be formed.
///     RuntimeError: If the model artifact is internally inconsistent.
///
/// Example:
///     >>> from finstack_quant.models.factor.credit import CreditCalibrator, decompose_levels
///     >>> config = {"hierarchy": {"levels": []}, "covariance_strategy": "diagonal",
///     ...           "bucket_weighting": "equal"}
///     >>> inputs = {"history_panel": {"dates": ["2024-01-01", "2024-02-01"],
///     ...                             "spreads": {"A": [0.010, 0.0101]}},
///     ...           "issuer_tags": {"tags": {"A": {}}},
///     ...           "generic_factor": {"spec": {"name": "G", "series_id": "G"},
///     ...                              "values": [0.010, 0.0101]},
///     ...           "as_of": "2024-02-01", "as_of_spreads": {"A": 0.0101},
///     ...           "idiosyncratic_overrides": {}}
///     >>> model = CreditCalibrator(config).calibrate(inputs)
///     >>> decompose_levels(model, {"A": 0.0125}, 0.0120, "2025-06-30").generic
///     120.0
#[pyfunction]
#[pyo3(signature = (model, observed_spreads, observed_generic, as_of, runtime_tags=None))]
fn decompose_levels(
    py: Python<'_>,
    model: &PyCreditFactorModel,
    observed_spreads: &Bound<'_, PyAny>,
    observed_generic: f64,
    as_of: &Bound<'_, PyAny>,
    runtime_tags: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyLevelsAtDate> {
    let observed_spreads: BTreeMap<IssuerId, f64> =
        py_to_serde_any(py, observed_spreads, "observed_spreads")?;

    let date = extract_date(as_of)?;

    let runtime_tags: Option<BTreeMap<IssuerId, IssuerTags>> = match runtime_tags {
        Some(obj) if !obj.is_none() => {
            let obj = if obj.hasattr("to_dict")? && !obj.is_instance_of::<PyDict>() {
                let kwargs = PyDict::new(py);
                kwargs.set_item("orient", "index")?;
                obj.call_method("to_dict", (), Some(&kwargs))?
            } else {
                obj.clone()
            };
            Some(py_to_serde_any(py, &obj, "runtime_tags")?)
        }
        _ => None,
    };

    let result = finstack_quant_models::factor::credit::decomposition::decompose_levels(
        &model.inner,
        &observed_spreads,
        observed_generic,
        date,
        runtime_tags.as_ref(),
    )
    .map_err(decomposition_error_to_py)?;

    Ok(PyLevelsAtDate::from_inner(result))
}

/// Difference two ``LevelsAtDate`` snapshots component-wise.
///
/// Output buckets and issuers are restricted to those present in **both**
/// snapshots so the linear reconciliation invariant holds for every entry.
///
/// Args:
///     from_levels: Earlier ``LevelsAtDate`` snapshot.
///     to_levels: Later ``LevelsAtDate`` snapshot.
///
/// Returns:
///     ``PeriodDecomposition`` with ``d_generic``, per-level bucket deltas and
///     per-issuer adder deltas (basis points).
///
/// Raises:
///     ValueError: If ``from_levels.date > to_levels.date`` or the two
///         snapshots disagree on hierarchy depth or dimensions.
///
/// Example:
///     >>> from finstack_quant.models.factor.credit import CreditCalibrator, decompose_levels, decompose_period
///     >>> config = {"hierarchy": {"levels": []}, "covariance_strategy": "diagonal",
///     ...           "bucket_weighting": "equal"}
///     >>> inputs = {"history_panel": {"dates": ["2024-01-01", "2024-02-01"],
///     ...                             "spreads": {"A": [0.010, 0.0101]}},
///     ...           "issuer_tags": {"tags": {"A": {}}},
///     ...           "generic_factor": {"spec": {"name": "G", "series_id": "G"},
///     ...                              "values": [0.010, 0.0101]},
///     ...           "as_of": "2024-02-01", "as_of_spreads": {"A": 0.0101},
///     ...           "idiosyncratic_overrides": {}}
///     >>> model = CreditCalibrator(config).calibrate(inputs)
///     >>> start = decompose_levels(model, {"A": 0.0105}, 0.010, "2024-03-01")
///     >>> end = decompose_levels(model, {"A": 0.01065}, 0.01015, "2024-03-02")
///     >>> decompose_period(start, end).d_generic
///     1.5
#[pyfunction]
fn decompose_period(
    from_levels: &PyLevelsAtDate,
    to_levels: &PyLevelsAtDate,
) -> PyResult<PyPeriodDecomposition> {
    let result = finstack_quant_models::factor::credit::decomposition::decompose_period(
        &from_levels.inner,
        &to_levels.inner,
    )
    .map_err(decomposition_error_to_py)?;
    Ok(PyPeriodDecomposition::from_inner(result))
}

/// Validated factor covariance matrix with deterministic row-major storage.
///
/// Entries are annualized covariances in the factors' native units. The
/// constructor validates squareness, unique identifiers, symmetry and
/// positive semidefiniteness.
///
/// Example:
///     >>> from finstack_quant.models.factor.credit import FactorCovarianceMatrix
///     >>> matrix = FactorCovarianceMatrix(["a", "b"], [[0.04, 0.0], [0.0, 0.01]])
///     >>> (matrix.variance("a"), matrix.to_dataframe().loc["b", "b"])
///     (0.04, 0.01)
#[pyclass(
    name = "FactorCovarianceMatrix",
    module = "finstack_quant.models.factor.credit",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyFactorCovarianceMatrix {
    pub(crate) inner: FactorCovarianceMatrix,
}

impl PyFactorCovarianceMatrix {
    fn from_inner(inner: FactorCovarianceMatrix) -> Self {
        Self { inner }
    }
}

/// Flatten `data` (flat list, nested list, or 2-D NumPy array) into a
/// row-major buffer of `n * n` entries.
fn extract_covariance_data(data: &Bound<'_, PyAny>, n: usize) -> PyResult<Vec<f64>> {
    if let Ok(array) = data.extract::<PyReadonlyArray2<'_, f64>>() {
        let shape = array.shape();
        if shape[0] != n || shape[1] != n {
            return Err(value_error(format!(
                "data must be a {n} x {n} matrix, got shape ({}, {})",
                shape[0], shape[1]
            )));
        }
        return Ok(array.as_array().iter().copied().collect());
    }
    if let Ok(flat) = data.extract::<Vec<f64>>() {
        return Ok(flat);
    }
    let nested = data.extract::<Vec<Vec<f64>>>().map_err(|_| {
        value_error("data must be a flat list, a nested list, or a 2-D float64 NumPy array")
    })?;
    if nested.len() != n || nested.iter().any(|row| row.len() != n) {
        return Err(value_error(format!(
            "data must be a {n} x {n} nested list matching factor_ids"
        )));
    }
    Ok(nested.into_iter().flatten().collect())
}

#[pymethods]
impl PyFactorCovarianceMatrix {
    /// Build and validate a covariance matrix.
    ///
    /// Args:
    ///     factor_ids: Ordered, unique factor identifiers defining both axes.
    ///     data: Annualized covariances — a nested list or 2-D NumPy array of
    ///         shape ``(n, n)``, or a flat row-major list of ``n * n`` values,
    ///         in ``factor_ids`` order.
    ///
    /// Raises:
    ///     ValueError: If ``data`` is not ``n x n``, an identifier repeats, the
    ///         matrix is asymmetric, or it is not positive semidefinite.
    #[new]
    #[pyo3(signature = (factor_ids, data))]
    fn new(factor_ids: Vec<String>, data: &Bound<'_, PyAny>) -> PyResult<Self> {
        let n = factor_ids.len();
        let flat = extract_covariance_data(data, n)?;
        let ids: Vec<FactorId> = factor_ids.into_iter().map(FactorId::new).collect();
        let inner = FactorCovarianceMatrix::new(ids, flat).map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Deserialize and validate a covariance matrix from canonical JSON
    /// (``{"factor_ids": [...], "n": N, "data": [...]}``).
    ///
    /// Raises:
    ///     ValueError: If the JSON is malformed or the matrix fails validation.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid FactorCovarianceMatrix JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize this covariance matrix to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "cannot serialize FactorCovarianceMatrix"))
    }

    /// Number of factors represented by the matrix.
    #[getter]
    fn n_factors(&self) -> usize {
        self.inner.n_factors()
    }

    /// Ordered factor identifiers corresponding to rows and columns.
    #[getter]
    fn factor_ids(&self) -> Vec<String> {
        self.inner
            .factor_ids()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// Row-major covariance data with ``n_factors * n_factors`` entries.
    #[getter]
    fn data(&self) -> Vec<f64> {
        self.inner.as_slice().to_vec()
    }

    /// Variance for ``factor_id``, or zero when the factor is unknown.
    fn variance(&self, factor_id: &str) -> f64 {
        self.inner.variance(&FactorId::new(factor_id))
    }

    /// Covariance between two factors, or zero when either factor is unknown.
    fn covariance(&self, lhs: &str, rhs: &str) -> f64 {
        self.inner
            .covariance(&FactorId::new(lhs), &FactorId::new(rhs))
    }

    /// Correlation between two factors, or zero for unknown/zero-variance factors.
    fn correlation(&self, lhs: &str, rhs: &str) -> f64 {
        self.inner
            .correlation(&FactorId::new(lhs), &FactorId::new(rhs))
    }

    /// The matrix as an ``(n, n)`` float64 NumPy array in ``factor_ids`` order.
    #[pyo3(text_signature = "($self)")]
    fn to_numpy<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray2<f64>>> {
        let n = self.inner.n_factors();
        let flat = PyArray2::<f64>::zeros(py, [n, n], false);
        {
            let mut view = flat.readwrite();
            let mut target = view.as_array_mut();
            for (i, row) in target.rows_mut().into_iter().enumerate() {
                for (j, cell) in row.into_iter().enumerate() {
                    *cell = self.inner.covariance_at(i, j);
                }
            }
        }
        Ok(flat)
    }

    /// The matrix as a square pandas ``DataFrame`` indexed and columned by
    /// ``factor_ids``.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let ids = self.factor_ids();
        let n = ids.len();
        let data = PyDict::new(py);
        for (j, id) in ids.iter().enumerate() {
            let column: Vec<f64> = (0..n).map(|i| self.inner.covariance_at(i, j)).collect();
            data.set_item(id, column)?;
        }
        let index = pyo3::types::PyList::new(py, &ids)?;
        dict_to_dataframe(py, &data, Some(index.into_any()))
    }

    /// Support pickle through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "FactorCovarianceMatrix(factor_ids={:?}, n_factors={})",
            self.factor_ids(),
            self.inner.n_factors()
        )
    }
}

/// Portfolio factor-model configuration assembled at a forecast horizon.
///
/// Example:
///     >>> from finstack_quant.models.factor.credit import FactorModelConfig
///     >>> config = FactorModelConfig.from_json('{"factors":[],"covariance":{"factor_ids":[],"n":0,"data":[]},"matching":{"mapping_table":[]},"pricing_mode":"delta_based","risk_measure":"variance"}')
///     >>> config.n_factors
///     0
#[pyclass(
    name = "FactorModelConfig",
    module = "finstack_quant.models.factor.credit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFactorModelConfig {
    pub(crate) inner: FactorModelConfig,
}

impl PyFactorModelConfig {
    fn from_inner(inner: FactorModelConfig) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFactorModelConfig {
    /// Deserialize and validate a factor-model configuration from canonical JSON.
    ///
    /// Raises:
    ///     ValueError: If the JSON is malformed or the configuration is
    ///         inconsistent (unknown factor in a matching rule, bad covariance).
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: FactorModelConfig = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid FactorModelConfig JSON"))?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize this configuration to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "cannot serialize FactorModelConfig"))
    }

    /// Number of configured factors.
    #[getter]
    fn n_factors(&self) -> usize {
        self.inner.factors.len()
    }

    /// Ordered factor identifiers used by definitions and covariance axes.
    #[getter]
    fn factor_ids(&self) -> Vec<String> {
        self.inner
            .factors
            .iter()
            .map(|factor| factor.id.to_string())
            .collect()
    }

    /// Factor definitions as Python dictionaries following canonical serde fields.
    #[getter]
    fn factors<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.factors)
    }

    /// Covariance matrix aligned to ``factor_ids``.
    #[getter]
    fn covariance(&self) -> PyFactorCovarianceMatrix {
        PyFactorCovarianceMatrix::from_inner(self.inner.covariance.clone())
    }

    /// Declarative dependency-to-factor matching configuration.
    #[getter]
    fn matching<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.matching)
    }

    /// Sensitivity extraction strategy (``delta_based`` or ``full_repricing``).
    #[getter]
    fn pricing_mode(&self) -> String {
        self.inner.pricing_mode.to_string()
    }

    /// Risk measure as its canonical Python value: ``"variance"``,
    /// ``"volatility"``, ``{"var": {"confidence": c}}`` or
    /// ``{"expected_shortfall": {"confidence": c}}``.
    #[getter]
    fn risk_measure<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.risk_measure)
    }

    /// Optional finite-difference bump overrides as a Python dictionary.
    #[getter]
    fn bump_size<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .bump_size
            .as_ref()
            .map(|value| serde_to_py(py, value))
            .transpose()
    }

    /// Policy for unmatched dependencies, or ``None`` when the default applies.
    #[getter]
    fn unmatched_policy(&self) -> Option<String> {
        self.inner.unmatched_policy.map(|policy| policy.to_string())
    }

    /// Validate that matching rules emit only declared factor identifiers.
    ///
    /// Raises:
    ///     ValueError: If a matching rule references an undeclared factor.
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Support pickle through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "FactorModelConfig(n_factors={}, pricing_mode={:?})",
            self.inner.factors.len(),
            self.inner.pricing_mode.to_string(),
        )
    }
}

/// Vol-forecast view over a calibrated ``CreditFactorModel``.
///
/// Every method takes a ``horizon`` that is either a ``VolHorizon`` instance
/// or a descriptor string:
///
/// - ``"one_step"`` — calibrated annualized variance unchanged.
/// - ``"unconditional"`` — long-run variance (identical to ``"one_step"``
///   for the ``Sample`` and ``Ewma`` vol models).
/// - ``'{"n_steps": N}'`` — variance scaled by ``N`` model periods.
/// - ``'{"years": Y}'`` — variance scaled by a fractional year.
///
/// Example:
///     >>> from finstack_quant.models.factor.credit import CreditCalibrator, FactorCovarianceForecast, VolHorizon
///     >>> config = {"hierarchy": {"levels": []}, "covariance_strategy": "diagonal",
///     ...           "bucket_weighting": "equal"}
///     >>> inputs = {"history_panel": {"dates": ["2024-01-01", "2024-02-01"],
///     ...                             "spreads": {"A": [0.010, 0.0101]}},
///     ...           "issuer_tags": {"tags": {"A": {}}},
///     ...           "generic_factor": {"spec": {"name": "G", "series_id": "G"},
///     ...                              "values": [0.010, 0.0101]},
///     ...           "as_of": "2024-02-01", "as_of_spreads": {"A": 0.0101},
///     ...           "idiosyncratic_overrides": {}}
///     >>> model = CreditCalibrator(config).calibrate(inputs)
///     >>> forecast = FactorCovarianceForecast(model)
///     >>> forecast.covariance_at(VolHorizon.one_step()).factor_ids
///     ['credit::generic']
#[pyclass(
    name = "FactorCovarianceForecast",
    module = "finstack_quant.models.factor.credit",
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyFactorCovarianceForecast {
    /// The model is stored by value (cloned from the Python wrapper) so that
    /// `FactorCovarianceForecast<'a>` lifetime requirements don't escape.
    model: CreditFactorModel,
}

#[pymethods]
impl PyFactorCovarianceForecast {
    /// Wrap a ``CreditFactorModel`` for vol forecasting.
    ///
    /// Args:
    ///     model: Calibrated ``CreditFactorModel`` artifact.
    #[new]
    fn new(model: &PyCreditFactorModel) -> Self {
        Self {
            model: model.inner.clone(),
        }
    }

    /// Build the factor covariance matrix ``Σ(t, h) = D · ρ_static · D``.
    ///
    /// Args:
    ///     horizon: ``VolHorizon`` or descriptor string (see the class doc).
    ///
    /// Returns:
    ///     Typed ``FactorCovarianceMatrix`` at the requested horizon.
    ///
    /// Raises:
    ///     ValueError: If the horizon is invalid or the model data is
    ///         inconsistent (mismatched axes, negative variance).
    fn covariance_at(
        &self,
        py: Python<'_>,
        horizon: &Bound<'_, PyAny>,
    ) -> PyResult<PyFactorCovarianceMatrix> {
        let h = extract_vol_horizon(horizon)?;
        let cov = py
            .detach(|| {
                let forecast = finstack_quant_models::factor::credit::FactorCovarianceForecast::new(
                    &self.model,
                );
                forecast.covariance_at(h)
            })
            .map_err(core_to_py)?;
        Ok(PyFactorCovarianceMatrix::from_inner(cov))
    }

    /// Idiosyncratic vol (std dev) for a specific issuer at the requested horizon.
    ///
    /// Args:
    ///     issuer_id: Issuer identifier string.
    ///     horizon: ``VolHorizon`` or descriptor string (see the class doc).
    ///
    /// Returns:
    ///     Idiosyncratic standard deviation in basis points of spread,
    ///     scaled to the horizon.
    ///
    /// Raises:
    ///     ValueError: If the issuer is not present in the model's vol state,
    ///         the horizon is invalid, or the calibrated variance is negative.
    fn idiosyncratic_vol(
        &self,
        py: Python<'_>,
        issuer_id: &str,
        horizon: &Bound<'_, PyAny>,
    ) -> PyResult<f64> {
        let h = extract_vol_horizon(horizon)?;
        let id = IssuerId::new(issuer_id);
        py.detach(|| {
            let forecast =
                finstack_quant_models::factor::credit::FactorCovarianceForecast::new(&self.model);
            forecast.idiosyncratic_vol(&id, h)
        })
        .map_err(core_to_py)
    }

    /// Build a typed portfolio-level factor-model configuration using
    /// ``Σ(t, h)`` at the given horizon and risk measure.
    ///
    /// Args:
    ///     horizon: ``VolHorizon`` or descriptor string (see the class doc).
    ///     risk_measure: ``"variance"`` (default), ``"volatility"``, or a dict
    ///         such as ``{"var": {"confidence": 0.99}}`` /
    ///         ``{"expected_shortfall": {"confidence": 0.975}}``; a JSON string
    ///         of any of these is also accepted.
    ///
    /// Returns:
    ///     Typed ``FactorModelConfig`` ready for portfolio risk or ``to_json()``.
    ///
    /// Raises:
    ///     ValueError: If the horizon or risk measure is invalid (confidence
    ///         outside ``(0.5, 1)``), or the covariance cannot be built.
    #[pyo3(signature = (horizon, risk_measure = None))]
    fn factor_model_at(
        &self,
        py: Python<'_>,
        horizon: &Bound<'_, PyAny>,
        risk_measure: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyFactorModelConfig> {
        let h = extract_vol_horizon(horizon)?;
        let measure: finstack_quant_models::factor::RiskMeasure = match risk_measure {
            Some(obj) if !obj.is_none() => {
                let value = py_to_json_value(py, obj, "risk_measure")?;
                serde_json::from_value(value)
                    .map_err(|e| serde_json_to_py(e, "invalid risk_measure"))?
            }
            _ => finstack_quant_models::factor::RiskMeasure::default(),
        };
        let config = py
            .detach(|| {
                let forecast = finstack_quant_models::factor::credit::FactorCovarianceForecast::new(
                    &self.model,
                );
                forecast.factor_model_config_at(h, measure)
            })
            .map_err(core_to_py)?;
        Ok(PyFactorModelConfig::from_inner(config))
    }

    fn __repr__(&self) -> String {
        format!(
            "FactorCovarianceForecast(as_of={:?}, n_factors={})",
            self.model.as_of.to_string(),
            self.model.config.factors.len(),
        )
    }
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCreditFactorModel>()?;
    m.add_class::<PyCreditCalibrator>()?;
    m.add_class::<PyLevelsAtDate>()?;
    m.add_class::<PyPeriodDecomposition>()?;
    m.add_class::<PyFactorCovarianceMatrix>()?;
    m.add_class::<PyFactorModelConfig>()?;
    m.add_class::<PyFactorCovarianceForecast>()?;
    m.add_function(pyo3::wrap_pyfunction!(decompose_levels, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(decompose_period, m)?)?;
    Ok(())
}
