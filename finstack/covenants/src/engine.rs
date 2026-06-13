//! Covenant engine for evaluating and applying covenant consequences.
//!
//! This module provides a comprehensive covenant evaluation system that:
//! - Evaluates financial covenants against current metrics
//! - Manages grace/cure periods
//! - Applies consequences when covenants are breached
//! - Supports both financial and non-financial covenants
//!
//! # Scope and conventions
//!
//! - **Test dates are caller-controlled.** [`Covenant::test_frequency`] is
//!   descriptive metadata only: the engine evaluates whenever the caller
//!   invokes [`CovenantEngine::evaluate`] with a `test_date` and does not
//!   itself generate or enforce a testing schedule.
//! - **Equity cures are not modeled.** A breach can only be neutralized via
//!   a [`CovenantWaiver`] (full waiver or amended threshold) or by the metric
//!   recovering before the cure deadline; there is no mechanism for injecting
//!   sponsor equity into the tested metric.
//! - **Metric values are taken as-is (LTM contract).** The engine performs no
//!   trailing-twelve-month or other window aggregation. If a covenant is
//!   defined on an LTM basis (as most leverage/coverage covenants are), the
//!   supplied metric node must already encode it — e.g. a statements node
//!   defined via `ttm(ebitda)` — before being exposed through
//!   [`crate::metric::CovenantMetricSource`].

use crate::metric::{CovenantMetricId, CovenantMetricSource};
use crate::schedule::{threshold_for_date, ThresholdSchedule};
use crate::CovenantReport;
use finstack_core::dates::Date;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

// Covenant type definitions were previously under loan; re-introduce minimal versions locally
/// Whether a covenant is tested periodically or only upon an action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CovenantScope {
    /// Tested on a schedule (e.g., quarterly leverage tests).
    Maintenance,
    /// Tested only upon specific actions (e.g., incurrence of debt).
    Incurrence,
}

/// Optional activation condition for springing covenants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpringingCondition {
    /// Metric that controls activation (e.g., revolver utilization).
    pub metric_id: CovenantMetricId,
    /// Threshold test applied to the metric.
    pub test: ThresholdTest,
}

/// Financial covenant specification with test frequency and consequences.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Covenant {
    /// Type of covenant (leverage, coverage, etc.)
    pub covenant_type: CovenantType,
    /// How frequently the covenant is tested.
    ///
    /// Descriptive metadata only: the engine does **not** enforce this
    /// schedule. Callers control test dates by choosing when to invoke
    /// [`CovenantEngine::evaluate`].
    pub test_frequency: finstack_core::dates::Tenor,
    /// Optional cure period in days before default
    pub cure_period_days: Option<i32>,
    /// Actions taken if covenant is breached
    pub consequences: Vec<CovenantConsequence>,
    /// Whether the covenant is currently active
    pub is_active: bool,
    /// Whether the covenant is maintenance or incurrence.
    pub scope: CovenantScope,
    /// Optional activation condition for springing covenants.
    pub springing_condition: Option<SpringingCondition>,
    /// Optional instance label disambiguating covenants of the same type.
    ///
    /// [`CovenantType::covenant_id`] is discriminant-only, so two covenants of
    /// the same type (e.g. a senior and a total leverage test, or two baskets)
    /// would otherwise collide in compliance reports and breach tracking. Set a
    /// distinct label here and waivers/breaches will key off it; when `None`,
    /// the identity falls back to the type's `covenant_id` (legacy behavior).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl Covenant {
    /// Create a new covenant with default cure period
    pub fn new(covenant_type: CovenantType, test_frequency: finstack_core::dates::Tenor) -> Self {
        Self {
            covenant_type,
            test_frequency,
            cure_period_days: Some(30),
            consequences: Vec::new(),
            is_active: true,
            scope: CovenantScope::Maintenance,
            springing_condition: None,
            label: None,
        }
    }

    /// Set cure period (days before breach becomes default)
    pub fn with_cure_period(mut self, days: Option<i32>) -> Self {
        self.cure_period_days = days;
        self
    }

    /// Add a consequence for covenant breach
    pub fn with_consequence(mut self, consequence: CovenantConsequence) -> Self {
        self.consequences.push(consequence);
        self
    }

    /// Set covenant scope (maintenance vs incurrence).
    pub fn with_scope(mut self, scope: CovenantScope) -> Self {
        self.scope = scope;
        self
    }

    /// Attach a springing condition that controls activation.
    pub fn with_springing_condition(mut self, condition: SpringingCondition) -> Self {
        self.springing_condition = Some(condition);
        self
    }

    /// Set an instance label disambiguating covenants that share a type.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Get human-readable description of the covenant
    pub fn description(&self) -> String {
        self.covenant_type.to_string()
    }

    /// Stable identity key for reports, breaches, and waivers.
    ///
    /// Returns the instance [`label`](Self::label) if set, otherwise falls back
    /// to the type's discriminant-only [`CovenantType::covenant_id`]. Using this
    /// (rather than `covenant_id` alone) prevents two same-type covenants from
    /// silently overwriting each other in reports/breach tracking.
    pub fn instance_key(&self) -> String {
        self.label
            .clone()
            .unwrap_or_else(|| self.covenant_type.covenant_id().to_string())
    }

    pub(crate) fn validate(&self) -> finstack_core::Result<()> {
        if self.cure_period_days.is_some_and(|days| days < 0) {
            return Err(finstack_core::Error::Validation(
                "cure_period_days must be non-negative".to_string(),
            ));
        }
        self.covenant_type.validate()
    }
}

