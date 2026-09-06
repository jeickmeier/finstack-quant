//! Python bindings for the comparable company analysis module.
//!
//! Exposes typed peer-set construction and cross-sectional peer analytics:
//!
//! - `CompanyMetrics`, `PeerFilter`, `PeerSet` (with `from_dataframe` and
//!   `from_universe`), `ScoringDimension`.
//! - Descriptive peer statistics (`peer_stats` -> `PeerStats`).
//! - Percentile rank and z-score of a subject within a peer distribution.
//! - Single-factor OLS regression (`regression_fair_value` -> `RegressionResult`).
//! - Canonical valuation multiple computation on `CompanyMetrics`.
//! - Multi-dimension composite rich/cheap scoring
//!   (`score_relative_value` -> `RelativeValueResult`).

use finstack_quant_core::types::Attributes;
use finstack_quant_statements_analytics::analysis::{
    compute_multiple as core_compute_multiple, peer_stats as core_peer_stats,
    percentile_rank as core_percentile_rank, regression_fair_value as core_regression,
    score_relative_value as core_score, z_score as core_z_score, CompanyMetrics, DimensionScore,
    MetricExtractor, Multiple, PeerFilter, PeerSet, PeerStats, PeriodBasis, RegressionResult,
    RelativeValueResult, ScoreDirection, ScoringDimension,
};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};
use std::collections::{BTreeMap, BTreeSet};

use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_rows_to_dataframe_with_schema,
    ColumnSchema,
};
use crate::bindings::statements_analytics::extract_serde_any;
use crate::errors::{core_to_py, display_to_py, serde_json_to_py};

/// Column schema for `RelativeValueResult.to_dataframe`.
const DIMENSION_COLUMNS: [ColumnSchema<'static>; 6] = [
    ("label", "str"),
    ("percentile", "float64"),
    ("z_score", "float64"),
    ("regression_residual", "float64"),
    ("r_squared", "float64"),
    ("weight", "float64"),
];

/// Parse a period basis: ``"ltm"``, ``"ntm"`` or any other label as ``custom``.
fn parse_period_basis(label: &str) -> PeriodBasis {
    finstack_quant_core::wire::serde_parse::<PeriodBasis>(label)
        .unwrap_or_else(|_| PeriodBasis::Custom(label.to_string()))
}

fn period_basis_label(basis: &PeriodBasis) -> String {
    match basis {
        PeriodBasis::Custom(label) => label.clone(),
        other => finstack_quant_core::wire::serde_label(other).unwrap_or_default(),
    }
}

fn parse_extractor(name: &str) -> PyResult<MetricExtractor> {
    name.parse().map_err(display_to_py)
}

fn extractor_label(extractor: &MetricExtractor) -> String {
    match extractor {
        MetricExtractor::Named(name) | MetricExtractor::Custom(name) => name.clone(),
        MetricExtractor::Multiple(multiple) => {
            format!(
                "multiple:{}",
                finstack_quant_core::wire::serde_label(multiple).unwrap_or_default()
            )
        }
    }
}

/// Metrics for one company in a peer set.
///
/// Monetary values must already be in one currency; ratios are plain scalars
/// (``6.5`` = 6.5x) and growth/margin inputs are decimals (``0.05`` = 5%).
/// Canonical metric names populate dedicated fields; any other name is kept
/// in ``custom``.
///
/// Parameters
/// ----------
/// id : str
///     Company identifier.
/// metrics : dict[str, float | None]
///     Flat ``{metric_name: value}`` map. Known names (``enterprise_value``,
///     ``market_cap``, ``share_price``, ``oas_bp``, ``yield_pct``, ``ebitda``,
///     ``revenue``, ``ebit``, ``ufcf``, ``lfcf``, ``net_income``,
///     ``book_value``, ``tangible_book_value``, ``dividends_per_share``,
///     ``leverage``, ``interest_coverage``, ``revenue_growth``,
///     ``ebitda_margin``) fill their fields; other names go to ``custom``.
///     ``None`` values are treated as missing.
/// tags : list[str] | None
///     Attribute tags used by ``PeerFilter.required_tags`` / ``excluded_tags``.
/// meta : dict[str, str] | None
///     Attribute metadata (``gics_sector``, ``gics_industry``, ``country``,
///     ``rating``) used by ``PeerFilter``.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import CompanyMetrics
/// >>> CompanyMetrics("ACME", {"leverage": 3.0, "oas_bp": 250.0}).get("leverage")
/// 3.0
#[pyclass(
    name = "CompanyMetrics",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyCompanyMetrics {
    pub(crate) inner: CompanyMetrics,
}

