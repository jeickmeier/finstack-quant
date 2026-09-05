//! Corkscrew analysis extension.
//!
//! This extension provides roll-forward validation for balance sheet accounts, ensuring
//! that opening balances + changes = closing balances across periods.
//!
//! # Features
//!
//! - Validate balance sheet articulation (Assets = Liabilities + Equity)
//! - Track roll-forward schedules (beginning balance to changes to ending balance)
//! - Detect inconsistencies in period-to-period transitions
//! - Support for multiple balance sheet sections (assets, liabilities, equity)
//! - Configurable tolerance for rounding differences
//! - Optional fail-on-error mode for strict validation
//!
//! # Configuration Schema
//!
//! ```json
//! {
//!   "accounts": [
//!     {
//!       "node_id": "inventory_end",
//!       "account_type": "asset",
//!       "beginning_balance_node": "inventory_beg",
//!       "changes": ["additions"],
//!       "decreases": ["disposals"]
//!     },
//!     {
//!       "node_id": "debt",
//!       "account_type": "liability",
//!       "changes": ["debt_issuance"],
//!       "decreases": ["debt_repayment"]
//!     }
//!   ],
//!   "tolerance": 0.01
//! }
//! ```
//!
//! Identity: `expected = prev + Σ changes − Σ decreases` (or
//! `beginning + Σ changes − Σ decreases` when `beginning_balance_node` is
//! set). Decrease nodes stay **positive** outflows. Pair a roll-forward
//! template `{name}` as `node_id = "{name}_end"`,
//! `beginning_balance_node = "{name}_beg"`, `changes = increases`,
//! `decreases = disposals`.
//!
//! # Example Usage
//!
//! ```no_run
//! use finstack_quant_statements_analytics::extensions::{
//!     CorkscrewExtension, CorkscrewConfig, CorkscrewAccount, AccountType,
//! };
//! use finstack_quant_statements::evaluator::{Evaluator, StatementResult};
//! use finstack_quant_statements::types::FinancialModelSpec;
//!
//! # fn main() -> finstack_quant_statements::Result<()> {
//! # let model: FinancialModelSpec = unimplemented!("build a model");
//! let mut evaluator = Evaluator::new();
//! let results = evaluator.evaluate(&model)?;
//!
//! let config = CorkscrewConfig {
//!     accounts: vec![CorkscrewAccount {
//!         node_id: "cash".into(),
//!         account_type: AccountType::Asset,
//!         changes: vec!["cash_inflows".into()],
//!         decreases: vec!["cash_outflows".into()],
//!         beginning_balance_node: None,
//!     }],
//!     tolerance: 0.01,
//!     fail_on_error: false,
//! };
//!
//! let mut extension = CorkscrewExtension::new(config);
//! let report = extension.execute(&model, &results)?;
//! assert_eq!(report.status, finstack_quant_statements_analytics::extensions::CorkscrewStatus::Success);
//! # Ok(())
//! # }
//! ```

