//! Completeness tracking — compare hierarchy-declared curves against MarketContext.

use crate::types::CurveId;
use serde::{Deserialize, Serialize};

/// Report comparing hierarchy-declared `CurveId`s against what exists in `MarketContext`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletenessReport {
    /// CurveIds declared in hierarchy but missing from MarketContext.
    pub missing: Vec<(Vec<String>, CurveId)>,

    /// CurveIds in MarketContext that aren't in any hierarchy node.
    pub unclassified: Vec<CurveId>,

    /// Per-subtree coverage statistics.
    pub coverage: Vec<SubtreeCoverage>,
}

/// Coverage statistics for a single subtree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtreeCoverage {
    /// Path to the subtree root.
    pub path: Vec<String>,
    /// Number of CurveIds declared in this subtree.
    pub total_expected: usize,
    /// Number of those CurveIds that are present in MarketContext.
    pub total_present: usize,
    /// Coverage percentage (0.0–100.0).
    pub percent: f64,
}
