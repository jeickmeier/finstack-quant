//! Python bindings for the credit factor hierarchy.
//!
//! Exposes [`PyCreditFactorModel`], [`PyCreditCalibrator`], [`PyLevelsAtDate`],
//! [`PyPeriodDecomposition`], the free functions [`decompose_levels`] and
//! [`decompose_period`], [`PyVolHorizon`], and [`PyFactorCovarianceForecast`].

use super::config::{PyFactorCovarianceMatrix, PyFactorModelConfig};
use crate::bindings::core::table::{PyArrowTable, PyLabeledSeries};
use crate::bindings::dataframe_utils::{
    extract_f64_vec, has_arrow_c_stream, is_dataframe_like, is_missing_cell, is_polars_dataframe,
    table_column_names, table_column_to_pylist, table_to_py_records,
};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::{py_getters, tabular_methods, typed_or_label, wire_methods};
use crate::bindings::module_utils::py_to_json_value;
use crate::bindings::pickle_support::reduce_via_json;
use crate::bindings::pydoc::pydoc;
use crate::bindings::table_builders::serde_rows_to_arrow_table;
use crate::errors::{core_to_py, kinded_to_py, serde_json_to_py, value_error};
use finstack_quant_core::types::IssuerId;
use finstack_quant_core::wire::serde_label;
use finstack_quant_factor::credit::VolHorizon;
use finstack_quant_factor::credit::calibration::{
    BucketWeighting, CovarianceStrategy, CreditCalibrationConfig, CreditCalibrationInputs,
    GenericFactorSeries, HistoryPanel, IssuerTagPanel, VolModelChoice,
};
use finstack_quant_factor::credit::hierarchy::{
    CreditFactorModel, GenericFactorSpec, IssuerTags, dimension_key,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple, PyType};
use std::collections::BTreeMap;

/// Column schema of `PyLevelsAtDate::to_arrow_table`, kept so a level-free
/// snapshot still exports the documented columns.
const LEVEL_VALUE_COLUMNS: &[(&str, &str)] = &[
    ("date", "str"),
    ("level_index", "int64"),
    ("dimension", "str"),
    ("bucket", "str"),
    ("value", "float64"),
];

/// Column schema of `PyPeriodDecomposition::to_level_table`, kept so a
/// level-free decomposition still exports the documented columns.
const LEVEL_DELTA_COLUMNS: &[(&str, &str)] = &[
    ("from_date", "str"),
    ("to_date", "str"),
    ("level_index", "int64"),
    ("dimension", "str"),
    ("bucket", "str"),
    ("delta", "float64"),
];

/// Column schema of `PyPeriodDecomposition::to_adder_table`, kept so a
/// decomposition with no shared issuers still exports the documented columns.
const ADDER_DELTA_COLUMNS: &[(&str, &str)] = &[
    ("from_date", "str"),
    ("to_date", "str"),
    ("issuer_id", "str"),
    ("d_adder", "float64"),
];

/// Column schema of `PyCreditFactorModel::to_arrow_table`, kept so a model with
/// no issuer rows still exports the documented columns.
const ISSUER_ROW_COLUMNS: &[(&str, &str)] = &[
    ("issuer_id", "str"),
    ("tags", "str"),
    ("mode", "str"),
    ("beta_pc", "float64"),
    ("beta_levels", "str"),
    ("adder_at_anchor", "float64"),
    ("adder_vol_annualized", "float64"),
    ("adder_vol_source", "str"),
    ("r_squared", "float64"),
    ("n_obs", "float64"),
    ("spread_duration", "float64"),
];

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

#[doc = pydoc!("finstack_quant.models.factor.credit", "CreditFactorModel")]
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

wire_methods!(
    PyCreditFactorModel,
    CreditFactorModel,
    "CreditFactorModel",
    module = "finstack_quant.models.factor.credit",
    rust_json
);