use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::types::FinancialModelSpec;
use finstack_quant_statements::Result;
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
        &mut self,
        model: &FinancialModelSpec,
        results: &StatementResult,
    ) -> Result<CorkscrewReport> {
        let _span = tracing::info_span!("statements_analytics.corkscrew.execute").entered();

        let config = self.config.clone();

        let mut validations = Vec::new();
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Process each configured account
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

        // Check for balance sheet articulation using actual balances
        match self.check_articulation(model, results, &config, config.tolerance) {
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

        // Build report
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
            serde_json::json!(validations
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
                .collect::<Vec<_>>()),
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
    fn validate_account(
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

        // Get balance values from results
        let balance_values = results.nodes.get(&account.node_id).ok_or_else(|| {
            finstack_quant_statements::error::Error::missing_data(format!(
                "Balance account '{}' not found in results",
                account.node_id
            ))
        })?;

        // Get change values and validate roll-forward
        let periods: Vec<_> = model.periods.iter().collect();

        for i in 1..periods.len() {
            let prev_period = &periods[i - 1].id;
            let curr_period = &periods[i].id;

            // Get previous and current balance. A missing value means the
            // balance node was not evaluated for that period — a genuine
            // modeling error, not a zero balance. Treating it as zero would
            // let an incomplete model pass roll-forward validation.
            let prev_balance = balance_values.get(prev_period).copied().ok_or_else(|| {
                finstack_quant_statements::error::Error::missing_data(format!(
                    "Balance account '{}' has no value for period '{prev_period}'",
                    account.node_id
                ))
            })?;
            let curr_balance = balance_values.get(curr_period).copied().ok_or_else(|| {
                finstack_quant_statements::error::Error::missing_data(format!(
                    "Balance account '{}' has no value for period '{curr_period}'",
                    account.node_id
                ))
            })?;

            // Calculate expected balance from changes and decreases.
            // Identity: expected = prev + Σ changes − Σ decreases.
            let mut expected_balance = prev_balance;

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

            // Check if beginning balance override is used. A missing period
            // value is a hard error, consistent with the other missing-value
            // checks above — silently falling back to `prev_balance` could
            // mask a real roll-forward break.
            if let Some(beginning_node) = &account.beginning_balance_node {
                let beginning_values = results.nodes.get(beginning_node).ok_or_else(|| {
                    finstack_quant_statements::error::Error::missing_data(format!(
                        "Beginning-balance node '{beginning_node}' for account '{}' not found in results",
                        account.node_id
                    ))
                })?;
                let beginning = beginning_values.get(curr_period).ok_or_else(|| {
                    finstack_quant_statements::error::Error::missing_data(format!(
                        "Beginning-balance node '{beginning_node}' has no value for period '{curr_period}'"
                    ))
                })?;
                expected_balance = beginning + expected_balance - prev_balance;
            }

            // Validate the roll-forward using an absolute tolerance.
            let error = (curr_balance - expected_balance).abs();
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
            let mut equity = 0.0;

            for account in &config.accounts {
                let node_values = results.nodes.get(&account.node_id).ok_or_else(|| {
                    finstack_quant_statements::error::Error::missing_data(format!(
                        "Articulation account '{}' not found in results",
                        account.node_id
                    ))
                })?;
                let balance = node_values.get(period_id).ok_or_else(|| {
                    finstack_quant_statements::error::Error::missing_data(format!(
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
            max_imbalance = max_imbalance.max(imbalance);
        }

        Ok(Some(ArticulationResult {
            total_imbalance: max_imbalance,
            is_balanced: max_imbalance <= tolerance,
        }))
    }
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
        let values = results.nodes.get(node_id).ok_or_else(|| {
            finstack_quant_statements::error::Error::missing_data(format!(
                "{role} node '{node_id}' for account '{account_id}' not found in results"
            ))
        })?;
        let value = values.get(curr_period).ok_or_else(|| {
            finstack_quant_statements::error::Error::missing_data(format!(
                "{role} node '{node_id}' has no value for period '{curr_period}'"
            ))
        })?;
        *expected_balance += sign * value;
    }
    Ok(())
}

/// Result of validating a single account.
struct AccountValidation {
    account_id: String,
    account_type: AccountType,
    periods_validated: usize,
    max_error: f64,
    is_valid: bool,
}

/// Result of checking balance sheet articulation.
struct ArticulationResult {
    total_imbalance: f64,
    is_balanced: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::PeriodId;
    use finstack_quant_statements::builder::ModelBuilder;
    use finstack_quant_statements::evaluator::Evaluator;
    use finstack_quant_statements::types::AmountOrScalar;

    #[test]
    fn test_corkscrew_extension_with_config() {
        let config = CorkscrewConfig {
            accounts: vec![CorkscrewAccount {
                node_id: "cash".into(),
                account_type: AccountType::Asset,
                changes: vec!["cash_inflows".into(), "cash_outflows".into()],
                decreases: vec![],
                beginning_balance_node: None,
            }],
            tolerance: 0.01,
            fail_on_error: false,
        };

        let extension = CorkscrewExtension::new(config);
        assert_eq!(extension.config().accounts.len(), 1);
    }

    #[test]
    fn test_corkscrew_execute_with_empty_accounts() {
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q1", None)
            .expect("valid periods")
            .value(
                "cash",
                &[(
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                )],
            )
            .build()
            .expect("model should build");
        let mut evaluator = Evaluator::new();
        let results = evaluator
            .evaluate(&model)
            .expect("evaluation should succeed");

        let config = CorkscrewConfig {
            accounts: vec![],
            tolerance: 0.01,
            fail_on_error: false,
        };

        let mut extension = CorkscrewExtension::new(config);
        let report = extension
            .execute(&model, &results)
            .expect("empty accounts should succeed");
        assert_eq!(report.status, CorkscrewStatus::Success);
    }

    #[test]
    fn test_account_type_serialization() {
        let account_type = AccountType::Asset;
        let json = serde_json::to_string(&account_type).expect("test should succeed");
        assert_eq!(json, r#""asset""#);

        let deserialized: AccountType = serde_json::from_str(&json).expect("test should succeed");
        assert_eq!(deserialized, AccountType::Asset);
    }

    fn broken_rollforward_model() -> (FinancialModelSpec, StatementResult) {
        // cash: 100 → 250 with only +100 of inflows → 50 break.
        let model = ModelBuilder::new("rollforward_break")
            .periods("2025Q1..Q2", None)
            .expect("valid periods")
            .value(
                "cash",
                &[
                    (
                        PeriodId::quarter(2025, 1).expect("valid period fixture"),
                        AmountOrScalar::scalar(100.0),
                    ),
                    (
                        PeriodId::quarter(2025, 2).expect("valid period fixture"),
                        AmountOrScalar::scalar(250.0),
                    ),
                ],
            )
            .value(
                "inflows",
                &[
                    (
                        PeriodId::quarter(2025, 1).expect("valid period fixture"),
                        AmountOrScalar::scalar(0.0),
                    ),
                    (
                        PeriodId::quarter(2025, 2).expect("valid period fixture"),
                        AmountOrScalar::scalar(100.0),
                    ),
                ],
            )
            .build()
            .expect("model should build");
        let mut evaluator = Evaluator::new();
        let results = evaluator
            .evaluate(&model)
            .expect("evaluation should succeed");
        (model, results)
    }

    #[test]
    fn rollforward_break_fails_report_in_strict_mode() {
        let (model, results) = broken_rollforward_model();

        let config = CorkscrewConfig {
            accounts: vec![CorkscrewAccount {
                node_id: "cash".into(),
                account_type: AccountType::Asset,
                changes: vec!["inflows".into()],
                decreases: vec![],
                beginning_balance_node: None,
            }],
            tolerance: 0.01,
            fail_on_error: true,
        };

        let mut extension = CorkscrewExtension::new(config);
        let report = extension
            .execute(&model, &results)
            .expect("extension should execute");

        assert_eq!(
            report.status,
            CorkscrewStatus::Failed,
            "a roll-forward identity break must fail the report in strict mode"
        );
        assert!(report
            .errors
            .iter()
            .any(|e| e.contains("roll-forward identity failed")));
    }

    #[test]
    fn rollforward_break_warns_in_lenient_mode() {
        let (model, results) = broken_rollforward_model();

        let config = CorkscrewConfig {
            accounts: vec![CorkscrewAccount {
                node_id: "cash".into(),
                account_type: AccountType::Asset,
                changes: vec!["inflows".into()],
                decreases: vec![],
                beginning_balance_node: None,
            }],
            tolerance: 0.01,
            fail_on_error: false,
        };

        let mut extension = CorkscrewExtension::new(config);
        let report = extension
            .execute(&model, &results)
            .expect("extension should execute");

        assert_eq!(report.status, CorkscrewStatus::Success);
        assert_eq!(report.data["validations"][0]["type"], "asset");
        assert!(
            report
                .warnings
                .iter()
                .any(|w| w.contains("roll-forward identity failed")),
            "break must surface as a warning, got {:?}",
            report.warnings
        );
    }

    #[test]
    fn missing_beginning_balance_period_value_is_hard_error() {
        let (model, mut results) = broken_rollforward_model();
        // Add a beginning-balance node with NO value in Q2.
        results.nodes.insert(
            "cash_beg".to_string(),
            indexmap::IndexMap::from([(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                100.0,
            )]),
        );

        let config = CorkscrewConfig {
            accounts: vec![CorkscrewAccount {
                node_id: "cash".into(),
                account_type: AccountType::Asset,
                changes: vec!["inflows".into()],
                decreases: vec![],
                beginning_balance_node: Some("cash_beg".into()),
            }],
            tolerance: 0.01,
            fail_on_error: false,
        };

        let mut extension = CorkscrewExtension::new(config);
        let report = extension
            .execute(&model, &results)
            .expect("extension should execute in lenient mode");

        assert_eq!(report.status, CorkscrewStatus::Failed);
        assert!(
            report
                .errors
                .iter()
                .any(|e| e.contains("cash_beg") && e.contains("no value")),
            "missing beginning-balance period value must be an error, got {:?}",
            report.errors
        );
    }

    #[test]
    fn single_period_model_flags_nothing_to_validate() {
        let model = ModelBuilder::new("single_period")
            .periods("2025Q1..Q1", None)
            .expect("valid periods")
            .value(
                "cash",
                &[(
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                )],
            )
            .build()
            .expect("model should build");
        let mut evaluator = Evaluator::new();
        let results = evaluator
            .evaluate(&model)
            .expect("evaluation should succeed");

        let config = CorkscrewConfig {
            accounts: vec![CorkscrewAccount {
                node_id: "cash".into(),
                account_type: AccountType::Asset,
                changes: vec![],
                decreases: vec![],
                beginning_balance_node: None,
            }],
            tolerance: 0.01,
            fail_on_error: false,
        };

        let mut extension = CorkscrewExtension::new(config);
        let report = extension
            .execute(&model, &results)
            .expect("extension should execute");

        assert!(
            report
                .warnings
                .iter()
                .any(|w| w.contains("no roll-forward transitions")),
            "single-period run must be flagged as vacuous, got {:?}",
            report.warnings
        );
    }

    #[test]
    fn articulation_checked_at_every_period() {
        // Balanced in the last period but imbalanced in the first — the
        // check must catch the earlier imbalance.
        let model = ModelBuilder::new("articulation_all_periods")
            .periods("2025Q1..Q2", None)
            .expect("valid periods")
            .value(
                "assets",
                &[
                    (
                        PeriodId::quarter(2025, 1).expect("valid period fixture"),
                        AmountOrScalar::scalar(110.0),
                    ),
                    (
                        PeriodId::quarter(2025, 2).expect("valid period fixture"),
                        AmountOrScalar::scalar(100.0),
                    ),
                ],
            )
            .value(
                "liabilities",
                &[
                    (
                        PeriodId::quarter(2025, 1).expect("valid period fixture"),
                        AmountOrScalar::scalar(100.0),
                    ),
                    (
                        PeriodId::quarter(2025, 2).expect("valid period fixture"),
                        AmountOrScalar::scalar(100.0),
                    ),
                ],
            )
            .build()
            .expect("model should build");
        let mut evaluator = Evaluator::new();
        let results = evaluator
            .evaluate(&model)
            .expect("evaluation should succeed");

        let config = CorkscrewConfig {
            accounts: vec![
                CorkscrewAccount {
                    node_id: "assets".into(),
                    account_type: AccountType::Asset,
                    changes: vec![],
                    decreases: vec![],
                    beginning_balance_node: None,
                },
                CorkscrewAccount {
                    node_id: "liabilities".into(),
                    account_type: AccountType::Liability,
                    changes: vec![],
                    decreases: vec![],
                    beginning_balance_node: None,
                },
            ],
            tolerance: 0.01,
            fail_on_error: true,
        };

        let mut extension = CorkscrewExtension::new(config);
        let report = extension
            .execute(&model, &results)
            .expect("extension should execute");

        assert_eq!(report.status, CorkscrewStatus::Failed);
        assert!(report
            .errors
            .iter()
            .any(|e| e.contains("Balance sheet not articulated")));
    }

    #[test]
    fn test_corkscrew_uses_absolute_tolerance_for_articulation() {
        let model = ModelBuilder::new("articulation_tolerance")
            .periods("2025Q1..Q1", None)
            .expect("valid periods")
            .value(
                "assets",
                &[(
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(1_000_000.00),
                )],
            )
            .value(
                "liabilities",
                &[(
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(999_999.98),
                )],
            )
            .value(
                "equity",
                &[(
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(0.0),
                )],
            )
            .build()
            .expect("model should build");

        let mut evaluator = Evaluator::new();
        let results = evaluator
            .evaluate(&model)
            .expect("evaluation should succeed");

        let config = CorkscrewConfig {
            accounts: vec![
                CorkscrewAccount {
                    node_id: "assets".into(),
                    account_type: AccountType::Asset,
                    changes: vec![],
                    decreases: vec![],
                    beginning_balance_node: None,
                },
                CorkscrewAccount {
                    node_id: "liabilities".into(),
                    account_type: AccountType::Liability,
                    changes: vec![],
                    decreases: vec![],
                    beginning_balance_node: None,
                },
                CorkscrewAccount {
                    node_id: "equity".into(),
                    account_type: AccountType::Equity,
                    changes: vec![],
                    decreases: vec![],
                    beginning_balance_node: None,
                },
            ],
            tolerance: 0.01,
            fail_on_error: true,
        };

        let mut extension = CorkscrewExtension::new(config);
        let report = extension
            .execute(&model, &results)
            .expect("extension should execute");

        assert_eq!(report.status, CorkscrewStatus::Failed);
        assert!(
            report
                .errors
                .iter()
                .any(|msg| msg.contains("Balance sheet not articulated")),
            "expected articulation failure, got {:?}",
            report.errors
        );
    }

    #[test]
    fn decreases_are_subtracted_from_expected_balance() {
        let model = ModelBuilder::new("decreases_identity")
            .periods("2025Q1..Q2", None)
            .expect("valid periods")
            .value(
                "inventory",
                &[
                    (
                        PeriodId::quarter(2025, 1).expect("valid period fixture"),
                        AmountOrScalar::scalar(100.0),
                    ),
                    (
                        PeriodId::quarter(2025, 2).expect("valid period fixture"),
                        AmountOrScalar::scalar(130.0),
                    ),
                ],
            )
            .value(
                "additions",
                &[
                    (
                        PeriodId::quarter(2025, 1).expect("valid period fixture"),
                        AmountOrScalar::scalar(0.0),
                    ),
                    (
                        PeriodId::quarter(2025, 2).expect("valid period fixture"),
                        AmountOrScalar::scalar(50.0),
                    ),
                ],
            )
            .value(
                "disposals",
                &[
                    (
                        PeriodId::quarter(2025, 1).expect("valid period fixture"),
                        AmountOrScalar::scalar(0.0),
                    ),
                    (
                        PeriodId::quarter(2025, 2).expect("valid period fixture"),
                        AmountOrScalar::scalar(20.0),
                    ),
                ],
            )
            .build()
            .expect("model should build");
        let mut evaluator = Evaluator::new();
        let results = evaluator
            .evaluate(&model)
            .expect("evaluation should succeed");

        let config = CorkscrewConfig {
            accounts: vec![CorkscrewAccount {
                node_id: "inventory".into(),
                account_type: AccountType::Asset,
                changes: vec!["additions".into()],
                decreases: vec!["disposals".into()],
                beginning_balance_node: None,
            }],
            tolerance: 0.01,
            fail_on_error: false,
        };

        let mut extension = CorkscrewExtension::new(config);
        let report = extension
            .execute(&model, &results)
            .expect("extension should execute");
        assert_eq!(
            report.data["validations"][0]["is_valid"].as_bool(),
            Some(true)
        );
        assert!(
            report
                .warnings
                .iter()
                .all(|w| !w.contains("roll-forward identity failed")),
            "decreases must keep the identity intact, got {:?}",
            report.warnings
        );
    }

    #[test]
    fn omitted_decreases_field_deserializes_as_empty() {
        let json = r#"{
            "node_id": "cash",
            "account_type": "asset",
            "changes": ["inflows"]
        }"#;
        let account: CorkscrewAccount =
            serde_json::from_str(json).expect("legacy corkscrew JSON should parse");
        assert!(account.decreases.is_empty());
        assert_eq!(account.changes, vec!["inflows".to_string()]);
    }
}
