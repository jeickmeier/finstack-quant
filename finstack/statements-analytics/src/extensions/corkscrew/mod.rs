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
//!       "node_id": "cash",
//!       "account_type": "asset",
//!       "changes": ["cash_inflows", "cash_outflows"]
//!     },
//!     {
//!       "node_id": "debt",
//!       "account_type": "liability",
//!       "changes": ["debt_issuance", "debt_repayment"]
//!     }
//!   ],
//!   "tolerance": 0.01
//! }
//! ```
//!
//! # Example Usage
//!
//! ```ignore
//! use finstack_statements_analytics::extensions::{
//!     CorkscrewExtension, CorkscrewConfig, CorkscrewAccount, AccountType,
//! };
//! use finstack_statements::evaluator::{Evaluator, StatementResult};
//! use finstack_statements::types::FinancialModelSpec;
//!
//! # fn main() -> finstack_statements::Result<()> {
//! # let model: FinancialModelSpec = unimplemented!("build a model");
//! let mut evaluator = Evaluator::new();
//! let results = evaluator.evaluate(&model)?;
//!
//! let config = CorkscrewConfig {
//!     accounts: vec![CorkscrewAccount {
//!         node_id: "cash".into(),
//!         account_type: AccountType::Asset,
//!         changes: vec!["cash_inflows".into(), "cash_outflows".into()],
//!         beginning_balance_node: None,
//!     }],
//!     tolerance: 0.01,
//!     fail_on_error: false,
//! };
//!
//! let mut extension = CorkscrewExtension::with_config(config);
//! let report = extension.execute(&model, &results)?;
//! assert_eq!(report.status, finstack_statements_analytics::extensions::CorkscrewStatus::Success);
//! # Ok(())
//! # }
//! ```

use finstack_statements::evaluator::StatementResult;
use finstack_statements::types::FinancialModelSpec;
use finstack_statements::Result;
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
    config: Option<CorkscrewConfig>,
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

    /// Node IDs representing changes to the balance
    #[serde(default)]
    pub changes: Vec<String>,

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

