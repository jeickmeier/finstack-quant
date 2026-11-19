//! Types for EBITDA Normalization & Adjustments.

use crate::evaluator::UnavailableReason;
use crate::types::AmountOrScalar;
use finstack_quant_core::dates::PeriodId;
use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};

/// An ordered bridge from a reported metric to a separately named adjusted
/// metric (for example reported EBITDA to adjusted EBITDA).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct NormalizationConfig {
    /// The reported node the bridge starts from (e.g., "ebitda"). It is never
    /// modified.
    pub target_node: String,

    /// The new node the adjusted metric is published under (e.g.,
    /// "adjusted_ebitda"). It must differ from `target_node` and from every
    /// node the result already holds.
    pub output_node: String,

    /// Adjustments, applied in this order
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adjustments: Vec<Adjustment>,
}

/// Specification for a single adjustment (add-back or deduction).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AdjustmentValue {
    /// Signed fixed amount per period, in the target's units: a scalar for a
    /// scalar target, money in the target's currency for a monetary one. A
    /// period of the plan that is not listed adjusts by zero; a period
    /// outside the plan is rejected.
    Fixed {
        /// Map of period_id -> signed amount
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "IndexMap<String, AmountOrScalar>")
        )]
        amounts: IndexMap<PeriodId, AmountOrScalar>,
    },
    /// Signed decimal percentage of a reference node's reported value in the
    /// same period. The reference must share the target's units.
    PercentageOfNode {
        /// Node ID to reference (e.g., "revenue")
        node_id: String,
        /// Signed decimal percentage to apply (0.05 for 5%)
        percentage: f64,
    },
}

/// How a self-referential cap base (where `base_node == target_node`) is
/// resolved each period.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
    /// adjustments widen the cap room for later ones. Use this only when a
    /// document explicitly caps against "adjusted" EBITDA at each step.
    Progressive,
}

/// A cap on the magnitude of one adjustment. The adjustment keeps its sign.
/// Each cap applies to its own adjustment only; it is not a shared basket.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AdjustmentCap {
    /// A fixed limit in the target's units.
    Absolute {
        /// Non-negative limit: a scalar for a scalar target, money in the
        /// target's currency for a monetary one.
        amount: AmountOrScalar,
    },
    /// A limit of `fraction` times the positive part of a base node's value
    /// in the same period. A negative base gives no cap room.
    Reference {
        /// Node the cap is sized against; it must share the target's units.
        base_node: String,
        /// Non-negative decimal fraction of the base (0.30 for 30%). It may
        /// exceed 1.
        fraction: f64,
        /// When `base_node` is the target: size the cap against its reported
        /// value (the default) or against the reported value plus every
        /// earlier applied adjustment. Any other base node is always read at
        /// its reported value.
        #[serde(default, skip_serializing_if = "is_default_cap_base_mode")]
        #[cfg_attr(feature = "json-schema", schemars(extend("default" = "reported")))]
        base_mode: CapBaseMode,
    },
}

fn is_default_cap_base_mode(mode: &CapBaseMode) -> bool {
    *mode == CapBaseMode::default()
}

/// One period of a normalization bridge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct NormalizationResult {
    /// Unit and currency shared by the base, adjustments, and final value.
    pub value_type: crate::types::NodeValueType,

    /// The period this result applies to
    pub period: PeriodId,

    /// The reported value of the target node. Absent when the period is
    /// unavailable because the target itself has no value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_value: Option<AmountOrScalar>,

    /// The bridge rows in declaration order. Empty when the period is
    /// unavailable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adjustments: Vec<AppliedAdjustment>,

    /// The adjusted value. Absent when the period is unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_value: Option<AmountOrScalar>,

    /// Why the period has no adjusted value: the target, a percentage
    /// reference or a cap base is missing, masked or unavailable. Absent
    /// when the period has a value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable: Option<UnavailableReason>,
}

