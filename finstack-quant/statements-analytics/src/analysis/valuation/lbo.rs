//! Leveraged-buyout transaction arithmetic over a statement model.
//!
//! Layers the standard LBO transaction structure on top of an evaluated
//! [`FinancialModelSpec`]: entry enterprise value from a multiple of an
//! operating metric, a sources-and-uses schedule that solves for the sponsor
//! equity check, and exit proceeds net of the debt balance still outstanding at
//! the exit period.
//!
//! # Conventions
//!
//! - Monetary outputs are [`Money`] in the model currency inferred from
//!   `FinancialModelSpec::meta["currency"]`.
//! - Multiples are plain scalars, so `8.5` means `8.5x`.
//! - Entry is priced at the model's **first** period (the transaction close);
//!   exit at [`LboConfig::exit_period`].
//!
//! # What this module does not do
//!
//! - **Debt amortisation.** Tranche paydown belongs in the statement model
//!   itself, where it is already expressible through the roll-forward
//!   machinery — see [`crate::templates::add_roll_forward_with_opening`]. This
//!   module reads the resulting closing balance through
//!   [`LboConfig::exit_net_debt_node`]; [`LboConfig::sources`] carries only the
//!   day-one funded amounts.
//! - **IRR.** Money-weighted return on the sponsor's dated cash flows is owned
//!   by `finstack_quant_portfolio::mwr_xirr`, which takes the equity outflow at
//!   close and the [`LboResult::exit_equity_proceeds`] inflow at exit. MOIC (a
//!   date-free proceeds-to-invested ratio) is returned here; IRR is not
//!   duplicated.
//! - **Model validation.** Structural and credit checks run through the
//!   existing [`crate::analysis::lbo_model_checks`] suite, wired in via
//!   [`LboConfig::check_mappings`], rather than a parallel validation path.
//!
//! # References
//!
//! - Rosenbaum, J., & Pearl, J. (2020). *Investment Banking: Valuation, LBOs,
//!   M&A, and IPOs* (3rd ed.). Wiley. Ch. 5 (LBO analysis): entry valuation,
//!   sources and uses, and exit / return analysis.
//! - Koller, T., Goedhart, M., & Wessels, D. (2020). *Valuation: Measuring and
//!   Managing the Value of Companies* (7th ed.). Wiley. Ch. 36.

use crate::analysis::checks::{lbo_model_checks, CreditMapping, ThreeStatementMapping};
use crate::analysis::valuation::corporate::extract_currency_from_model;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::money::Money;
use finstack_quant_statements::checks::CheckReport;
use finstack_quant_statements::error::{Error, Result};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::FinancialModelSpec;

/// Relative tolerance for the sources-and-uses reconciliation.
///
/// The equity check is solved as a residual, so the two sides agree
/// analytically; the tolerance absorbs the floating-point rounding of that
/// residual at deal-size magnitudes. It is applied relative to the larger of
/// the two totals so the guard is scale-free.
const SOURCES_USES_REL_TOLERANCE: f64 = 1e-9;

/// One funded debt tranche in the day-one capital structure.
///
/// Amounts are in the model currency and are the amounts drawn at close, not
/// commitments. Tranche paydown over the hold period is modelled inside the
/// statement model (see the module-level note on roll-forwards).
#[derive(Debug, Clone, PartialEq)]
pub struct LboTranche {
    /// Tranche label, e.g. `"term_loan_a"` or `"mezzanine"`.
    pub name: String,
    /// Amount funded at close, in the model currency. Must be finite and
    /// non-negative.
    pub amount: f64,
}

/// Node-id mappings forwarded to the [`lbo_model_checks`] suite.
///
/// Supplying this runs the crate's existing LBO check suite against the same
/// evaluated statement results used for the transaction arithmetic, so the
/// report and the returns are guaranteed to describe one evaluation.
#[derive(Debug, Clone)]
pub struct LboCheckMappings {
    /// Three-statement node mapping (balance sheet, income statement, cash flow).
    pub three_statement: ThreeStatementMapping,
    /// Credit node mapping used for the leverage and coverage checks.
    pub credit: CreditMapping,
}

