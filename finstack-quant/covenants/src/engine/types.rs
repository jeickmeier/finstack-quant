use crate::metric::CovenantMetricId;
use crate::schedule::ThresholdSchedule;
use finstack_quant_core::dates::Date;
use serde::{Deserialize, Serialize};

/// Whether a covenant is tested periodically or only upon an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    /// [`super::CovenantEngine::evaluate`].
    pub test_frequency: finstack_quant_core::dates::Tenor,
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
    /// would collide in compliance reports and breach tracking without a
    /// distinct label. Waivers and breaches key off it.
    pub label: String,
}

impl Covenant {
    /// Create a new covenant with default cure period.
    ///
    /// `label` is the covenant's identity in reports, breaches and waivers;
    /// two covenants of the same type must carry different labels.
    pub fn new(
        covenant_type: CovenantType,
        test_frequency: finstack_quant_core::dates::Tenor,
        label: impl Into<String>,
    ) -> Self {
        Self {
            covenant_type,
            test_frequency,
            cure_period_days: Some(30),
            consequences: Vec::new(),
            is_active: true,
            scope: CovenantScope::Maintenance,
            springing_condition: None,
            label: label.into(),
        }
    }

    /// Set cure period (days before breach becomes default)
    #[must_use]
    pub fn with_cure_period(mut self, days: Option<i32>) -> Self {
        self.cure_period_days = days;
        self
    }

    /// Add a consequence for covenant breach
    #[must_use]
    pub fn with_consequence(mut self, consequence: CovenantConsequence) -> Self {
        self.consequences.push(consequence);
        self
    }

    /// Set covenant scope (maintenance vs incurrence).
    #[must_use]
    pub fn with_scope(mut self, scope: CovenantScope) -> Self {
        self.scope = scope;
        self
    }

    /// Attach a springing condition that controls activation.
    #[must_use]
    pub fn with_springing_condition(mut self, condition: SpringingCondition) -> Self {
        self.springing_condition = Some(condition);
        self
    }

    /// Get human-readable description of the covenant
    pub fn description(&self) -> String {
        self.covenant_type.to_string()
    }

    /// Stable identity key for reports, breaches, and waivers.
    pub fn instance_key(&self) -> String {
        self.label.clone()
    }

    pub(crate) fn validate(&self) -> finstack_quant_core::Result<()> {
        if self.cure_period_days.is_some_and(|days| days < 0) {
            return Err(finstack_quant_core::Error::Validation(
                "cure_period_days must be non-negative".to_string(),
            ));
        }
        if self.label.trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "covenant label must not be empty".into(),
            ));
        }
        if let Some(condition) = &self.springing_condition {
            let (ThresholdTest::Minimum(value) | ThresholdTest::Maximum(value)) = condition.test;
            if condition.metric_id.as_str().trim().is_empty() || !value.is_finite() {
                return Err(finstack_quant_core::Error::Validation(
                    "springing metric must be named and its threshold finite".into(),
                ));
            }
        }
        for consequence in &self.consequences {
            consequence.validate()?;
        }
        self.covenant_type.validate()
    }
}

