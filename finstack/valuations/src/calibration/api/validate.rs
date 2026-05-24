//! Static envelope validator and dependency-graph utilities.
//!
//! [`validate`] runs all structural checks (missing dependencies, undefined
//! `quote_set`s) and returns a [`ValidationReport`] listing every error found
//! plus the dependency graph of the steps. No solver is invoked — the
//! validator runs in microseconds.
//!
//! Genuine cycles cannot occur in the current `CalibrationPlan` model:
//! steps execute in declared order and read only from `market_data` /
//! `prior_market` or curves produced by *earlier* steps. A self-referential
//! step would surface as a [`EnvelopeError::MissingDependency`] rather than a
//! cycle.
//!
//! [`dry_run`] and [`dependency_graph_json`] are JSON-string wrappers for
//! cross-binding consumption (Python / WASM).

// `EnvelopeError` is intentionally large (carries available-IDs lists, etc.)
// because the cross-binding consumers want all the diagnostic context in
// one shot. Boxing the error would harm ergonomics on a cold error path.
#![allow(clippy::result_large_err)]

use serde::Serialize;
use std::collections::{BTreeSet, HashSet};

use crate::calibration::api::errors::EnvelopeError;
use crate::calibration::api::schema::{CalibrationEnvelope, CalibrationStep};
use crate::market::quotes::{
    bond::BondQuote, cds::CdsQuote, cds_tranche::CDSTrancheQuote, fx::FxQuote,
    inflation::InflationQuote, market_quote::MarketQuote, rates::RateQuote, vol::VolQuote,
    xccy::XccyQuote,
};

/// Result of [`validate`]. Always contains the dependency graph; `errors` is
/// empty when the envelope is structurally valid.
#[derive(Debug, Clone, Serialize)]
pub struct ValidationReport {
    /// All errors found in a single pass; empty if the envelope is valid.
    pub errors: Vec<EnvelopeError>,
    /// Topological view of the steps' inputs and outputs.
    pub dependency_graph: DependencyGraph,
}

/// Static dependency graph derived from a [`CalibrationEnvelope`].
#[derive(Debug, Clone, Serialize)]
pub struct DependencyGraph {
    /// Curve / surface IDs available at the start of execution, contributed
    /// by `market_data` and `prior_market`.
    pub initial_ids: Vec<String>,
    /// Per-step inputs and outputs in declared order.
    pub nodes: Vec<DependencyNode>,
}

/// A single step's view of the dependency graph.
#[derive(Debug, Clone, Serialize)]
pub struct DependencyNode {
    /// Zero-based index in `plan.steps`.
    pub step_index: usize,
    /// Step identifier.
    pub step_id: String,
    /// Step kind (`"discount"`, `"forward"`, ...).
    pub kind: String,
    /// Curve / surface IDs the step depends on. Each must be either in
    /// `initial_ids` or produced by an earlier step.
    pub reads: Vec<String>,
    /// Curve / surface ID(s) the step produces.
    pub writes: Vec<String>,
}

/// Run all static validation checks.
///
/// Always returns a [`ValidationReport`]; inspect `errors` to see what failed.
/// The validator is solver-free — runs in microseconds, suitable as a
/// pre-flight check before invoking [`engine::execute`](super::engine::execute).
pub fn validate(envelope: &CalibrationEnvelope) -> ValidationReport {
    let mut errors = Vec::new();
    let initial_ids = collect_initial_ids(envelope);
    let nodes = build_nodes(&envelope.plan.steps);

    check_quote_sets(envelope, &mut errors);
    check_market_data_uniqueness(envelope, &mut errors);
    check_quote_sets_resolve(envelope, &mut errors);
    check_quote_data(envelope, &mut errors);
    check_dependencies(&initial_ids, &nodes, &mut errors);

    let mut sorted_initial: Vec<String> = initial_ids.into_iter().collect();
    sorted_initial.sort();

    ValidationReport {
        errors,
        dependency_graph: DependencyGraph {
            initial_ids: sorted_initial,
            nodes,
        },
    }
}