#[pymethods]
impl PyCompanyMetrics {
    #[new]
    #[pyo3(signature = (id, metrics=None, tags=None, meta=None))]
    fn new(
        id: &str,
        metrics: Option<&Bound<'_, PyDict>>,
        tags: Option<Vec<String>>,
        meta: Option<BTreeMap<String, String>>,
    ) -> PyResult<Self> {
        let mut inner = match metrics {
            Some(metrics) => dict_to_company_metrics(id, metrics)?,
            None => CompanyMetrics::new(id),
        };
        inner.attributes = Attributes {
            tags: tags
                .unwrap_or_default()
                .into_iter()
                .collect::<BTreeSet<_>>(),
            meta: meta.unwrap_or_default(),
        };
        Ok(Self { inner })
    }

    /// Company identifier.
    #[getter]
    fn id(&self) -> &str {
        self.inner.id.as_str()
    }

    /// Attribute tags.
    #[getter]
    fn tags(&self) -> Vec<String> {
        self.inner.attributes.tags.iter().cloned().collect()
    }

    /// Attribute metadata.
    #[getter]
    fn meta(&self) -> BTreeMap<String, String> {
        self.inner.attributes.meta.clone()
    }

    /// Custom (non-canonical) metrics.
    #[getter]
    fn custom(&self) -> Vec<(String, f64)> {
        self.inner
            .custom
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    /// Read one metric by name (canonical field or ``custom`` key).
    ///
    /// Returns ``None`` when the metric is absent.
    #[pyo3(text_signature = "($self, name)")]
    fn get(&self, name: &str) -> Option<f64> {
        self.inner
            .named_metric(name)
            .or_else(|| self.inner.custom.get(name).copied())
    }

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "CompanyMetrics"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``CompanyMetrics`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid CompanyMetrics JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CompanyMetrics", &self.inner)
    }
}

/// Convert a ``{metric_name: value}`` dict into a `CompanyMetrics`.
///
/// Known field names are mapped onto their dedicated optional fields;
/// everything else is stored in the `custom` map. ``None`` values are
/// treated as missing; any other non-numeric value raises ``ValueError``.
fn dict_to_company_metrics(id: &str, d: &Bound<'_, PyDict>) -> PyResult<CompanyMetrics> {
    let mut values = Vec::with_capacity(d.len());
    for (key, val) in d.iter() {
        let name: String = key.extract()?;
        if val.is_none() {
            continue;
        }
        let Ok(v) = val.extract::<f64>() else {
            return Err(crate::errors::value_error(format!(
                "metric '{name}' for company '{id}' must be a number or None, got {}",
                val.get_type().name().map_or_else(
                    |_| "unknown".to_string(),
                    |t| t.to_string_lossy().into_owned()
                )
            )));
        };
        values.push((name, v));
    }
    Ok(CompanyMetrics::from_flat_metrics(id, values))
}

/// Extract a `CompanyMetrics` from a typed object, a dict of the serde shape,
/// or a JSON string.
fn extract_company_metrics(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<CompanyMetrics> {
    if let Ok(metrics) = obj.extract::<PyRef<'_, PyCompanyMetrics>>() {
        return Ok(metrics.inner.clone());
    }
    extract_serde_any(py, obj, "company_metrics")
}

/// Screening criteria for building a peer set from a universe.
///
/// All non-empty criteria are AND-ed; list criteria are OR-ed within.
///
/// Parameters
/// ----------
/// gics_sectors : list[str]
///     GICS sector codes to include (``meta["gics_sector"]``).
/// gics_industries : list[str]
///     GICS industry codes to include (``meta["gics_industry"]``).
/// countries : list[str]
///     ISO country codes to include (``meta["country"]``).
/// market_cap_min : float | None
///     Inclusive market-cap floor.
/// market_cap_max : float | None
///     Inclusive market-cap ceiling.
/// ratings : list[str]
///     Rating bands to include (``meta["rating"]``).
/// required_tags : list[str]
///     Tags every peer must carry.
/// excluded_tags : list[str]
///     Tags no peer may carry.
/// selectors : list[str]
///     Attribute selector strings (``Attributes.matches_selector``).
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import PeerFilter
/// >>> PeerFilter(ratings=["BB", "B"]).ratings
/// ['BB', 'B']
#[pyclass(
    name = "PeerFilter",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyPeerFilter {
    pub(crate) inner: PeerFilter,
}

#[pymethods]
impl PyPeerFilter {
    #[new]
    #[pyo3(signature = (
        gics_sectors=Vec::new(),
        gics_industries=Vec::new(),
        countries=Vec::new(),
        market_cap_min=None,
        market_cap_max=None,
        ratings=Vec::new(),
        required_tags=Vec::new(),
        excluded_tags=Vec::new(),
        selectors=Vec::new(),
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        gics_sectors: Vec<String>,
        gics_industries: Vec<String>,
        countries: Vec<String>,
        market_cap_min: Option<f64>,
        market_cap_max: Option<f64>,
        ratings: Vec<String>,
        required_tags: Vec<String>,
        excluded_tags: Vec<String>,
        selectors: Vec<String>,
    ) -> Self {
        Self {
            inner: PeerFilter {
                gics_sectors,
                gics_industries,
                countries,
                market_cap_min,
                market_cap_max,
                ratings,
                required_tags,
                excluded_tags,
                selectors,
            },
        }
    }

    /// GICS sector codes to include.
    #[getter]
    fn gics_sectors(&self) -> Vec<String> {
        self.inner.gics_sectors.clone()
    }

    /// GICS industry codes to include.
    #[getter]
    fn gics_industries(&self) -> Vec<String> {
        self.inner.gics_industries.clone()
    }

    /// ISO country codes to include.
    #[getter]
    fn countries(&self) -> Vec<String> {
        self.inner.countries.clone()
    }

    /// Inclusive market-cap floor, or ``None``.
    #[getter]
    fn market_cap_min(&self) -> Option<f64> {
        self.inner.market_cap_min
    }

    /// Inclusive market-cap ceiling, or ``None``.
    #[getter]
    fn market_cap_max(&self) -> Option<f64> {
        self.inner.market_cap_max
    }

    /// Rating bands to include.
    #[getter]
    fn ratings(&self) -> Vec<String> {
        self.inner.ratings.clone()
    }

    /// Tags every peer must carry.
    #[getter]
    fn required_tags(&self) -> Vec<String> {
        self.inner.required_tags.clone()
    }

    /// Tags no peer may carry.
    #[getter]
    fn excluded_tags(&self) -> Vec<String> {
        self.inner.excluded_tags.clone()
    }

    /// Attribute selector strings.
    #[getter]
    fn selectors(&self) -> Vec<String> {
        self.inner.selectors.clone()
    }

    /// Whether ``company`` satisfies every criterion.
    #[pyo3(text_signature = "($self, company)")]
    fn accepts(&self, company: &PyCompanyMetrics) -> bool {
        self.inner.accepts(&company.inner)
    }

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "PeerFilter"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``PeerFilter`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid PeerFilter JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("PeerFilter", &self.inner)
    }
}

