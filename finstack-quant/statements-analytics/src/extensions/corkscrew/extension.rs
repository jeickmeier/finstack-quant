//! Corkscrew balance-sheet roll-forward validation extension.

use finstack_quant_statements::Result;
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::types::FinancialModelSpec;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Corkscrew analysis extension for balance sheet roll-forward validation.
///
/// **Features:**
/// - Validates period-to-period balance roll-forwards
/// - Checks balance sheet articulation (Assets = Liabilities + Equity)
/// - Configurable tolerance for rounding differences
/// - Detailed validation reports with errors and warnings
pub struct CorkscrewExtension {
    /// Extension configuration
    config: CorkscrewConfig,
}

/// Configuration for corkscrew analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CorkscrewConfig {
    /// List of balance sheet accounts to validate
    #[serde(default)]
    pub accounts: Vec<CorkscrewAccount>,

    /// Tolerance for rounding differences (default: 0.01)
    #[serde(default = "default_tolerance")]
    pub tolerance: f64,

    /// Whether to fail on inconsistencies (default: false)
    #[serde(default)]
    pub fail_on_error: bool,
}

/// Configuration for a single corkscrew account.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CorkscrewAccount {
    /// Node ID for the balance account
    pub node_id: String,

    /// Account type (asset, liability, equity)
    pub account_type: AccountType,

    /// Node IDs representing increases (or signed net changes) to the balance.
    ///
    /// Each change node is **added** to the prior (or beginning) balance.
    /// Prefer [`Self::decreases`] for positive outflows so roll-forward
    /// decrease nodes do not need to be negated.
    #[serde(default)]
    pub changes: Vec<String>,

    /// Node IDs representing positive decreases (repayments, outflows,
    /// disposals) subtracted from the balance.
    ///
    /// Identity: `expected = prev + Σ changes − Σ decreases`, or
    /// `beginning + Σ changes − Σ decreases` when
    /// [`Self::beginning_balance_node`] is set. Empty by default so existing
    /// configs that store reductions as negative `changes` still parse.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decreases: Vec<String>,

    /// Optional: Node ID for beginning balance override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beginning_balance_node: Option<String>,
}

/// Type of balance sheet account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    /// Asset account
    Asset,
    /// Liability account
    Liability,
    /// Equity account
    Equity,
}

/// Default absolute tolerance for corkscrew validation.
///
/// Set to 0.01 **currency units** (e.g. 1 cent for USD-denominated models)
/// to accommodate normal rounding differences in financial calculations
/// while catching meaningful discrepancies. This is an absolute amount, not
/// a relative measure such as a basis point.
const DEFAULT_CORKSCREW_TOLERANCE: f64 = 0.01;

fn default_tolerance() -> f64 {
    DEFAULT_CORKSCREW_TOLERANCE
}

/// Status of a corkscrew validation run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CorkscrewStatus {
    /// Validation completed without fatal errors
    Success,
    /// Validation surfaced fatal errors
    Failed,
}

/// Report produced by [`CorkscrewExtension::execute`].
///
/// Uses the extension result shape shared by analytical reports: status,
/// message, structured data, warnings, and errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CorkscrewReport {
    /// Overall execution status
    pub status: CorkscrewStatus,

    /// Human-readable summary
    pub message: String,

    /// Structured output (e.g. per-account validations as JSON)
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub data: IndexMap<String, serde_json::Value>,

    /// Warnings (non-fatal)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,

    /// Errors (fatal in strict mode, otherwise reported)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

impl CorkscrewExtension {
    /// Create a new corkscrew extension with the given configuration.
    ///
    /// # Arguments
    /// * `config` - Pre-built [`CorkscrewConfig`] driving the analysis
    pub fn new(config: CorkscrewConfig) -> Self {
        Self { config }
    }

    /// Get the current configuration.
    pub fn config(&self) -> &CorkscrewConfig {
        &self.config
    }