/// Wrap [`validate`] to take a JSON string.
///
/// Returns the report serialized as pretty-printed JSON. Returns an
/// [`EnvelopeError::JsonParse`] if the envelope is malformed.
pub fn dry_run(envelope_json: &str) -> Result<String, EnvelopeError> {
    let envelope = parse_envelope_v3(envelope_json)?;
    let report = validate(&envelope);
    serialize_pretty_json(&report, "ValidationReport")
}

/// JSON-friendly wrapper that returns just the dependency graph.
pub fn dependency_graph_json(envelope_json: &str) -> Result<String, EnvelopeError> {
    let envelope = parse_envelope_v3(envelope_json)?;
    let nodes = build_nodes(&envelope.plan.steps);
    let mut sorted_initial: Vec<String> = collect_initial_ids(&envelope).into_iter().collect();
    sorted_initial.sort();
    let graph = DependencyGraph {
        initial_ids: sorted_initial,
        nodes,
    };
    serialize_pretty_json(&graph, "DependencyGraph")
}

// =============================================================================
// Internal helpers
// =============================================================================

fn serialize_pretty_json<T: Serialize>(value: &T, target: &str) -> Result<String, EnvelopeError> {
    serde_json::to_string_pretty(value).map_err(|err| EnvelopeError::JsonSerialize {
        target: target.to_string(),
        message: err.to_string(),
    })
}

/// Parse a JSON envelope, returning a friendly error for legacy v2 envelopes.
///
/// The v3 envelope (see [`CalibrationEnvelope`]) replaced the v2 `initial_market`
/// section with flat `market_data` / `prior_market` lists. Envelopes that still
/// carry the v2 `initial_market` key get a targeted message pointing at the
/// migration design doc rather than the generic serde `unknown field` error.
pub fn parse_envelope_v3(json: &str) -> Result<CalibrationEnvelope, EnvelopeError> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| EnvelopeError::JsonParse {
            message: e.to_string(),
            line: Some(e.line() as u32),
            col: Some(e.column() as u32),
        })?;
    if value.get("initial_market").is_some() {
        return Err(EnvelopeError::JsonParse {
            message: "envelope schema v2 is no longer supported; see \
                      docs/archive/plans/2026-05-10-calibration-envelope-cleanup-design.md \
                      for the v3 shape"
                .to_string(),
            line: None,
            col: None,
        });
    }
    serde_json::from_value(value).map_err(|e| EnvelopeError::JsonParse {
        message: e.to_string(),
        line: None,
        col: None,
    })
}

fn collect_initial_ids(envelope: &CalibrationEnvelope) -> HashSet<String> {
    let mut ids = HashSet::new();
    for obj in &envelope.prior_market {
        ids.insert(obj.id().to_string());
    }
    ids
}

fn build_nodes(steps: &[CalibrationStep]) -> Vec<DependencyNode> {
    steps
        .iter()
        .enumerate()
        .map(|(idx, step)| {
            let io = step.params.io();
            DependencyNode {
                step_index: idx,
                step_id: step.id.clone(),
                kind: io.kind.to_string(),
                reads: io.reads,
                writes: io.writes,
            }
        })
        .collect()
}

fn check_quote_sets(envelope: &CalibrationEnvelope, errors: &mut Vec<EnvelopeError>) {
    let mut available: Vec<String> = envelope.plan.quote_sets.keys().cloned().collect();
    available.sort();
    for (idx, step) in envelope.plan.steps.iter().enumerate() {
        if !envelope.plan.quote_sets.contains_key(&step.quote_set) {
            errors.push(EnvelopeError::UndefinedQuoteSet {
                step_index: idx,
                step_id: step.id.clone(),
                ref_name: step.quote_set.clone(),
                available: available.clone(),
                suggestion: closest_match(&step.quote_set, &available),
            });
        }
    }
}

fn check_market_data_uniqueness(envelope: &CalibrationEnvelope, errors: &mut Vec<EnvelopeError>) {
    use std::collections::HashMap;
    let mut seen: HashMap<&str, BTreeSet<String>> = HashMap::new();
    for datum in &envelope.market_data {
        let key = if datum.is_quote() {
            "quote"
        } else {
            datum.kind_name()
        };
        if !seen.entry(key).or_default().insert(datum.id().to_string()) {
            errors.push(EnvelopeError::DuplicateMarketDatumId {
                datum_kind: key.to_string(),
                id: datum.id().to_string(),
            });
        }
    }
}