/// Type of financial or operational covenant
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum CovenantType {
    /// Maximum debt-to-EBITDA ratio
    MaxDebtToEbitda {
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
    MinDscr {
        /// Minimum required coverage
        threshold: f64,
    },
    /// Maximum net debt to EBITDA ratio (net of cash)
    MaxNetDebtToEbitda {
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
            CovenantType::MaxDebtToEbitda { threshold } => {
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
            CovenantType::MinDscr { threshold } => {
                write!(f, "DSCR >= {:.2}x", threshold)
            }
            CovenantType::MaxNetDebtToEbitda { threshold } => {
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
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ThresholdTest {
    /// Maximum allowed value
    Maximum(f64),
    /// Minimum required value
    Minimum(f64),
}

/// Direction of inequality for numeric covenants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundKind {
    /// Covenant passes when the metric is less than or equal to the threshold.
    AtMost,
    /// Covenant passes when the metric is greater than or equal to the threshold.
    AtLeast,
}

impl CovenantType {
    fn validate(&self) -> finstack_quant_core::Result<()> {
        if self
            .threshold_value()
            .is_some_and(|value| !value.is_finite())
        {
            return Err(finstack_quant_core::Error::Validation(
                "covenant thresholds and limits must be finite".to_string(),
            ));
        }
        Ok(())
    }

    /// Returns the inequality direction required for numeric covenants.
    pub fn bound_kind(&self) -> Option<BoundKind> {
        match self {
            CovenantType::MaxDebtToEbitda { .. }
            | CovenantType::MaxTotalLeverage { .. }
            | CovenantType::MaxSeniorLeverage { .. }
            | CovenantType::MaxNetDebtToEbitda { .. }
            | CovenantType::MaxCapex { .. }
            | CovenantType::Basket { .. }
            | CovenantType::Custom {
                test: ThresholdTest::Maximum(_),
                ..
            } => Some(BoundKind::AtMost),
            CovenantType::MinInterestCoverage { .. }
            | CovenantType::MinFixedChargeCoverage { .. }
            | CovenantType::MinAssetCoverage { .. }
            | CovenantType::MinDscr { .. }
            | CovenantType::MinLiquidity { .. }
            | CovenantType::Custom {
                test: ThresholdTest::Minimum(_),
                ..
            } => Some(BoundKind::AtLeast),
            CovenantType::Negative { .. } | CovenantType::Affirmative { .. } => None,
        }
    }

    /// Returns the scalar threshold (if any) associated with the covenant type.
    ///
    /// Ratio covenants return the ratio in turns, `Custom` returns the bound
    /// value of its test, `Basket` returns its limit, and the non-numeric
    /// `Negative` / `Affirmative` covenants return `None`.
    pub fn threshold_value(&self) -> Option<f64> {
        match self {
            CovenantType::MaxDebtToEbitda { threshold }
            | CovenantType::MinInterestCoverage { threshold }
            | CovenantType::MinFixedChargeCoverage { threshold }
            | CovenantType::MaxTotalLeverage { threshold }
            | CovenantType::MaxSeniorLeverage { threshold }
            | CovenantType::MinAssetCoverage { threshold }
            | CovenantType::MinDscr { threshold }
            | CovenantType::MaxNetDebtToEbitda { threshold }
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
            CovenantType::MaxDebtToEbitda { .. } => Some("debt_to_ebitda"),
            CovenantType::MinInterestCoverage { .. } => Some("interest_coverage"),
            CovenantType::MinFixedChargeCoverage { .. } => Some("fixed_charge_coverage"),
            CovenantType::MaxTotalLeverage { .. } => Some("total_leverage"),
            CovenantType::MaxSeniorLeverage { .. } => Some("senior_leverage"),
            CovenantType::MinAssetCoverage { .. } => Some("asset_coverage"),
            CovenantType::MinDscr { .. } => Some("dscr"),
            CovenantType::MaxNetDebtToEbitda { .. } => Some("net_debt_to_ebitda"),
            CovenantType::MaxCapex { .. } => Some("capex"),
            CovenantType::MinLiquidity { .. } => Some("liquidity"),
            CovenantType::Custom { .. }
            | CovenantType::Basket { .. }
            | CovenantType::Negative { .. }
            | CovenantType::Affirmative { .. } => None,
        }
    }

    /// Whether the covenant is a built-in maximum leverage ratio.
    /// Gross leverage cannot be negative. Net leverage can be negative when
    /// eligible cash exceeds debt and requires an explicit earnings denominator.
    pub(crate) fn is_ratio_max(&self) -> bool {
        matches!(
            self,
            CovenantType::MaxDebtToEbitda { .. }
                | CovenantType::MaxTotalLeverage { .. }
                | CovenantType::MaxSeniorLeverage { .. }
                | CovenantType::MaxNetDebtToEbitda { .. }
        )
    }

    /// Stable machine-readable identifier based on the variant discriminant only.
    ///
    /// Thresholds are **not** included because they can be amended by waivers or
    /// overridden by threshold schedules. If multiple covenants of the same type
    /// exist, callers should assign a disambiguating label externally.
    pub fn covenant_id(&self) -> &'static str {
        match self {
            CovenantType::MaxDebtToEbitda { .. } => "max_debt_ebitda",
            CovenantType::MinInterestCoverage { .. } => "min_interest_coverage",
            CovenantType::MinFixedChargeCoverage { .. } => "min_fcc",
            CovenantType::MaxTotalLeverage { .. } => "max_total_leverage",
            CovenantType::MaxSeniorLeverage { .. } => "max_senior_leverage",
            CovenantType::MinAssetCoverage { .. } => "min_asset_coverage",
            CovenantType::MinDscr { .. } => "min_dscr",
            CovenantType::MaxNetDebtToEbitda { .. } => "max_net_debt_ebitda",
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
#[serde(rename_all = "snake_case", deny_unknown_fields)]
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

impl CovenantConsequence {
    pub(crate) fn validate(&self) -> finstack_quant_core::Result<()> {
        let valid = match self {
            Self::RateIncrease { bp_increase } => bp_increase.is_finite() && *bp_increase >= 0.0,
            Self::CashSweep { sweep_percentage } => (0.0..=1.0).contains(sweep_percentage),
            Self::RequireCollateral { description } => !description.trim().is_empty(),
            Self::Default | Self::BlockDistributions | Self::AccelerateMaturity { .. } => true,
        };
        if !valid {
            return Err(finstack_quant_core::Error::Validation(
                "consequence requires a finite non-negative rate increase, a sweep fraction in [0, 1], or a non-empty collateral description".into(),
            ));
        }
        Ok(())
    }
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

/// Covenant evaluation specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CovenantSpec {
    /// The covenant to evaluate
    pub covenant: Covenant,
    /// Metric ID to use for evaluation (for financial covenants)
    pub metric_id: Option<CovenantMetricId>,
    /// Earnings denominator used to validate a leverage ratio. Required for net
    /// debt/EBITDA, where a negative ratio can mean net cash or negative earnings.
    /// Values must be finite and in the same reporting-period convention as the
    /// ratio. A non-positive denominator makes the test a breach with no headroom.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub denominator_metric_id: Option<CovenantMetricId>,
    /// Time-varying threshold schedule that overrides the static threshold in
    /// [`CovenantType`] when present. Enables leverage step-down schedules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold_schedule: Option<ThresholdSchedule>,
}

impl CovenantSpec {
    /// Create a covenant spec with an explicit metric identifier.
    /// Net-debt/EBITDA also selects `ebitda` as its denominator metric.
    ///
    /// # Arguments
    ///
    /// * `covenant` - Contractual threshold, scope, cure, and consequence terms.
    /// * `metric_id` - Identifier of the precomputed covenant ratio in turns or
    ///   monetary amount in the same currency and period as the threshold.
    pub fn with_metric(covenant: Covenant, metric_id: impl Into<CovenantMetricId>) -> Self {
        let denominator_metric_id = matches!(
            covenant.covenant_type,
            CovenantType::MaxNetDebtToEbitda { .. }
        )
        .then(|| CovenantMetricId::from("ebitda"));
        Self {
            covenant,
            metric_id: Some(metric_id.into()),
            denominator_metric_id,
            threshold_schedule: None,
        }
    }

    /// Attach a time-varying threshold schedule (e.g., leverage step-downs).
    #[must_use]
    pub fn with_threshold_schedule(mut self, schedule: ThresholdSchedule) -> Self {
        self.threshold_schedule = Some(schedule);
        self
    }

    /// Select the earnings denominator used to validate a built-in leverage ratio.
    ///
    /// # Arguments
    ///
    /// * `metric_id` - Identifier of the finite earnings amount for the same period
    ///   as the ratio. Net-debt constructors default to `ebitda`; use this to
    ///   select adjusted or covenant-specific earnings. Non-positive earnings
    ///   produce an indeterminate ratio breach rather than apparent headroom.
    #[must_use]
    pub fn with_denominator_metric(mut self, metric_id: impl Into<CovenantMetricId>) -> Self {
        self.denominator_metric_id = Some(metric_id.into());
        self
    }

    pub(crate) fn validate(&self) -> finstack_quant_core::Result<()> {
        self.covenant.validate()?;
        if self
            .metric_id
            .as_ref()
            .is_some_and(|id| id.as_str().trim().is_empty())
        {
            return Err(finstack_quant_core::Error::Validation(
                "metric_id must not be empty".into(),
            ));
        }
        if matches!(
            self.covenant.covenant_type,
            CovenantType::MaxNetDebtToEbitda { .. }
        ) && self.denominator_metric_id.is_none()
        {
            return Err(finstack_quant_core::Error::Validation(
                "net debt/EBITDA requires denominator_metric_id".into(),
            ));
        }
        if let Some(id) = &self.denominator_metric_id {
            if id.as_str().trim().is_empty() || !self.covenant.covenant_type.is_ratio_max() {
                return Err(finstack_quant_core::Error::Validation(
                    "denominator_metric_id must name an earnings metric for a built-in leverage covenant".into(),
                ));
            }
        }
        Ok(())
    }
}

/// Covenant window for scheduled testing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Consequences captured from the effective specification at the breach date.
    /// This immutable execution order survives later amendments and window changes.
    pub consequences: Vec<CovenantConsequence>,
    /// Successfully applied prefix of `consequences`; retries resume after it.
    pub applied_consequences: Vec<CovenantConsequence>,
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
