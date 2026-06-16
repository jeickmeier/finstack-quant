//! Shared credit rating-scale registry for scorecards and analytics.

use crate::config::FinstackConfig;
use crate::embedded_registry::EmbeddedJsonRegistry;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Configuration extension key for replacing the embedded rating-scale registry.
pub const RATING_SCALES_EXTENSION_KEY: &str = "core.rating_scales.v1";

static EMBEDDED_REGISTRY: EmbeddedJsonRegistry<RatingScaleRegistry> = EmbeddedJsonRegistry::new(
    include_str!("../data/rating_scales/rating_scales.v1.json"),
    RATING_SCALES_EXTENSION_KEY,
    "rating-scale",
);

/// Rating level for credit rating scales.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RatingLevel {
    /// Rating name, for example `AAA` or `Aaa`.
    pub name: String,
    /// Numeric score on a 0-100 scale.
    pub score: f64,
    /// Minimum score threshold for this rating.
    pub min_score: f64,
}

/// Scorecard rating-scale definition: a named, ordered list of rating
/// thresholds used by scorecards.
///
/// Named `ScorecardScale` (rather than just `RatingScale`) to disambiguate
/// from [`crate::credit::migration::RatingScale`], which models the ordered
/// state set of a credit-migration / transition matrix.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScorecardScale {
    /// Scale name, for example `S&P` or `Moody's`.
    pub scale_name: String,
    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Ordered list of rating levels from best to worst.
    pub ratings: Vec<RatingLevel>,
}

impl RatingLevel {
    /// Deserialize a rating level from JSON and validate scores and name.
    pub fn from_json(json: &str) -> Result<Self> {
        let level: RatingLevel = serde_json::from_str(json)
            .map_err(|err| Error::Validation(format!("invalid rating level JSON: {err}")))?;
        validate_rating_level(&level)?;
        Ok(level)
    }
}

impl ScorecardScale {
    /// Deserialize a scorecard scale from JSON and validate its rating levels.
    pub fn from_json(json: &str) -> Result<Self> {
        let scale: ScorecardScale = serde_json::from_str(json)
            .map_err(|err| Error::Validation(format!("invalid scorecard scale JSON: {err}")))?;
        validate_scorecard_scale(&scale, &scale.scale_name)?;
        Ok(scale)
    }
}

/// Policy for unknown scorecard rating-scale names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownScalePolicy {
    /// Reject unknown scale names.
    Error,
    /// Use the configured default scale for unknown scale names.
    FallbackToDefault,
    /// Use the configured default scale for unknown scale names and let callers warn.
    WarnAndFallback,
}

impl UnknownScalePolicy {
    /// Deserialize a policy from JSON.
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json)
            .map_err(|err| Error::Validation(format!("invalid unknown scale policy JSON: {err}")))
    }
}

/// Versioned registry of rating scales and scorecard defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RatingScaleRegistry {
    schema_version: String,
    scorecard_policy: ScorecardPolicy,
    rating_scales: Vec<RatingScaleEntry>,
}

impl RatingScaleRegistry {
    /// Deserialize a registry from JSON and validate it.
    ///
    /// Validation enforces the supported schema version, unique scale ids and
    /// aliases, existence of the default scale, and in-range scores — the same
    /// checks applied to the embedded and config-loaded registries.
    ///
    /// # Errors
    /// Returns [`Error::Validation`] when the JSON cannot be parsed or the
    /// deserialized registry fails validation.
    pub fn from_json(json: &str) -> Result<Self> {
        let registry: RatingScaleRegistry = serde_json::from_str(json).map_err(|err| {
            Error::Validation(format!("invalid rating scale registry JSON: {err}"))
        })?;
        validate_registry(registry)
    }

    /// Returns the configured default scorecard score for threshold gaps.
    pub fn default_scorecard_score(&self) -> f64 {
        self.scorecard_policy.default_score
    }

    /// Returns the configured default rating-scale id.
    pub fn default_scale_id(&self) -> &str {
        &self.scorecard_policy.default_scale_id
    }

