//! Comparable company metrics, peer filters, and peer-set Python bindings.

use crate::bindings::macros::wire_methods;
use finstack_quant_core::types::Attributes;
use finstack_quant_statements_analytics::analysis::{
    CompanyMetrics, MetricExtractor, PeerFilter, PeerSet, PeriodBasis,
};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};
use std::collections::{BTreeMap, BTreeSet};

use crate::errors::display_to_py;

/// Parse a period basis: ``"ltm"``, ``"ntm"`` or any other label as ``custom``.
pub(super) fn parse_period_basis(label: &str) -> PeriodBasis {
    finstack_quant_core::wire::serde_parse::<PeriodBasis>(label)
        .unwrap_or_else(|_| PeriodBasis::Custom(label.to_string()))
}

pub(crate) fn period_basis_label(basis: &PeriodBasis) -> String {
    match basis {
        PeriodBasis::Custom(label) => label.clone(),
        other => finstack_quant_core::wire::serde_label(other).unwrap_or_default(),
    }
}

pub(super) fn parse_extractor(name: &str) -> PyResult<MetricExtractor> {
    name.parse().map_err(display_to_py)
}

pub(super) fn extractor_label(extractor: &MetricExtractor) -> String {
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
        self.inner.get(name)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CompanyMetrics", &self.inner)
    }
}

wire_methods!(PyCompanyMetrics, CompanyMetrics, "CompanyMetrics");

/// Convert a ``{metric_name: value}`` dict into a `CompanyMetrics`.
///
/// Known field names are mapped onto their dedicated optional fields;
/// everything else is stored in the `custom` map. ``None`` is a missing
/// metric (the Rust ``CompanyMetrics::from_flat_metrics`` rule); any other
/// non-numeric value raises ``ValueError``.
pub(super) fn dict_to_company_metrics(id: &str, d: &Bound<'_, PyDict>) -> PyResult<CompanyMetrics> {
    let mut values = Vec::with_capacity(d.len());
    for (key, val) in d.iter() {
        let name: String = key.extract()?;
        let Ok(v) = val.extract::<Option<f64>>() else {
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
pub(crate) fn extract_company_metrics(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
) -> PyResult<CompanyMetrics> {
    if let Ok(metrics) = obj.extract::<PyRef<'_, PyCompanyMetrics>>() {
        return Ok(metrics.inner.clone());
    }
    crate::bindings::module_utils::spec_from_py(py, obj, "company_metrics")
}

/// Screening criteria for building a peer set from a universe.
///
/// All non-empty criteria are AND-ed; list criteria are OR-ed within. Every
/// parameter is keyword-only and optional; Rust reads them as the serde
/// struct, so an unknown keyword raises ``ValueError``.
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
    #[pyo3(
        signature = (**fields),
        text_signature = "(*, gics_sectors=[], gics_industries=[], countries=[], market_cap_min=None, market_cap_max=None, ratings=[], required_tags=[], excluded_tags=[], selectors=[])"
    )]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let inner: PeerFilter =
            crate::bindings::module_utils::fields_from_py(py, fields, "PeerFilter")?;
        Ok(Self { inner })
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

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("PeerFilter", &self.inner)
    }
}