/// Transaction assumptions for an LBO evaluation.
///
/// Rates and multiples follow the crate-wide conventions: multiples are plain
/// scalars (`8.5` means `8.5x`) and monetary inputs are plain `f64` amounts in
/// the model currency.
#[derive(Debug, Clone)]
pub struct LboConfig {
    /// Entry valuation multiple applied to `entry_metric_node`, e.g. `8.5`
    /// for 8.5x EBITDA.
    pub entry_multiple: f64,
    /// Node id supplying the entry valuation metric (typically `"ebitda"`),
    /// read at the model's first period.
    pub entry_metric_node: String,
    /// Transaction fees and expenses funded at close, in the model currency.
    pub transaction_fees: f64,
    /// Funded debt tranches at close. The sponsor equity check is solved as
    /// the residual that balances sources against uses.
    pub sources: Vec<LboTranche>,
    /// Exit valuation multiple applied to `exit_metric_node`.
    pub exit_multiple: f64,
    /// Node id supplying the exit valuation metric, read at `exit_period`.
    pub exit_metric_node: String,
    /// Node id supplying net debt outstanding at exit, read at `exit_period`.
    /// This is where a modelled amortisation schedule lands.
    pub exit_net_debt_node: String,
    /// Period at which the sponsor exits. Periods are half-open `[start, end)`.
    pub exit_period: PeriodId,
    /// Optional mappings enabling the [`lbo_model_checks`] suite.
    pub check_mappings: Option<LboCheckMappings>,
}

/// Outputs of an LBO evaluation.
///
/// All monetary fields are in the model currency; `moic` is a plain scalar
/// (`2.4` means 2.4x the invested equity returned).
#[derive(Debug, Clone)]
pub struct LboResult {
    /// Entry enterprise value: `entry_multiple × entry metric`.
    pub entry_enterprise_value: Money,
    /// Entry metric value read from the model at the first period.
    pub entry_metric: f64,
    /// Total funded debt at close (sum of [`LboConfig::sources`]).
    pub debt_total: Money,
    /// Sponsor equity required to close: `uses_total − debt_total`.
    pub equity_check: Money,
    /// Total sources: funded debt plus the sponsor equity check.
    pub sources_total: Money,
    /// Total uses: entry enterprise value plus transaction fees.
    pub uses_total: Money,
    /// `true` when sources reconcile to uses within a relative tolerance of
    /// `1e-9` scaled by deal size. False signals a
    /// numerically degenerate capital structure (for example, tranche amounts
    /// so large relative to the purchase price that the residual equity check
    /// loses all significance).
    pub sources_uses_balanced: bool,
    /// Exit enterprise value: `exit_multiple × exit metric`.
    pub exit_enterprise_value: Money,
    /// Exit metric value read from the model at `exit_period`.
    pub exit_metric: f64,
    /// Net debt outstanding at `exit_period`.
    pub exit_net_debt: Money,
    /// Equity proceeds at exit: `exit_enterprise_value − exit_net_debt`.
    pub exit_equity_proceeds: Money,
    /// Multiple of invested capital: `exit_equity_proceeds / equity_check`.
    pub moic: f64,
    /// Report from the [`lbo_model_checks`] suite when
    /// [`LboConfig::check_mappings`] was supplied.
    pub checks: Option<CheckReport>,
}

