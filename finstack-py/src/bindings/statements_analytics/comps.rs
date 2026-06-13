//! Python bindings for the comparable company analysis module.
//!
//! Exposes a function-based API for cross-sectional peer analytics:
//!
//! - Descriptive peer statistics (`peer_stats`).
//! - Percentile rank and z-score of a subject within a peer distribution.
//! - Single-factor OLS regression for fair-value estimation.
//! - Canonical valuation multiple computation on `CompanyMetrics`.
//! - Multi-dimension composite rich/cheap scoring (`score_relative_value`).
//!
//! The scoring API takes plain dicts/lists from Python rather than the
//! strongly-typed `CompanyMetrics`/`PeerSet` structs used in Rust. Each
//! peer is a dict keyed by metric name; dimensions are either `(name, weight)`
//! tuples for univariate scoring or dicts with `label`, `y`, optional `x`
//! (one or more selectors), optional `direction`, and `weight` keys for
//! regression-based scoring. Metric selectors map 1:1 onto the Rust
//! `MetricExtractor` enum: named fields, custom keys, and
//! `"multiple:<id>"` for canonical valuation multiples.

use finstack_statements_analytics::analysis::{
    compute_multiple as core_compute_multiple, peer_stats as core_peer_stats,
    percentile_rank as core_percentile_rank, regression_fair_value as core_regression,
    score_relative_value as core_score, z_score as core_z_score, CompanyMetrics, MetricExtractor,
    Multiple, PeerSet, PeriodBasis, ScoreDirection, ScoringDimension,
};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};

use crate::errors::{core_to_py, display_to_py};

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

/// Percentile rank of ``value`` within ``peer_values`` (0-1 scale).
///
/// Uses the "fraction of values less than or equal" convention. Returns
/// ``None`` when ``peer_values`` is empty.
///
/// Arguments:
///     value: The subject value to rank.
///     peer_values: Peer distribution (need not be sorted).
///
/// Returns:
///     Percentile rank in [0, 1], or ``None`` when ``peer_values`` is empty.
#[pyfunction]
#[pyo3(text_signature = "(value, peer_values)")]
fn percentile_rank(value: f64, peer_values: Vec<f64>) -> Option<f64> {
    core_percentile_rank(&peer_values, value)
}

/// Standard (z-) score of ``value`` in the peer distribution.
///
/// Returns ``None`` if fewer than two peers are provided or the peer
/// distribution has zero variance.
///
/// Arguments:
///     value: The subject value.
///     peer_values: Peer distribution.
///
/// Returns:
///     ``(value - mean(peers)) / stddev(peers)``, or ``None`` when undefined.
#[pyfunction]
#[pyo3(text_signature = "(value, peer_values)")]
fn z_score(value: f64, peer_values: Vec<f64>) -> Option<f64> {
    core_z_score(&peer_values, value)
}

/// Descriptive statistics for a peer distribution.
///
/// Arguments:
///     peer_values: Peer distribution (need not be sorted).
///
/// Returns:
///     Dict with keys ``{"mean", "median", "q1", "q3", "iqr", "std_dev",
///     "min", "max", "count"}`` mirroring the Rust ``PeerStats`` field
///     names. Returns an empty dict when ``peer_values`` is empty.
#[pyfunction]
#[pyo3(text_signature = "(peer_values)")]
fn peer_stats<'py>(py: Python<'py>, peer_values: Vec<f64>) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    if let Some(stats) = core_peer_stats(&peer_values) {
        d.set_item("mean", stats.mean)?;
        d.set_item("median", stats.median)?;
        d.set_item("q1", stats.q1)?;
        d.set_item("q3", stats.q3)?;
        d.set_item("iqr", stats.iqr)?;
        d.set_item("std_dev", stats.std_dev)?;
        d.set_item("min", stats.min)?;
        d.set_item("max", stats.max)?;
        d.set_item("count", stats.count)?;
    }
    Ok(d)
}