wire_methods!(PyPeerFilter, PeerFilter, "PeerFilter");

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
///     Required: Rust ``PeerSet`` has no default period basis.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import CompanyMetrics, PeerSet
/// >>> subject = CompanyMetrics("SUBJ", {"leverage": 2.0})
/// >>> peers = [CompanyMetrics("P1", {"leverage": 1.0}), CompanyMetrics("P2", {"leverage": 3.0})]
/// >>> PeerSet(subject, peers, "ltm").peer_count
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
    #[pyo3(signature = (subject, peers, period_basis))]
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
    ///     ``"ltm"``, ``"ntm"`` or a custom label (required).
    ///
    /// Returns
    /// -------
    /// tuple[PeerSet, int]
    ///     The peer set and the number of universe members the filter dropped;
    ///     a member with the subject's id is not counted.
    #[staticmethod]
    #[pyo3(signature = (subject, universe, filter, period_basis))]
    fn from_universe(
        py: Python<'_>,
        subject: &Bound<'_, PyAny>,
        universe: Vec<Bound<'_, PyAny>>,
        filter: &PyPeerFilter,
        period_basis: &str,
    ) -> PyResult<(Self, usize)> {
        let subject = extract_company_metrics(py, subject)?;
        let universe = universe
            .iter()
            .map(|company| extract_company_metrics(py, company))
            .collect::<PyResult<Vec<_>>>()?;
        let (inner, excluded_count) = PeerSet::from_universe(
            subject,
            &universe,
            &filter.inner,
            parse_period_basis(period_basis),
        );
        Ok((Self { inner }, excluded_count))
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
    ///     ``"ltm"``, ``"ntm"`` or a custom label (required).
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
    #[pyo3(signature = (df, subject_id, period_basis, id_column=None))]
    fn from_dataframe(
        df: &Bound<'_, PyAny>,
        subject_id: &str,
        period_basis: &str,
        id_column: Option<&str>,
    ) -> PyResult<Self> {
        use crate::bindings::dataframe_utils::{
            has_arrow_c_stream, is_missing_cell, is_polars_dataframe, table_column_names,
            table_to_py_records,
        };
        let py = df.py();
        if is_polars_dataframe(df)? || has_arrow_c_stream(df)? {
            let cols = table_column_names(df)?;
            if cols.is_empty() {
                return Err(crate::errors::value_error("DataFrame has no columns"));
            }
            let id_col = match id_column {
                Some(c) => {
                    if !cols.iter().any(|name| name == c) {
                        return Err(pyo3::exceptions::PyKeyError::new_err(format!(
                            "id_column {c:?} not found in DataFrame"
                        )));
                    }
                    c.to_owned()
                }
                None => cols
                    .iter()
                    .find(|c| matches!(c.as_str(), "id" | "company_id"))
                    .cloned()
                    .unwrap_or_else(|| cols[0].clone()),
            };
            let rows_py = table_to_py_records(py, df)?
                .ok_or_else(|| crate::errors::value_error("expected table records"))?;
            let rows: Vec<Bound<'_, PyDict>> = rows_py.extract()?;
            let mut subject = None;
            let mut peers = Vec::with_capacity(rows.len());
            for row in rows {
                let copy = row.copy()?;
                let id_val = copy.call_method1("pop", (&id_col,))?;
                let id: String = id_val.str()?.extract()?;
                let mut values = Vec::with_capacity(copy.len());
                let mut meta = BTreeMap::new();
                for (col_key, cell) in copy.iter() {
                    let column: String = col_key.extract()?;
                    if is_missing_cell(&cell)? {
                        values.push((column, None));
                    } else if let Ok(v) = cell.extract::<f64>()
                        && !cell.is_instance_of::<pyo3::types::PyBool>()
                    {
                        values.push((column, (!v.is_nan()).then_some(v)));
                    } else if let Ok(s) = cell.extract::<String>() {
                        meta.insert(column, s);
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
            return Ok(Self {
                inner: PeerSet::new(subject, peers, parse_period_basis(period_basis)),
            });
        }

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

        let mut subject = None;
        let mut peers = Vec::with_capacity(rows.len());
        for (id, row) in index.iter().zip(rows.iter()) {
            let id: String = id.str()?.extract()?;
            let mut values = Vec::with_capacity(row.len());
            let mut meta = BTreeMap::new();
            for (column, cell) in columns.iter().zip(row.iter()) {
                if is_missing_cell(cell)? {
                    values.push((column.clone(), None));
                } else {
                    match cell.extract::<f64>() {
                        Ok(v) if !cell.is_instance_of::<pyo3::types::PyBool>() => {
                            values.push((column.clone(), (!v.is_nan()).then_some(v)));
                        }
                        _ => match cell.extract::<String>() {
                            Ok(s) => {
                                meta.insert(column.clone(), s);
                            }
                            _ => {
                                return Err(crate::errors::value_error(format!(
                                    "column '{column}' for company '{id}' must be numeric, string or missing"
                                )));
                            }
                        },
                    }
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

    fn __repr__(&self) -> String {
        format!(
            "PeerSet(subject='{}', peers={}, period_basis='{}')",
            self.inner.subject.id,
            self.inner.peers.len(),
            self.period_basis()
        )
    }
}

wire_methods!(PyPeerSet, PeerSet, "PeerSet");