/// A subject company alongside its comparison peers.
///
/// Parameters
/// ----------
/// subject : CompanyMetrics | dict | str
///     The company being evaluated (typed, serde dict, or JSON).
/// peers : list[CompanyMetrics | dict | str]
///     Peer companies.
/// period_basis : str
///     ``"ltm"``, ``"ntm"`` or a custom label such as ``"FY2025E"``.
///     Default ``"ltm"``.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import CompanyMetrics, PeerSet
/// >>> subject = CompanyMetrics("SUBJ", {"leverage": 2.0})
/// >>> peers = [CompanyMetrics("P1", {"leverage": 1.0}), CompanyMetrics("P2", {"leverage": 3.0})]
/// >>> PeerSet(subject, peers).peer_count
/// 2
#[pyclass(
    name = "PeerSet",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyPeerSet {
    pub(crate) inner: PeerSet,
}

#[pymethods]
impl PyPeerSet {
    #[new]
    #[pyo3(signature = (subject, peers, period_basis="ltm"))]
    fn new(
        py: Python<'_>,
        subject: &Bound<'_, PyAny>,
        peers: Vec<Bound<'_, PyAny>>,
        period_basis: &str,
    ) -> PyResult<Self> {
        let subject = extract_company_metrics(py, subject)?;
        let peers = peers
            .iter()
            .map(|peer| extract_company_metrics(py, peer))
            .collect::<PyResult<Vec<_>>>()?;
        Ok(Self {
            inner: PeerSet::new(subject, peers, parse_period_basis(period_basis)),
        })
    }

    /// Build a peer set from a universe by applying a ``PeerFilter``.
    ///
    /// The subject is never included in the peers even when it passes.
    ///
    /// Parameters
    /// ----------
    /// subject : CompanyMetrics | dict | str
    ///     The company being evaluated.
    /// universe : list[CompanyMetrics | dict | str]
    ///     Candidate companies.
    /// filter : PeerFilter
    ///     Screening criteria.
    /// period_basis : str
    ///     ``"ltm"``, ``"ntm"`` or a custom label. Default ``"ltm"``.
    #[staticmethod]
    #[pyo3(signature = (subject, universe, filter, period_basis="ltm"))]
    fn from_universe(
        py: Python<'_>,
        subject: &Bound<'_, PyAny>,
        universe: Vec<Bound<'_, PyAny>>,
        filter: &PyPeerFilter,
        period_basis: &str,
    ) -> PyResult<Self> {
        let subject = extract_company_metrics(py, subject)?;
        let universe = universe
            .iter()
            .map(|company| extract_company_metrics(py, company))
            .collect::<PyResult<Vec<_>>>()?;
        Ok(Self {
            inner: PeerSet::from_universe(
                subject,
                &universe,
                &filter.inner,
                parse_period_basis(period_basis),
            ),
        })
    }