fn check_quote_sets_resolve(envelope: &CalibrationEnvelope, errors: &mut Vec<EnvelopeError>) {
    let quote_ids: HashSet<&str> = envelope
        .market_data
        .iter()
        .filter(|d| d.is_quote())
        .map(|d| d.id())
        .collect();
    for (set_name, ids) in &envelope.plan.quote_sets {
        for qid in ids {
            if !quote_ids.contains(qid.as_str()) {
                errors.push(EnvelopeError::QuoteIdNotInMarketData {
                    quote_set: set_name.clone(),
                    id: qid.to_string(),
                });
            }
        }
    }
}

fn check_quote_data(envelope: &CalibrationEnvelope, errors: &mut Vec<EnvelopeError>) {
    let quote_by_id: std::collections::HashMap<&str, MarketQuote> = envelope
        .market_data
        .iter()
        .filter_map(|datum| datum.as_quote().map(|quote| (datum.id(), quote)))
        .collect();

    for step in &envelope.plan.steps {
        let Some(quote_ids) = envelope.plan.quote_sets.get(&step.quote_set) else {
            continue;
        };
        for quote_id in quote_ids {
            if let Some(quote) = quote_by_id.get(quote_id.as_str()) {
                validate_quote_payload(&step.id, quote, errors);
            }
        }
    }
}

fn validate_quote_payload(step_id: &str, quote: &MarketQuote, errors: &mut Vec<EnvelopeError>) {
    match quote {
        MarketQuote::Bond(q) => validate_bond_quote(step_id, q, errors),
        MarketQuote::Rates(q) => validate_rate_quote(step_id, q, errors),
        MarketQuote::Cds(q) => validate_cds_quote(step_id, q, errors),
        MarketQuote::CDSTranche(q) => validate_cds_tranche_quote(step_id, q, errors),
        MarketQuote::Fx(q) => validate_fx_quote(step_id, q, errors),
        MarketQuote::Inflation(q) => validate_inflation_quote(step_id, q, errors),
        MarketQuote::Vol(q) => validate_vol_quote(step_id, q, errors),
        MarketQuote::Xccy(q) => validate_xccy_quote(step_id, q, errors),
    }
}

fn push_quote_error(
    step_id: &str,
    quote_id: &str,
    reason: impl Into<String>,
    errors: &mut Vec<EnvelopeError>,
) {
    errors.push(EnvelopeError::QuoteDataInvalid {
        step_id: step_id.to_string(),
        quote_id: quote_id.to_string(),
        reason: reason.into(),
    });
}

fn require_finite(
    step_id: &str,
    quote_id: &str,
    field: &str,
    value: f64,
    errors: &mut Vec<EnvelopeError>,
) {
    if !value.is_finite() {
        push_quote_error(
            step_id,
            quote_id,
            format!("{field} must be finite; got {value}"),
            errors,
        );
    }
}

fn require_positive(
    step_id: &str,
    quote_id: &str,
    field: &str,
    value: f64,
    errors: &mut Vec<EnvelopeError>,
) {
    require_finite(step_id, quote_id, field, value, errors);
    if value <= 0.0 {
        push_quote_error(
            step_id,
            quote_id,
            format!("{field} must be positive; got {value}"),
            errors,
        );
    }
}

fn require_unit_interval(
    step_id: &str,
    quote_id: &str,
    field: &str,
    value: f64,
    errors: &mut Vec<EnvelopeError>,
) {
    require_finite(step_id, quote_id, field, value, errors);
    if !(0.0..=1.0).contains(&value) {
        push_quote_error(
            step_id,
            quote_id,
            format!("{field} must be in [0, 1]; got {value}"),
            errors,
        );
    }
}

fn validate_rate_quote(step_id: &str, quote: &RateQuote, errors: &mut Vec<EnvelopeError>) {
    let quote_id = quote.id().as_str();
    match quote {
        RateQuote::Deposit { rate, .. }
        | RateQuote::Fra { rate, .. }
        | RateQuote::Swap { rate, .. } => {
            require_finite(step_id, quote_id, "rate", *rate, errors);
        }
        RateQuote::Futures {
            price,
            convexity_adjustment,
            ..
        } => {
            require_finite(step_id, quote_id, "price", *price, errors);
            if let Some(value) = convexity_adjustment {
                require_finite(step_id, quote_id, "convexity_adjustment", *value, errors);
            }
        }
    }
}