py_getters!(PyCreditFactorModel |s| {
    #[doc = pydoc!("finstack_quant.models.factor.credit", "CreditFactorModel.schema")]
    schema -> &'static str = s.inner.schema.as_str();
    #[doc = pydoc!("finstack_quant.models.factor.credit", "CreditFactorModel.as_of")]
    as_of -> String = s.inner.as_of.to_string();
    #[doc = pydoc!("finstack_quant.models.factor.credit", "CreditFactorModel.policy")]
    policy -> &'static str = s.inner.policy.kind();
    #[doc = pydoc!("finstack_quant.models.factor.credit", "CreditFactorModel.config")]
    config -> PyFactorModelConfig = PyFactorModelConfig::from_inner(s.inner.config.clone());
    #[doc = pydoc!("finstack_quant.models.factor.credit", "CreditFactorModel.covariance")]
    covariance -> PyFactorCovarianceMatrix =
        PyFactorCovarianceMatrix::from_inner(s.inner.config.covariance.clone());
    #[doc = pydoc!("finstack_quant.models.factor.credit", "CreditFactorModel.n_levels")]
    n_levels -> usize = s.inner.hierarchy.levels.len();
    #[doc = pydoc!("finstack_quant.models.factor.credit", "CreditFactorModel.n_issuers")]
    n_issuers -> usize = s.inner.issuer_betas.len();
    #[doc = pydoc!("finstack_quant.models.factor.credit", "CreditFactorModel.n_factors")]
    n_factors -> usize = s.inner.config.factors.len();
});

#[pymethods]
impl PyCreditFactorModel {
    #[doc = pydoc!("finstack_quant.models.factor.credit", "CreditFactorModel.calibration_window")]
    #[getter]
    fn calibration_window<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let start = date_to_py(py, self.inner.calibration_window.start)?;
        let end = date_to_py(py, self.inner.calibration_window.end)?;
        PyTuple::new(py, [start, end])
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "CreditFactorModel.panel_frequency")]
    #[getter]
    fn panel_frequency(&self) -> PyResult<String> {
        serde_label(&self.inner.panel_frequency).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "CreditFactorModel.bucket_weighting")]
    #[getter]
    fn bucket_weighting(&self) -> PyResult<String> {
        serde_label(&self.inner.bucket_weighting).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "CreditFactorModel.diagnostics")]
    #[getter]
    fn diagnostics<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.diagnostics)
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "CreditFactorModel.static_correlation")]
    #[getter]
    fn static_correlation<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.static_correlation)
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "CreditFactorModel.level_names")]
    fn level_names(&self) -> Vec<String> {
        self.inner.level_names()
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "CreditFactorModel.issuer_ids")]
    fn issuer_ids(&self) -> Vec<String> {
        self.inner.issuer_ids()
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "CreditFactorModel.factor_ids")]
    fn factor_ids(&self) -> Vec<String> {
        self.inner.factor_ids()
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

impl PyCreditFactorModel {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let mut rows: Vec<serde_json::Value> = Vec::with_capacity(self.inner.issuer_betas.len());
        for row in &self.inner.issuer_betas {
            let fit = row.fit_quality.as_ref();
            rows.push(serde_json::json!({
                "issuer_id": row.issuer_id.as_str(),
                "tags": row.tags,
                "mode": serde_label(&row.mode).map_err(core_to_py)?,
                "beta_pc": row.betas.pc,
                "beta_levels": row.betas.levels,
                "adder_at_anchor": row.adder_at_anchor,
                "adder_vol_annualized": row.adder_vol_annualized,
                "adder_vol_source": serde_label(&row.adder_vol_source).map_err(core_to_py)?,
                "r_squared": fit.map(|f| f.r_squared),
                "n_obs": fit.map(|f| f.n_obs),
                "spread_duration": row.spread_duration,
            }));
        }
        serde_rows_to_arrow_table(&rows, ISSUER_ROW_COLUMNS)
    }
}

tabular_methods!(
    PyCreditFactorModel,
    "CreditFactorModel",
    module = "finstack_quant.models.factor.credit"
);