/// Type of financial or operational covenant
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum CovenantType {
    /// Maximum debt-to-EBITDA ratio
    MaxDebtToEBITDA {
        /// Maximum allowed ratio
        threshold: f64,
    },
    /// Minimum interest coverage ratio (EBIT/Interest)
    MinInterestCoverage {
        /// Minimum required ratio
        threshold: f64,
    },
    /// Minimum fixed charge coverage ratio
    MinFixedChargeCoverage {
        /// Minimum required coverage
        threshold: f64,
    },
    /// Maximum total leverage ratio
    MaxTotalLeverage {
        /// Maximum allowed leverage
        threshold: f64,
    },
    /// Maximum senior leverage ratio
    MaxSeniorLeverage {
        /// Maximum allowed senior leverage
        threshold: f64,
    },
    /// Minimum asset coverage ratio
    MinAssetCoverage {
        /// Minimum required coverage
        threshold: f64,
    },
    /// Negative covenant (prohibition)
    Negative {
        /// Description of restriction
        restriction: String,
    },
    /// Affirmative covenant (requirement)
    Affirmative {
        /// Description of requirement
        requirement: String,
    },
    /// Custom covenant with metric and threshold test
    Custom {
        /// Name of metric to test
        metric: String,
        /// Threshold test (min or max)
        test: ThresholdTest,
    },
    /// Basket tracking covenant (e.g., available debt baskets)
    Basket {
        /// Basket identifier/metric name
        name: String,
        /// Maximum allowed utilization of the basket
        limit: f64,
    },
    /// Minimum debt service coverage ratio (EBITDA / Debt Service)
    MinDSCR {
        /// Minimum required coverage
        threshold: f64,
    },
    /// Maximum net debt to EBITDA ratio (net of cash)
    MaxNetDebtToEBITDA {
        /// Maximum allowed ratio
        threshold: f64,
    },
    /// Maximum capital expenditure
    MaxCapex {
        /// Maximum allowed capex amount
        threshold: f64,
    },
    /// Minimum liquidity (cash + available revolver)
    MinLiquidity {
        /// Minimum required liquidity
        threshold: f64,
    },
}

impl std::fmt::Display for CovenantType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CovenantType::MaxDebtToEBITDA { threshold } => {
                write!(f, "Debt/EBITDA <= {:.2}x", threshold)
            }
            CovenantType::MinInterestCoverage { threshold } => {
                write!(f, "Interest Coverage >= {:.2}x", threshold)
            }
            CovenantType::MinFixedChargeCoverage { threshold } => {
                write!(f, "Fixed Charge Coverage >= {:.2}x", threshold)
            }
            CovenantType::MaxTotalLeverage { threshold } => {
                write!(f, "Total Leverage <= {:.2}x", threshold)
            }
            CovenantType::MaxSeniorLeverage { threshold } => {
                write!(f, "Senior Leverage <= {:.2}x", threshold)
            }
            CovenantType::MinAssetCoverage { threshold } => {
                write!(f, "Asset Coverage >= {:.2}x", threshold)
            }
            CovenantType::Negative { restriction } => write!(f, "Negative: {}", restriction),
            CovenantType::Affirmative { requirement } => {
                write!(f, "Affirmative: {}", requirement)
            }
            CovenantType::Custom { metric, test } => match test {
                ThresholdTest::Maximum(v) => write!(f, "{} <= {:.2}", metric, v),
                ThresholdTest::Minimum(v) => write!(f, "{} >= {:.2}", metric, v),
            },
            CovenantType::Basket { name, limit } => {
                write!(f, "{} Utilization <= {:.2}", name, limit)
            }
            CovenantType::MinDSCR { threshold } => {
                write!(f, "DSCR >= {:.2}x", threshold)
            }
            CovenantType::MaxNetDebtToEBITDA { threshold } => {
                write!(f, "Net Debt/EBITDA <= {:.2}x", threshold)
            }
            CovenantType::MaxCapex { threshold } => {
                write!(f, "Capex <= {:.2}", threshold)
            }
            CovenantType::MinLiquidity { threshold } => {
                write!(f, "Liquidity >= {:.2}", threshold)
            }
        }
    }
}

/// Threshold test type (maximum or minimum bound)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ThresholdTest {
    /// Maximum allowed value
    Maximum(f64),
    /// Minimum required value
    Minimum(f64),
}

/// Direction of inequality for numeric covenants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoundKind {
    /// Covenant passes when the metric is less than or equal to the threshold.
    AtMost,
    /// Covenant passes when the metric is greater than or equal to the threshold.
    AtLeast,
}

impl CovenantType {
    fn validate(&self) -> finstack_core::Result<()> {
        let value = match self {
            CovenantType::MaxDebtToEBITDA { threshold }
            | CovenantType::MinInterestCoverage { threshold }
            | CovenantType::MinFixedChargeCoverage { threshold }
            | CovenantType::MaxTotalLeverage { threshold }
            | CovenantType::MaxSeniorLeverage { threshold }
            | CovenantType::MinAssetCoverage { threshold }
            | CovenantType::MinDSCR { threshold }
            | CovenantType::MaxNetDebtToEBITDA { threshold }
            | CovenantType::MaxCapex { threshold }
            | CovenantType::MinLiquidity { threshold } => Some(*threshold),
            CovenantType::Custom { test, .. } => match test {
                ThresholdTest::Maximum(t) | ThresholdTest::Minimum(t) => Some(*t),
            },
            CovenantType::Basket { limit, .. } => Some(*limit),
            CovenantType::Negative { .. } | CovenantType::Affirmative { .. } => None,
        };
        if value.is_some_and(|v| !v.is_finite()) {
            return Err(finstack_core::Error::Validation(
                "covenant thresholds and limits must be finite".to_string(),
            ));
        }
        Ok(())
    }

