//! Named suites of checks with filtering and merge support.

use serde::{Deserialize, Serialize};

use super::builtins::{
    BalanceSheetArticulation, CashReconciliation, MissingValueCheck, NonFiniteCheck,
    RetainedEarningsReconciliation, SignConventionCheck,
};
use super::traits::{Check, CheckContext};
use super::types::{
    CheckCategory, CheckConfig, CheckReport, CheckResult, CheckSummary, PeriodScope, Severity,
    SignConventionPolicy,
};
use crate::evaluator::StatementResult;
use crate::types::{FinancialModelSpec, NodeId};
use crate::Result;

/// A named, self-contained collection of checks with its own configuration.
pub struct CheckSuite {
    /// Suite name for display/logging.
    name: String,
    /// Optional description.
    description: Option<String>,
    /// Checks in this suite.
    checks: Vec<Box<dyn Check>>,
    /// Configuration applied when running the suite.
    config: CheckConfig,
}

impl CheckSuite {
    /// Start building a new suite.
    #[must_use]
    pub fn builder(name: impl Into<String>) -> CheckSuiteBuilder {
        CheckSuiteBuilder {
            name: name.into(),
            description: None,
            checks: Vec::new(),
            config: CheckConfig::default(),
        }
    }

    /// Merge another suite's checks into this one, consuming the other suite.
    pub fn merge(mut self, other: CheckSuite) -> Self {
        self.checks.extend(other.checks);
        self
    }

    /// Execute all checks in the suite, applying `min_severity` and
    /// `materiality_threshold` filters from the config.
    pub fn run(
        &self,
        model: &FinancialModelSpec,
        results: &StatementResult,
    ) -> Result<CheckReport> {
        let context = CheckContext {
            model,
            results,
            config: self.config.clone(),
        };
        let min_severity = context.config.min_severity;
        let mat_threshold = context.config.materiality_threshold;

        let mut filtered_results: Vec<CheckResult> = Vec::with_capacity(self.checks.len());

        for check in &self.checks {
            let mut result = check.execute(&context)?;

            result.findings.retain(|f| {
                if f.severity < min_severity {
                    return false;
                }
                if mat_threshold > 0.0 {
                    if let Some(ref m) = f.materiality {
                        if m.absolute.abs() < mat_threshold {
                            return false;
                        }
                    }
                }
                true
            });

            result.passed = !result
                .findings
                .iter()
                .any(|f| f.severity == Severity::Error);

            filtered_results.push(result);
        }

        let total_checks = filtered_results.len();
        let passed = filtered_results.iter().filter(|r| r.passed).count();
        let failed = total_checks - passed;

        let mut errors: usize = 0;
        let mut warnings: usize = 0;
        let mut infos: usize = 0;
        for finding in filtered_results.iter().flat_map(|r| &r.findings) {
            match finding.severity {
                Severity::Error => errors += 1,
                Severity::Warning => warnings += 1,
                Severity::Info => infos += 1,
            }
        }

        Ok(CheckReport {
            results: filtered_results,
            summary: CheckSummary {
                total_checks,
                passed,
                failed,
                errors,
                warnings,
                infos,
            },
        })
    }

    /// Number of checks in the suite.
    pub fn len(&self) -> usize {
        self.checks.len()
    }

    /// True when the suite contains no checks.
    pub fn is_empty(&self) -> bool {
        self.checks.is_empty()
    }

    /// Suite name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Suite description, if set.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

/// Fluent builder for [`CheckSuite`].
pub struct CheckSuiteBuilder {
    name: String,
    description: Option<String>,
    checks: Vec<Box<dyn Check>>,
    config: CheckConfig,
}

impl CheckSuiteBuilder {
    /// Set the suite description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add a check to the suite.
    pub fn add_check(mut self, check: impl Check + 'static) -> Self {
        self.checks.push(Box::new(check));
        self
    }

    /// Override the default configuration.
    pub fn config(mut self, config: CheckConfig) -> Self {
        self.config = config;
        self
    }

    /// Consume the builder and produce a [`CheckSuite`].
    pub fn build(self) -> CheckSuite {
        CheckSuite {
            name: self.name,
            description: self.description,
            checks: self.checks,
            config: self.config,
        }
    }
}

// ---------------------------------------------------------------------------
// Serializable suite spec
// ---------------------------------------------------------------------------

/// Serializable descriptor for a [`CheckSuite`] that can be saved/loaded as
/// JSON for team-wide check policies.
///
/// Only built-in checks are resolved by [`CheckSuiteSpec::resolve`];
/// [`FormulaCheckSpec`] entries are stored for later resolution by the
/// analytics crate, which owns the [`FormulaCheck`](super::super) type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckSuiteSpec {
    /// Suite name.
    pub name: String,
    /// Suite description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Built-in checks to include.
    #[serde(default)]
    pub builtin_checks: Vec<BuiltinCheckSpec>,
    /// User-defined formula checks (resolved by the analytics crate).
    #[serde(default)]
    pub formula_checks: Vec<FormulaCheckSpec>,
    /// Suite configuration.
    #[serde(default)]
    pub config: CheckConfig,
}

impl CheckSuiteSpec {
    /// Resolve the spec into a runnable [`CheckSuite`].
    ///
    /// Only built-in checks are materialized; [`FormulaCheckSpec`] entries
    /// require the analytics crate's `FormulaCheck` and must be resolved
    /// separately.
    pub fn resolve(&self) -> Result<CheckSuite> {
        let checks: Vec<Box<dyn Check>> = self
            .builtin_checks
            .iter()
            .map(BuiltinCheckSpec::to_check)
            .collect();
        Ok(CheckSuite {
            name: self.name.clone(),
            description: self.description.clone(),
            checks,
            config: self.config.clone(),
        })
    }
}