    /// Returns the configured unknown-scale policy.
    pub fn unknown_scale_policy(&self) -> UnknownScalePolicy {
        self.scorecard_policy.unknown_scale_policy
    }

    /// Returns true when the provided name is a known scale id or alias.
    pub fn is_known_rating_scale(&self, name: &str) -> bool {
        self.resolve_scale_id(name).is_some()
    }

    /// Resolve a scale name or alias to a rating scale.
    pub fn rating_scale(&self, name: &str) -> Result<&ScorecardScale> {
        let id = match self.resolve_scale_id(name) {
            Some(id) => id,
            None => match self.scorecard_policy.unknown_scale_policy {
                UnknownScalePolicy::Error => return Err(not_found(name)),
                UnknownScalePolicy::FallbackToDefault | UnknownScalePolicy::WarnAndFallback => {
                    self.default_scale_id()
                }
            },
        };
        self.rating_scale_by_id(id).ok_or_else(|| not_found(name))
    }

    fn rating_scale_by_id(&self, id: &str) -> Option<&ScorecardScale> {
        self.rating_scales
            .iter()
            .find(|entry| has_id(&entry.ids, id))
            .map(|entry| &entry.scale)
    }

    fn resolve_scale_id<'a>(&'a self, name: &'a str) -> Option<&'a str> {
        if self.rating_scale_by_id(name).is_some() {
            return Some(name);
        }
        self.scorecard_policy
            .aliases
            .iter()
            .find(|alias| alias.alias == name)
            .map(|alias| alias.scale_id.as_str())
    }

    fn validate(&self) -> Result<()> {
        if self.schema_version != "finstack_quant.rating_scales/1" {
            return Err(Error::Validation(format!(
                "unsupported rating scale registry schema version '{}'",
                self.schema_version
            )));
        }
        validate_score(self.scorecard_policy.default_score, "default score")?;
        validate_ids(
            "rating scale",
            self.rating_scales.iter().map(|entry| entry.ids.as_slice()),
        )?;
        if self
            .rating_scale_by_id(&self.scorecard_policy.default_scale_id)
            .is_none()
        {
            return Err(Error::Validation(format!(
                "rating scale registry default scale id '{}' does not exist",
                self.scorecard_policy.default_scale_id
            )));
        }

        let mut aliases = BTreeSet::new();
        for alias in &self.scorecard_policy.aliases {
            if alias.alias.trim().is_empty() {
                return Err(Error::Validation(
                    "rating scale registry contains blank alias".to_string(),
                ));
            }
            if !aliases.insert(alias.alias.clone()) {
                return Err(Error::Validation(format!(
                    "rating scale registry contains duplicate alias '{}'",
                    alias.alias
                )));
            }
            if self.rating_scale_by_id(&alias.scale_id).is_none() {
                return Err(Error::Validation(format!(
                    "rating scale alias '{}' targets unknown scale id '{}'",
                    alias.alias, alias.scale_id
                )));
            }
        }

        for entry in &self.rating_scales {
            validate_scorecard_scale(&entry.scale, first_id(&entry.ids))?;
        }

        Ok(())
    }
}

/// Returns the embedded rating-scale registry.
pub fn embedded_registry() -> Result<&'static RatingScaleRegistry> {
    EMBEDDED_REGISTRY.load(validate_registry)
}

/// Loads a rating-scale registry from configuration or falls back to the embedded registry.
pub fn registry_from_config(config: &FinstackConfig) -> Result<RatingScaleRegistry> {
    EMBEDDED_REGISTRY.load_from_config(config, validate_registry)
}

fn validate_registry(registry: RatingScaleRegistry) -> Result<RatingScaleRegistry> {
    registry.validate()?;
    Ok(registry)
}

fn validate_rating_level(level: &RatingLevel) -> Result<()> {
    if level.name.trim().is_empty() {
        return Err(Error::Validation(
            "rating level contains a blank name".to_string(),
        ));
    }
    validate_score(level.score, "rating level score")?;
    validate_score(level.min_score, "rating level minimum score")?;
    Ok(())
}