    /// Returns the inequality direction required for numeric covenants.
    pub fn bound_kind(&self) -> Option<BoundKind> {
        match self {
            CovenantType::MaxDebtToEBITDA { .. }
            | CovenantType::MaxTotalLeverage { .. }
            | CovenantType::MaxSeniorLeverage { .. }
            | CovenantType::MaxNetDebtToEBITDA { .. }
            | CovenantType::MaxCapex { .. }
            | CovenantType::Basket { .. }
            | CovenantType::Custom {
                test: ThresholdTest::Maximum(_),
                ..
            } => Some(BoundKind::AtMost),
            CovenantType::MinInterestCoverage { .. }
            | CovenantType::MinFixedChargeCoverage { .. }
            | CovenantType::MinAssetCoverage { .. }
            | CovenantType::MinDSCR { .. }
            | CovenantType::MinLiquidity { .. }
            | CovenantType::Custom {
                test: ThresholdTest::Minimum(_),
                ..
            } => Some(BoundKind::AtLeast),
            CovenantType::Negative { .. } | CovenantType::Affirmative { .. } => None,
        }
    }

    /// Returns the scalar threshold (if any) associated with the covenant type.
    pub(crate) fn threshold_value(&self) -> Option<f64> {
        match self {
            CovenantType::MaxDebtToEBITDA { threshold }
            | CovenantType::MinInterestCoverage { threshold }
            | CovenantType::MinFixedChargeCoverage { threshold }
            | CovenantType::MaxTotalLeverage { threshold }
            | CovenantType::MaxSeniorLeverage { threshold }
            | CovenantType::MinAssetCoverage { threshold }
            | CovenantType::MinDSCR { threshold }
            | CovenantType::MaxNetDebtToEBITDA { threshold }
            | CovenantType::MaxCapex { threshold }
            | CovenantType::MinLiquidity { threshold } => Some(*threshold),
            CovenantType::Custom { test, .. } => match test {
                ThresholdTest::Maximum(t) | ThresholdTest::Minimum(t) => Some(*t),
            },
            CovenantType::Basket { limit, .. } => Some(*limit),
            CovenantType::Negative { .. } | CovenantType::Affirmative { .. } => None,
        }
    }

    /// Returns the canonical metric identifier for the covenant type when one exists.
    pub(crate) fn default_metric_name(&self) -> Option<&'static str> {
        match self {
            CovenantType::MaxDebtToEBITDA { .. } => Some("debt_to_ebitda"),
            CovenantType::MinInterestCoverage { .. } => Some("interest_coverage"),
            CovenantType::MinFixedChargeCoverage { .. } => Some("fixed_charge_coverage"),
            CovenantType::MaxTotalLeverage { .. } => Some("total_leverage"),
            CovenantType::MaxSeniorLeverage { .. } => Some("senior_leverage"),
            CovenantType::MinAssetCoverage { .. } => Some("asset_coverage"),
            CovenantType::MinDSCR { .. } => Some("dscr"),
            CovenantType::MaxNetDebtToEBITDA { .. } => Some("net_debt_to_ebitda"),
            CovenantType::MaxCapex { .. } => Some("capex"),
            CovenantType::MinLiquidity { .. } => Some("liquidity"),
            CovenantType::Custom { .. }
            | CovenantType::Basket { .. }
            | CovenantType::Negative { .. }
            | CovenantType::Affirmative { .. } => None,
        }
    }

    /// Returns true for built-in maximum covenants whose metric is a ratio
    /// with an earnings-style denominator (leverage-type tests).
    ///
    /// For these covenants a *negative* metric value almost always means the
    /// denominator (EBITDA) has gone negative, i.e. the ratio is not
    /// meaningful ("NM" in rating-agency parlance) rather than extraordinarily
    /// good. A naive `value <= threshold` test would let a distressed,
    /// negative-EBITDA borrower pass a max-leverage covenant with huge
    /// apparent headroom. The engine therefore treats negative values on
    /// these covenants as breaches. `Custom` maximum covenants are *not*
    /// included: their metric semantics are caller-defined and negative
    /// values may be legitimate.
    pub(crate) fn is_ratio_max(&self) -> bool {
        matches!(
            self,
            CovenantType::MaxDebtToEBITDA { .. }
                | CovenantType::MaxTotalLeverage { .. }
                | CovenantType::MaxSeniorLeverage { .. }
                | CovenantType::MaxNetDebtToEBITDA { .. }
        )
    }

    /// Stable machine-readable identifier based on the variant discriminant only.
    ///
    /// Thresholds are **not** included because they can be amended by waivers or
    /// overridden by threshold schedules. If multiple covenants of the same type
    /// exist, callers should assign a disambiguating label externally.
    pub fn covenant_id(&self) -> &'static str {
        match self {
            CovenantType::MaxDebtToEBITDA { .. } => "max_debt_ebitda",
            CovenantType::MinInterestCoverage { .. } => "min_interest_coverage",
            CovenantType::MinFixedChargeCoverage { .. } => "min_fcc",
            CovenantType::MaxTotalLeverage { .. } => "max_total_leverage",
            CovenantType::MaxSeniorLeverage { .. } => "max_senior_leverage",
            CovenantType::MinAssetCoverage { .. } => "min_asset_coverage",
            CovenantType::MinDSCR { .. } => "min_dscr",
            CovenantType::MaxNetDebtToEBITDA { .. } => "max_net_debt_ebitda",
            CovenantType::MaxCapex { .. } => "max_capex",
            CovenantType::MinLiquidity { .. } => "min_liquidity",
            CovenantType::Negative { .. } => "negative",
            CovenantType::Affirmative { .. } => "affirmative",
            CovenantType::Custom { .. } => "custom",
            CovenantType::Basket { .. } => "basket",
        }
    }
}

/// Consequence of covenant breach
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum CovenantConsequence {
    /// Event of default
    Default,
    /// Interest rate margin increase
    RateIncrease {
        /// Increase in basis points
        bp_increase: f64,
    },
    /// Mandatory cash sweep of excess cash flow
    CashSweep {
        /// Percentage of cash flow to sweep
        sweep_percentage: f64,
    },
    /// Block distributions to equity holders
    BlockDistributions,
    /// Require additional collateral
    RequireCollateral {
        /// Description of collateral requirement
        description: String,
    },
    /// Accelerate loan maturity date
    AccelerateMaturity {
        /// New accelerated maturity date
        new_maturity: Date,
    },
}

