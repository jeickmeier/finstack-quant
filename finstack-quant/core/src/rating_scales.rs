//! Caller-supplied credit rating-scale registry for scorecards and analytics.
//!
//! There is no embedded scale and no default (record CORE-CONVENTIONS-019
//! #7–#11): every registry is supplied by the caller, directly or through the
//! [`RATING_SCALES_EXTENSION_KEY`](crate::rating_scales::RATING_SCALES_EXTENSION_KEY) configuration extension, and validated on
//! every construction and restore path. A scale's numbers are the scorecard
//! author's utility policy, not an agency calibration.

use crate::config::FinstackConfig;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Configuration extension key holding a caller-supplied rating-scale registry.
///
/// This names the configuration slot; the document schema is independently
/// validated as `finstack_quant.rating_scales/2`.
pub const RATING_SCALES_EXTENSION_KEY: &str = "core.rating_scales.v1";

/// Schema identifier of a rating-scale registry document.
const REGISTRY_SCHEMA: &str = "finstack_quant.rating_scales/2";

/// Unrated status spellings reserved as missing statuses, never numeric bands.
const RESERVED_UNRATED_LABELS: [&str; 4] = ["nr", "notrated", "not rated", "unrated"];

/// One band of a scorecard scale: a label, its representative score and the
/// band's lower bound.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RatingLevelWire")]
#[cfg_attr(feature = "json-schema", schemars(!try_from, deny_unknown_fields))]
pub struct RatingLevel {
    /// Band label, for example `AAA` or `Aaa`; outer whitespace trimmed.
    name: String,
    /// Representative score of the band on the inclusive 0–100 scale.
    score: f64,
    /// Inclusive lower bound of the band on the 0–100 scale.
    min_score: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RatingLevelWire {
    name: String,
    score: f64,
    min_score: f64,
}

impl TryFrom<RatingLevelWire> for RatingLevel {
    type Error = Error;

    fn try_from(wire: RatingLevelWire) -> Result<Self> {
        Self::try_new(wire.name, wire.score, wire.min_score)
    }
}

impl RatingLevel {
    /// Build and validate one scorecard band.
    ///
    /// # Arguments
    ///
    /// * `name` - Band label such as `AAA` or `Aaa`; outer whitespace is
    ///   trimmed and the trimmed label must be nonblank and not an unrated
    ///   status spelling (`NR`, `NOTRATED`, `NOT RATED`, `UNRATED`).
    /// * `score` - Representative score of the band, finite in `[0, 100]`.
    /// * `min_score` - Inclusive lower bound of the band, finite in
    ///   `[0, 100]` and not above `score`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the label is blank or reserved, a
    /// score is non-finite or outside `[0, 100]`, or `score < min_score`.
    pub fn try_new(name: impl Into<String>, score: f64, min_score: f64) -> Result<Self> {
        let level = Self {
            name: name.into().trim().to_string(),
            score,
            min_score,
        };
        validate_rating_level(&level)?;
        Ok(level)
    }

    /// Band label as supplied, outer whitespace trimmed.
    #[must_use]
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Representative score of the band on the 0–100 scale.
    #[must_use]
    pub fn get_score(&self) -> f64 {
        self.score
    }

    /// Inclusive lower bound of the band on the 0–100 scale.
    #[must_use]
    pub fn get_min_score(&self) -> f64 {
        self.min_score
    }
}

/// Scorecard rating-scale definition: a named, best-to-worst list of bands
/// that covers the whole `[0, 100]` score domain.
///
/// Named `ScorecardScale` (rather than just `RatingScale`) to disambiguate
/// from `finstack_quant_models::credit::migration::RatingScale`, which models
/// the ordered state set of a credit-migration / transition matrix.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "ScorecardScaleWire")]
#[cfg_attr(feature = "json-schema", schemars(!try_from, deny_unknown_fields))]
pub struct ScorecardScale {
    /// Scale name, for example `Internal corporate`; outer whitespace trimmed.
    scale_name: String,
    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    /// Bands ordered best to worst.
    ratings: Vec<RatingLevel>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScorecardScaleWire {
    scale_name: String,
    #[serde(default)]
    description: Option<String>,
    ratings: Vec<RatingLevel>,
}

impl TryFrom<ScorecardScaleWire> for ScorecardScale {
    type Error = Error;