/// Single-factor OLS fit and evaluation at the subject's X.
///
/// Regresses ``y_values`` on ``x_values`` and returns the fitted value
/// and residual for the subject. Conventions:
///
/// - ``fitted_value = intercept + slope * subject_x``
/// - ``residual = subject_y - fitted_value``.
///
/// Arguments:
///     x_values: Peer X observations (independent variable).
///     y_values: Peer Y observations (dependent variable). Must be
///         the same length as ``x_values``.
///     subject_x: Subject's X value at which to evaluate the fit.
///     subject_y: Subject's observed Y value for residual computation.
///
/// Returns:
///     Dict with keys ``{"slope", "intercept", "r_squared",
///     "fitted_value", "residual", "n"}``. Returns an empty dict if
///     fewer than three observations are available or the regression
///     cannot be computed (e.g., zero variance in X).
#[pyfunction]
#[pyo3(text_signature = "(x_values, y_values, subject_x, subject_y)")]
fn regression_fair_value<'py>(
    py: Python<'py>,
    x_values: Vec<f64>,
    y_values: Vec<f64>,
    subject_x: f64,
    subject_y: f64,
) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    if let Some(reg) = core_regression(&x_values, &y_values, subject_x, subject_y) {
        d.set_item("slope", reg.slope)?;
        d.set_item("intercept", reg.intercept)?;
        d.set_item("r_squared", reg.r_squared)?;
        d.set_item("fitted_value", reg.fitted_value)?;
        d.set_item("residual", reg.residual)?;
        d.set_item("n", reg.n)?;
    }
    Ok(d)
}

// ---------------------------------------------------------------------------
// Multiples
// ---------------------------------------------------------------------------

/// Compute a canonical valuation multiple for one company.
///
/// ``company_metrics`` is a Python dict matching the Rust
/// ``CompanyMetrics`` shape; only the fields needed for the chosen
/// multiple must be populated.
///
/// Arguments:
///     company_metrics: Dict of company metrics keyed by canonical field name.
///     multiple: Canonical multiple selector such as ``"EvEbitda"`` or ``"Pe"``.
///
/// Returns:
///     Multiple value, or ``None`` when required inputs are missing or invalid.
#[pyfunction]
#[pyo3(text_signature = "(company_metrics, multiple)")]
fn compute_multiple(company_metrics: &Bound<'_, PyDict>, multiple: &str) -> PyResult<Option<f64>> {
    let metrics = dict_to_company_metrics("subject", company_metrics)?;
    let multiple: Multiple = multiple.parse().map_err(display_to_py)?;
    Ok(core_compute_multiple(&metrics, multiple))
}

// ---------------------------------------------------------------------------
// Composite relative-value scoring
// ---------------------------------------------------------------------------

/// Convert a ``{metric_name: value}`` dict into a `CompanyMetrics`.
///
/// Known field names (e.g. ``"leverage"``, ``"oas_bps"``, ``"ebitda"``)
/// are mapped onto their dedicated optional fields; everything else is
/// stored in the `custom` map. ``None`` values are treated as missing;
/// any other non-numeric value raises ``ValueError`` naming the key.
fn dict_to_company_metrics(id: &str, d: &Bound<'_, PyDict>) -> PyResult<CompanyMetrics> {
    let mut m = CompanyMetrics::new(id);
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
        match name.as_str() {
            "enterprise_value" => m.enterprise_value = Some(v),
            "market_cap" => m.market_cap = Some(v),
            "share_price" => m.share_price = Some(v),
            "oas_bps" => m.oas_bps = Some(v),
            "yield_pct" => m.yield_pct = Some(v),
            "ebitda" => m.ebitda = Some(v),
            "revenue" => m.revenue = Some(v),
            "ebit" => m.ebit = Some(v),
            "ufcf" => m.ufcf = Some(v),
            "lfcf" => m.lfcf = Some(v),
            "net_income" => m.net_income = Some(v),
            "book_value" => m.book_value = Some(v),
            "tangible_book_value" => m.tangible_book_value = Some(v),
            "dividends_per_share" => m.dividends_per_share = Some(v),
            "leverage" => m.leverage = Some(v),
            "interest_coverage" => m.interest_coverage = Some(v),
            "revenue_growth" => m.revenue_growth = Some(v),
            "ebitda_margin" => m.ebitda_margin = Some(v),
            _ => {
                m.custom.insert(name, v);
            }
        }
    }
    Ok(m)
}