/// Whether the covenant test is triggered by a scheduled maintenance check or
/// a specific incurrence action. The engine uses this to filter specs by scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum EvaluationTrigger {
    /// Scheduled periodic test (e.g., quarterly compliance).
    Maintenance,
    /// Test triggered by a specific action (e.g., new debt issuance).
    Incurrence {
        /// Description of the triggering action.
        action: String,
    },
}

/// A covenant waiver or amendment granted by lenders.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CovenantWaiver {
    /// Stable instance identifier of the waived covenant
    /// (from [`Covenant::instance_key`]).
    pub covenant_id: String,
    /// Start date of the waiver period.
    pub effective_date: Date,
    /// End date of the waiver period (None = permanent amendment).
    pub expiry_date: Option<Date>,
    /// Amended threshold (if this is an amendment rather than a full waiver).
    pub amended_threshold: Option<f64>,
    /// Free-text description of the waiver terms.
    pub description: String,
}

use finstack_core::HashMap;
use indexmap::IndexMap;
use std::sync::Arc;

/// Covenant evaluation context passed to custom evaluators and metric calculators.
pub struct CovenantEvalCtx<'a> {
    /// Metric source for operating metrics such as EBITDA, leverage, or DSCR.
    pub metrics: &'a mut (dyn CovenantMetricSource + 'a),
    /// Covenant evaluation date.
    pub as_of: Date,
}

/// Type alias for custom evaluator functions.
pub(crate) type CustomEvaluator =
    Arc<dyn for<'a> Fn(&mut CovenantEvalCtx<'a>) -> finstack_core::Result<bool> + Send + Sync>;

/// Type alias for custom metric calculators.
pub(crate) type CustomMetricCalculator =
    Arc<dyn for<'a> Fn(&mut CovenantEvalCtx<'a>) -> finstack_core::Result<f64> + Send + Sync>;

/// Covenant evaluation specification.
///
/// Note: The `custom_evaluator` field is not serialized as it contains
/// a function pointer. When deserializing, it will be set to `None`.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CovenantSpec {
    /// The covenant to evaluate
    pub covenant: Covenant,
    /// Metric ID to use for evaluation (for financial covenants)
    pub metric_id: Option<CovenantMetricId>,
    /// Time-varying threshold schedule that overrides the static threshold in
    /// [`CovenantType`] when present. Enables leverage step-down schedules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold_schedule: Option<ThresholdSchedule>,
    /// Custom evaluation function (for complex covenants).
    /// Not serializable - will be `None` after deserialization.
    #[serde(skip)]
    pub custom_evaluator: Option<CustomEvaluator>,
}

// Derive-based Clone now works because custom_evaluator uses Arc

impl std::fmt::Debug for CovenantSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CovenantSpec")
            .field("covenant", &self.covenant)
            .field("metric_id", &self.metric_id)
            .field("custom_evaluator", &self.custom_evaluator.is_some())
            .finish()
    }
}

impl CovenantSpec {
    /// Create a new covenant spec with a standard metric.
    pub fn with_metric(covenant: Covenant, metric_id: impl Into<CovenantMetricId>) -> Self {
        Self {
            covenant,
            metric_id: Some(metric_id.into()),
            threshold_schedule: None,
            custom_evaluator: None,
        }
    }

    /// Create a new covenant spec with a custom evaluator.
    pub fn with_evaluator<F>(covenant: Covenant, evaluator: F) -> Self
    where
        F: for<'a> Fn(&mut CovenantEvalCtx<'a>) -> finstack_core::Result<bool>
            + Send
            + Sync
            + 'static,
    {
        Self {
            covenant,
            metric_id: None,
            threshold_schedule: None,
            custom_evaluator: Some(Arc::new(evaluator)),
        }
    }

    /// Attach a time-varying threshold schedule (e.g., leverage step-downs).
    pub fn with_threshold_schedule(mut self, schedule: ThresholdSchedule) -> Self {
        self.threshold_schedule = Some(schedule);
        self
    }

    pub(crate) fn validate(&self) -> finstack_core::Result<()> {
        self.covenant.validate()?;
        if let Some(schedule) = &self.threshold_schedule {
            schedule.validate()?;
        }
        Ok(())
    }
}

/// Covenant test specification with timing windows.
///
/// This is a serialization-friendly envelope used by higher-level tooling.
/// The `CovenantEngine` does not currently evaluate `CovenantTestSpec`
/// instances directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CovenantTestSpec {
    /// Covenant specifications to test
    pub specs: Vec<CovenantSpec>,
    /// Test date
    pub test_date: Date,
    /// Reference date for calculating cure periods
    pub reference_date: Option<Date>,
}

/// Covenant window for scheduled testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CovenantWindow {
    /// Start date of the window
    pub start: Date,
    /// End date of the window
    pub end: Date,
    /// Covenants active during this window
    pub covenants: Vec<CovenantSpec>,
}

/// Covenant breach tracking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CovenantBreach {
    /// Stable identifier matching [`Covenant::instance_key`].
    #[serde(default)]
    pub covenant_id: String,
    /// Human-readable description (from `Display`).
    pub covenant_type: String,
    /// Date of the breach
    pub breach_date: Date,
    /// Actual value that caused the breach
    pub actual_value: Option<f64>,
    /// Required threshold
    pub threshold: Option<f64>,
    /// Cure period end date (if applicable)
    pub cure_deadline: Option<Date>,
    /// Whether the breach has been cured
    pub is_cured: bool,
    /// Applied consequences
    pub applied_consequences: Vec<CovenantConsequence>,
}