    /// Build a peer set from a pandas ``DataFrame`` (rows = companies).
    ///
    /// Parameters
    /// ----------
    /// df : pandas.DataFrame
    ///     One row per company. The index (or ``id_column``) supplies company
    ///     ids; numeric columns become metrics (canonical names fill their
    ///     fields, others go to ``custom``); string columns become attribute
    ///     ``meta`` entries; ``NaN``/``None`` cells are treated as missing.
    /// subject_id : str
    ///     Id of the subject row; every other row becomes a peer.
    /// period_basis : str
    ///     ``"ltm"``, ``"ntm"`` or a custom label. Default ``"ltm"``.
    /// id_column : str | None
    ///     Column holding company ids; ``None`` uses the index.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``subject_id`` is not present.
    /// ValueError
    ///     If a cell is neither numeric, string, nor missing.
    #[staticmethod]
    #[pyo3(signature = (df, subject_id, period_basis="ltm", id_column=None))]
    fn from_dataframe(
        py: Python<'_>,
        df: &Bound<'_, PyAny>,
        subject_id: &str,
        period_basis: &str,
        id_column: Option<&str>,
    ) -> PyResult<Self> {
        let records = match id_column {
            Some(column) => df.call_method1("set_index", (column,))?,
            None => df.clone(),
        };
        let columns: Vec<String> = records.getattr("columns")?.extract()?;
        let index: Vec<Bound<'_, PyAny>> = records
            .getattr("index")?
            .call_method0("tolist")?
            .extract()?;
        let rows: Vec<Vec<Bound<'_, PyAny>>> = records
            .getattr("values")?
            .call_method0("tolist")?
            .extract()?;
        let math = py.import("math")?;

        let mut subject = None;
        let mut peers = Vec::with_capacity(rows.len());
        for (id, row) in index.iter().zip(rows.iter()) {
            let id: String = id.str()?.extract()?;
            let mut values = Vec::with_capacity(row.len());
            let mut meta = BTreeMap::new();
            for (column, cell) in columns.iter().zip(row.iter()) {
                if cell.is_none() {
                    continue;
                }
                if let Ok(v) = cell.extract::<f64>() {
                    let is_nan: bool = math.call_method1("isnan", (v,))?.extract()?;
                    if !is_nan {
                        values.push((column.clone(), v));
                    }
                } else if let Ok(s) = cell.extract::<String>() {
                    meta.insert(column.clone(), s);
                } else {
                    return Err(crate::errors::value_error(format!(
                        "column '{column}' for company '{id}' must be numeric, string or missing"
                    )));
                }
            }
            let mut metrics = CompanyMetrics::from_flat_metrics(id.clone(), values);
            metrics.attributes.meta = meta;
            if id == subject_id {
                subject = Some(metrics);
            } else {
                peers.push(metrics);
            }
        }
        let subject = subject.ok_or_else(|| {
            pyo3::exceptions::PyKeyError::new_err(format!(
                "subject_id '{subject_id}' not found in DataFrame"
            ))
        })?;
        Ok(Self {
            inner: PeerSet::new(subject, peers, parse_period_basis(period_basis)),
        })
    }

    /// The subject company.
    #[getter]
    fn subject(&self) -> PyCompanyMetrics {
        PyCompanyMetrics {
            inner: self.inner.subject.clone(),
        }
    }

    /// Peer companies.
    #[getter]
    fn peers(&self) -> Vec<PyCompanyMetrics> {
        self.inner
            .peers
            .iter()
            .cloned()
            .map(|inner| PyCompanyMetrics { inner })
            .collect()
    }

    /// Period basis label (``"ltm"``, ``"ntm"`` or the custom label).
    #[getter]
    fn period_basis(&self) -> String {
        period_basis_label(&self.inner.period_basis)
    }