    fn try_from(wire: ScorecardScaleWire) -> Result<Self> {
        Self::try_new(wire.scale_name, wire.description, wire.ratings)
    }
}

impl ScorecardScale {
    /// Build and validate one best-to-worst scorecard scale.
    ///
    /// For bands with minima `m0 > m1 > … > m_last`, the best band is
    /// `[m0, 100]` and each later band is `[m_i, m_(i-1))`. The worst band's
    /// minimum must be `0` so the whole domain is covered, and every
    /// representative score must classify to its own band.
    ///
    /// # Arguments
    ///
    /// * `scale_name` - Scale name; outer whitespace is trimmed and the
    ///   trimmed name must be nonblank. Also labels validation messages.
    /// * `description` - Optional human-readable description.
    /// * `ratings` - Bands ordered best to worst; scores and minima strictly
    ///   descend, labels are unique after outer trim and ASCII case folding.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the name is blank, the band list is
    /// empty, labels collide, the order is not strictly best-to-worst, the
    /// worst minimum is not `0`, or a representative falls outside its band.
    pub fn try_new(
        scale_name: impl Into<String>,
        description: Option<String>,
        ratings: Vec<RatingLevel>,
    ) -> Result<Self> {
        let scale = Self {
            scale_name: scale_name.into().trim().to_string(),
            description,
            ratings,
        };
        validate_scorecard_scale(&scale)?;
        Ok(scale)
    }

    /// Scale name, outer whitespace trimmed.
    #[must_use]
    pub fn get_scale_name(&self) -> &str {
        &self.scale_name
    }

    /// Human-readable description, if any.
    #[must_use]
    pub fn get_description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Bands ordered best to worst.
    #[must_use]
    pub fn get_ratings(&self) -> &[RatingLevel] {
        &self.ratings
    }

    /// Classify a score into its band.
    ///
    /// Returns the first band whose minimum the score meets; each boundary
    /// belongs to the better band. There is no epsilon, clipping or fallback.
    ///
    /// # Arguments
    ///
    /// * `score` - Score on the inclusive 0–100 scale.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `score` is non-finite or outside
    /// `[0, 100]`.
    pub fn classify(&self, score: f64) -> Result<&RatingLevel> {
        validate_score(score, "scorecard score")?;
        self.ratings
            .iter()
            .find(|level| score >= level.min_score)
            .ok_or_else(|| {
                Error::Validation(format!(
                    "rating scale '{}' does not cover score {score}",
                    self.scale_name
                ))
            })
    }

    /// Position of a band label in best-to-worst order (`0` is best).
    ///
    /// The label is matched after outer trim and ASCII case folding, the same
    /// normalization that makes labels unique within the scale.
    ///
    /// # Arguments
    ///
    /// * `label` - Band label to look up, for example a threshold key or a
    ///   minimum-rating requirement.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the scale has no band with that label.
    pub fn level_position(&self, label: &str) -> Result<usize> {
        let key = normalize_label(label);
        self.ratings
            .iter()
            .position(|level| normalize_label(&level.name) == key)
            .ok_or_else(|| {
                Error::Validation(format!(
                    "rating scale '{}' has no band '{}'",
                    self.scale_name,
                    label.trim()
                ))
            })
    }