/// Evaluate an LBO transaction against a statement model.
///
/// The model is evaluated once. Entry enterprise value is priced off the first
/// model period, the sponsor equity check is solved as the sources-and-uses
/// residual, and exit proceeds are the exit enterprise value less the net debt
/// the model still carries at [`LboConfig::exit_period`]:
///
/// ```text
/// EV_entry  = entry_multiple × metric_entry
/// uses      = EV_entry + fees
/// debt      = Σ tranche amounts
/// equity    = uses − debt                       (sponsor check)
/// EV_exit   = exit_multiple × metric_exit
/// proceeds  = EV_exit − net_debt_exit
/// MOIC      = proceeds / equity
/// ```
///
/// IRR is deliberately out of scope: pair `exit_equity_proceeds` with the
/// equity outflow at close and call `finstack_quant_portfolio::mwr_xirr`.
///
/// # Arguments
///
/// * `model` - Statement model carrying the operating forecast, the debt
///   schedule, and a `"currency"` key in its metadata
/// * `config` - Entry multiple and metric node, transaction fees, funded debt
///   tranches, exit multiple / metric / net-debt nodes, exit period, and
///   optional check mappings
///
/// # Returns
///
/// Returns [`LboResult`] with the entry and exit bridges, the sources-and-uses
/// reconciliation, MOIC, and the optional check report.
///
/// # Errors
///
/// Returns an error if the model cannot be evaluated, if the model has no
/// periods or no `"currency"` metadata, if any configured node is missing at
/// the period it is read from, if any numeric input or model value is not
/// finite, if a tranche amount is negative, if the resulting sponsor equity
/// check is not strictly positive (MOIC would be undefined), or if the check
/// suite fails to run.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_core::dates::PeriodId;
/// use finstack_quant_statements_analytics::analysis::{evaluate_lbo, LboConfig, LboTranche};
///
/// # fn run(model: &finstack_quant_statements::types::FinancialModelSpec)
/// #     -> Result<(), Box<dyn std::error::Error>> {
/// let config = LboConfig {
///     entry_multiple: 8.5,
///     entry_metric_node: "ebitda".to_string(),
///     transaction_fees: 3.0,
///     sources: vec![LboTranche { name: "term_loan_a".to_string(), amount: 90.0 }],
///     exit_multiple: 9.5,
///     exit_metric_node: "ebitda".to_string(),
///     exit_net_debt_node: "total_debt".to_string(),
///     exit_period: PeriodId::annual(2029),
///     check_mappings: None,
/// };
///
/// let result = evaluate_lbo(model, &config)?;
/// assert!(result.sources_uses_balanced);
/// # Ok(())
/// # }
/// ```
pub fn evaluate_lbo(model: &FinancialModelSpec, config: &LboConfig) -> Result<LboResult> {
    let currency = extract_currency_from_model(model)?;

    validate_finite("entry_multiple", config.entry_multiple)?;
    validate_finite("exit_multiple", config.exit_multiple)?;
    validate_finite("transaction_fees", config.transaction_fees)?;

    let entry_period = model
        .periods
        .first()
        .ok_or_else(|| {
            Error::Eval("LBO evaluation requires a model with at least one period".into())
        })?
        .id;

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(model)?;

    let node_value = |node: &str, period: &PeriodId, role: &str| -> Result<f64> {
        let value = results.get(node, period).ok_or_else(|| {
            Error::Eval(format!(
                "LBO {role} requires node '{node}' at period {period}; the node is absent from \
                 the evaluated model results"
            ))
        })?;
        validate_finite(&format!("node '{node}' at period {period}"), value)?;
        Ok(value)
    };

    // ---- Entry ----
    let entry_metric = node_value(&config.entry_metric_node, &entry_period, "entry valuation")?;
    let entry_enterprise_value = config.entry_multiple * entry_metric;
    let uses_total = entry_enterprise_value + config.transaction_fees;

    // ---- Sources & uses ----
    let mut debt_total = 0.0;
    for tranche in &config.sources {
        validate_finite(
            &format!("tranche '{}' amount", tranche.name),
            tranche.amount,
        )?;
        if tranche.amount < 0.0 {
            return Err(Error::Eval(format!(
                "LBO tranche '{}' must have a non-negative funded amount, got {:.4}",
                tranche.name, tranche.amount
            )));
        }
        debt_total += tranche.amount;
    }

    // The sponsor equity check is the residual that closes the funding gap.
    let equity_check = uses_total - debt_total;
    let sources_total = debt_total + equity_check;
    let scale = sources_total.abs().max(uses_total.abs()).max(1.0);
    let sources_uses_balanced =
        (sources_total - uses_total).abs() <= SOURCES_USES_REL_TOLERANCE * scale;

    // Fail-closed: an overflowed (non-finite) bridge is rejected alongside a
    // non-positive one rather than yielding a meaningless MOIC.
    if !matches!(
        equity_check.partial_cmp(&0.0),
        Some(std::cmp::Ordering::Greater)
    ) {
        return Err(Error::Eval(format!(
            "LBO sources of {debt_total:.4} fully fund uses of {uses_total:.4}, leaving a \
             sponsor equity check of {equity_check:.4}; MOIC requires a strictly positive \
             equity check"
        )));
    }

    // ---- Exit ----
    let exit_metric = node_value(
        &config.exit_metric_node,
        &config.exit_period,
        "exit valuation",
    )?;
    let exit_net_debt = node_value(
        &config.exit_net_debt_node,
        &config.exit_period,
        "exit bridge",
    )?;
    let exit_enterprise_value = config.exit_multiple * exit_metric;
    let exit_equity_proceeds = exit_enterprise_value - exit_net_debt;
    let moic = exit_equity_proceeds / equity_check;

    // ---- Model checks (existing suite, same evaluation) ----
    let checks = match &config.check_mappings {
        Some(mappings) => {
            let suite = lbo_model_checks(mappings.three_statement.clone(), mappings.credit.clone());
            Some(suite.run(model, &results)?)
        }
        None => None,
    };

    Ok(LboResult {
        entry_enterprise_value: Money::new(entry_enterprise_value, currency),
        entry_metric,
        debt_total: Money::new(debt_total, currency),
        equity_check: Money::new(equity_check, currency),
        sources_total: Money::new(sources_total, currency),
        uses_total: Money::new(uses_total, currency),
        sources_uses_balanced,
        exit_enterprise_value: Money::new(exit_enterprise_value, currency),
        exit_metric,
        exit_net_debt: Money::new(exit_net_debt, currency),
        exit_equity_proceeds: Money::new(exit_equity_proceeds, currency),
        moic,
        checks,
    })
}