/// Whether ``name`` maps onto a named field on `CompanyMetrics` (vs. a
/// custom-map entry). Used to pick the right `MetricExtractor` variant.
fn is_named_field(name: &str) -> bool {
    matches!(
        name,
        "enterprise_value"
            | "market_cap"
            | "share_price"
            | "oas_bps"
            | "yield_pct"
            | "ebitda"
            | "revenue"
            | "ebit"
            | "ufcf"
            | "lfcf"
            | "net_income"
            | "book_value"
            | "tangible_book_value"
            | "dividends_per_share"
            | "leverage"
            | "interest_coverage"
            | "revenue_growth"
            | "ebitda_margin"
    )
}

/// Map a metric selector string onto a `MetricExtractor`.
///
/// - ``"multiple:<id>"`` (e.g. ``"multiple:ev_ebitda"``) selects a canonical
///   valuation multiple computed on the fly from `CompanyMetrics`.
/// - Known field names select the dedicated optional field.
/// - Anything else selects an entry in the `custom` map.
fn metric_extractor(name: &str) -> PyResult<MetricExtractor> {
    if let Some(multiple) = name.strip_prefix("multiple:") {
        let multiple: Multiple = multiple.parse().map_err(display_to_py)?;
        Ok(MetricExtractor::Multiple(multiple))
    } else if is_named_field(name) {
        Ok(MetricExtractor::Named(name.to_string()))
    } else {
        Ok(MetricExtractor::Custom(name.to_string()))
    }
}

/// Parse an optional ``direction`` key (``"higher_is_cheap"`` /
/// ``"higher_is_rich"``) from a dimension dict; defaults to the Rust
/// `ScoreDirection` default (`HigherIsCheap`).
fn parse_direction(dict: &Bound<'_, PyDict>) -> PyResult<ScoreDirection> {
    match dict.get_item("direction")? {
        None => Ok(ScoreDirection::default()),
        Some(value) => {
            let s: String = value.extract()?;
            match s.as_str() {
                "higher_is_cheap" => Ok(ScoreDirection::HigherIsCheap),
                "higher_is_rich" => Ok(ScoreDirection::HigherIsRich),
                other => Err(crate::errors::value_error(format!(
                    "unknown direction '{other}' (expected 'higher_is_cheap' or 'higher_is_rich')"
                ))),
            }
        }
    }
}

fn dict_get_string_any(dict: &Bound<'_, PyDict>, keys: &[&str]) -> PyResult<Option<String>> {
    for key in keys {
        if let Some(value) = dict.get_item(*key)? {
            return value.extract::<String>().map(Some);
        }
    }
    Ok(None)
}

fn parse_scoring_dimension(obj: &Bound<'_, PyAny>) -> PyResult<ScoringDimension> {
    if let Ok((name, weight)) = obj.extract::<(String, f64)>() {
        return Ok(ScoringDimension {
            label: name.clone(),
            y_extractor: metric_extractor(&name)?,
            x_extractors: vec![],
            weight,
            direction: ScoreDirection::default(),
        });
    }

    let dict = obj.cast::<PyDict>().map_err(|_| {
        crate::errors::value_error(
            "dimension must be a (metric_name, weight) tuple or a dict with \
             label/y/x/weight/direction",
        )
    })?;

    let y_name = dict_get_string_any(dict, &["y", "y_extractor", "metric"])?
        .ok_or_else(|| crate::errors::value_error("dimension dict missing required key 'y'"))?;
    let label = dict_get_string_any(dict, &["label", "name"])?.unwrap_or_else(|| y_name.clone());
    let weight = match dict.get_item("weight")? {
        Some(value) => value.extract::<f64>()?,
        None => 1.0,
    };
    let x_extractors = match dict.get_item("x")?.or(dict.get_item("x_extractors")?) {
        Some(value) => {
            if let Ok(name) = value.extract::<String>() {
                vec![metric_extractor(&name)?]
            } else {
                let names = value.extract::<Vec<String>>()?;
                names
                    .into_iter()
                    .map(|name| metric_extractor(&name))
                    .collect::<PyResult<_>>()?
            }
        }
        None => vec![],
    };

    Ok(ScoringDimension {
        label,
        y_extractor: metric_extractor(&y_name)?,
        x_extractors,
        weight,
        direction: parse_direction(dict)?,
    })
}