fn validate_cds_quote(step_id: &str, quote: &CdsQuote, errors: &mut Vec<EnvelopeError>) {
    let quote_id = quote.id().as_str();
    match quote {
        CdsQuote::CdsParSpread {
            spread_bp,
            recovery_rate,
            ..
        } => {
            require_positive(step_id, quote_id, "spread_bp", *spread_bp, errors);
            require_unit_interval(step_id, quote_id, "recovery_rate", *recovery_rate, errors);
        }
        CdsQuote::CdsUpfront {
            running_spread_bp,
            upfront_pct,
            recovery_rate,
            ..
        } => {
            require_positive(
                step_id,
                quote_id,
                "running_spread_bp",
                *running_spread_bp,
                errors,
            );
            require_finite(step_id, quote_id, "upfront_pct", *upfront_pct, errors);
            require_unit_interval(step_id, quote_id, "recovery_rate", *recovery_rate, errors);
        }
    }
}

fn validate_cds_tranche_quote(
    step_id: &str,
    quote: &CDSTrancheQuote,
    errors: &mut Vec<EnvelopeError>,
) {
    let quote_id = quote.id().as_str();
    let CDSTrancheQuote::CDSTranche {
        attachment,
        detachment,
        upfront_pct,
        running_spread_bp,
        ..
    } = quote;
    require_unit_interval(step_id, quote_id, "attachment", *attachment, errors);
    require_unit_interval(step_id, quote_id, "detachment", *detachment, errors);
    if attachment >= detachment {
        push_quote_error(
            step_id,
            quote_id,
            format!(
                "attachment must be less than detachment; got attachment={attachment}, detachment={detachment}"
            ),
            errors,
        );
    }
    require_finite(step_id, quote_id, "upfront_pct", *upfront_pct, errors);
    require_positive(
        step_id,
        quote_id,
        "running_spread_bp",
        *running_spread_bp,
        errors,
    );
}

fn validate_fx_quote(step_id: &str, quote: &FxQuote, errors: &mut Vec<EnvelopeError>) {
    let quote_id = quote.id().as_str();
    match quote {
        FxQuote::ForwardOutright { forward_rate, .. } => {
            require_positive(step_id, quote_id, "forward_rate", *forward_rate, errors);
        }
        FxQuote::SwapOutright {
            near_rate,
            far_rate,
            ..
        } => {
            require_positive(step_id, quote_id, "near_rate", *near_rate, errors);
            require_positive(step_id, quote_id, "far_rate", *far_rate, errors);
        }
        FxQuote::OptionVanilla { strike, .. } => {
            require_positive(step_id, quote_id, "strike", *strike, errors);
        }
    }
}

fn validate_inflation_quote(
    step_id: &str,
    quote: &InflationQuote,
    errors: &mut Vec<EnvelopeError>,
) {
    let quote_id = quote.id().as_str();
    match quote {
        InflationQuote::InflationSwap { rate, .. }
        | InflationQuote::YoYInflationSwap { rate, .. } => {
            require_finite(step_id, quote_id, "rate", *rate, errors);
        }
    }
}

fn validate_vol_quote(step_id: &str, quote: &VolQuote, errors: &mut Vec<EnvelopeError>) {
    let quote_id = quote.id().as_str();
    match quote {
        VolQuote::OptionVol { strike, vol, .. } => {
            require_positive(step_id, quote_id, "strike", *strike, errors);
            require_positive(step_id, quote_id, "vol", *vol, errors);
        }
        VolQuote::SwaptionVol { strike, vol, .. } | VolQuote::CapFloorVol { strike, vol, .. } => {
            require_finite(step_id, quote_id, "strike", *strike, errors);
            require_positive(step_id, quote_id, "vol", *vol, errors);
        }
    }
}

fn validate_xccy_quote(step_id: &str, quote: &XccyQuote, errors: &mut Vec<EnvelopeError>) {
    let quote_id = quote.id().as_str();
    let XccyQuote::BasisSwap {
        basis_spread_bp,
        spot_fx,
        ..
    } = quote;
    require_finite(
        step_id,
        quote_id,
        "basis_spread_bp",
        *basis_spread_bp,
        errors,
    );
    if let Some(value) = spot_fx {
        require_positive(step_id, quote_id, "spot_fx", *value, errors);
    }
}