#[doc = pydoc!("finstack_quant.models.factor.credit", "CreditCalibrator")]
#[pyclass(
    name = "CreditCalibrator",
    module = "finstack_quant.models.factor.credit",
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyCreditCalibrator {
    inner: finstack_quant_factor::credit::calibration::CreditCalibrator,
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

/// Convert DataFrame/dict tags into an `IssuerId -> IssuerTags` map.
fn extract_issuer_tags_map(
    py: Python<'_>,
    tags: &Bound<'_, PyAny>,
    label: &str,
) -> PyResult<BTreeMap<IssuerId, IssuerTags>> {
    if is_polars_dataframe(tags)? || has_arrow_c_stream(tags)? {
        let cols = table_column_names(tags)?;
        let id_col = cols
            .iter()
            .find(|c| c.as_str() == "issuer" || c.as_str() == "issuer_id")
            .cloned()
            .or_else(|| cols.first().cloned())
            .ok_or_else(|| value_error("tags DataFrame cannot be empty"))?;
        let records = table_to_py_records(py, tags)?
            .ok_or_else(|| value_error("tags DataFrame has no records"))?;
        let rows: Vec<Bound<'_, PyDict>> = records.extract()?;
        let mut out = BTreeMap::new();
        for row in rows {
            let copy = row.copy()?;
            let issuer: String = copy
                .call_method1("pop", (&id_col,))
                .map_err(|_| {
                    value_error(format!("tags DataFrame row missing issuer key {id_col:?}"))
                })?
                .str()?
                .extract()?;
            let mut tags_map = BTreeMap::new();
            for (k, v) in copy.iter() {
                if !is_missing_cell(&v)? {
                    tags_map.insert(k.extract::<String>()?, v.str()?.extract::<String>()?);
                }
            }
            out.insert(IssuerId::new(issuer), IssuerTags(tags_map));
        }
        return Ok(out);
    }
    let obj = if tags.hasattr("to_dict")? && !tags.is_instance_of::<PyDict>() {
        let kwargs = PyDict::new(py);
        kwargs.set_item("orient", "index")?;
        tags.call_method("to_dict", (), Some(&kwargs))?
    } else {
        tags.clone()
    };
    let value = py_to_json_value(py, &obj, label)?;
    serde_json::from_value(value).map_err(|e| serde_json_to_py(e, &format!("invalid {label}")))
}

fn extract_issuer_f64_map(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
    label: &str,
    col_hint: &str,
) -> PyResult<BTreeMap<IssuerId, f64>> {
    if is_dataframe_like(obj)? {
        let cols = table_column_names(obj)?;
        if cols.len() != 2 {
            return Err(value_error(format!(
                "{label} DataFrame must have 2 columns (issuer, {col_hint}), got {}",
                cols.len()
            )));
        }
        let ids: Vec<String> = table_column_to_pylist(obj, &cols[0])?
            .unwrap_or_default()
            .into_iter()
            .map(|item| item.str()?.extract::<String>())
            .collect::<PyResult<_>>()?;
        let vals = if is_polars_dataframe(obj)? {
            extract_f64_vec(&obj.get_item(&cols[1])?, &cols[1])?
        } else {
            let vals_py = table_column_to_pylist(obj, &cols[1])?.unwrap_or_default();
            vals_py
                .into_iter()
                .map(|v| v.extract::<f64>())
                .collect::<PyResult<_>>()?
        };
        return Ok(ids
            .into_iter()
            .zip(vals)
            .map(|(k, v)| (IssuerId::new(k), v))
            .collect());
    }
    py_to_serde_any(py, obj, label)
}

/// Convert pandas, Polars, or Arrow objects into the Rust panel types and hand
/// them to [`CreditCalibrationInputs::from_panel`].
fn inputs_from_dataframe(
    spreads: &Bound<'_, PyAny>,
    tags: &Bound<'_, PyAny>,
    generic: &Bound<'_, PyAny>,
    generic_spec: &Bound<'_, PyAny>,
    spread_durations: Option<&Bound<'_, PyAny>>,
    date_col: Option<&str>,
) -> PyResult<CreditCalibrationInputs> {
    let py = spreads.py();
    let extracted = crate::bindings::dataframe_utils::extract_dataframe_panel(
        spreads,
        date_col,
        "spreads must be a pandas DataFrame, Polars DataFrame, or Arrow table",
    )?;
    let dates = extracted.dates;
    let mut panel: BTreeMap<IssuerId, Vec<Option<f64>>> = BTreeMap::new();
    for (issuer, col_vals) in extracted.ticker_names.into_iter().zip(extracted.columns) {
        let values: Vec<Option<f64>> = col_vals
            .into_iter()
            .map(|f| if f.is_nan() { None } else { Some(f) })
            .collect();
        panel.insert(IssuerId::new(issuer), values);
    }

    let tag_map = extract_issuer_tags_map(py, tags, "issuer tags")?;
    let values = extract_f64_vec(generic, "generic")?;
    let spec: GenericFactorSpec = py_to_serde_any(py, generic_spec, "GenericFactorSpec")?;

    let spread_durations: BTreeMap<IssuerId, f64> = match spread_durations {
        Some(obj) if !obj.is_none() => {
            extract_issuer_f64_map(py, obj, "spread_durations", "duration")?
        }
        _ => BTreeMap::new(),
    };

    CreditCalibrationInputs::from_panel(
        HistoryPanel {
            dates,
            spreads: panel,
        },
        IssuerTagPanel { tags: tag_map },
        GenericFactorSeries { spec, values },
        spread_durations,
    )
    .map_err(core_to_py)
}

#[pymethods]
impl PyCreditCalibrator {
    #[new]
    #[pyo3(signature = (config = None))]
    fn new(py: Python<'_>, config: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let config = extract_calibration_config(py, config)?;
        Ok(Self {
            inner: finstack_quant_factor::credit::calibration::CreditCalibrator::new(config),
        })
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "CreditCalibrator.calibrate")]
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

    #[doc = pydoc!("finstack_quant.models.factor.credit", "CreditCalibrator.from_dataframe")]
    #[staticmethod]
    #[pyo3(signature = (spreads, tags, generic, generic_spec, spread_durations = None, config = None, *, date_col = None))]
    fn from_dataframe(
        spreads: &Bound<'_, PyAny>,
        tags: &Bound<'_, PyAny>,
        generic: &Bound<'_, PyAny>,
        generic_spec: &Bound<'_, PyAny>,
        spread_durations: Option<&Bound<'_, PyAny>>,
        config: Option<&Bound<'_, PyAny>>,
        date_col: Option<&str>,
    ) -> PyResult<PyCreditFactorModel> {
        let py = spreads.py();
        let config = extract_calibration_config(py, config)?;
        let inputs = inputs_from_dataframe(
            spreads,
            tags,
            generic,
            generic_spec,
            spread_durations,
            date_col,
        )?;
        let calibrator = finstack_quant_factor::credit::calibration::CreditCalibrator::new(config);
        let model = py
            .detach(|| calibrator.calibrate(inputs))
            .map_err(core_to_py)?;
        Ok(PyCreditFactorModel::from_inner(model))
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "CreditCalibrator.config")]
    #[getter]
    fn config<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, self.inner.config())
    }

    fn __repr__(&self) -> String {
        let config = self.inner.config();
        let vol_model = match config.vol_model {
            VolModelChoice::Sample => "sample",
            VolModelChoice::Ewma { .. } => "ewma",
            _ => "unknown",
        };
        let covariance_strategy = match config.covariance_strategy {
            CovarianceStrategy::Diagonal => "diagonal",
            CovarianceStrategy::Ridge { .. } => "ridge",
            CovarianceStrategy::FullSampleRepaired => "full_sample_repaired",
            CovarianceStrategy::LedoitWolf => "ledoit_wolf",
            _ => "unknown",
        };
        let bucket_weighting = match config.bucket_weighting {
            BucketWeighting::Equal => "equal",
            BucketWeighting::Dts => "dts",
            _ => "unknown",
        };
        format!(
            "CreditCalibrator(policy={:?}, n_levels={}, vol_model={:?}, covariance_strategy={:?}, bucket_weighting={:?})",
            config.policy.kind(),
            config.hierarchy.levels.len(),
            vol_model,
            covariance_strategy,
            bucket_weighting,
        )
    }
}