    /// Number of peers (excluding the subject).
    #[getter]
    fn peer_count(&self) -> usize {
        self.inner.peer_count()
    }

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "PeerSet"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``PeerSet`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner =
            serde_json::from_str(json).map_err(|e| serde_json_to_py(e, "invalid PeerSet JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "PeerSet(subject='{}', peers={}, period_basis='{}')",
            self.inner.subject.id,
            self.inner.peers.len(),
            self.period_basis()
        )
    }
}

/// One weighted rich/cheap scoring dimension.
///
/// Parameters
/// ----------
/// label : str
///     Human-readable label (e.g. ``"Spread vs Leverage"``).
/// y : str
///     Dependent metric: a canonical name (``"oas_bp"``), a custom key, or
///     ``"multiple:<name>"`` for a valuation multiple (``"multiple:ev_ebitda"``).
/// x : str | None
///     Optional explanatory metric in the same notation. ``None`` (default)
///     scores the dependent metric against its peer distribution.
/// weight : float
///     Weight in the composite score. Default ``1.0``.
/// direction : str
///     ``"higher_is_cheap"`` (spread-like, default) or ``"higher_is_rich"``
///     (multiple-like).
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import ScoringDimension
/// >>> ScoringDimension("Spread vs Leverage", "oas_bp", "leverage").direction
/// 'higher_is_cheap'
#[pyclass(
    name = "ScoringDimension",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyScoringDimension {
    pub(crate) inner: ScoringDimension,
}

#[pymethods]
impl PyScoringDimension {
    #[new]
    #[pyo3(signature = (label, y, x=None, weight=1.0, direction="higher_is_cheap"))]
    fn new(label: &str, y: &str, x: Option<&str>, weight: f64, direction: &str) -> PyResult<Self> {
        let direction: ScoreDirection = direction.parse().map_err(display_to_py)?;
        Ok(Self {
            inner: ScoringDimension {
                label: label.to_string(),
                y_extractor: parse_extractor(y)?,
                x_extractor: x.map(parse_extractor).transpose()?,
                weight,
                direction,
            },
        })
    }

    /// Dimension label.
    #[getter]
    fn label(&self) -> &str {
        &self.inner.label
    }

    /// Dependent metric in ``name`` / ``multiple:<name>`` notation.
    #[getter]
    fn y(&self) -> String {
        extractor_label(&self.inner.y_extractor)
    }

    /// Optional explanatory metric in ``name`` / ``multiple:<name>`` notation.
    #[getter]
    fn x(&self) -> Option<String> {
        self.inner.x_extractor.as_ref().map(extractor_label)
    }

    /// Weight in the composite score.
    #[getter]
    fn weight(&self) -> f64 {
        self.inner.weight
    }

    /// ``"higher_is_cheap"`` or ``"higher_is_rich"``.
    #[getter]
    fn direction(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_label(&self.inner.direction).map_err(core_to_py)
    }

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "ScoringDimension"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``ScoringDimension`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid ScoringDimension JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ScoringDimension", &self.inner)
    }
}

/// Descriptive statistics of a peer distribution.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import peer_stats
/// >>> peer_stats([1.0, 2.0, 3.0, 4.0, 5.0]).median
/// 3.0
#[pyclass(
    name = "PeerStats",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyPeerStats {
    pub(crate) inner: PeerStats,
}

#[pymethods]
impl PyPeerStats {
    /// Number of observations.
    #[getter]
    fn count(&self) -> usize {
        self.inner.count
    }

    /// Arithmetic mean.
    #[getter]
    fn mean(&self) -> f64 {
        self.inner.mean
    }

    /// Median.
    #[getter]
    fn median(&self) -> f64 {
        self.inner.median
    }

    /// Sample standard deviation.
    #[getter]
    fn std_dev(&self) -> f64 {
        self.inner.std_dev
    }

    /// Minimum.
    #[getter]
    fn min(&self) -> f64 {
        self.inner.min
    }

    /// Maximum.
    #[getter]
    fn max(&self) -> f64 {
        self.inner.max
    }

    /// First quartile.
    #[getter]
    fn q1(&self) -> f64 {
        self.inner.q1
    }

    /// Third quartile.
    #[getter]
    fn q3(&self) -> f64 {
        self.inner.q3
    }

    /// Interquartile range ``q3 - q1``.
    #[getter]
    fn iqr(&self) -> f64 {
        self.inner.iqr
    }