fn validate_bond_quote(step_id: &str, quote: &BondQuote, errors: &mut Vec<EnvelopeError>) {
    let quote_id = quote.id().as_str();
    match quote {
        BondQuote::FixedRateBulletCleanPrice {
            coupon_rate,
            clean_price_pct,
            ..
        } => {
            require_finite(step_id, quote_id, "coupon_rate", *coupon_rate, errors);
            require_positive(
                step_id,
                quote_id,
                "clean_price_pct",
                *clean_price_pct,
                errors,
            );
        }
        BondQuote::FixedRateBulletZSpread {
            coupon_rate,
            z_spread,
            ..
        } => {
            require_finite(step_id, quote_id, "coupon_rate", *coupon_rate, errors);
            require_finite(step_id, quote_id, "z_spread", *z_spread, errors);
        }
        BondQuote::FixedRateBulletOas {
            coupon_rate, oas, ..
        } => {
            require_finite(step_id, quote_id, "coupon_rate", *coupon_rate, errors);
            require_finite(step_id, quote_id, "oas", *oas, errors);
        }
        BondQuote::FixedRateBulletYtm {
            coupon_rate, ytm, ..
        } => {
            require_finite(step_id, quote_id, "coupon_rate", *coupon_rate, errors);
            require_finite(step_id, quote_id, "ytm", *ytm, errors);
        }
    }
}

fn check_dependencies(
    initial_ids: &HashSet<String>,
    nodes: &[DependencyNode],
    errors: &mut Vec<EnvelopeError>,
) {
    let mut available: BTreeSet<String> = initial_ids.iter().cloned().collect();
    for node in nodes {
        for read_id in &node.reads {
            if !available.contains(read_id) {
                errors.push(EnvelopeError::MissingDependency {
                    step_index: node.step_index,
                    step_id: node.step_id.clone(),
                    step_kind: node.kind.clone(),
                    missing_id: read_id.clone(),
                    missing_kind: "curve".to_string(),
                    available: available.iter().cloned().collect(),
                });
            }
        }
        for write_id in &node.writes {
            available.insert(write_id.clone());
        }
    }
}