#[doc = pydoc!("finstack_quant.models.factor.credit", "LevelsAtDate")]
#[pyclass(
    name = "LevelsAtDate",
    module = "finstack_quant.models.factor.credit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyLevelsAtDate {
    inner: finstack_quant_factor::credit::decomposition::LevelsAtDate,
}

impl PyLevelsAtDate {
    fn from_inner(inner: finstack_quant_factor::credit::decomposition::LevelsAtDate) -> Self {
        Self { inner }
    }
}

wire_methods!(
    PyLevelsAtDate,
    finstack_quant_factor::credit::decomposition::LevelsAtDate,
    "LevelsAtDate",
    module = "finstack_quant.models.factor.credit",
    rust_json
);

py_getters!(PyLevelsAtDate |s| {
    #[doc = pydoc!("finstack_quant.models.factor.credit", "LevelsAtDate.date")]
    date -> String = s.inner.date.to_string();
    #[doc = pydoc!("finstack_quant.models.factor.credit", "LevelsAtDate.generic")]
    generic -> f64 = s.inner.generic;
    #[doc = pydoc!("finstack_quant.models.factor.credit", "LevelsAtDate.n_levels")]
    n_levels -> usize = s.inner.by_level.len();
});

#[pymethods]
impl PyLevelsAtDate {
    #[doc = pydoc!("finstack_quant.models.factor.credit", "LevelsAtDate.level_values")]
    fn level_values<'py>(
        &self,
        py: Python<'py>,
        level_index: usize,
    ) -> PyResult<Bound<'py, PyDict>> {
        let lev = self.inner.level(level_index).map_err(core_to_py)?;
        let d = PyDict::new(py);
        for (k, v) in &lev.values {
            d.set_item(k, v)?;
        }
        Ok(d)
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "LevelsAtDate.adder")]
    fn adder<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new(py);
        for (issuer, val) in &self.inner.adder {
            d.set_item(issuer.as_str(), val)?;
        }
        Ok(d)
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "LevelsAtDate.to_series")]
    #[pyo3(text_signature = "($self)")]
    fn to_series(&self) -> PyResult<PyLabeledSeries> {
        let labels: Vec<String> = self
            .inner
            .adder
            .keys()
            .map(|issuer| issuer.as_str().to_owned())
            .collect();
        let values: Vec<f64> = self.inner.adder.values().copied().collect();
        Ok(PyLabeledSeries::from_f64("adder", labels, values)?.with_index_name("issuer_id"))
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

impl PyLevelsAtDate {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let date = self.inner.date.to_string();
        let mut rows: Vec<serde_json::Value> = Vec::new();
        for level in &self.inner.by_level {
            let dimension = dimension_key(&level.dimension);
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
        serde_rows_to_arrow_table(&rows, LEVEL_VALUE_COLUMNS)
    }
}

tabular_methods!(
    PyLevelsAtDate,
    "LevelsAtDate",
    module = "finstack_quant.models.factor.credit"
);

#[doc = pydoc!("finstack_quant.models.factor.credit", "PeriodDecomposition")]
#[pyclass(
    name = "PeriodDecomposition",
    module = "finstack_quant.models.factor.credit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyPeriodDecomposition {
    inner: finstack_quant_factor::credit::decomposition::PeriodDecomposition,
}