    /// Export as a single-row pandas ``DataFrame`` with one column per statistic.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &self.inner,
            &[
                "count", "mean", "median", "std_dev", "min", "max", "q1", "q3", "iqr",
            ],
        )
    }

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "PeerStats"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``PeerStats`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid PeerStats JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("PeerStats", &self.inner)
    }
}

/// Single-factor OLS fit evaluated at the subject.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import regression_fair_value
/// >>> regression_fair_value([1.0, 2.0, 3.0, 4.0], [3.0, 5.0, 7.0, 9.0], 3.0, 10.0).fitted_value
/// 7.0
#[pyclass(
    name = "RegressionResult",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyRegressionResult {
    pub(crate) inner: RegressionResult,
}

#[pymethods]
impl PyRegressionResult {
    /// Intercept (alpha).
    #[getter]
    fn intercept(&self) -> f64 {
        self.inner.intercept
    }

    /// Slope (beta).
    #[getter]
    fn slope(&self) -> f64 {
        self.inner.slope
    }

    /// Coefficient of determination.
    #[getter]
    fn r_squared(&self) -> f64 {
        self.inner.r_squared
    }

    /// ``intercept + slope * subject_x``.
    #[getter]
    fn fitted_value(&self) -> f64 {
        self.inner.fitted_value
    }

    /// ``subject_y - fitted_value``.
    #[getter]
    fn residual(&self) -> f64 {
        self.inner.residual
    }

    /// Number of observations used.
    #[getter]
    fn n(&self) -> usize {
        self.inner.n
    }

    /// Export as a single-row pandas ``DataFrame``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &self.inner,
            &[
                "intercept",
                "slope",
                "r_squared",
                "fitted_value",
                "residual",
                "n",
            ],
        )
    }

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "RegressionResult"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``RegressionResult`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid RegressionResult JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("RegressionResult", &self.inner)
    }
}

/// Decomposed score of one dimension in a `RelativeValueResult`.
#[pyclass(
    name = "DimensionScore",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyDimensionScore {
    pub(crate) inner: DimensionScore,
}

#[pymethods]
impl PyDimensionScore {
    /// Dimension label.
    #[getter]
    fn label(&self) -> &str {
        &self.inner.label
    }

    /// Percentile rank of the subject within peers (0-1).
    #[getter]
    fn percentile(&self) -> f64 {
        self.inner.percentile
    }

    /// Raw z-score of the subject versus the peer distribution.
    #[getter]
    fn z_score(&self) -> f64 {
        self.inner.z_score
    }

    /// Raw regression residual in Y units, or ``None`` without explanatory X.
    #[getter]
    fn regression_residual(&self) -> Option<f64> {
        self.inner.regression_residual
    }

    /// Regression R-squared, or ``None`` without explanatory X.
    #[getter]
    fn r_squared(&self) -> Option<f64> {
        self.inner.r_squared
    }

    /// Dimension weight in the composite.
    #[getter]
    fn weight(&self) -> f64 {
        self.inner.weight
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("DimensionScore", &self.inner)
    }
}

/// Composite rich/cheap score of a subject against its peers.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import (
/// ...     CompanyMetrics, PeerSet, ScoringDimension, score_relative_value,
/// ... )
/// >>> peers = [CompanyMetrics(f"P{i}", {"leverage": float(i), "oas_bp": 100.0 * i}) for i in (1, 2, 3)]
/// >>> peer_set = PeerSet(CompanyMetrics("SUBJ", {"leverage": 2.0, "oas_bp": 250.0}), peers)
/// >>> result = score_relative_value(peer_set, [ScoringDimension("Spread vs Leverage", "oas_bp", "leverage")])
/// >>> result.company_id, result.peer_count
/// ('SUBJ', 3)
#[pyclass(
    name = "RelativeValueResult",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyRelativeValueResult {
    pub(crate) inner: RelativeValueResult,
}

#[pymethods]
impl PyRelativeValueResult {
    /// Subject company id.
    #[getter]
    fn company_id(&self) -> &str {
        self.inner.company_id.as_str()
    }

    /// Weighted composite score: positive = cheap, negative = rich.
    #[getter]
    fn composite_score(&self) -> f64 {
        self.inner.composite_score
    }

    /// Per-dimension decomposition.
    #[getter]
    fn dimensions(&self) -> Vec<PyDimensionScore> {
        self.inner
            .dimensions
            .iter()
            .cloned()
            .map(|inner| PyDimensionScore { inner })
            .collect()
    }

    /// Confidence in ``[0, 1]`` from peer count and regression fit.
    #[getter]
    fn confidence(&self) -> f64 {
        self.inner.confidence
    }

    /// Number of peers scored against.
    #[getter]
    fn peer_count(&self) -> usize {
        self.inner.peer_count
    }