/// Tagged enum describing any built-in check in a serializable form.
///
/// Each variant mirrors its corresponding check struct and can be converted
/// into a boxed [`Check`] via [`BuiltinCheckSpec::to_check`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BuiltinCheckSpec {
    /// Balance sheet articulation: Assets = Liabilities + Equity.
    BalanceSheetArticulation {
        /// Nodes representing total assets.
        assets_nodes: Vec<NodeId>,
        /// Nodes representing total liabilities.
        liabilities_nodes: Vec<NodeId>,
        /// Nodes representing total equity.
        equity_nodes: Vec<NodeId>,
        /// Tolerance override.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tolerance: Option<f64>,
    },
    /// Retained earnings reconciliation across periods.
    RetainedEarningsReconciliation {
        /// Node for retained earnings balance.
        retained_earnings_node: NodeId,
        /// Node for net income.
        net_income_node: NodeId,
        /// Optional node for dividends paid.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dividends_node: Option<NodeId>,
        /// Additional adjustment nodes (buybacks, AOCI, etc.).
        #[serde(default)]
        other_adjustments: Vec<NodeId>,
        /// Tolerance override.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tolerance: Option<f64>,
    },
    /// Cash balance reconciliation: Cash(t) = Cash(t-1) + TotalCF(t).
    CashReconciliation {
        /// Node for cash balance.
        cash_balance_node: NodeId,
        /// Node for total cash flow.
        total_cash_flow_node: NodeId,
        /// Optional node for cash from operations.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cfo_node: Option<NodeId>,
        /// Optional node for cash from investing.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cfi_node: Option<NodeId>,
        /// Optional node for cash from financing.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cff_node: Option<NodeId>,
        /// Tolerance override.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tolerance: Option<f64>,
    },
    /// Flags required nodes that lack values in applicable periods.
    MissingValue {
        /// Nodes that must have values in every in-scope period.
        required_nodes: Vec<NodeId>,
        /// Which periods to inspect.
        scope: PeriodScope,
    },
    /// Flags values with unexpected signs.
    SignConvention {
        /// Nodes expected to carry positive values.
        #[serde(default)]
        positive_nodes: Vec<NodeId>,
        /// Nodes expected to carry negative values.
        #[serde(default)]
        negative_nodes: Vec<NodeId>,
    },
    /// Detects NaN or infinite values.
    NonFinite {
        /// Specific nodes to check; if empty, all nodes are inspected.
        #[serde(default)]
        nodes: Vec<NodeId>,
    },
}

impl BuiltinCheckSpec {
    /// Convert this spec into a boxed [`Check`] implementation.
    pub fn to_check(&self) -> Box<dyn Check> {
        match self {
            Self::BalanceSheetArticulation {
                assets_nodes,
                liabilities_nodes,
                equity_nodes,
                tolerance,
            } => Box::new(BalanceSheetArticulation {
                assets_nodes: assets_nodes.clone(),
                liabilities_nodes: liabilities_nodes.clone(),
                equity_nodes: equity_nodes.clone(),
                tolerance: *tolerance,
            }),
            Self::RetainedEarningsReconciliation {
                retained_earnings_node,
                net_income_node,
                dividends_node,
                other_adjustments,
                tolerance,
            } => Box::new(RetainedEarningsReconciliation {
                retained_earnings_node: retained_earnings_node.clone(),
                net_income_node: net_income_node.clone(),
                dividends_node: dividends_node.clone(),
                other_adjustments: other_adjustments.clone(),
                tolerance: *tolerance,
                dividends_sign_convention: SignConventionPolicy::default(),
            }),
            Self::CashReconciliation {
                cash_balance_node,
                total_cash_flow_node,
                cfo_node,
                cfi_node,
                cff_node,
                tolerance,
            } => Box::new(CashReconciliation {
                cash_balance_node: cash_balance_node.clone(),
                total_cash_flow_node: total_cash_flow_node.clone(),
                cfo_node: cfo_node.clone(),
                cfi_node: cfi_node.clone(),
                cff_node: cff_node.clone(),
                tolerance: *tolerance,
            }),
            Self::MissingValue {
                required_nodes,
                scope,
            } => Box::new(MissingValueCheck {
                required_nodes: required_nodes.clone(),
                scope: *scope,
            }),
            Self::SignConvention {
                positive_nodes,
                negative_nodes,
            } => Box::new(SignConventionCheck {
                positive_nodes: positive_nodes.clone(),
                negative_nodes: negative_nodes.clone(),
            }),
            Self::NonFinite { nodes } => Box::new(NonFiniteCheck {
                nodes: nodes.clone(),
            }),
        }
    }
}

/// Serializable descriptor for a user-defined formula check.
///
/// This mirrors the analytics crate's `FormulaCheck` fields so that full
/// suite definitions (built-in + formula) can be stored as a single JSON
/// document. Resolution into a runnable check requires the analytics crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormulaCheckSpec {
    /// Unique identifier for this check instance.
    pub id: String,
    /// Human-readable name shown in reports.
    pub name: String,
    /// Category grouping.
    pub category: CheckCategory,
    /// Severity assigned to findings when the formula fails.
    pub severity: Severity,
    /// Expression to evaluate (e.g. `"revenue > 0"`).
    pub formula: String,
    /// Template for the finding message (`{period}` is replaced at runtime).
    pub message_template: String,
    /// Numeric tolerance for floating-point comparisons.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<f64>,
}