impl PyPeriodDecomposition {
    fn from_inner(
        inner: finstack_quant_factor::credit::decomposition::PeriodDecomposition,
    ) -> Self {
        Self { inner }
    }
}

wire_methods!(
    PyPeriodDecomposition,
    finstack_quant_factor::credit::decomposition::PeriodDecomposition,
    "PeriodDecomposition",
    module = "finstack_quant.models.factor.credit",
    rust_json
);

py_getters!(PyPeriodDecomposition |s| {
    #[doc = pydoc!("finstack_quant.models.factor.credit", "PeriodDecomposition.to_date")]
    to_date -> String = s.inner.to.to_string();
    #[doc = pydoc!("finstack_quant.models.factor.credit", "PeriodDecomposition.d_generic")]
    d_generic -> f64 = s.inner.d_generic;
    #[doc = pydoc!("finstack_quant.models.factor.credit", "PeriodDecomposition.n_levels")]
    n_levels -> usize = s.inner.by_level.len();
});

#[pymethods]
impl PyPeriodDecomposition {
    #[doc = pydoc!("finstack_quant.models.factor.credit", "PeriodDecomposition.from_date")]
    #[getter(from_date)]
    fn get_from_date(&self) -> String {
        self.inner.from.to_string()
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "PeriodDecomposition.level_deltas")]
    fn level_deltas<'py>(
        &self,
        py: Python<'py>,
        level_index: usize,
    ) -> PyResult<Bound<'py, PyDict>> {
        let lev = self.inner.level(level_index).map_err(core_to_py)?;
        let d = PyDict::new(py);
        for (k, v) in &lev.deltas {
            d.set_item(k, v)?;
        }
        Ok(d)
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "PeriodDecomposition.d_adder")]
    fn d_adder<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new(py);
        for (issuer, val) in &self.inner.d_adder {
            d.set_item(issuer.as_str(), val)?;
        }
        Ok(d)
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "PeriodDecomposition.to_level_table")]
    #[pyo3(text_signature = "($self)")]
    fn to_level_table(&self) -> PyResult<PyArrowTable> {
        let from_date = self.inner.from.to_string();
        let to_date = self.inner.to.to_string();
        let mut rows: Vec<serde_json::Value> = Vec::new();
        for level in &self.inner.by_level {
            let dimension = dimension_key(&level.dimension);
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
        serde_rows_to_arrow_table(&rows, LEVEL_DELTA_COLUMNS)
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "PeriodDecomposition.to_adder_table")]
    #[pyo3(text_signature = "($self)")]
    fn to_adder_table(&self) -> PyResult<PyArrowTable> {
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
        serde_rows_to_arrow_table(&rows, ADDER_DELTA_COLUMNS)
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

impl PyPeriodDecomposition {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        self.to_level_table()
    }
}