/// One row of a normalization bridge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AppliedAdjustment {
    /// ID of the adjustment
    pub adjustment_id: String,

    /// Name of the adjustment
    pub name: String,

    /// The signed amount before capping
    pub raw_amount: AmountOrScalar,

    /// The value the cap was sized against, for a reference cap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap_base: Option<AmountOrScalar>,

    /// The non-negative limit on the adjustment's magnitude, when it has a
    /// cap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap_limit: Option<AmountOrScalar>,

    /// The signed amount applied after any cap
    pub capped_amount: AmountOrScalar,

    /// Whether the cap reduced the amount
    pub is_capped: bool,
    /// Target value through this adjustment: the period's `base_value` plus
    /// the applied amounts of this and every earlier adjustment, in
    /// declaration order. The last row's value equals the period's
    /// `final_value`.
    pub adjusted_value: AmountOrScalar,
}

impl NormalizationConfig {
    /// Create a bridge configuration with no adjustments.
    ///
    /// # Arguments
    ///
    /// * `target_node` - Reported node the bridge starts from.
    /// * `output_node` - New node the adjusted metric is published under.
    pub fn new(target_node: impl Into<String>, output_node: impl Into<String>) -> Self {
        Self {
            target_node: target_node.into(),
            output_node: output_node.into(),
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
    ///
    /// # Arguments
    ///
    /// * `adjustment` - Adjustment used by the algorithm, subject to the enclosing type invariants and documented units.
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

    /// Check the configuration on its own: a named output node that differs
    /// from the target, and no duplicate adjustment IDs.
    ///
    /// [`add_adjustment`](Self::add_adjustment) enforces this while building,
    /// but a config can also arrive fully-formed from JSON, where the derived
    /// `Deserialize` performs no such check. Since the normalization engine
    /// applies every entry unconditionally, a duplicated add-back would be
    /// silently counted twice and report a wrong adjusted value. This is
    /// called by [`normalize`], so every path is covered;
    /// call it directly to reject a bad config earlier.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty or target-equal output node, and one
    /// naming the first adjustment ID that appears more than once.
    ///
    /// [`normalize`]: crate::adjustments::normalize
    pub fn validate(&self) -> crate::error::Result<()> {
        if self.output_node.is_empty() || self.output_node == self.target_node {
            return Err(crate::error::Error::invalid_input(format!(
                "Normalization of '{}' needs an output node with a new id; the reported node is \
                 never overwritten",
                self.target_node
            )));
        }
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
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier, unique within the configuration.
    /// * `name` - Label shown in the bridge.
    /// * `amounts` - Signed amount per period in the target's units: scalars
    ///   for a scalar target, money in its currency for a monetary one.
    ///   Periods not listed adjust by zero.
    pub fn fixed(
        id: impl Into<String>,
        name: impl Into<String>,
        amounts: IndexMap<PeriodId, AmountOrScalar>,
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

    /// Cap this adjustment's magnitude.
    ///
    /// # Arguments
    ///
    /// * `cap` - An absolute limit in the target's units, or a fraction of a
    ///   base node's value in the same period.
    #[must_use]
    pub fn with_cap(mut self, cap: AdjustmentCap) -> Self {
        self.cap = Some(cap);
        self
    }

    /// Set the grouping category used by reports.
    ///
    /// # Arguments
    ///
    /// * `category` - Free-form grouping label such as `"one_time"` or
    ///   `"run_rate"`; it replaces any category already set and does not
    ///   affect the adjustment amount.
    ///
    /// # Example
    ///
    /// ```rust
    /// use finstack_quant_statements::adjustments::Adjustment;
    ///
    /// let adjustment =
    ///     Adjustment::percentage("synergies", "Synergies", "revenue", 0.05).with_category("run_rate");
    /// assert_eq!(adjustment.category.as_deref(), Some("run_rate"));
    /// ```
    #[must_use]
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_category_sets_the_category_and_keeps_the_amount() {
        let adjustment = Adjustment::percentage("synergies", "Synergies", "revenue", 0.05);
        assert_eq!(adjustment.category, None);

        let tagged = adjustment.clone().with_category("run_rate");

        assert_eq!(tagged.category.as_deref(), Some("run_rate"));
        assert_eq!(tagged.value, adjustment.value);
        assert_eq!(
            tagged.with_category("one_time").category.as_deref(),
            Some("one_time")
        );
    }
}