/// Covenant engine for evaluation and consequence application.
///
/// Note: The `custom_metrics` field is not serialized as it contains
/// function pointers. When deserializing, it will be set to default (empty).
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CovenantEngine {
    /// Active covenant specifications
    pub specs: Vec<CovenantSpec>,
    /// Historical breaches
    pub breach_history: Vec<CovenantBreach>,
    /// Covenant testing windows
    pub windows: Vec<CovenantWindow>,
    /// Active waivers and amendments
    #[serde(default)]
    pub waivers: Vec<CovenantWaiver>,
    /// Custom metric calculators.
    /// Not serializable - will be empty after deserialization.
    #[serde(skip)]
    pub custom_metrics: HashMap<String, CustomMetricCalculator>,
}

impl std::fmt::Debug for CovenantEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CovenantEngine")
            .field("specs", &self.specs)
            .field("breach_history", &self.breach_history)
            .field("windows", &self.windows)
            .field("waivers", &self.waivers)
            .field(
                "custom_metrics",
                &self.custom_metrics.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl Default for CovenantEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CovenantEngine {
    /// Create a new covenant engine.
    pub fn new() -> Self {
        Self {
            specs: Vec::new(),
            breach_history: Vec::new(),
            windows: Vec::new(),
            waivers: Vec::new(),
            custom_metrics: HashMap::default(),
        }
    }

    /// Validate engine configuration before evaluation or JSON canonicalization.
    pub fn validate(&self) -> finstack_core::Result<()> {
        for spec in &self.specs {
            spec.validate()?;
        }
        for window in &self.windows {
            if window.start > window.end {
                return Err(finstack_core::Error::Validation(format!(
                    "covenant window start {} must be on or before end {}",
                    window.start, window.end
                )));
            }
            for spec in &window.covenants {
                spec.validate()?;
            }
        }
        for left_index in 0..self.windows.len() {
            for right_index in (left_index + 1)..self.windows.len() {
                let left = &self.windows[left_index];
                let right = &self.windows[right_index];
                if left.start <= right.end && left.end >= right.start {
                    return Err(finstack_core::Error::Validation(format!(
                        "covenant windows must not overlap: [{}, {}] overlaps [{}, {}]",
                        left.start, left.end, right.start, right.end
                    )));
                }
            }
        }
        let mut seen_windows = BTreeSet::new();
        for window in &self.windows {
            let key = (window.start, window.end);
            if !seen_windows.insert(key) {
                return Err(finstack_core::Error::Validation(format!(
                    "duplicate covenant window [{}, {}]",
                    window.start, window.end
                )));
            }
        }
        for waiver in &self.waivers {
            if waiver
                .expiry_date
                .is_some_and(|expiry| expiry < waiver.effective_date)
            {
                return Err(finstack_core::Error::Validation(format!(
                    "waiver '{}' expiry date must be on or after effective date",
                    waiver.covenant_id
                )));
            }
            if waiver
                .amended_threshold
                .is_some_and(|value| !value.is_finite())
            {
                return Err(finstack_core::Error::Validation(format!(
                    "waiver '{}' amended_threshold must be finite",
                    waiver.covenant_id
                )));
            }
        }
        Ok(())
    }

    /// Add a covenant specification.
    pub fn add_spec(&mut self, spec: CovenantSpec) -> &mut Self {
        self.specs.push(spec);
        self
    }

    /// Add a covenant window.
    ///
    /// Window overlap is validated by [`validate`](Self::validate) before
    /// evaluation and JSON canonicalization.
    pub fn add_window(&mut self, window: CovenantWindow) -> &mut Self {
        self.windows.push(window);
        self
    }

    /// Record a covenant waiver or amendment.
    pub fn add_waiver(&mut self, waiver: CovenantWaiver) -> &mut Self {
        self.waivers.push(waiver);
        self
    }

    /// Register a custom metric calculator.
    pub fn register_metric<CalcFn>(
        &mut self,
        name: impl Into<String>,
        calculator: CalcFn,
    ) -> &mut Self
    where
        CalcFn: for<'a> Fn(&mut CovenantEvalCtx<'a>) -> finstack_core::Result<f64>
            + Send
            + Sync
            + 'static,
    {
        self.custom_metrics
            .insert(name.into(), Arc::new(calculator));
        self
    }

    /// Evaluate all covenants against current metrics (both maintenance and incurrence).
    ///
    /// Use [`evaluate_for_trigger`](Self::evaluate_for_trigger) to test only
    /// covenants matching a specific scope.
    pub fn evaluate(
        &self,
        context: &mut dyn CovenantMetricSource,
        test_date: Date,
    ) -> finstack_core::Result<IndexMap<String, CovenantReport>> {
        self.validate()?;
        let applicable_specs = self.get_applicable_specs_internal(test_date);
        self.evaluate_specs(&applicable_specs, context, test_date)
    }

    fn evaluate_specs(
        &self,
        specs: &[&CovenantSpec],
        context: &mut dyn CovenantMetricSource,
        test_date: Date,
    ) -> finstack_core::Result<IndexMap<String, CovenantReport>> {
        tracing::debug!(spec_count = specs.len(), %test_date, "evaluating covenants");

        // Reject duplicate instance keys up front. Two specs sharing an
        // identity would silently overwrite each other in the report map and
        // make consequence resolution ambiguous (e.g. a distribution-lockup
        // breach resolving to a same-type covenant carrying a Default
        // consequence). Same-type covenants must be disambiguated via
        // [`Covenant::with_label`].
        {
            let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            for spec in specs {
                let key = spec.covenant.instance_key();
                if !seen.insert(key.clone()) {
                    return Err(finstack_core::Error::Validation(format!(
                        "duplicate covenant instance key '{key}': covenants sharing a type must \
                         be disambiguated with Covenant::with_label",
                    )));
                }
            }
        }

        let mut reports = IndexMap::new();

        for spec in specs {
            // Identity key (instance label if set, else the type discriminant).
            // Reports and breaches are keyed on this so two same-type covenants
            // don't silently overwrite each other.
            let cid = spec.covenant.instance_key();
            let cid = cid.as_str();
            let description = spec.covenant.description();

            if !spec.covenant.is_active {
                reports.insert(
                    cid.to_string(),
                    CovenantReport::passed(&description)
                        .with_covenant_id(cid)
                        .with_details("Covenant inactive"),
                );
                continue;
            }

            if let Some(waiver) = self.active_waiver(cid, test_date) {
                if waiver.amended_threshold.is_none() {
                    tracing::info!(covenant_id = cid, %test_date, "covenant waived by lender agreement");
                    reports.insert(
                        cid.to_string(),
                        CovenantReport::passed(&description)
                            .with_covenant_id(cid)
                            .with_details("Waived by lender agreement"),
                    );
                    continue;
                }
            }

            let evaluation = self.evaluate_spec(spec, context, test_date)?;

            let mut report = if evaluation.passed {
                CovenantReport::passed(&description)
            } else {
                CovenantReport::failed(&description)
            };
            report = report.with_covenant_id(cid);

            if let Some(value) = evaluation.actual_value {
                report = report.with_actual(value);
            }
            if let Some(thresh) = evaluation.threshold {
                report = report.with_threshold(thresh);
            }
            if let Some(hr) = evaluation.headroom {
                report = report.with_headroom(hr);
            }

            if !evaluation.passed {
                tracing::warn!(
                    covenant_id = cid,
                    actual = evaluation.actual_value,
                    threshold = evaluation.threshold,
                    %test_date,
                    "covenant breach detected",
                );
                if let Some(breach) = self.find_active_breach(cid, test_date) {
                    if breach.cure_deadline.is_some_and(|d| test_date <= d) {
                        report = report.with_details("In cure period");
                    }
                }
            }

            if let Some(detail) = evaluation.detail {
                report = report.with_details(&detail);
            }

            reports.insert(cid.to_string(), report);
        }

        Ok(reports)
    }

    /// Evaluate only covenants matching the given trigger scope.
    ///
    /// `Maintenance` triggers test covenants with [`CovenantScope::Maintenance`].
    /// `Incurrence` triggers test covenants with [`CovenantScope::Incurrence`].
    /// This avoids the common error of testing incurrence covenants on a
    /// periodic schedule when they should only fire on specific actions.
    pub fn evaluate_for_trigger(
        &self,
        context: &mut dyn CovenantMetricSource,
        test_date: Date,
        trigger: &EvaluationTrigger,
    ) -> finstack_core::Result<IndexMap<String, CovenantReport>> {
        self.validate()?;
        let required_scope = match trigger {
            EvaluationTrigger::Maintenance => CovenantScope::Maintenance,
            EvaluationTrigger::Incurrence { .. } => CovenantScope::Incurrence,
        };

        let applicable_specs = self.get_applicable_specs_internal(test_date);
        let filtered: Vec<&CovenantSpec> = applicable_specs
            .into_iter()
            .filter(|s| s.covenant.scope == required_scope)
            .collect();

        self.evaluate_specs(&filtered, context, test_date)
    }

    /// Evaluate covenants and automatically record breaches in history.
    ///
    /// Combines [`evaluate`](Self::evaluate) with breach tracking: any failing
    /// covenant that doesn't already have an active (uncured) breach record
    /// gets a new [`CovenantBreach`] entry in `breach_history`.
    pub fn evaluate_and_track(
        &mut self,
        context: &mut dyn CovenantMetricSource,
        test_date: Date,
    ) -> finstack_core::Result<IndexMap<String, CovenantReport>> {
        let reports = self.evaluate(context, test_date)?;

        for (_key, report) in &reports {
            if !report.passed {
                continue;
            }
            let Some(cid) = report.covenant_id.as_deref() else {
                continue;
            };
            if let Some(breach) = self
                .breach_history
                .iter_mut()
                .filter(|b| b.covenant_id == cid && !b.is_cured && b.breach_date <= test_date)
                .max_by_key(|b| b.breach_date)
            {
                if breach
                    .cure_deadline
                    .is_some_and(|deadline| test_date <= deadline)
                {
                    tracing::info!(
                        covenant_id = cid,
                        breach_date = %breach.breach_date,
                        %test_date,
                        "marking covenant breach cured by metric recovery",
                    );
                    breach.is_cured = true;
                }
            }
        }

        for (_key, report) in &reports {
            if report.passed {
                continue;
            }

            let cid = report.covenant_id.as_deref().unwrap_or("unknown");
            // Human-readable label for the breach record (the map key is the
            // stable identity key, not the display name).
            let description = report.covenant_type.clone();

            let already_tracked = self
                .breach_history
                .iter()
                .any(|b| b.covenant_id == cid && !b.is_cured && b.breach_date <= test_date);
            if already_tracked {
                continue;
            }

            let spec = self.specs.iter().find(|s| s.covenant.instance_key() == cid);

            let cure_deadline = spec.and_then(|s| {
                s.covenant
                    .cure_period_days
                    .map(|d| test_date + time::Duration::days(d as i64))
            });

            tracing::warn!(
                covenant_id = cid,
                actual = report.actual_value,
                threshold = report.threshold,
                %test_date,
                "recording new covenant breach",
            );

            self.breach_history.push(CovenantBreach {
                covenant_id: cid.to_string(),
                covenant_type: description.clone(),
                breach_date: test_date,
                actual_value: report.actual_value,
                threshold: report.threshold,
                cure_deadline,
                is_cured: false,
                applied_consequences: Vec::new(),
            });
        }

        Ok(reports)
    }

    /// Apply consequences for breached covenants.
    ///
    /// Consequences that have already been applied (recorded in `breach_history`)
    /// are skipped to prevent double-application.
    pub fn apply_consequences<T>(
        &mut self,
        instrument: &mut T,
        breaches: &[CovenantBreach],
        as_of: Date,
    ) -> finstack_core::Result<Vec<ConsequenceApplication>>
    where
        T: InstrumentMutator,
    {
        let mut applications = Vec::new();

        for breach in breaches {
            if breach.is_cured {
                continue;
            }
            if let Some(deadline) = breach.cure_deadline {
                if as_of <= deadline {
                    continue;
                }
            }

            // Guard: skip if consequences were already applied for this breach
            let already_applied = self.breach_history.iter().any(|b| {
                b.covenant_id == breach.covenant_id
                    && b.breach_date == breach.breach_date
                    && !b.applied_consequences.is_empty()
            });
            if already_applied {
                tracing::debug!(
                    covenant_id = %breach.covenant_id,
                    breach_date = %breach.breach_date,
                    "skipping consequence application — already applied",
                );
                continue;
            }

            let spec = self
                .specs
                .iter()
                .find(|s| s.covenant.instance_key() == breach.covenant_id)
                .ok_or(finstack_core::InputError::NotFound {
                    id: format!("covenant_spec:{}", breach.covenant_id),
                })?;

            for consequence in &spec.covenant.consequences {
                let application = self.apply_single_consequence(instrument, consequence, as_of)?;
                tracing::info!(
                    covenant_id = %breach.covenant_id,
                    consequence = %application.consequence_type,
                    %as_of,
                    "applied covenant consequence",
                );
                applications.push(application);

                if let Some(historical_breach) = self.breach_history.iter_mut().find(|b| {
                    b.covenant_id == breach.covenant_id && b.breach_date == breach.breach_date
                }) {
                    historical_breach
                        .applied_consequences
                        .push(consequence.clone());
                }
            }
        }

        Ok(applications)
    }

    /// Get applicable specs for a given date (public for testing).
    pub fn get_applicable_specs(&self, test_date: Date) -> Vec<&CovenantSpec> {
        self.get_applicable_specs_internal(test_date)
    }

    // Helper methods

    fn get_applicable_specs_internal(&self, test_date: Date) -> Vec<&CovenantSpec> {
        // Check windows first
        for window in &self.windows {
            if test_date >= window.start && test_date <= window.end {
                return window.covenants.iter().collect();
            }
        }

        // Fall back to all specs
        self.specs.iter().collect()
    }

    fn evaluate_spec(
        &self,
        spec: &CovenantSpec,
        context: &mut dyn CovenantMetricSource,
        test_date: Date,
    ) -> finstack_core::Result<SpecEvaluation> {
        // Springing conditions: skip evaluation until activation criteria met.
        if let Some(condition) = &spec.covenant.springing_condition {
            let condition_value =
                self.get_metric_value(context, &condition.metric_id, test_date)?;
            let condition_met = match condition.test {
                ThresholdTest::Maximum(t) => condition_value <= t,
                ThresholdTest::Minimum(t) => condition_value >= t,
            };

            if !condition_met {
                tracing::debug!(
                    metric = condition.metric_id.as_str(),
                    value = condition_value,
                    "springing condition not met — covenant inactive",
                );
                return Ok(SpecEvaluation {
                    passed: true,
                    actual_value: None,
                    threshold: None,
                    headroom: None,
                    detail: Some("Springing condition not met".to_string()),
                });
            }
        }

        // Use custom evaluator if provided
        if let Some(ref evaluator) = spec.custom_evaluator {
            let mut eval_ctx = CovenantEvalCtx {
                metrics: context,
                as_of: test_date,
            };
            let passed = evaluator(&mut eval_ctx)?;
            return Ok(SpecEvaluation {
                passed,
                actual_value: None,
                threshold: None,
                headroom: None,
                detail: None,
            });
        }

        let covenant_type = &spec.covenant.covenant_type;

        // Non-numeric covenants auto-pass until they have explicit evaluators.
        let Some(base_threshold) = covenant_type.threshold_value() else {
            return Ok(SpecEvaluation {
                passed: true,
                actual_value: None,
                threshold: None,
                headroom: None,
                detail: None,
            });
        };

        // Resolve the effective threshold: waiver amendment > schedule > static.
        let covenant_cid = spec.covenant.instance_key();
        let threshold = self
            .active_waiver(&covenant_cid, test_date)
            .and_then(|w| w.amended_threshold)
            .or_else(|| {
                spec.threshold_schedule
                    .as_ref()
                    .and_then(|s| threshold_for_date(s, test_date))
            })
            .unwrap_or(base_threshold);

        // Otherwise use metric-based evaluation
        let metric_value = if let Some(metric_id) = &spec.metric_id {
            self.get_metric_value(context, metric_id, test_date)?
        } else if let Some(name) = covenant_type.default_metric_name() {
            self.get_metric_value(context, &CovenantMetricId::from(name), test_date)?
        } else {
            match covenant_type {
                CovenantType::Custom { metric, .. } => {
                    self.get_metric_value(context, &CovenantMetricId::from(metric), test_date)?
                }
                CovenantType::Basket { name, .. } => {
                    self.get_metric_value(context, &CovenantMetricId::from(name), test_date)?
                }
                _ => unreachable!("Non-numeric covenants return early above"),
            }
        };

        let mut detail = None;
        let passed = if covenant_type.is_ratio_max() && metric_value < 0.0 {
            // Negative leverage-type ratio: the denominator (EBITDA) has gone
            // negative, so the ratio is not meaningful. Treat as a breach
            // rather than letting `value <= threshold` pass with huge
            // apparent headroom. See [`CovenantType::is_ratio_max`].
            detail = Some(
                "Negative ratio value (negative denominator) — not meaningful, treated as breach"
                    .to_string(),
            );
            false
        } else {
            !is_covenant_breached(covenant_type, metric_value, threshold)
        };

        let headroom = Some(headroom_for(
            covenant_type.bound_kind(),
            metric_value,
            threshold,
        ));

        Ok(SpecEvaluation {
            passed,
            actual_value: Some(metric_value),
            threshold: Some(threshold),
            headroom,
            detail,
        })
    }

    fn get_metric_value(
        &self,
        source: &mut dyn CovenantMetricSource,
        metric_id: &CovenantMetricId,
        as_of: Date,
    ) -> finstack_core::Result<f64> {
        if let Some(calculator) = self.custom_metrics.get(metric_id.as_str()) {
            let mut eval_ctx = CovenantEvalCtx {
                metrics: source,
                as_of,
            };
            return calculator(&mut eval_ctx);
        }

        source.get_metric(metric_id)
    }

    fn active_waiver(&self, covenant_id: &str, as_of: Date) -> Option<&CovenantWaiver> {
        self.waivers.iter().find(|w| {
            w.covenant_id == covenant_id
                && w.effective_date <= as_of
                && w.expiry_date.is_none_or(|exp| as_of <= exp)
        })
    }

    fn find_active_breach(&self, cid: &str, as_of: Date) -> Option<&CovenantBreach> {
        self.breach_history
            .iter()
            .filter(|b| b.covenant_id == cid && !b.is_cured)
            .filter(|b| b.breach_date <= as_of)
            .max_by_key(|b| b.breach_date)
    }

    fn apply_single_consequence<T>(
        &self,
        instrument: &mut T,
        consequence: &CovenantConsequence,
        as_of: Date,
    ) -> finstack_core::Result<ConsequenceApplication>
    where
        T: InstrumentMutator,
    {
        match consequence {
            CovenantConsequence::Default => {
                instrument.set_default_status(true, as_of)?;
                Ok(ConsequenceApplication {
                    consequence_type: "Default".to_string(),
                    applied_date: as_of,
                    details: "Loan in default".to_string(),
                })
            }
            CovenantConsequence::RateIncrease { bp_increase } => {
                instrument.increase_rate(*bp_increase / 10000.0)?;
                Ok(ConsequenceApplication {
                    consequence_type: "Rate Increase".to_string(),
                    applied_date: as_of,
                    details: format!("Rate increased by {} bps", bp_increase),
                })
            }
            CovenantConsequence::CashSweep { sweep_percentage } => {
                instrument.set_cash_sweep(*sweep_percentage)?;
                Ok(ConsequenceApplication {
                    consequence_type: "Cash Sweep".to_string(),
                    applied_date: as_of,
                    details: format!("{}% cash sweep activated", sweep_percentage * 100.0),
                })
            }
            CovenantConsequence::BlockDistributions => {
                instrument.set_distribution_block(true)?;
                Ok(ConsequenceApplication {
                    consequence_type: "Block Distributions".to_string(),
                    applied_date: as_of,
                    details: "Distributions blocked".to_string(),
                })
            }
            CovenantConsequence::RequireCollateral { description } => Ok(ConsequenceApplication {
                consequence_type: "Require Collateral".to_string(),
                applied_date: as_of,
                details: description.clone(),
            }),
            CovenantConsequence::AccelerateMaturity { new_maturity } => {
                instrument.set_maturity(*new_maturity)?;
                Ok(ConsequenceApplication {
                    consequence_type: "Accelerate Maturity".to_string(),
                    applied_date: as_of,
                    details: format!("Maturity accelerated to {}", new_maturity),
                })
            }
        }
    }
}