    /// Export the bands in best-to-worst order as a [`TableEnvelope`](crate::table::TableEnvelope)
    /// with columns `name` (`String`), `score` (`Float64`), and `min_score` (`Float64`).
    ///
    /// This is a presentation view, not a complete policy snapshot.
    #[must_use]
    pub fn to_table(&self) -> crate::table::TableEnvelope {
        use crate::table::{TableColumn, TableColumnData, TableColumnRole, TableEnvelope};
        let mut names = Vec::with_capacity(self.ratings.len());
        let mut scores = Vec::with_capacity(self.ratings.len());
        let mut min_scores = Vec::with_capacity(self.ratings.len());
        for level in &self.ratings {
            names.push(level.name.clone());
            scores.push(level.score);
            min_scores.push(level.min_score);
        }
        TableEnvelope {
            row_count: self.ratings.len(),
            columns: vec![
                TableColumn::new("name", TableColumnData::String(names))
                    .with_role(TableColumnRole::Dimension),
                TableColumn::new("score", TableColumnData::Float64(scores))
                    .with_role(TableColumnRole::Measure),
                TableColumn::new("min_score", TableColumnData::Float64(min_scores))
                    .with_role(TableColumnRole::Measure),
            ],
            metadata: indexmap::IndexMap::new(),
        }
    }
}

/// One registered scale with its identifier and provenance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RatingScaleEntryWire")]
#[cfg_attr(feature = "json-schema", schemars(!try_from, deny_unknown_fields))]
pub struct RatingScaleEntry {
    /// Scale identifier, outer-trimmed and ASCII-lowercased.
    id: String,
    /// Provenance of the scale's numbers (policy, owner, vintage).
    source: String,
    /// The scale definition.
    scale: ScorecardScale,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RatingScaleEntryWire {
    id: String,
    source: String,
    scale: ScorecardScale,
}

impl TryFrom<RatingScaleEntryWire> for RatingScaleEntry {
    type Error = Error;

    fn try_from(wire: RatingScaleEntryWire) -> Result<Self> {
        Self::try_new(wire.id, wire.source, wire.scale)
    }
}

impl RatingScaleEntry {
    /// Build and validate one registry entry.
    ///
    /// # Arguments
    ///
    /// * `id` - Scale identifier; normalized by outer trim and ASCII
    ///   lowercasing and nonblank after normalization.
    /// * `source` - Provenance of the scale's numbers (policy, owner, vintage);
    ///   outer whitespace trimmed and nonblank. A brand name alone is not a
    ///   quantitative calibration.
    /// * `scale` - The validated scale definition.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the normalized id or the source is blank.
    pub fn try_new(
        id: impl Into<String>,
        source: impl Into<String>,
        scale: ScorecardScale,
    ) -> Result<Self> {
        let id = normalize_id(&id.into());
        if id.is_empty() {
            return Err(Error::Validation(
                "rating scale registry contains a blank scale id".to_string(),
            ));
        }
        let source = source.into().trim().to_string();
        if source.is_empty() {
            return Err(Error::Validation(format!(
                "rating scale '{id}' has a blank source"
            )));
        }
        Ok(Self { id, source, scale })
    }

    /// Normalized scale identifier.
    #[must_use]
    pub fn get_id(&self) -> &str {
        &self.id
    }

    /// Provenance of the scale's numbers.
    #[must_use]
    pub fn get_source(&self) -> &str {
        &self.source
    }

    /// The scale definition.
    #[must_use]
    pub fn get_scale(&self) -> &ScorecardScale {
        &self.scale
    }
}

/// Caller-supplied registry of scorecard rating scales.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(try_from = "RatingScaleRegistryWire")]
pub struct RatingScaleRegistry {
    schema: String,
    rating_scales: Vec<RatingScaleEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RatingScaleRegistryWire {
    schema: String,
    rating_scales: Vec<RatingScaleEntry>,
}

impl TryFrom<RatingScaleRegistryWire> for RatingScaleRegistry {
    type Error = Error;

    fn try_from(wire: RatingScaleRegistryWire) -> Result<Self> {
        if wire.schema != REGISTRY_SCHEMA {
            return Err(Error::Validation(format!(
                "unsupported rating scale registry schema version '{}'",
                wire.schema
            )));
        }
        Self::try_new(wire.rating_scales)
    }
}

impl RatingScaleRegistry {
    /// Build a registry from validated entries.
    ///
    /// # Arguments
    ///
    /// * `rating_scales` - Entries in registry order; at least one, with
    ///   unique normalized ids.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the list is empty or ids repeat.
    pub fn try_new(rating_scales: Vec<RatingScaleEntry>) -> Result<Self> {
        if rating_scales.is_empty() {
            return Err(Error::Validation(
                "rating scale registry contains no scales".to_string(),
            ));
        }
        crate::validation::validate_unique_ids(
            "rating scale registry",
            "rating scale",
            rating_scales
                .iter()
                .map(|entry| std::slice::from_ref(&entry.id)),
        )?;
        Ok(Self {
            schema: REGISTRY_SCHEMA.to_string(),
            rating_scales,
        })
    }