tabular_methods!(
    PyPeriodDecomposition,
    "PeriodDecomposition",
    module = "finstack_quant.models.factor.credit"
);

#[doc = pydoc!("finstack_quant.models.factor.credit", "decompose_levels")]
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
    let observed_spreads =
        extract_issuer_f64_map(py, observed_spreads, "observed_spreads", "spread")?;
    let date = py_to_date(as_of)?;
    let runtime_tags: Option<BTreeMap<IssuerId, IssuerTags>> = match runtime_tags {
        Some(obj) if !obj.is_none() => Some(extract_issuer_tags_map(py, obj, "runtime_tags")?),
        _ => None,
    };
    let result = finstack_quant_factor::credit::decomposition::decompose_levels(
        &model.inner,
        &observed_spreads,
        observed_generic,
        date,
        runtime_tags.as_ref(),
    )
    .map_err(kinded_to_py)?;
    Ok(PyLevelsAtDate::from_inner(result))
}

#[doc = pydoc!("finstack_quant.models.factor.credit", "decompose_period")]
#[pyfunction]
fn decompose_period(
    from_levels: &PyLevelsAtDate,
    to_levels: &PyLevelsAtDate,
) -> PyResult<PyPeriodDecomposition> {
    let result = finstack_quant_factor::credit::decomposition::decompose_period(
        &from_levels.inner,
        &to_levels.inner,
    )
    .map_err(kinded_to_py)?;
    Ok(PyPeriodDecomposition::from_inner(result))
}

typed_or_label!(
    pub(crate) fn extract_vol_horizon -> VolHorizon,
    PyVolHorizon,
    "VolHorizon or descriptor string (\"one_step\", \"unconditional\", '{\"n_steps\": N}', '{\"years\": Y}')",
    |label| VolHorizon::parse(label).map_err(core_to_py)
);

#[doc = pydoc!("finstack_quant.models.factor.credit", "VolHorizon")]
#[pyclass(
    name = "VolHorizon",
    module = "finstack_quant.models.factor.credit",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct PyVolHorizon {
    pub(crate) inner: VolHorizon,
}

impl PyVolHorizon {
    pub(crate) fn from_inner(inner: VolHorizon) -> Self {
        Self { inner }
    }
}

py_getters!(PyVolHorizon |s| {
    #[doc = pydoc!("finstack_quant.models.factor.credit", "VolHorizon.kind")]
    kind -> &'static str = s.inner.kind();
    #[doc = pydoc!("finstack_quant.models.factor.credit", "VolHorizon.n")]
    n -> Option<usize> = match s.inner {
        VolHorizon::NSteps(n) => Some(n),
        _ => None,
    };
    #[doc = pydoc!("finstack_quant.models.factor.credit", "VolHorizon.years_value")]
    years_value -> Option<f64> = match s.inner {
        VolHorizon::Years(years) => Some(years),
        _ => None,
    };
});

