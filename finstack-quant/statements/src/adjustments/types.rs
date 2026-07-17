//! Types for EBITDA Normalization & Adjustments.

use finstack_quant_core::dates::PeriodId;
use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};

/// Configuration for normalizing a financial metric (e.g., EBITDA).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizationConfig {
    /// The target node to normalize (e.g., "EBITDA")
    pub target_node: String,

    /// List of adjustments to apply
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adjustments: Vec<Adjustment>,
}

/// Specification for a single adjustment (add-back or deduction).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Adjustment {
    /// Unique identifier for this adjustment
    pub id: String,

    /// Human-readable name (e.g., "Synergies", "Management Fees")
    pub name: String,

    /// Category for grouping (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    /// How the adjustment value is calculated
    pub value: AdjustmentValue,

    /// Optional cap on the adjustment amount
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cap: Option<AdjustmentCap>,
}

/// Defines how an adjustment value is derived.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AdjustmentValue {
    /// Fixed amount per period
    Fixed {
        /// Map of period_id -> amount
        amounts: IndexMap<PeriodId, f64>,
    },
    /// Percentage of a reference node's value
    PercentageOfNode {
        /// Node ID to reference (e.g., "revenue")
        node_id: String,
        /// Percentage to apply (e.g., 0.05 for 5%)
        percentage: f64,
    },
}

/// How a self-referential cap base (where `base_node == target_node`) is
/// resolved each period.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapBaseMode {
    /// Cap against the **reported** (pre-adjustment) value of the base node.
    /// This is the standard credit-agreement convention — e.g., "add-backs
    /// capped at 25% of reported EBITDA" uses a fixed denominator that does
    /// not widen as earlier add-backs are applied.
    #[default]
    Reported,
    /// Cap against the **progressively adjusted** value
    /// (`base_value + running_total_of_earlier_adjustments`). Earlier
    /// adjustments widen the cap room for later ones. Retained for
    /// backwards compatibility and edge cases where a document explicitly
    /// caps against "adjusted" EBITDA at each step.
    Progressive,
}

/// Defines a cap on an adjustment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdjustmentCap {
    /// The base node to calculate the cap against (e.g., "EBITDA")
    /// If None, the cap is a fixed absolute amount.
    pub base_node: Option<String>,

    /// The percentage of the base node to cap at (e.g., 0.20 for 20%)
    /// Or the absolute amount if base_node is None.
    pub value: f64,

    /// For self-referential caps (`base_node == target_node`), choose whether
    /// to size the cap against the reported or the progressively adjusted
    /// base. Defaults to `Reported`, the standard LBO / credit-agreement
    /// convention. Ignored when `base_node` is `None` or points to a
    /// different node.
    #[serde(default, skip_serializing_if = "is_default_cap_base_mode")]
    pub base_mode: CapBaseMode,
}

fn is_default_cap_base_mode(mode: &CapBaseMode) -> bool {
    *mode == CapBaseMode::default()
}

/// Result of a normalization process for a single period.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizationResult {
    /// The period this result applies to
    pub period: PeriodId,

    /// The original raw value of the target node
    pub base_value: f64,

    /// Detailed breakdown of applied adjustments
    pub adjustments: Vec<AppliedAdjustment>,

    /// The final adjusted value
    pub final_value: f64,
}

/// Details of a single applied adjustment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppliedAdjustment {
    /// ID of the adjustment
    pub adjustment_id: String,

    /// Name of the adjustment
    pub name: String,

    /// The raw calculated amount before capping
    pub raw_amount: f64,

    /// The final amount after applying any caps
    pub capped_amount: f64,

    /// Whether the amount was capped
    pub is_capped: bool,
}

impl NormalizationConfig {
    /// Create a new normalization configuration.
    pub fn new(target_node: impl Into<String>) -> Self {
        Self {
            target_node: target_node.into(),
            adjustments: Vec::new(),
        }
    }

    /// Add an adjustment to the configuration.
    ///
    /// Returns an error if an adjustment with the same `id` is already present,
    /// preventing accidental double-counting. Order matters for progressive
    /// self-referential caps, so the adjustment is appended rather than sorted.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error if another adjustment already uses
    /// `adjustment.id`. It does not validate the referenced nodes or cap
    /// economics; those are checked while normalization runs against results.
    pub fn add_adjustment(mut self, adjustment: Adjustment) -> crate::error::Result<Self> {
        if self.adjustments.iter().any(|a| a.id == adjustment.id) {
            return Err(crate::error::Error::invalid_input(format!(
                "Duplicate adjustment ID '{}' — each adjustment must have a unique id",
                adjustment.id
            )));
        }
        self.adjustments.push(adjustment);
        Ok(self)
    }

    /// Check that the configuration holds no duplicate adjustment IDs.
    ///
    /// [`add_adjustment`](Self::add_adjustment) enforces this while building,
    /// but a config can also arrive fully-formed from JSON, where the derived
    /// `Deserialize` performs no such check. Since the normalization engine
    /// applies every entry unconditionally, a duplicated add-back would be
    /// silently counted twice and report a wrong adjusted value. This is
    /// called by [`NormalizationEngine::normalize`], so every path is covered;
    /// call it directly to reject a bad config earlier.
    ///
    /// # Errors
    ///
    /// Returns an error naming the first adjustment ID that appears more than
    /// once.
    ///
    /// [`NormalizationEngine::normalize`]: crate::adjustments::NormalizationEngine::normalize
    pub fn validate(&self) -> crate::error::Result<()> {
        let mut seen: IndexSet<&str> = IndexSet::with_capacity(self.adjustments.len());
        for adjustment in &self.adjustments {
            if !seen.insert(adjustment.id.as_str()) {
                return Err(crate::error::Error::invalid_input(format!(
                    "Duplicate adjustment ID '{}' — each adjustment must have a unique id. \
                     Applying both would double-count the adjustment.",
                    adjustment.id
                )));
            }
        }
        Ok(())
    }
}

impl Adjustment {
    /// Create a fixed amount adjustment.
    pub fn fixed(
        id: impl Into<String>,
        name: impl Into<String>,
        amounts: IndexMap<PeriodId, f64>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            category: None,
            value: AdjustmentValue::Fixed { amounts },
            cap: None,
        }
    }

    /// Create a percentage of node adjustment.
    pub fn percentage(
        id: impl Into<String>,
        name: impl Into<String>,
        node_id: impl Into<String>,
        percentage: f64,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            category: None,
            value: AdjustmentValue::PercentageOfNode {
                node_id: node_id.into(),
                percentage,
            },
            cap: None,
        }
    }

    /// Add a cap to the adjustment. Uses the default `Reported` base mode.
    #[must_use]
    pub fn with_cap(mut self, base_node: Option<String>, value: f64) -> Self {
        self.cap = Some(AdjustmentCap {
            base_node,
            value,
            base_mode: CapBaseMode::default(),
        });
        self
    }

    /// Add a cap with an explicit self-referential base mode.
    pub fn with_cap_mode(
        mut self,
        base_node: Option<String>,
        value: f64,
        base_mode: CapBaseMode,
    ) -> Self {
        self.cap = Some(AdjustmentCap {
            base_node,
            value,
            base_mode,
        });
        self
    }
}