fn validate_scorecard_scale(scale: &ScorecardScale, scale_label: &str) -> Result<()> {
    if scale.ratings.is_empty() {
        return Err(Error::Validation(format!(
            "rating scale '{scale_label}' has no rating levels"
        )));
    }
    let mut names = BTreeSet::new();
    for level in &scale.ratings {
        if !names.insert(level.name.clone()) {
            return Err(Error::Validation(format!(
                "rating scale '{scale_label}' contains duplicate level '{}'",
                level.name
            )));
        }
        validate_rating_level(level).map_err(|err| match err {
            Error::Validation(msg) if msg == "rating level contains a blank name" => {
                Error::Validation(format!(
                    "rating scale '{scale_label}' contains a blank rating level"
                ))
            }
            other => other,
        })?;
    }

    // Ratings must be ordered best-to-worst: `determine_rating`-style lookups
    // scan in order and return the first level whose `min_score` is met, so a
    // shuffled scale silently mis-rates. Enforce strictly descending `score`
    // and `min_score`.
    for pair in scale.ratings.windows(2) {
        let (prev, next) = (&pair[0], &pair[1]);
        if next.score >= prev.score {
            return Err(Error::Validation(format!(
                "rating scale '{scale_label}' is not ordered best-to-worst: level '{}' has score {} >= preceding level '{}' score {}",
                next.name, next.score, prev.name, prev.score
            )));
        }
        if next.min_score >= prev.min_score {
            return Err(Error::Validation(format!(
                "rating scale '{scale_label}' is not ordered best-to-worst: level '{}' has min_score {} >= preceding level '{}' min_score {}",
                next.name, next.min_score, prev.name, prev.min_score
            )));
        }
    }
    Ok(())
}

fn validate_score(value: f64, label: &str) -> Result<()> {
    if value.is_finite() && (0.0..=100.0).contains(&value) {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "rating scale registry has invalid {label} {value}"
        )))
    }
}

fn validate_ids<'a>(kind: &str, records: impl Iterator<Item = &'a [String]>) -> Result<()> {
    let mut seen = BTreeSet::new();
    for ids in records {
        if ids.is_empty() {
            return Err(Error::Validation(format!(
                "rating scale registry contains {kind} without an id"
            )));
        }
        for id in ids {
            if id.trim().is_empty() {
                return Err(Error::Validation(format!(
                    "rating scale registry contains blank {kind} id"
                )));
            }
            if !seen.insert(id.clone()) {
                return Err(Error::Validation(format!(
                    "rating scale registry contains duplicate {kind} id '{id}'"
                )));
            }
        }
    }
    Ok(())
}

fn has_id(ids: &[String], id: &str) -> bool {
    ids.iter().any(|candidate| candidate == id)
}

fn first_id(ids: &[String]) -> &str {
    ids.first().map_or("<missing>", String::as_str)
}