/// Score a subject against its peers across multiple weighted dimensions.
///
/// Dimensions may be ``(metric_name, weight)`` tuples for univariate
/// scoring or dicts of the form ``{"label": str, "y": str, "x": [str],
/// "weight": float, "direction": str}`` for regression-based fair-value
/// scoring. Metric selectors are plain metric names (named field or custom
/// key) or ``"multiple:<id>"`` (e.g. ``"multiple:ev_ebitda"``) for canonical
/// valuation multiples — the same capabilities as the Rust
/// ``MetricExtractor`` enum. ``direction`` is ``"higher_is_cheap"``
/// (default, spread-like: higher Y than peers scores positive = cheap) or
/// ``"higher_is_rich"`` (multiple-like: higher Y scores negative = rich);
/// it applies consistently to both the univariate z-score path and the
/// regression-residual path. The composite is the weighted average where
/// positive = cheap, negative = rich.
///
/// Arguments:
///     subject_metrics: Dict of ``{metric_name: value}`` for the subject.
///     peer_metrics: List of dicts, one per peer, same schema as the
///         subject.
///     dimensions: List of tuple or dict dimensions selecting which metrics
///         to score and their composite weights.
///
/// Returns:
///     Dict with canonical keys ``{"company_id", "composite_score",
///     "dimensions", "confidence", "peer_count"}``. ``dimensions`` is a
///     list of dicts with ``label``, ``percentile``, ``z_score``,
///     ``regression_residual``, ``r_squared``, and ``weight``.
#[pyfunction]
#[pyo3(text_signature = "(subject_metrics, peer_metrics, dimensions)")]
fn score_relative_value<'py>(
    py: Python<'py>,
    subject_metrics: &Bound<'_, PyDict>,
    peer_metrics: Vec<Bound<'_, PyDict>>,
    dimensions: Vec<Bound<'_, PyAny>>,
) -> PyResult<Bound<'py, PyDict>> {
    // Build CompanyMetrics for subject + peers.
    let subject = dict_to_company_metrics("SUBJECT", subject_metrics)?;
    let mut peers: Vec<CompanyMetrics> = Vec::with_capacity(peer_metrics.len());
    for (i, pd) in peer_metrics.iter().enumerate() {
        peers.push(dict_to_company_metrics(&format!("PEER_{i}"), pd)?);
    }

    let peer_set = PeerSet::new(subject, peers, PeriodBasis::Ltm);

    let scoring_dims: Vec<ScoringDimension> = dimensions
        .iter()
        .map(parse_scoring_dimension)
        .collect::<PyResult<_>>()?;

    let result = core_score(&peer_set, &scoring_dims).map_err(core_to_py)?;

    let out = PyDict::new(py);
    out.set_item("company_id", result.company_id)?;
    out.set_item("composite_score", result.composite_score)?;
    out.set_item("confidence", result.confidence)?;
    out.set_item("peer_count", result.peer_count)?;

    let dimensions = PyList::empty(py);
    for d in &result.dimensions {
        let dim_dict = PyDict::new(py);
        dim_dict.set_item("label", &d.label)?;
        dim_dict.set_item("percentile", d.percentile)?;
        dim_dict.set_item("z_score", d.z_score)?;
        dim_dict.set_item("regression_residual", d.regression_residual)?;
        dim_dict.set_item("r_squared", d.r_squared)?;
        dim_dict.set_item("weight", d.weight)?;
        dimensions.append(dim_dict)?;
    }
    out.set_item("dimensions", dimensions)?;

    Ok(out)
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register comps bindings on the analytics submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(percentile_rank, m)?)?;
    m.add_function(wrap_pyfunction!(z_score, m)?)?;
    m.add_function(wrap_pyfunction!(peer_stats, m)?)?;
    m.add_function(wrap_pyfunction!(regression_fair_value, m)?)?;
    m.add_function(wrap_pyfunction!(compute_multiple, m)?)?;
    m.add_function(wrap_pyfunction!(score_relative_value, m)?)?;
    Ok(())
}