#[pymethods]
impl PyVolHorizon {
    #[doc = pydoc!("finstack_quant.models.factor.credit", "VolHorizon.one_step")]
    #[classmethod]
    #[pyo3(text_signature = "(cls)")]
    fn one_step(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(VolHorizon::OneStep)
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "VolHorizon.unconditional")]
    #[classmethod]
    #[pyo3(text_signature = "(cls)")]
    fn unconditional(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(VolHorizon::Unconditional)
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "VolHorizon.n_steps")]
    #[classmethod]
    #[pyo3(text_signature = "(cls, n)")]
    fn n_steps(_cls: &Bound<'_, PyType>, n: usize) -> Self {
        Self::from_inner(VolHorizon::NSteps(n))
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "VolHorizon.years")]
    #[classmethod]
    #[pyo3(text_signature = "(cls, years)")]
    fn years(_cls: &Bound<'_, PyType>, years: f64) -> PyResult<Self> {
        VolHorizon::years(years)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "VolHorizon.parse")]
    #[classmethod]
    #[pyo3(text_signature = "(cls, s)")]
    fn parse(_cls: &Bound<'_, PyType>, s: &str) -> PyResult<Self> {
        VolHorizon::parse(s)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "VolHorizon.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let parse = py.get_type::<Self>().getattr("parse")?;
        reduce_via_json(parse, self.inner.descriptor())
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "VolHorizon.__repr__")]
    fn __repr__(&self) -> String {
        match self.inner {
            VolHorizon::OneStep => "VolHorizon.one_step()".to_owned(),
            VolHorizon::Unconditional => "VolHorizon.unconditional()".to_owned(),
            VolHorizon::NSteps(n) => format!("VolHorizon.n_steps({n})"),
            VolHorizon::Years(years) => format!("VolHorizon.years({years})"),
        }
    }
}

#[doc = pydoc!("finstack_quant.models.factor.credit", "FactorCovarianceForecast")]
#[pyclass(
    name = "FactorCovarianceForecast",
    module = "finstack_quant.models.factor.credit",
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyFactorCovarianceForecast {
    model: CreditFactorModel,
}

#[pymethods]
impl PyFactorCovarianceForecast {
    #[new]
    fn new(model: &PyCreditFactorModel) -> Self {
        Self {
            model: model.inner.clone(),
        }
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "FactorCovarianceForecast.covariance_at")]
    fn covariance_at(
        &self,
        py: Python<'_>,
        horizon: &Bound<'_, PyAny>,
    ) -> PyResult<PyFactorCovarianceMatrix> {
        let h = extract_vol_horizon(horizon)?;
        let cov = py
            .detach(|| {
                let forecast =
                    finstack_quant_factor::credit::FactorCovarianceForecast::new(&self.model);
                forecast.covariance_at(h)
            })
            .map_err(core_to_py)?;
        Ok(PyFactorCovarianceMatrix::from_inner(cov))
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "FactorCovarianceForecast.idiosyncratic_vol")]
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
                finstack_quant_factor::credit::FactorCovarianceForecast::new(&self.model);
            forecast.idiosyncratic_vol(&id, h)
        })
        .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.factor.credit", "FactorCovarianceForecast.factor_model_at")]
    #[pyo3(signature = (horizon, risk_measure = None))]
    fn factor_model_at(
        &self,
        py: Python<'_>,
        horizon: &Bound<'_, PyAny>,
        risk_measure: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyFactorModelConfig> {
        let h = extract_vol_horizon(horizon)?;
        let measure = super::risk::functions::extract_risk_measure(py, risk_measure)?;
        let config = py
            .detach(|| {
                let forecast =
                    finstack_quant_factor::credit::FactorCovarianceForecast::new(&self.model);
                forecast.factor_model_at(h, measure)
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
    m.add_class::<PyFactorCovarianceForecast>()?;
    m.add_function(pyo3::wrap_pyfunction!(decompose_levels, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(decompose_period, m)?)?;
    m.add_class::<PyVolHorizon>()?;
    Ok(())
}