fn not_found(name: &str) -> Error {
    Error::Validation(format!(
        "rating scale registry does not contain scale '{name}'"
    ))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScorecardPolicy {
    default_score: f64,
    default_scale_id: String,
    unknown_scale_policy: UnknownScalePolicy,
    aliases: Vec<RatingScaleAlias>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RatingScaleAlias {
    alias: String,
    scale_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RatingScaleEntry {
    ids: Vec<String>,
    source: String,
    scale: ScorecardScale,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_registry_preserves_scorecard_policy() {
        let registry = embedded_registry().expect("registry should load");
        assert_eq!(registry.default_scorecard_score(), 50.0);
        assert_eq!(registry.default_scale_id(), "sp");
        assert_eq!(
            registry.unknown_scale_policy(),
            UnknownScalePolicy::FallbackToDefault
        );
    }

    #[test]
    fn aliases_resolve_current_supported_names() {
        let registry = embedded_registry().expect("registry should load");
        assert_eq!(registry.rating_scale("S&P").expect("S&P").scale_name, "S&P");
        assert_eq!(
            registry.rating_scale("Fitch").expect("Fitch").scale_name,
            "S&P"
        );
        assert_eq!(
            registry
                .rating_scale("Moody's")
                .expect("Moody's")
                .scale_name,
            "Moody's"
        );
        assert!(!registry.is_known_rating_scale("unknown"));
        assert_eq!(
            registry
                .rating_scale("unknown")
                .expect("fallback")
                .scale_name,
            "S&P"
        );
    }

    #[test]
    fn config_extension_loads_registry_schema() {
        let embedded = embedded_registry().expect("registry should load").clone();
        let value = serde_json::to_value(&embedded).expect("registry should serialize");
        let mut config = FinstackConfig::default();
        config.extensions.insert(RATING_SCALES_EXTENSION_KEY, value);

        let loaded = registry_from_config(&config).expect("config registry should load");
        assert_eq!(
            loaded.default_scorecard_score(),
            embedded.default_scorecard_score()
        );
    }

    #[test]
    fn from_json_round_trips_valid_registry() {
        let embedded = embedded_registry().expect("registry should load").clone();
        let json = serde_json::to_string(&embedded).expect("registry should serialize");
        let parsed = RatingScaleRegistry::from_json(&json).expect("valid registry should parse");
        assert_eq!(parsed.default_scale_id(), embedded.default_scale_id());
    }

    #[test]
    fn from_json_rejects_invalid_default_scale_id() {
        let embedded = embedded_registry().expect("registry should load").clone();
        let mut value = serde_json::to_value(&embedded).expect("registry should serialize");
        value["scorecard_policy"]["default_scale_id"] =
            serde_json::Value::String("nonexistent".to_string());
        let json = serde_json::to_string(&value).expect("value should serialize");

        let err = RatingScaleRegistry::from_json(&json)
            .expect_err("registry with unknown default scale id must be rejected");
        assert!(matches!(err, Error::Validation(_)));
    }

    #[test]
    fn rating_level_from_json_rejects_out_of_range_score() {
        let json = r#"{"name":"BB","score":150.0,"min_score":50.0}"#;
        let err = RatingLevel::from_json(json).expect_err("score out of range must be rejected");
        assert!(matches!(err, Error::Validation(_)));
    }

    #[test]
    fn scorecard_scale_from_json_rejects_shuffled_ordering() {
        // Levels out of best-to-worst order must be rejected: ordered lookups
        // (first level whose min_score is met) depend on monotonicity.
        let json = r#"{"scale_name":"Test","ratings":[
            {"name":"A","score":75.0,"min_score":70.0},
            {"name":"AAA","score":100.0,"min_score":95.0},
            {"name":"BBB","score":60.0,"min_score":55.0}
        ]}"#;
        let err = ScorecardScale::from_json(json).expect_err("shuffled scale must be rejected");
        assert!(matches!(err, Error::Validation(_)));
        assert!(err.to_string().contains("not ordered best-to-worst"));
    }

    #[test]
    fn scorecard_scale_from_json_rejects_non_descending_min_score() {
        let json = r#"{"scale_name":"Test","ratings":[
            {"name":"AAA","score":100.0,"min_score":95.0},
            {"name":"AA","score":90.0,"min_score":95.0}
        ]}"#;
        let err = ScorecardScale::from_json(json).expect_err("tied min_score must be rejected");
        assert!(err.to_string().contains("min_score"));
    }

    #[test]
    fn scorecard_scale_from_json_accepts_descending_scale() {
        let json = r#"{"scale_name":"Test","ratings":[
            {"name":"AAA","score":100.0,"min_score":95.0},
            {"name":"AA","score":90.0,"min_score":85.0},
            {"name":"A","score":75.0,"min_score":70.0}
        ]}"#;
        let scale = ScorecardScale::from_json(json).expect("descending scale should validate");
        assert_eq!(scale.ratings.len(), 3);
    }

    #[test]
    fn scorecard_scale_from_json_rejects_empty_ratings() {
        let json = r#"{"scale_name":"Test","ratings":[]}"#;
        let err = ScorecardScale::from_json(json).expect_err("empty ratings must be rejected");
        assert!(matches!(err, Error::Validation(_)));
    }
}