/// Reject non-finite inputs before they propagate into the transaction bridge.
fn validate_finite(name: &str, value: f64) -> Result<()> {
    if value.is_finite() {
        return Ok(());
    }
    Err(Error::Eval(format!(
        "LBO evaluation requires a finite {name}, got {value}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_statements::builder::ModelBuilder;
    use finstack_quant_statements::types::AmountOrScalar;

    /// Mirrors the notebook deal: 8.5x entry on 22.0 EBITDA, 3.0 of fees,
    /// 115.0 of funded debt, 9.5x exit on 26.4 EBITDA with 35.0 of net debt.
    fn demo_model() -> FinancialModelSpec {
        ModelBuilder::new("lbo-demo")
            .periods("2025..2026", Some("2025"))
            .expect("valid periods")
            .value(
                "ebitda",
                &[
                    (PeriodId::annual(2025), AmountOrScalar::scalar(22.0)),
                    (PeriodId::annual(2026), AmountOrScalar::scalar(26.4)),
                ],
            )
            .value(
                "total_debt",
                &[
                    (PeriodId::annual(2025), AmountOrScalar::scalar(115.0)),
                    (PeriodId::annual(2026), AmountOrScalar::scalar(35.0)),
                ],
            )
            .with_meta("currency", serde_json::json!("USD"))
            .build()
            .expect("valid model")
    }

    fn demo_config() -> LboConfig {
        LboConfig {
            entry_multiple: 8.5,
            entry_metric_node: "ebitda".to_string(),
            transaction_fees: 3.0,
            sources: vec![
                LboTranche {
                    name: "term_loan_a".to_string(),
                    amount: 90.0,
                },
                LboTranche {
                    name: "revolver".to_string(),
                    amount: 10.0,
                },
                LboTranche {
                    name: "mezzanine".to_string(),
                    amount: 15.0,
                },
            ],
            exit_multiple: 9.5,
            exit_metric_node: "ebitda".to_string(),
            exit_net_debt_node: "total_debt".to_string(),
            exit_period: PeriodId::annual(2026),
            check_mappings: None,
        }
    }

    #[test]
    fn evaluate_lbo_reproduces_the_transaction_bridge() {
        let result = evaluate_lbo(&demo_model(), &demo_config()).expect("lbo evaluates");

        // Entry: 8.5 x 22.0 = 187.0; uses = 187.0 + 3.0 = 190.0.
        assert!((result.entry_enterprise_value.amount() - 187.0).abs() < 1e-9);
        assert!((result.uses_total.amount() - 190.0).abs() < 1e-9);
        // Debt of 115.0 leaves a 75.0 sponsor check, matching the notebook.
        assert!((result.debt_total.amount() - 115.0).abs() < 1e-9);
        assert!((result.equity_check.amount() - 75.0).abs() < 1e-9);
        assert!(result.sources_uses_balanced);
        assert!((result.sources_total.amount() - result.uses_total.amount()).abs() < 1e-9);

        // Exit: 9.5 x 26.4 = 250.8; proceeds = 250.8 - 35.0 = 215.8.
        assert!((result.exit_enterprise_value.amount() - 250.8).abs() < 1e-9);
        assert!((result.exit_equity_proceeds.amount() - 215.8).abs() < 1e-9);
        assert!((result.moic - 215.8 / 75.0).abs() < 1e-9);

        assert_eq!(result.entry_enterprise_value.currency().to_string(), "USD");
        assert!(result.checks.is_none());
    }

    #[test]
    fn evaluate_lbo_rejects_a_fully_debt_funded_deal() {
        let mut config = demo_config();
        config.sources = vec![LboTranche {
            name: "unitranche".to_string(),
            amount: 250.0,
        }];

        let err = evaluate_lbo(&demo_model(), &config).expect_err("no equity check");
        assert!(
            err.to_string().contains("equity check"),
            "unexpected message: {err}"
        );
    }

    #[test]
    fn evaluate_lbo_reports_a_missing_node() {
        let mut config = demo_config();
        config.exit_net_debt_node = "not_a_node".to_string();

        let err = evaluate_lbo(&demo_model(), &config).expect_err("missing node");
        assert!(
            err.to_string().contains("not_a_node"),
            "unexpected message: {err}"
        );
    }

    #[test]
    fn evaluate_lbo_requires_currency_metadata() {
        let model = ModelBuilder::new("lbo-no-currency")
            .periods("2025..2026", Some("2025"))
            .expect("valid periods")
            .value(
                "ebitda",
                &[(PeriodId::annual(2025), AmountOrScalar::scalar(22.0))],
            )
            .build()
            .expect("valid model");

        assert!(evaluate_lbo(&model, &demo_config()).is_err());
    }

    #[test]
    fn evaluate_lbo_rejects_a_negative_tranche() {
        let mut config = demo_config();
        config.sources.push(LboTranche {
            name: "bad".to_string(),
            amount: -1.0,
        });

        let err = evaluate_lbo(&demo_model(), &config).expect_err("negative tranche");
        assert!(
            err.to_string().contains("non-negative"),
            "unexpected message: {err}"
        );
    }
}