    /// Deserialize a registry from JSON and validate it.
    ///
    /// # Arguments
    ///
    /// * `json` - Registry document with schema `finstack_quant.rating_scales/2`
    ///   and a nonempty `rating_scales` list of `{id, source, scale}` entries.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when the JSON cannot be parsed or the
    /// registry, an entry or a scale fails validation.
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json)
            .map_err(|err| Error::Validation(format!("invalid rating scale registry JSON: {err}")))
    }

    /// Registered entries in registry order.
    #[must_use]
    pub fn get_rating_scales(&self) -> &[RatingScaleEntry] {
        &self.rating_scales
    }

    /// Normalized id of every registered scale, in registry order.
    #[must_use]
    pub fn scale_ids(&self) -> Vec<&str> {
        self.rating_scales
            .iter()
            .map(|entry| entry.id.as_str())
            .collect()
    }

    /// Whether a scale id is registered after outer trim and ASCII lowercasing.
    ///
    /// # Arguments
    ///
    /// * `id` - Scale id to look up.
    #[must_use]
    pub fn is_known_rating_scale(&self, id: &str) -> bool {
        self.entry(id).is_ok()
    }

    /// Resolve a registered entry by id.
    ///
    /// # Arguments
    ///
    /// * `id` - Scale id; outer whitespace and ASCII case are ignored. No
    ///   brand, fuzzy or punctuation alias is inferred.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if no scale has that id.
    pub fn entry(&self, id: &str) -> Result<&RatingScaleEntry> {
        let key = normalize_id(id);
        self.rating_scales
            .iter()
            .find(|entry| entry.id == key)
            .ok_or_else(|| {
                Error::Validation(format!(
                    "rating scale registry does not contain scale '{}'",
                    id.trim()
                ))
            })
    }