    /// Run corkscrew validation against the provided model and evaluation results.
    ///
    ///
    /// # Arguments
    /// * `model` - The evaluated financial model
    /// * `results` - Evaluation output to inspect
    ///
    /// With `fail_on_error = false`, account and articulation failures are
    /// retained in the report. With it enabled, the first such data failure is
    /// returned instead.
    ///
    /// # Errors
    ///
    /// Returns an error when no configuration is supplied, or when strict mode
    /// encounters a missing/invalid account, period value, change node, or
    /// balance-sheet articulation input.
    pub fn execute(
        &self,
        model: &FinancialModelSpec,
        results: &StatementResult,
    ) -> Result<CorkscrewReport> {
        let _span = tracing::info_span!("statements_analytics.corkscrew.execute").entered();

        let config = &self.config;
        if !config.tolerance.is_finite() || config.tolerance < 0.0 {
            return Err(finstack_quant_statements::Error::invalid_input(
                "Corkscrew tolerance must be finite and non-negative",
            ));
        }

        let mut validations = Vec::new();
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        for account in &config.accounts {
            match self.validate_account(account, model, results, config.tolerance) {
                Ok(validation) => {
                    // A failed roll-forward identity must surface in the
                    // report (and fail it in strict mode), not just sit in
                    // `data.validations[].is_valid`.
                    if !validation.is_valid {
                        let msg = format!(
                            "Account '{}': roll-forward identity failed (max error {:.6} > tolerance {:.6})",
                            account.node_id, validation.max_error, config.tolerance
                        );
                        if config.fail_on_error {
                            errors.push(msg);
                        } else {
                            warnings.push(msg);
                        }
                    }
                    if validation.periods_validated == 0 {
                        warnings.push(format!(
                            "Account '{}': single-period model — no roll-forward transitions to validate",
                            account.node_id
                        ));
                    }
                    validations.push(validation);
                }
                Err(e) => {
                    if config.fail_on_error {
                        return Err(e);
                    } else {
                        errors.push(format!("Account '{}': {}", account.node_id, e));
                    }
                }
            }
        }

        match self.check_articulation(model, results, config, config.tolerance) {
            Ok(Some(articulation_result)) => {
                if !articulation_result.is_balanced {
                    let msg = format!(
                        "Balance sheet not articulated. Total imbalance: {:.2}",
                        articulation_result.total_imbalance
                    );
                    if config.fail_on_error {
                        errors.push(msg);
                    } else {
                        warnings.push(msg);
                    }
                }
            }
            Ok(None) => {}
            Err(e) => {
                if config.fail_on_error {
                    return Err(e);
                }
                errors.push(format!("Articulation: {e}"));
            }
        }

        let (status, message) = if errors.is_empty() {
            (
                CorkscrewStatus::Success,
                format!(
                    "Corkscrew validation complete. {} accounts validated.",
                    validations.len()
                ),
            )
        } else {
            (
                CorkscrewStatus::Failed,
                format!("Corkscrew validation failed with {} errors", errors.len()),
            )
        };

        let mut data = IndexMap::new();
        data.insert(
            "validations".into(),
            serde_json::json!(
                validations
                    .iter()
                    .map(|v| {
                        serde_json::json!({
                            "account": v.account_id,
                            "type": v.account_type,
                            "periods_validated": v.periods_validated,
                            "max_error": v.max_error,
                            "is_valid": v.is_valid,
                        })
                    })
                    .collect::<Vec<_>>()
            ),
        );

        Ok(CorkscrewReport {
            status,
            message,
            data,
            warnings,
            errors,
        })
    }

    /// Validate a single account's roll-forward schedule.
    pub(super) fn validate_account(
        &self,
        account: &CorkscrewAccount,
        model: &FinancialModelSpec,
        results: &StatementResult,
        tolerance: f64,
    ) -> Result<AccountValidation> {
        let mut validation = AccountValidation {
            account_id: account.node_id.clone(),
            account_type: account.account_type,
            periods_validated: 0,
            max_error: 0.0,
            is_valid: true,
        };

        let balance_values = results.numeric_series(&account.node_id).ok_or_else(|| {
            finstack_quant_statements::Error::missing_data(format!(
                "Balance account '{}' not found in results",
                account.node_id
            ))
        })?;

        let periods: Vec<_> = model.periods.iter().collect();

        for i in 1..periods.len() {
            let prev_period = &periods[i - 1].id;
            let curr_period = &periods[i].id;

            // A missing value means the balance node was not evaluated for
            // that period — a genuine modeling error, not a zero balance.
            // Treating it as zero would let an incomplete model pass
            // roll-forward validation.
            let prev_balance = balance_values.get(prev_period).copied().ok_or_else(|| {
                finstack_quant_statements::Error::missing_data(format!(
                    "Balance account '{}' has no value for period '{prev_period}'",
                    account.node_id
                ))
            })?;
            let curr_balance = balance_values.get(curr_period).copied().ok_or_else(|| {
                finstack_quant_statements::Error::missing_data(format!(
                    "Balance account '{}' has no value for period '{curr_period}'",
                    account.node_id
                ))
            })?;

            // A missing beginning-balance period value is a hard error,
            // consistent with the other missing-value checks above —
            // silently falling back to `prev_balance` could mask a real
            // roll-forward break.
            let mut expected_balance = if let Some(beginning_node) = &account.beginning_balance_node
            {
                let beginning_values = results.numeric_series(beginning_node).ok_or_else(|| {
                    finstack_quant_statements::Error::missing_data(format!(
                        "Beginning-balance node '{beginning_node}' for account '{}' not found in results",
                        account.node_id
                    ))
                })?;
                let beginning = beginning_values.get(curr_period).ok_or_else(|| {
                    finstack_quant_statements::Error::missing_data(format!(
                        "Beginning-balance node '{beginning_node}' has no value for period '{curr_period}'"
                    ))
                })?;
                *beginning
            } else {
                prev_balance
            };

            // A missing change/decrease node or value is a configuration
            // error: silently skipping it understates the expected balance
            // and can mask a real roll-forward break.
            apply_flow_nodes(
                &mut expected_balance,
                &account.changes,
                1.0,
                "Change",
                &account.node_id,
                curr_period,
                results,
            )?;
            apply_flow_nodes(
                &mut expected_balance,
                &account.decreases,
                -1.0,
                "Decrease",
                &account.node_id,
                curr_period,
                results,
            )?;
            let error = (curr_balance - expected_balance).abs();
            require_finite(error)?;
            validation.max_error = validation.max_error.max(error);
            validation.periods_validated += 1;

            if error > tolerance {
                validation.is_valid = false;
            }
        }

        Ok(validation)
    }