struct SpecEvaluation {
    passed: bool,
    actual_value: Option<f64>,
    threshold: Option<f64>,
    headroom: Option<f64>,
    detail: Option<String>,
}

/// Relative headroom: signed distance from the threshold, normalized by
/// `|threshold|` so the sign convention (positive = cushion, negative =
/// deficit) is preserved for negative thresholds too. A zero threshold falls
/// back to an absolute distance (denominator 1).
pub(crate) fn headroom_for(bound: Option<BoundKind>, value: f64, threshold: f64) -> f64 {
    if !value.is_finite() || !threshold.is_finite() {
        return f64::NAN;
    }

    let denom = if threshold.abs() < f64::EPSILON {
        1.0
    } else {
        threshold.abs()
    };

    match bound {
        Some(BoundKind::AtMost) => (threshold - value) / denom,
        Some(BoundKind::AtLeast) => (value - threshold) / denom,
        None => 0.0,
    }
}

/// Shared point-in-time and forecast breach convention.
pub(crate) fn is_covenant_breached(
    covenant_type: &CovenantType,
    value: f64,
    threshold: f64,
) -> bool {
    if value.is_nan() {
        // Only NaN is genuinely indeterminate. Infinities retain IEEE ordering:
        // +inf is good for minimum covenants and bad for maximum covenants.
        return true;
    }
    if covenant_type.is_ratio_max() && value < 0.0 {
        return true;
    }
    match covenant_type.bound_kind() {
        Some(BoundKind::AtMost) => value > threshold,
        Some(BoundKind::AtLeast) => value < threshold,
        None => false,
    }
}

/// Result of applying a covenant consequence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsequenceApplication {
    /// Type of consequence applied
    pub consequence_type: String,
    /// Date when applied
    pub applied_date: Date,
    /// Details about the application
    pub details: String,
}

/// Trait for instruments that can be mutated by covenant consequences.
pub trait InstrumentMutator {
    /// Set default status.
    fn set_default_status(&mut self, is_default: bool, as_of: Date) -> finstack_core::Result<()>;

    /// Increase interest rate.
    fn increase_rate(&mut self, increase: f64) -> finstack_core::Result<()>;

    /// Set cash sweep percentage.
    fn set_cash_sweep(&mut self, percentage: f64) -> finstack_core::Result<()>;

    /// Block distributions.
    fn set_distribution_block(&mut self, blocked: bool) -> finstack_core::Result<()>;

    /// Change maturity date.
    fn set_maturity(&mut self, new_maturity: Date) -> finstack_core::Result<()>;
}