/// Default tolerance for corkscrew validation (basis points).
///
/// Set to 0.01 (1 cent or 1 basis point) to accommodate normal rounding differences
/// in financial calculations while catching meaningful discrepancies.
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
/// Mirrors the historical extension result shape so existing callers can be
/// migrated mechanically: status, message, structured data, warnings, errors.
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
    /// Create a new corkscrew extension with default configuration.
    ///
    /// # Example
    /// ```rust
    /// # use finstack_statements_analytics::extensions::CorkscrewExtension;
    /// let extension = CorkscrewExtension::new();
    /// assert!(extension.config().is_none());
    /// ```
    pub fn new() -> Self {
        Self { config: None }
    }

    /// Create a new corkscrew extension with the given configuration.
    ///
    /// # Arguments
    /// * `config` - Pre-built [`CorkscrewConfig`] describing the accounts to validate
    pub fn with_config(config: CorkscrewConfig) -> Self {
        Self {
            config: Some(config),
        }
    }

    /// Get the current configuration.
    pub fn config(&self) -> Option<&CorkscrewConfig> {
        self.config.as_ref()
    }

    /// Set the configuration.
    ///
    /// # Arguments
    /// * `config` - New configuration to assign
    pub fn set_config(&mut self, config: CorkscrewConfig) {
        self.config = Some(config);
    }

    /// Run corkscrew validation against the provided model and evaluation results.
    ///
    /// Requires that [`CorkscrewExtension::with_config`] or
    /// [`CorkscrewExtension::set_config`] has supplied a configuration; otherwise
    /// returns an error.
    ///
    /// # Arguments
    /// * `model` - The evaluated financial model
    /// * `results` - Evaluation output to inspect
    pub fn execute(
        &mut self,
        model: &FinancialModelSpec,
        results: &StatementResult,
    ) -> Result<CorkscrewReport> {
        let _span = tracing::info_span!("statements_analytics.corkscrew.execute").entered();

        let config = self.config.clone().ok_or_else(|| {
            finstack_statements::error::Error::registry(
                "Corkscrew extension requires configuration",
            )
        })?;

        let mut validations = Vec::new();
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Process each configured account
        for account in &config.accounts {
            match self.validate_account(account, model, results, config.tolerance) {
                Ok(validation) => validations.push(validation),
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
            account_type: format!("{:?}", account.account_type),
            periods_validated: 0,
            max_error: 0.0,
            is_valid: true,
        };

        // Get balance values from results
        let balance_values = results.nodes.get(&account.node_id).ok_or_else(|| {
            finstack_statements::error::Error::registry(format!(
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
                finstack_statements::error::Error::registry(format!(
                    "Balance account '{}' has no value for period '{prev_period}'",
                    account.node_id
                ))
            })?;
            let curr_balance = balance_values.get(curr_period).copied().ok_or_else(|| {
                finstack_statements::error::Error::registry(format!(
                    "Balance account '{}' has no value for period '{curr_period}'",
                    account.node_id
                ))
            })?;

            // Calculate expected balance from changes
            let mut expected_balance = prev_balance;

            // Add changes for this period. A missing change node or value is a
            // configuration error: silently skipping it understates the
            // expected balance and can mask a real roll-forward break.
            for change_node_id in &account.changes {
                let change_values = results.nodes.get(change_node_id).ok_or_else(|| {
                    finstack_statements::error::Error::registry(format!(
                        "Change node '{change_node_id}' for account '{}' not found in results",
                        account.node_id
                    ))
                })?;
                let change = change_values.get(curr_period).ok_or_else(|| {
                    finstack_statements::error::Error::registry(format!(
                        "Change node '{change_node_id}' has no value for period '{curr_period}'"
                    ))
                })?;
                expected_balance += change;
            }

            // Check if beginning balance override is used
            if let Some(beginning_node) = &account.beginning_balance_node {
                let beginning_values = results.nodes.get(beginning_node).ok_or_else(|| {
                    finstack_statements::error::Error::registry(format!(
                        "Beginning-balance node '{beginning_node}' for account '{}' not found in results",
                        account.node_id
                    ))
                })?;
                if let Some(beginning) = beginning_values.get(curr_period) {
                    expected_balance = beginning + expected_balance - prev_balance;
                }
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
    /// Sums the most recent period's balance for each configured account,
    /// grouped by account type, and checks that Assets = Liabilities + Equity.
    /// Uses an absolute tolerance matching the configured rounding threshold.
    fn check_articulation(
        &self,
        model: &FinancialModelSpec,
        results: &StatementResult,
        config: &CorkscrewConfig,
        tolerance: f64,
    ) -> Result<Option<ArticulationResult>> {
        let Some(last_period) = model.periods.last() else {
            return Ok(None);
        };
        let period_id = &last_period.id;

        let mut assets = 0.0;
        let mut liabilities = 0.0;
        let mut equity = 0.0;

        for account in &config.accounts {
            let node_values = results.nodes.get(&account.node_id).ok_or_else(|| {
                finstack_statements::error::Error::registry(format!(
                    "Articulation account '{}' not found in results",
                    account.node_id
                ))
            })?;
            let balance = node_values.get(period_id).ok_or_else(|| {
                finstack_statements::error::Error::registry(format!(
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

        if config.accounts.is_empty() {
            return Ok(None);
        }

        let imbalance = assets - (liabilities + equity);
        let is_balanced = imbalance.abs() <= tolerance;

        Ok(Some(ArticulationResult {
            total_imbalance: imbalance.abs(),
            is_balanced,
        }))
    }
}

/// Result of validating a single account.
struct AccountValidation {
    account_id: String,
    account_type: String,
    periods_validated: usize,
    max_error: f64,
    is_valid: bool,
}

/// Result of checking balance sheet articulation.
struct ArticulationResult {
    total_imbalance: f64,
    is_balanced: bool,
}

impl Default for CorkscrewExtension {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::dates::PeriodId;
    use finstack_statements::builder::ModelBuilder;
    use finstack_statements::evaluator::Evaluator;
    use finstack_statements::types::AmountOrScalar;

    #[test]
    fn test_corkscrew_extension_creation() {
        let extension = CorkscrewExtension::new();
        assert!(extension.config().is_none());
    }

    #[test]
    fn test_corkscrew_extension_with_config() {
        let config = CorkscrewConfig {
            accounts: vec![CorkscrewAccount {
                node_id: "cash".into(),
                account_type: AccountType::Asset,
                changes: vec!["cash_inflows".into(), "cash_outflows".into()],
                beginning_balance_node: None,
            }],
            tolerance: 0.01,
            fail_on_error: false,
        };

        let extension = CorkscrewExtension::with_config(config);
        assert!(extension.config().is_some());
        assert_eq!(
            extension
                .config()
                .expect("test should succeed")
                .accounts
                .len(),
            1
        );
    }

    #[test]
    fn test_corkscrew_execute_requires_config() {
        let model = FinancialModelSpec::new("test", Vec::new());
        let results = StatementResult::new();

        let mut extension = CorkscrewExtension::new();
        // Without config, should return an error
        let result = extension.execute(&model, &results);

        assert!(result.is_err());
        assert!(result
            .expect_err("should fail")
            .to_string()
            .contains("requires configuration"));
    }

    #[test]
    fn test_corkscrew_execute_with_empty_accounts() {
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q1", None)
            .expect("valid periods")
            .value(
                "cash",
                &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0))],
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

        let mut extension = CorkscrewExtension::with_config(config);
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

    #[test]
    fn test_corkscrew_uses_absolute_tolerance_for_articulation() {
        let model = ModelBuilder::new("articulation_tolerance")
            .periods("2025Q1..Q1", None)
            .expect("valid periods")
            .value(
                "assets",
                &[(
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(1_000_000.00),
                )],
            )
            .value(
                "liabilities",
                &[(
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(999_999.98),
                )],
            )
            .value(
                "equity",
                &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(0.0))],
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
                    beginning_balance_node: None,
                },
                CorkscrewAccount {
                    node_id: "liabilities".into(),
                    account_type: AccountType::Liability,
                    changes: vec![],
                    beginning_balance_node: None,
                },
                CorkscrewAccount {
                    node_id: "equity".into(),
                    account_type: AccountType::Equity,
                    changes: vec![],
                    beginning_balance_node: None,
                },
            ],
            tolerance: 0.01,
            fail_on_error: true,
        };

        let mut extension = CorkscrewExtension::with_config(config);
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
}