    /// Resolve a registered scale by id.
    ///
    /// # Arguments
    ///
    /// * `id` - Scale id; outer whitespace and ASCII case are ignored.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if no scale has that id.
    pub fn rating_scale(&self, id: &str) -> Result<&ScorecardScale> {
        self.entry(id).map(RatingScaleEntry::get_scale)
    }
}

/// Load the rating-scale registry held in a configuration.
///
/// # Arguments
///
/// * `config` - Library configuration whose [`RATING_SCALES_EXTENSION_KEY`]
///   extension holds the registry document.
///
/// # Errors
///
/// Returns [`Error::Validation`] if the extension is absent or the document
/// is malformed or invalid. There is no embedded fallback.
pub fn registry_from_config(config: &FinstackConfig) -> Result<RatingScaleRegistry> {
    let value = config
        .extensions
        .get(RATING_SCALES_EXTENSION_KEY)
        .ok_or_else(|| {
            Error::Validation(format!(
                "configuration has no rating scale registry under '{RATING_SCALES_EXTENSION_KEY}'"
            ))
        })?;
    serde_json::from_value(value.clone()).map_err(|err| {
        Error::Validation(format!(
            "failed to parse rating-scale registry extension: {err}"
        ))
    })
}

fn normalize_id(id: &str) -> String {
    id.trim().to_ascii_lowercase()
}

fn normalize_label(label: &str) -> String {
    label.trim().to_ascii_lowercase()
}

fn validate_rating_level(level: &RatingLevel) -> Result<()> {
    if level.name.is_empty() {
        return Err(Error::Validation(
            "rating level contains a blank name".to_string(),
        ));
    }
    if RESERVED_UNRATED_LABELS.contains(&normalize_label(&level.name).as_str()) {
        return Err(Error::Validation(format!(
            "rating level '{}' is an unrated status, not a scorecard band",
            level.name
        )));
    }
    validate_score(level.score, "rating level score")?;
    validate_score(level.min_score, "rating level minimum score")?;
    if level.score < level.min_score {
        return Err(Error::Validation(format!(
            "rating level '{}' score {} is below its minimum score {}",
            level.name, level.score, level.min_score
        )));
    }
    Ok(())
}

fn validate_scorecard_scale(scale: &ScorecardScale) -> Result<()> {
    let label = &scale.scale_name;
    if label.is_empty() {
        return Err(Error::Validation(
            "rating scale has a blank scale name".to_string(),
        ));
    }
    let Some(worst) = scale.ratings.last() else {
        return Err(Error::Validation(format!(
            "rating scale '{label}' has no rating levels"
        )));
    };
    let mut names = BTreeSet::new();
    for level in &scale.ratings {
        if !names.insert(normalize_label(&level.name)) {
            return Err(Error::Validation(format!(
                "rating scale '{label}' contains duplicate level '{}'",
                level.name
            )));
        }
    }
    for pair in scale.ratings.windows(2) {
        let (prev, next) = (&pair[0], &pair[1]);
        if next.score >= prev.score {
            return Err(Error::Validation(format!(
                "rating scale '{label}' is not ordered best-to-worst: level '{}' has score {} >= preceding level '{}' score {}",
                next.name, next.score, prev.name, prev.score
            )));
        }
        if next.min_score >= prev.min_score {
            return Err(Error::Validation(format!(
                "rating scale '{label}' is not ordered best-to-worst: level '{}' has min_score {} >= preceding level '{}' min_score {}",
                next.name, next.min_score, prev.name, prev.min_score
            )));
        }
        if next.score >= prev.min_score {
            return Err(Error::Validation(format!(
                "rating scale '{label}': representative score {} of level '{}' is not below the preceding minimum {} of '{}'",
                next.score, next.name, prev.min_score, prev.name
            )));
        }
    }
    if worst.min_score != 0.0 {
        return Err(Error::Validation(format!(
            "rating scale '{label}' leaves [0, {}) unclassified: the worst level '{}' must have min_score 0",
            worst.min_score, worst.name
        )));
    }
    Ok(())
}

fn validate_score(value: f64, label: &str) -> Result<()> {
    if value.is_finite() && (0.0..=100.0).contains(&value) {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "invalid {label} {value}: must be finite in [0, 100]"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(name: &str, score: f64, min_score: f64) -> RatingLevel {
        RatingLevel::try_new(name, score, min_score).expect("valid level")
    }

    fn scale() -> ScorecardScale {
        ScorecardScale::try_new(
            "Internal",
            Some("Test policy".to_string()),
            vec![
                level("A", 90.0, 80.0),
                level("B", 70.0, 50.0),
                level("C", 20.0, 0.0),
            ],
        )
        .expect("valid scale")
    }

    fn registry_json() -> serde_json::Value {
        serde_json::json!({
            "schema": "finstack_quant.rating_scales/2",
            "rating_scales": [{
                "id": "internal",
                "source": "test policy 2026",
                "scale": serde_json::to_value(scale()).expect("serialize")
            }]
        })
    }

    #[test]
    fn level_trims_and_rejects_blank_reserved_and_inverted() {
        assert_eq!(level(" BBB ", 70.0, 65.0).get_name(), "BBB");
        assert!(RatingLevel::try_new("  ", 70.0, 65.0).is_err());
        assert!(RatingLevel::try_new("Not Rated", 70.0, 65.0).is_err());
        assert!(RatingLevel::try_new("nr", 70.0, 65.0).is_err());
        assert!(RatingLevel::try_new("BBB", 170.0, 65.0).is_err());
        assert!(RatingLevel::try_new("BBB", 70.0, f64::NAN).is_err());
        assert!(RatingLevel::try_new("BBB", 60.0, 65.0).is_err());
    }

    #[test]
    fn scale_requires_order_coverage_and_representative_membership() {
        assert!(ScorecardScale::try_new("empty", None, Vec::new()).is_err());
        assert!(ScorecardScale::try_new(" ", None, vec![level("A", 50.0, 0.0)]).is_err());
        let gap = ScorecardScale::try_new("gap", None, vec![level("A", 90.0, 80.0)]);
        assert!(gap.is_err(), "worst minimum above 0 leaves a gap");
        let shuffled = ScorecardScale::try_new(
            "shuffled",
            None,
            vec![level("B", 70.0, 0.0), level("A", 90.0, 80.0)],
        );
        assert!(shuffled.is_err());
        let collide = ScorecardScale::try_new(
            "collide",
            None,
            vec![level("Aa", 90.0, 80.0), level(" AA ", 70.0, 0.0)],
        );
        assert!(collide.is_err());
        // Representative 95 is not below the preceding minimum 95.
        let outside = ScorecardScale::try_new(
            "outside",
            None,
            vec![level("AAA", 100.0, 95.0), level("AA+", 95.0, 0.0)],
        );
        assert!(outside.is_err());
    }

    #[test]
    fn classify_covers_domain_with_boundaries_in_better_band() {
        let scale = scale();
        assert_eq!(scale.classify(100.0).expect("top").get_name(), "A");
        assert_eq!(scale.classify(80.0).expect("boundary").get_name(), "A");
        assert_eq!(scale.classify(79.999).expect("below").get_name(), "B");
        assert_eq!(scale.classify(0.0).expect("floor").get_name(), "C");
        for level in scale.get_ratings() {
            assert_eq!(
                scale.classify(level.get_score()).expect("representative"),
                level
            );
        }
        assert!(scale.classify(-0.1).is_err());
        assert!(scale.classify(100.1).is_err());
        assert!(scale.classify(f64::NAN).is_err());
    }

    #[test]
    fn level_position_normalizes_and_rejects_unknown() {
        let scale = scale();
        assert_eq!(scale.level_position(" b ").expect("known"), 1);
        assert!(scale.level_position("D").is_err());
    }

    #[test]
    fn registry_is_strict_and_round_trips() {
        let registry: RatingScaleRegistry =
            serde_json::from_value(registry_json()).expect("valid registry");
        assert_eq!(registry.scale_ids(), ["internal"]);
        assert_eq!(
            registry.entry(" INTERNAL ").expect("known").get_source(),
            "test policy 2026"
        );
        assert!(registry.rating_scale("sp").is_err());
        let round_trip =
            RatingScaleRegistry::from_json(&serde_json::to_string(&registry).expect("serialize"))
                .expect("round trip");
        assert_eq!(round_trip, registry);
    }

    #[test]
    fn registry_rejects_invalid_documents() {
        let mut empty = registry_json();
        empty["rating_scales"] = serde_json::json!([]);
        assert!(RatingScaleRegistry::from_json(&empty.to_string()).is_err());
        let mut old = registry_json();
        old["schema"] = serde_json::json!("finstack_quant.rating_scales/1");
        assert!(RatingScaleRegistry::from_json(&old.to_string()).is_err());
        let mut policy = registry_json();
        policy["scorecard_policy"] = serde_json::json!({"default_score": 50.0});
        assert!(RatingScaleRegistry::from_json(&policy.to_string()).is_err());
        for (field, value) in [("id", " "), ("source", "  ")] {
            let mut blank = registry_json();
            blank["rating_scales"][0][field] = serde_json::json!(value);
            assert!(RatingScaleRegistry::from_json(&blank.to_string()).is_err());
        }
        let mut duplicate = registry_json();
        let entry = duplicate["rating_scales"][0].clone();
        let mut second = entry.clone();
        second["id"] = serde_json::json!(" Internal ");
        duplicate["rating_scales"] = serde_json::json!([entry, second]);
        assert!(RatingScaleRegistry::from_json(&duplicate.to_string()).is_err());
    }

    #[test]
    fn config_extension_is_required() {
        let mut config = FinstackConfig::default();
        assert!(registry_from_config(&config).is_err());
        config
            .extensions
            .insert(RATING_SCALES_EXTENSION_KEY, registry_json())
            .expect("static extension key");
        assert_eq!(
            registry_from_config(&config)
                .expect("configured")
                .scale_ids(),
            ["internal"]
        );
        let mut old = registry_json();
        old["schema"] = serde_json::json!("finstack_quant.rating_scales/1");
        config
            .extensions
            .insert(RATING_SCALES_EXTENSION_KEY, old)
            .expect("static extension key");
        assert!(registry_from_config(&config).is_err());
    }
}