    /// Check balance sheet articulation (A = L + E) using actual balances.
    ///
    /// Checks every model period: for each period, sums the configured
    /// accounts grouped by account type and verifies
    /// Assets = Liabilities + Equity. Reports the worst (maximum absolute)
    /// imbalance across periods. Uses an absolute tolerance matching the
    /// configured rounding threshold.
    fn check_articulation(
        &self,
        model: &FinancialModelSpec,
        results: &StatementResult,
        config: &CorkscrewConfig,
        tolerance: f64,
    ) -> Result<Option<ArticulationResult>> {
        if config.accounts.is_empty() || model.periods.is_empty() {
            return Ok(None);
        }

        let mut max_imbalance = 0.0f64;

        for period in &model.periods {
            let period_id = &period.id;
            let mut assets = 0.0;
            let mut liabilities = 0.0;
            let mut equity = 0.0_f64;

            for account in &config.accounts {
                let node_values = results.numeric_series(&account.node_id).ok_or_else(|| {
                    finstack_quant_statements::Error::missing_data(format!(
                        "Articulation account '{}' not found in results",
                        account.node_id
                    ))
                })?;
                let balance = node_values.get(period_id).ok_or_else(|| {
                    finstack_quant_statements::Error::missing_data(format!(
                        "Articulation account '{}' has no value for period '{period_id}'",
                        account.node_id
                    ))
                })?;
                match account.account_type {
                    AccountType::Asset => assets += balance,
                    AccountType::Liability => liabilities += balance,
                    AccountType::Equity => equity += balance,
                }
            }

            let imbalance = (assets - (liabilities + equity)).abs();
            require_finite(imbalance)?;
            max_imbalance = max_imbalance.max(imbalance);
        }

        Ok(Some(ArticulationResult {
            total_imbalance: max_imbalance,
            is_balanced: max_imbalance <= tolerance,
        }))
    }
}

fn require_finite(value: f64) -> Result<()> {
    if !value.is_finite() {
        return Err(finstack_quant_statements::Error::invalid_input(
            "Corkscrew balances, changes, and reconciliation errors must be finite",
        ));
    }
    Ok(())
}

/// Add signed flow-node values for the current period into `expected_balance`.
fn apply_flow_nodes(
    expected_balance: &mut f64,
    node_ids: &[String],
    sign: f64,
    role: &str,
    account_id: &str,
    curr_period: &finstack_quant_core::dates::PeriodId,
    results: &StatementResult,
) -> Result<()> {
    for node_id in node_ids {
        let values = results.numeric_series(node_id).ok_or_else(|| {
            finstack_quant_statements::Error::missing_data(format!(
                "{role} node '{node_id}' for account '{account_id}' not found in results"
            ))
        })?;
        let value = values.get(curr_period).ok_or_else(|| {
            finstack_quant_statements::Error::missing_data(format!(
                "{role} node '{node_id}' has no value for period '{curr_period}'"
            ))
        })?;
        *expected_balance += sign * value;
    }
    Ok(())
}

/// Result of validating a single account.
pub(super) struct AccountValidation {
    pub(super) account_id: String,
    pub(super) account_type: AccountType,
    pub(super) periods_validated: usize,
    pub(super) max_error: f64,
    pub(super) is_valid: bool,
}

/// Result of checking balance sheet articulation.
struct ArticulationResult {
    total_imbalance: f64,
    is_balanced: bool,
}