/// Closest-match suggestion for a misspelled identifier (Levenshtein ≤ 3).
fn closest_match(target: &str, candidates: &[String]) -> Option<String> {
    candidates
        .iter()
        .map(|c| (c, levenshtein(target, c)))
        .filter(|(_, d)| *d > 0 && *d <= 3)
        .min_by_key(|(_, d)| *d)
        .map(|(c, _)| c.clone())
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::api::market_datum::MarketDatum;
    use crate::calibration::api::schema::{
        CalibrationPlan, CalibrationStep, DiscountCurveParams, StepParams, CALIBRATION_SCHEMA,
    };
    use crate::market::conventions::ids::{CdsConventionKey, CdsDocClause};
    use crate::market::quotes::ids::{Pillar, QuoteId};
    use finstack_core::currency::Currency;
    use finstack_core::types::IndexId;
    use finstack_core::HashMap;

    fn empty_envelope(id: &str) -> CalibrationEnvelope {
        CalibrationEnvelope {
            schema_url: None,
            schema: CALIBRATION_SCHEMA.to_string(),
            plan: CalibrationPlan {
                id: id.to_string(),
                description: None,
                quote_sets: HashMap::default(),
                steps: Vec::new(),
                settings: Default::default(),
            },
            market_data: Vec::new(),
            prior_market: Vec::new(),
        }
    }

    fn discount_step(id: &str, quote_set: &str, curve_id: &str) -> CalibrationStep {
        // Use serde to construct DiscountCurveParams since several fields rely
        // on serde defaults (interpolation, conventions, etc.).
        let params: DiscountCurveParams = serde_json::from_value(serde_json::json!({
            "curve_id": curve_id,
            "currency": "USD",
            "base_date": "2026-05-08",
        }))
        .expect("default discount params deserialize");
        CalibrationStep {
            id: id.to_string(),
            quote_set: quote_set.to_string(),
            params: StepParams::Discount(params),
        }
    }

    #[test]
    fn empty_envelope_validates_clean() {
        let env = empty_envelope("smoke");
        let report = validate(&env);
        assert!(report.errors.is_empty());
        assert!(report.dependency_graph.nodes.is_empty());
    }

    #[test]
    fn undefined_quote_set_is_caught_with_suggestion() {
        let mut env = empty_envelope("test");
        env.plan
            .quote_sets
            .insert("usd_quotes".to_string(), Vec::new());
        env.plan
            .steps
            .push(discount_step("d", "usd_quotess", "USD-OIS"));

        let report = validate(&env);
        let err = report
            .errors
            .iter()
            .find(|e| matches!(e, EnvelopeError::UndefinedQuoteSet { .. }))
            .expect("undefined quote_set error");
        match err {
            EnvelopeError::UndefinedQuoteSet {
                ref_name,
                suggestion,
                ..
            } => {
                assert_eq!(ref_name, "usd_quotess");
                assert_eq!(suggestion.as_deref(), Some("usd_quotes"));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn dry_run_returns_pretty_json() {
        let env = empty_envelope("smoke");
        let json = serde_json::to_string(&env).expect("serialize");
        let report_json = dry_run(&json).expect("dry_run");
        assert!(report_json.contains("\"errors\""));
        assert!(report_json.contains("\"dependency_graph\""));
    }

    #[test]
    fn dependency_graph_json_is_well_formed() {
        let env = empty_envelope("smoke");
        let json = serde_json::to_string(&env).expect("serialize");
        let graph_json = dependency_graph_json(&json).expect("dep graph");
        assert!(graph_json.contains("\"initial_ids\""));
        assert!(graph_json.contains("\"nodes\""));
    }

    #[test]
    fn missing_dependency_for_forward_step_without_discount() {
        // Build a forward step that reads "USD-OIS" without a producing step
        // and without initial_market — should surface MissingDependency.
        let mut env = empty_envelope("missing");
        env.plan
            .quote_sets
            .insert("fwd_quotes".to_string(), Vec::new());
        let params: crate::calibration::api::schema::ForwardCurveParams =
            serde_json::from_value(serde_json::json!({
                "curve_id": "USD-3M-LIBOR",
                "currency": "USD",
                "base_date": "2026-05-08",
                "tenor_years": 0.25,
                "discount_curve_id": "USD-OIS",
            }))
            .expect("forward params");
        env.plan.steps.push(CalibrationStep {
            id: "fwd".to_string(),
            quote_set: "fwd_quotes".to_string(),
            params: StepParams::Forward(params),
        });

        let report = validate(&env);
        let missing = report
            .errors
            .iter()
            .find(|e| matches!(e, EnvelopeError::MissingDependency { .. }))
            .expect("missing dependency error");
        if let EnvelopeError::MissingDependency { missing_id, .. } = missing {
            assert_eq!(missing_id, "USD-OIS");
        }
    }

    #[test]
    fn validate_reports_non_finite_quote_payload() {
        let mut env = empty_envelope("bad-quote");
        env.plan
            .quote_sets
            .insert("rates".to_string(), vec![QuoteId::new("USD-DEP-1M")]);
        env.plan
            .steps
            .push(discount_step("discount", "rates", "USD-OIS"));
        env.market_data
            .push(MarketDatum::RateQuote(RateQuote::Deposit {
                id: QuoteId::new("USD-DEP-1M"),
                index: IndexId::new("USD-SOFR-1M"),
                pillar: Pillar::Tenor("1M".parse().expect("tenor")),
                rate: f64::NAN,
            }));

        let report = validate(&env);
        let err = report
            .errors
            .iter()
            .find(|e| matches!(e, EnvelopeError::QuoteDataInvalid { .. }))
            .expect("quote data error");
        if let EnvelopeError::QuoteDataInvalid {
            step_id,
            quote_id,
            reason,
        } = err
        {
            assert_eq!(step_id, "discount");
            assert_eq!(quote_id, "USD-DEP-1M");
            assert!(reason.contains("rate must be finite"));
        }
    }

    #[test]
    fn dry_run_reports_invalid_quote_payload() {
        let mut env = empty_envelope("bad-quote-json");
        env.plan
            .quote_sets
            .insert("cds".to_string(), vec![QuoteId::new("CDS-ACME-5Y")]);
        env.plan
            .steps
            .push(discount_step("discount", "cds", "USD-OIS"));
        env.market_data
            .push(MarketDatum::CdsQuote(CdsQuote::CdsParSpread {
                id: QuoteId::new("CDS-ACME-5Y"),
                entity: "ACME".to_string(),
                convention: CdsConventionKey {
                    currency: Currency::USD,
                    doc_clause: CdsDocClause::Cr14,
                },
                pillar: Pillar::Tenor("5Y".parse().expect("tenor")),
                spread_bp: -1.0,
                recovery_rate: 0.40,
            }));

        let json = serde_json::to_string(&env).expect("serialize");
        let report_json = dry_run(&json).expect("dry_run");
        assert!(report_json.contains("\"kind\": \"quote_data_invalid\""));
        assert!(report_json.contains("\"quote_id\": \"CDS-ACME-5Y\""));
    }

    #[test]
    fn json_parse_error_surfaces_line_and_col() {
        let err = dry_run("not json").unwrap_err();
        match err {
            EnvelopeError::JsonParse { line, col, .. } => {
                assert!(line.is_some());
                assert!(col.is_some());
            }
            _ => panic!("expected JsonParse"),
        }
    }

    #[test]
    fn v2_envelope_yields_friendly_error() {
        let v2 = r#"{
            "schema":"finstack.calibration",
            "plan":{"id":"x","description":null,"quote_sets":{},"steps":[],"settings":{}},
            "initial_market":null
        }"#;
        let err = parse_envelope_v3(v2).unwrap_err();
        match err {
            EnvelopeError::JsonParse { message, .. } => {
                assert!(message.contains("v3 shape"), "message was: {message}")
            }
            other => panic!("expected JsonParse error, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod v3_validation_tests {
    use super::*;
    use crate::calibration::api::market_datum::{MarketDatum, PriceDatum};
    use crate::calibration::api::schema::{CalibrationPlan, CALIBRATION_SCHEMA};
    use finstack_core::market_data::scalars::MarketScalar;
    use finstack_core::HashMap;

    #[test]
    fn duplicate_price_id_is_flagged() {
        let envelope = CalibrationEnvelope {
            schema_url: None,
            schema: CALIBRATION_SCHEMA.to_string(),
            plan: CalibrationPlan {
                id: "t".into(),
                description: None,
                quote_sets: HashMap::default(),
                steps: vec![],
                settings: Default::default(),
            },
            market_data: vec![
                MarketDatum::Price(PriceDatum {
                    id: "AAPL".into(),
                    scalar: MarketScalar::Unitless(100.0),
                }),
                MarketDatum::Price(PriceDatum {
                    id: "AAPL".into(),
                    scalar: MarketScalar::Unitless(101.0),
                }),
            ],
            prior_market: vec![],
        };
        let report = validate(&envelope);
        assert!(
            report.errors.iter().any(|e| matches!(
                e,
                EnvelopeError::DuplicateMarketDatumId { datum_kind, id }
                    if datum_kind == "price" && id == "AAPL"
            )),
            "expected DuplicateMarketDatumId, got: {:?}",
            report.errors
        );
    }

    #[test]
    fn unresolved_quote_id_is_flagged() {
        use crate::market::quotes::ids::QuoteId;
        let mut quote_sets = HashMap::default();
        quote_sets.insert("usd".to_string(), vec![QuoteId::new("MISSING")]);
        let envelope = CalibrationEnvelope {
            schema_url: None,
            schema: CALIBRATION_SCHEMA.to_string(),
            plan: CalibrationPlan {
                id: "t".into(),
                description: None,
                quote_sets,
                steps: vec![],
                settings: Default::default(),
            },
            market_data: vec![],
            prior_market: vec![],
        };
        let report = validate(&envelope);
        assert!(
            report.errors.iter().any(|e| matches!(
                e,
                EnvelopeError::QuoteIdNotInMarketData { quote_set, id }
                    if quote_set == "usd" && id == "MISSING"
            )),
            "expected QuoteIdNotInMarketData, got: {:?}",
            report.errors
        );
    }
}