    /// Export the per-dimension scores as a pandas ``DataFrame``.
    ///
    /// Columns: ``label``, ``percentile`` (0-1), ``z_score``,
    /// ``regression_residual`` (Y units, ``NaN`` without X), ``r_squared``
    /// (``NaN`` without X), ``weight``. One row per dimension. The composite
    /// score, confidence and peer count are result metadata.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .dimensions
            .iter()
            .map(|dim| {
                serde_json::json!({
                    "label": dim.label,
                    "percentile": dim.percentile,
                    "z_score": dim.z_score,
                    "regression_residual": dim.regression_residual,
                    "r_squared": dim.r_squared,
                    "weight": dim.weight,
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, &DIMENSION_COLUMNS)
    }

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "RelativeValueResult"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``RelativeValueResult`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid RelativeValueResult JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "RelativeValueResult(company_id='{}', composite_score={}, confidence={}, peer_count={}, dimensions={})",
            self.inner.company_id,
            self.inner.composite_score,
            self.inner.confidence,
            self.inner.peer_count,
            self.inner.dimensions.len()
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Percentile rank of ``value`` within ``peer_values`` (0-1 scale).
///
/// Uses the "fraction of values less than or equal" convention (Rust
/// ``percentile_rank(values, value)`` argument order).
///
/// Parameters
/// ----------
/// peer_values : list[float]
///     Peer distribution (need not be sorted).
/// value : float
///     The subject value to rank.
///
/// Returns
/// -------
/// float | None
///     Percentile rank in ``[0, 1]``, or ``None`` when ``peer_values`` is empty.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import percentile_rank
/// >>> percentile_rank([100.0, 200.0, 300.0, 400.0, 500.0], 250.0)
/// 0.4
#[pyfunction]
#[pyo3(text_signature = "(peer_values, value)")]
fn percentile_rank(peer_values: Vec<f64>, value: f64) -> Option<f64> {
    core_percentile_rank(&peer_values, value)
}

/// Standard (z-) score of ``value`` in the peer distribution.
///
/// Parameters
/// ----------
/// peer_values : list[float]
///     Peer distribution.
/// value : float
///     The subject value.
///
/// Returns
/// -------
/// float | None
///     ``(value - mean(peers)) / stddev(peers)``, or ``None`` when fewer than
///     two peers are provided or the peer variance is zero.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import z_score
/// >>> z_score([1.0, 2.0, 3.0, 4.0, 5.0], 3.0)
/// 0.0
#[pyfunction]
#[pyo3(text_signature = "(peer_values, value)")]
fn z_score(peer_values: Vec<f64>, value: f64) -> Option<f64> {
    core_z_score(&peer_values, value)
}

/// Descriptive statistics for a peer distribution.
///
/// Parameters
/// ----------
/// peer_values : list[float]
///     Peer distribution (need not be sorted).
///
/// Returns
/// -------
/// PeerStats | None
///     Typed statistics, or ``None`` when no statistics can be computed
///     (matching the WASM twin's ``undefined``).
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import peer_stats
/// >>> peer_stats([1.0, 2.0, 3.0, 4.0, 5.0]).count
/// 5
#[pyfunction]
#[pyo3(text_signature = "(peer_values)")]
fn peer_stats(peer_values: Vec<f64>) -> Option<PyPeerStats> {
    core_peer_stats(&peer_values).map(|inner| PyPeerStats { inner })
}

/// Single-factor OLS fit and evaluation at the subject's X.
///
/// Conventions: ``fitted_value = intercept + slope * subject_x`` and
/// ``residual = subject_y - fitted_value``.
///
/// Parameters
/// ----------
/// x_values : list[float]
///     Peer X observations (independent variable).
/// y_values : list[float]
///     Peer Y observations; same length as ``x_values``.
/// subject_x : float
///     Subject's X value at which to evaluate the fit.
/// subject_y : float
///     Subject's observed Y value for the residual.
///
/// Returns
/// -------
/// RegressionResult | None
///     Typed fit, or ``None`` if fewer than three observations are available
///     or X has zero variance.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import regression_fair_value
/// >>> regression_fair_value([1.0, 2.0, 3.0, 4.0], [3.0, 5.0, 7.0, 9.0], 3.0, 10.0).residual
/// 3.0
#[pyfunction]
#[pyo3(text_signature = "(x_values, y_values, subject_x, subject_y)")]
fn regression_fair_value(
    x_values: Vec<f64>,
    y_values: Vec<f64>,
    subject_x: f64,
    subject_y: f64,
) -> Option<PyRegressionResult> {
    core_regression(&x_values, &y_values, subject_x, subject_y)
        .map(|inner| PyRegressionResult { inner })
}

/// Compute a canonical valuation multiple for one company.
///
/// Parameters
/// ----------
/// company_metrics : CompanyMetrics | dict[str, float]
///     Typed metrics or a flat ``{metric_name: value}`` dict; only the fields
///     the multiple needs must be populated.
/// multiple : str
///     Serde name of the multiple: ``ev_ebitda``, ``ev_revenue``, ``ev_ebit``,
///     ``ev_fcf``, ``pe``, ``pb``, ``ptbv``, ``p_fcf``, ``dividend_yield``,
///     ``spread_per_turn`` or ``yield_per_coverage``.
///
/// Returns
/// -------
/// float | None
///     Multiple value, or ``None`` when a required input is missing or the
///     denominator is not positive.
///
/// Raises
/// ------
/// ValueError
///     If ``multiple`` is not a known name or a metric value is not numeric.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import compute_multiple
/// >>> compute_multiple({"enterprise_value": 8_500.0, "ebitda": 1_000.0}, "ev_ebitda")
/// 8.5
#[pyfunction]
#[pyo3(text_signature = "(company_metrics, multiple)")]
fn compute_multiple(company_metrics: &Bound<'_, PyAny>, multiple: &str) -> PyResult<Option<f64>> {
    let metrics = if let Ok(typed) = company_metrics.extract::<PyRef<'_, PyCompanyMetrics>>() {
        typed.inner.clone()
    } else {
        dict_to_company_metrics("subject", company_metrics.cast::<PyDict>()?)?
    };
    let multiple: Multiple = multiple.parse().map_err(display_to_py)?;
    Ok(core_compute_multiple(&metrics, multiple))
}

/// Extract scoring dimensions from typed objects, serde dicts, or JSON.
fn extract_dimensions(
    py: Python<'_>,
    dimensions: &Bound<'_, PyAny>,
) -> PyResult<Vec<ScoringDimension>> {
    if let Ok(list) = dimensions.cast::<PyList>() {
        return list
            .iter()
            .map(|item| {
                if let Ok(typed) = item.extract::<PyRef<'_, PyScoringDimension>>() {
                    Ok(typed.inner.clone())
                } else {
                    extract_serde_any(py, &item, "dimensions")
                }
            })
            .collect();
    }
    extract_serde_any(py, dimensions, "dimensions")
}

/// Score a subject against its peers across weighted dimensions.
///
/// The composite is the weighted average of the direction-adjusted dimension
/// scores: positive = cheap, negative = rich.
///
/// Parameters
/// ----------
/// peer_set : PeerSet | dict | str
///     Typed peer set, or the canonical serde ``PeerSet`` payload as a dict or
///     JSON string (``{"subject": ..., "peers": [...], "period_basis": "ltm"}``).
/// dimensions : list[ScoringDimension | dict] | str
///     Typed dimensions, canonical ``ScoringDimension`` dicts, or a JSON list.
///
/// Returns
/// -------
/// RelativeValueResult
///     Composite score, per-dimension breakdown, confidence and peer count.
///
/// Raises
/// ------
/// ValueError
///     If a payload is malformed, a direction or extractor is unknown, or the
///     peer set cannot be scored (no peers with the required metrics).
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import (
/// ...     CompanyMetrics, PeerSet, ScoringDimension, score_relative_value,
/// ... )
/// >>> peers = [CompanyMetrics(f"P{i}", {"pe": float(10 * i)}) for i in (1, 2, 3)]
/// >>> peer_set = PeerSet(CompanyMetrics("SUBJ", {"pe": 30.0}), peers)
/// >>> score_relative_value(peer_set, [ScoringDimension("pe", "pe", direction="higher_is_rich")]).composite_score < 0
/// True
#[pyfunction]
#[pyo3(text_signature = "(peer_set, dimensions)")]
fn score_relative_value(
    py: Python<'_>,
    peer_set: &Bound<'_, PyAny>,
    dimensions: &Bound<'_, PyAny>,
) -> PyResult<PyRelativeValueResult> {
    let peer_set: PeerSet = if let Ok(typed) = peer_set.extract::<PyRef<'_, PyPeerSet>>() {
        typed.inner.clone()
    } else {
        extract_serde_any(py, peer_set, "peer_set")?
    };
    let dims = extract_dimensions(py, dimensions)?;
    let inner = core_score(&peer_set, &dims).map_err(core_to_py)?;
    Ok(PyRelativeValueResult { inner })
}

/// Register comps bindings on the analytics submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCompanyMetrics>()?;
    m.add_class::<PyPeerFilter>()?;
    m.add_class::<PyPeerSet>()?;
    m.add_class::<PyScoringDimension>()?;
    m.add_class::<PyPeerStats>()?;
    m.add_class::<PyRegressionResult>()?;
    m.add_class::<PyDimensionScore>()?;
    m.add_class::<PyRelativeValueResult>()?;
    m.add_function(wrap_pyfunction!(percentile_rank, m)?)?;
    m.add_function(wrap_pyfunction!(z_score, m)?)?;
    m.add_function(wrap_pyfunction!(peer_stats, m)?)?;
    m.add_function(wrap_pyfunction!(regression_fair_value, m)?)?;
    m.add_function(wrap_pyfunction!(compute_multiple, m)?)?;
    m.add_function(wrap_pyfunction!(score_relative_value, m)?)?;
    Ok(())
}
