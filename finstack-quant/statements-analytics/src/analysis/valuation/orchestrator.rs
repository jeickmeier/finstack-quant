//! Corporate analysis orchestrator.
//!
//! Provides [`CorporateAnalysisBuilder`] --- a fluent API that coordinates
//! statement evaluation, credit instrument pricing, and equity valuation
//! in a single pipeline.

use crate::analysis::credit::{compute_credit_context, CreditContextMetrics, CreditNumeratorNodes};
use crate::analysis::valuation::corporate::{CorporateValuationResult, DcfOptions};
use finstack_quant_core::dates::{Date, Period, PeriodId};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_statements::checks::CheckSuite;
use finstack_quant_statements::error::{Error, Result};
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::types::FinancialModelSpec;
use finstack_quant_valuations::instruments::equity::dcf_equity::TerminalValueSpec;
use indexmap::IndexMap;

/// Unified analysis result combining statement, equity, and credit perspectives.
///
/// This is the highest-level analysis envelope in the crate. Monetary outputs
/// remain in the evaluated model currency, while coverage/leverage metrics are
/// plain scalar ratios.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CorporateAnalysis {
    /// Full statement evaluation (all nodes, all periods)
    pub statement: StatementResult,
    /// Equity valuation result (if DCF was configured)
    pub equity: Option<CorporateValuationResult>,
    /// Per-instrument coverage and leverage metrics.
    pub credit: IndexMap<String, CreditContextMetrics>,
    /// `true` when a DCF enterprise value was computed but suppressed as the
    /// LTV reference because it was non-positive. Credit metrics are then
    /// computed without an enterprise-value reference.
    pub ev_suppressed_non_positive: bool,
}

/// DCF equity valuation inputs configured on the builder.
struct DcfSpec {
    wacc: f64,
    terminal_value: TerminalValueSpec,
    ufcf_node: String,
    net_debt_override: Option<f64>,
    dcf_options: DcfOptions,
}

/// Builder for corporate analysis.
///
/// This builder is intended for "single button" analysis workflows where one
/// evaluated statement model should feed both equity and credit views.
///
/// # Example
///
/// ```
/// use finstack_quant_core::{currency::Currency, dates::PeriodId, money::Money};
/// use finstack_quant_statements::builder::ModelBuilder;
/// use finstack_quant_statements::checks::{builtins::NonFiniteCheck, CheckSuite};
/// use finstack_quant_statements_analytics::analysis::CorporateAnalysisBuilder;
/// use finstack_quant_statements::types::AmountOrScalar;
/// use finstack_quant_valuations::instruments::equity::dcf_equity::TerminalValueSpec;
///
/// # fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
/// let model = ModelBuilder::new("demo")
///     .periods("2025Q1..Q4", None)?
///     .value_money("ufcf", &[
///         (PeriodId::quarter(2025, 1).expect("valid period fixture"), Money::from((100_000_i64, Currency::USD))),
///         (PeriodId::quarter(2025, 2).expect("valid period fixture"), Money::from((105_000_i64, Currency::USD))),
///         (PeriodId::quarter(2025, 3).expect("valid period fixture"), Money::from((110_000_i64, Currency::USD))),
///         (PeriodId::quarter(2025, 4).expect("valid period fixture"), Money::from((115_000_i64, Currency::USD))),
///     ])
///     .value("total_debt", &[
///         (PeriodId::quarter(2025, 1).expect("valid period fixture"), AmountOrScalar::scalar(500_000.0)),
///         (PeriodId::quarter(2025, 2).expect("valid period fixture"), AmountOrScalar::scalar(500_000.0)),
///         (PeriodId::quarter(2025, 3).expect("valid period fixture"), AmountOrScalar::scalar(500_000.0)),
///         (PeriodId::quarter(2025, 4).expect("valid period fixture"), AmountOrScalar::scalar(500_000.0)),
///     ])
///     .value("cash", &[
///         (PeriodId::quarter(2025, 1).expect("valid period fixture"), AmountOrScalar::scalar(50_000.0)),
///         (PeriodId::quarter(2025, 2).expect("valid period fixture"), AmountOrScalar::scalar(50_000.0)),
///         (PeriodId::quarter(2025, 3).expect("valid period fixture"), AmountOrScalar::scalar(50_000.0)),
///         (PeriodId::quarter(2025, 4).expect("valid period fixture"), AmountOrScalar::scalar(50_000.0)),
///     ])
///     .with_meta("currency", serde_json::json!("USD"))
///     .build()?;
///
/// let checks = CheckSuite::builder("corporate")
///     .add_check(NonFiniteCheck { nodes: vec![] })
///     .build();
///
/// let _result = CorporateAnalysisBuilder::new(model)
///     .dcf(0.10, TerminalValueSpec::GordonGrowth { growth_rate: 0.02 })
///     .checks(checks)
///     .analyze()?;
/// # Ok(())
/// # }
/// ```
pub struct CorporateAnalysisBuilder {
    model: FinancialModelSpec,
    market: Option<MarketContext>,
    as_of: Option<Date>,
    dcf: Option<DcfSpec>,
    cfads_node: Option<String>,
    interest_coverage_node: String,
    ltv_value_node: Option<String>,
    check_suite: Option<CheckSuite>,
}

impl CorporateAnalysisBuilder {
    /// Create a new builder for the given financial model.
    ///
    /// # Arguments
    ///
    /// * `model` - Statement model to evaluate and analyze.
    ///
    /// # Returns
    ///
    /// A builder with no market context, no DCF valuation, no implicit CFADS
    /// assumption, and `"ebitda"` as the interest-coverage numerator.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_statements::builder::ModelBuilder;
    /// use finstack_quant_statements_analytics::analysis::CorporateAnalysisBuilder;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = ModelBuilder::new("demo").periods("2025Q1..Q1", None)?.build()?;
    /// let _builder = CorporateAnalysisBuilder::new(model);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(model: FinancialModelSpec) -> Self {
        Self {
            model,
            market: None,
            as_of: None,
            dcf: None,
            cfads_node: None,
            interest_coverage_node: "ebitda".to_string(),
            ltv_value_node: None,
            check_suite: None,
        }
    }

    /// Set the market context for curve-based discounting.
    ///
    /// This context is forwarded both to statement evaluation and to DCF
    /// valuation if equity analysis is enabled.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Market data context to use during statement evaluation and
    ///   valuation.
    ///
    /// # Returns
    ///
    /// The updated builder.
    pub fn market(mut self, ctx: MarketContext) -> Self {
        self.market = Some(ctx);
        self
    }

    /// The date anchors DCF discounting and controls market-context lookups and
    /// explicit-observation visibility during statement evaluation. It does
    /// not change the model's discrete period grid; forecast cashflows dated on
    /// or before this date are excluded from DCF.
    ///
    /// # Arguments
    ///
    /// * `date` - Shared statement and DCF valuation date.
    ///
    /// # Returns
    ///
    /// The updated builder.
    pub fn as_of(mut self, date: Date) -> Self {
        self.as_of = Some(date);
        self
    }

    /// Configure DCF equity valuation with default options.
    ///
    /// `wacc` uses decimal form, so `0.10` means `10%`.
    ///
    /// # Arguments
    ///
    /// * `wacc` - Weighted-average cost of capital in decimal form.
    /// * `terminal_value` - Terminal value methodology for the DCF bridge.
    ///
    /// # Returns
    ///
    /// The updated builder with equity valuation enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_statements::builder::ModelBuilder;
    /// use finstack_quant_statements_analytics::analysis::CorporateAnalysisBuilder;
    /// use finstack_quant_valuations::instruments::equity::dcf_equity::TerminalValueSpec;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = ModelBuilder::new("demo").periods("2025Q1..Q4", None)?.build()?;
    /// let _builder = CorporateAnalysisBuilder::new(model)
    ///     .dcf(0.10, TerminalValueSpec::GordonGrowth { growth_rate: 0.02 });
    /// # Ok(())
    /// # }
    /// ```
    pub fn dcf(mut self, wacc: f64, terminal_value: TerminalValueSpec) -> Self {
        self.dcf = Some(DcfSpec {
            wacc,
            terminal_value,
            ufcf_node: "ufcf".to_string(),
            net_debt_override: None,
            dcf_options: DcfOptions::default(),
        });
        self
    }

    /// Override net debt for equity bridge calculation.
    ///
    /// Must be called after [`Self::dcf`]; has no effect otherwise.
    ///
    /// # Arguments
    ///
    /// * `net_debt` - Net debt amount to subtract from enterprise value.
    ///
    /// # Returns
    ///
    /// The updated builder. If DCF has not been configured, the builder is
    /// returned unchanged.
    pub fn net_debt_override(mut self, net_debt: f64) -> Self {
        if let Some(dcf) = self.dcf.as_mut() {
            dcf.net_debt_override = Some(net_debt);
        }
        self
    }

    /// Set the cash-flow-available-for-debt-service node.
    ///
    /// This explicit mapping is required before capital-structure credit
    /// metrics are produced. EBITDA is not accepted as an implicit DSCR
    /// numerator.
    ///
    /// # Arguments
    ///
    /// * `node` - Statement node containing CFADS in model currency
    ///
    /// # Returns
    ///
    /// The updated builder.
    pub fn cfads_node(mut self, node: &str) -> Self {
        self.cfads_node = Some(node.to_string());
        self
    }

    /// Set the earnings node used for interest coverage.
    ///
    /// # Arguments
    ///
    /// * `node` - Statement node containing EBITDA, EBIT, or another documented
    ///   interest-coverage numerator
    ///
    /// # Returns
    ///
    /// The updated builder.
    pub fn interest_coverage_node(mut self, node: &str) -> Self {
        self.interest_coverage_node = node.to_string();
        self
    }

    /// Attach the validation suite required before valuation or credit output.
    ///
    /// The suite must include `NonFiniteCheck`; production three-statement
    /// models should pass [`crate::analysis::three_statement_checks`].
    ///
    /// # Arguments
    ///
    /// * `suite` - Accounting and data-quality checks run against evaluated
    ///   statements before analytics
    ///
    /// # Returns
    ///
    /// The updated builder.
    pub fn checks(mut self, suite: CheckSuite) -> Self {
        self.check_suite = Some(suite);
        self
    }

    /// Use a statement node as the per-period LTV denominator.
    ///
    /// When set, LTV is `debt_balance[t] / node[t]` for each requested
    /// period. A missing or non-positive node value omits LTV for that
    /// period only. This overrides a scalar DCF enterprise value.
    ///
    /// A DCF enterprise value used without this node is broadcast as a
    /// constant denominator (current valuation versus forward debt, not a
    /// rolled EV path).
    ///
    /// # Arguments
    ///
    /// * `node` - Statement node id whose per-period values are LTV
    ///   denominators, in the same currency as instrument debt balances
    ///   (typically an enterprise-value or collateral-value series).
    ///
    /// # Returns
    ///
    /// The updated builder.
    pub fn ltv_value_node(mut self, node: &str) -> Self {
        self.ltv_value_node = Some(node.to_string());
        self
    }

    /// Execute the analysis pipeline.
    ///
    /// Steps:
    /// 1. Evaluate the financial statement model
    /// 2. Run equity valuation (if configured)
    /// 3. Compute credit context metrics for each capital structure instrument.
    ///    LTV is a path: `debt_t / value_t`. A configured
    ///    [`Self::ltv_value_node`] supplies per-period statement values.
    ///    Otherwise a positive DCF enterprise value from step 2 is broadcast
    ///    as a constant denominator (current valuation versus forward debt,
    ///    not a rolled EV).
    ///
    /// **Note:** The DCF equity valuation reuses the already evaluated statement
    /// results so analysis stays consistent with the active `market` / `as_of` context.
    ///
    /// # Returns
    ///
    /// Returns [`CorporateAnalysis`] containing the statement result plus any
    /// configured equity and credit outputs.
    ///
    /// # Errors
    ///
    /// Returns an error if statement evaluation fails, if DCF valuation fails,
    /// or if capital-structure derived credit metrics cannot be computed.
    ///
    /// # References
    ///
    /// - Discounting context for DCF outputs: `docs/REFERENCES.md#hull-options-futures`
    /// - Coverage and leverage interpretation: `docs/REFERENCES.md#tuckman-serrat-fixed-income`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::PeriodId;
    /// use finstack_quant_statements::builder::ModelBuilder;
    /// use finstack_quant_statements::types::AmountOrScalar;
    /// use finstack_quant_statements_analytics::analysis::CorporateAnalysisBuilder;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    /// let model = ModelBuilder::new("demo")
    ///     .periods("2025Q1..Q1", None)?
    ///     .value("ebitda", &[(period, AmountOrScalar::scalar(100.0))])
    ///     .build()?;
    ///
    /// let analysis = CorporateAnalysisBuilder::new(model).analyze()?;
    /// assert!(analysis.equity.is_none());
    /// # Ok(())
    /// # }
    /// ```
    pub fn analyze(self) -> Result<CorporateAnalysis> {
        let requires_checks = self.dcf.is_some() || self.model.capital_structure.is_some();
        let mut evaluator = if requires_checks {
            let suite = self.check_suite.ok_or_else(|| {
                Error::eval(
                    "Corporate valuation and credit analysis require a CheckSuite; \
                     call CorporateAnalysisBuilder::checks before analyze"
                        .to_string(),
                )
            })?;
            if !suite.check_ids().contains(&"non_finite") {
                return Err(Error::eval(
                    "Corporate analysis CheckSuite must include NonFiniteCheck".to_string(),
                ));
            }
            finstack_quant_statements::evaluator::Evaluator::new().with_checks(suite)
        } else {
            finstack_quant_statements::evaluator::Evaluator::new()
        };
        let statement = match (self.market.as_ref(), self.as_of) {
            (Some(market), Some(as_of)) => {
                evaluator.evaluate_with_market(&self.model, market, as_of)?
            }
            (Some(_), None) => {
                return Err(Error::eval(
                    "Corporate analysis requires as_of when market context is provided".to_string(),
                ));
            }
            (None, _) => evaluator.evaluate(&self.model)?,
        };
        if requires_checks
            && statement
                .check_report
                .as_ref()
                .is_some_and(finstack_quant_statements::checks::CheckReport::has_errors)
        {
            return Err(Error::eval(
                "Corporate analysis blocked by error-severity statement checks".to_string(),
            ));
        }

        // Step 2: Equity valuation (if configured)
        let equity = match self.dcf {
            Some(DcfSpec {
                wacc,
                terminal_value,
                ufcf_node,
                net_debt_override,
                dcf_options,
            }) => {
                let result = crate::analysis::valuation::corporate::evaluate_dcf_from_results_impl(
                    &self.model,
                    &statement,
                    wacc,
                    terminal_value,
                    &ufcf_node,
                    crate::analysis::valuation::corporate::DcfEvalContext {
                        net_debt_override,
                        options: &dcf_options,
                        market: self.market.as_ref(),
                    },
                    self.as_of,
                )
                .map_err(|e| {
                    finstack_quant_statements::error::Error::Eval(format!(
                        "DCF equity valuation failed in corporate analysis pipeline: {e}"
                    ))
                })?;
                Some(result)
            }
            None => None,
        };

        // Step 3: Compute credit context for each instrument (single pass)
        // Use enterprise value as LTV reference when available from equity step.
        let ev_raw = equity.as_ref().map(|eq| eq.enterprise_value.amount());
        let ev_for_ltv = ev_raw.filter(|ev| *ev > 0.0);
        // Surface (rather than silently drop) a non-positive EV: LTV-style
        // metrics will be computed without an EV reference.
        let ev_suppressed_non_positive = ev_raw.is_some_and(|ev| ev <= 0.0);
        if ev_suppressed_non_positive {
            tracing::warn!(
                enterprise_value = ev_raw,
                "non-positive DCF enterprise value suppressed as LTV reference; \
                 credit metrics computed without an EV reference"
            );
        }

        let ltv_refs = ltv_reference_path(
            &self.model.periods,
            &statement,
            self.ltv_value_node.as_deref(),
            ev_for_ltv,
        );

        let mut credit = IndexMap::new();
        if let Some(ref cs) = statement.cs_cashflows {
            let cfads_node = self.cfads_node.as_deref().ok_or_else(|| {
                Error::eval(
                    "Corporate credit analysis requires an explicit CFADS node; \
                     call CorporateAnalysisBuilder::cfads_node before analyze"
                        .to_string(),
                )
            })?;
            let reporting_currency =
                crate::analysis::valuation::corporate::extract_currency_from_model(&self.model)?;
            for instrument_id in cs.by_instrument.keys() {
                let metrics = compute_credit_context(
                    &statement,
                    cs,
                    instrument_id,
                    CreditNumeratorNodes {
                        cfads: cfads_node,
                        interest_coverage: &self.interest_coverage_node,
                    },
                    reporting_currency,
                    &self.model.periods,
                    ltv_refs.as_deref(),
                )?;
                credit.insert(instrument_id.clone(), metrics);
            }
        }

        Ok(CorporateAnalysis {
            statement,
            equity,
            credit,
            ev_suppressed_non_positive,
        })
    }
}

/// Per-period LTV denominators for [`compute_credit_context`].
///
/// A statement node, when configured, supplies `value[t]` (missing or
/// non-positive periods are omitted). Otherwise a positive scalar DCF
/// enterprise value is broadcast to every requested period: current
/// valuation versus forward debt, not a rolled EV path.
fn ltv_reference_path(
    periods: &[Period],
    statement: &StatementResult,
    ltv_value_node: Option<&str>,
    ev_for_ltv: Option<f64>,
) -> Option<Vec<(PeriodId, f64)>> {
    if let Some(node) = ltv_value_node {
        let path: Vec<(PeriodId, f64)> = periods
            .iter()
            .filter_map(|period| {
                statement
                    .get(node, &period.id)
                    .filter(|value| value.is_finite() && *value > 0.0)
                    .map(|value| (period.id, value))
            })
            .collect();
        Some(path)
    } else {
        ev_for_ltv.map(|ev| periods.iter().map(|period| (period.id, ev)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::PeriodId;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use finstack_quant_statements::builder::ModelBuilder;
    use finstack_quant_statements::checks::builtins::NonFiniteCheck;
    use finstack_quant_statements::checks::CheckSuite;
    use finstack_quant_statements::types::AmountOrScalar;
    use time::macros::date;

    fn flat_discount_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
        let mut builder = DiscountCurve::builder(curve_id)
            .base_date(base_date)
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .knots([
                (0.0, 1.0),
                (1.0, (-rate).exp()),
                (5.0, (-rate * 5.0).exp()),
                (10.0, (-rate * 10.0).exp()),
                (30.0, (-rate * 30.0).exp()),
            ]);

        if rate.abs() < 1e-10 || rate < 0.0 {
            builder = builder.interp(InterpStyle::Linear).validation(
                finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                    allow_non_monotonic: true,
                    forward_floor: None,
                },
            );
        }

        builder.build().expect("valid flat discount curve")
    }

    fn non_finite_suite() -> CheckSuite {
        CheckSuite::builder("corporate-test")
            .add_check(NonFiniteCheck { nodes: vec![] })
            .build()
    }

    #[test]
    fn test_statement_only_analysis() {
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q2", None)
            .expect("periods")
            .value(
                "revenue",
                &[
                    (
                        PeriodId::quarter(2025, 1).expect("valid period fixture"),
                        AmountOrScalar::scalar(1_000_000.0),
                    ),
                    (
                        PeriodId::quarter(2025, 2).expect("valid period fixture"),
                        AmountOrScalar::scalar(1_100_000.0),
                    ),
                ],
            )
            .compute("ebitda", "revenue * 0.3")
            .expect("formula")
            .build()
            .expect("model");

        let result = CorporateAnalysisBuilder::new(model)
            .analyze()
            .expect("should succeed");

        assert!(result.equity.is_none());
        assert!(result.credit.is_empty());
        assert!(!result.ev_suppressed_non_positive);
        assert!(result
            .statement
            .get(
                "ebitda",
                &PeriodId::quarter(2025, 1).expect("valid period fixture")
            )
            .is_some());
    }

    #[test]
    fn test_dcf_analysis() {
        let model = ModelBuilder::new("dcf-test")
            .periods("2025Q1..Q4", None)
            .expect("periods")
            .value_money(
                "ufcf",
                &[
                    (
                        PeriodId::quarter(2025, 1).expect("valid period fixture"),
                        Money::from((100_000_i64, Currency::USD)),
                    ),
                    (
                        PeriodId::quarter(2025, 2).expect("valid period fixture"),
                        Money::from((110_000_i64, Currency::USD)),
                    ),
                    (
                        PeriodId::quarter(2025, 3).expect("valid period fixture"),
                        Money::from((120_000_i64, Currency::USD)),
                    ),
                    (
                        PeriodId::quarter(2025, 4).expect("valid period fixture"),
                        Money::from((130_000_i64, Currency::USD)),
                    ),
                ],
            )
            .with_meta("currency", serde_json::json!("USD"))
            .build()
            .expect("model");

        let result = CorporateAnalysisBuilder::new(model)
            .dcf(0.10, TerminalValueSpec::GordonGrowth { growth_rate: 0.02 })
            .net_debt_override(50_000.0)
            .checks(non_finite_suite())
            .analyze()
            .expect("should succeed");

        assert!(result.equity.is_some());
        let equity = result.equity.as_ref().expect("equity should be present");
        assert!(equity.equity_value.amount() > 0.0);
        assert!(equity.enterprise_value.amount() > equity.equity_value.amount());
        assert!(!result.ev_suppressed_non_positive);
    }

    #[test]
    fn test_dcf_requires_check_suite() {
        let model = ModelBuilder::new("unchecked-dcf")
            .periods("2025Q1..Q1", None)
            .expect("periods")
            .value(
                "ufcf",
                &[(
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100.0),
                )],
            )
            .with_meta("currency", serde_json::json!("USD"))
            .build()
            .expect("model");

        let error = CorporateAnalysisBuilder::new(model)
            .dcf(0.10, TerminalValueSpec::GordonGrowth { growth_rate: 0.02 })
            .net_debt_override(0.0)
            .analyze()
            .expect_err("unchecked valuation must fail");
        assert!(error.to_string().contains("require a CheckSuite"));
    }

    #[test]
    fn test_error_severity_check_blocks_dcf() {
        let model = ModelBuilder::new("non-finite-dcf")
            .periods("2025Q1..Q1", None)
            .expect("periods")
            .compute("ufcf", "0 / 0")
            .expect("formula")
            .with_meta("currency", serde_json::json!("USD"))
            .build()
            .expect("model");

        let error = CorporateAnalysisBuilder::new(model)
            .dcf(0.10, TerminalValueSpec::GordonGrowth { growth_rate: 0.02 })
            .net_debt_override(0.0)
            .checks(non_finite_suite())
            .analyze()
            .expect_err("error-severity check must block valuation");
        assert!(error.to_string().contains("blocked"));
    }

    #[test]
    fn test_non_positive_enterprise_value_status_is_top_level() {
        let model = ModelBuilder::new("non-positive-ev")
            .periods("2025Q1..Q1", None)
            .expect("periods")
            .value_money(
                "ufcf",
                &[(
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    Money::from((0_i64, Currency::USD)),
                )],
            )
            .with_meta("currency", serde_json::json!("USD"))
            .build()
            .expect("model");

        let result = CorporateAnalysisBuilder::new(model)
            .dcf(0.10, TerminalValueSpec::GordonGrowth { growth_rate: 0.02 })
            .net_debt_override(0.0)
            .checks(non_finite_suite())
            .analyze()
            .expect("analysis should succeed");

        assert!(result.ev_suppressed_non_positive);
    }

    #[test]
    fn test_dcf_analysis_with_as_of_and_capital_structure_succeeds() {
        let as_of = date!(2025 - 01 - 01);
        let market = MarketContext::new().insert(flat_discount_curve(0.05, as_of, "USD-OIS"));
        let model = ModelBuilder::new("dcf-cs-test")
            .periods("2025Q1..Q2", Some("2025Q1"))
            .expect("periods")
            .value_money(
                "revenue",
                &[
                    (
                        PeriodId::quarter(2025, 1).expect("valid period fixture"),
                        Money::from((1_000_000_i64, Currency::USD)),
                    ),
                    (
                        PeriodId::quarter(2025, 2).expect("valid period fixture"),
                        Money::from((1_100_000_i64, Currency::USD)),
                    ),
                ],
            )
            .availability_dates(
                "revenue",
                &[(
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    as_of,
                )],
            )
            .expect("availability")
            .add_bond(
                "BOND-001",
                Money::from((1_000_000_i64, finstack_quant_core::currency::Currency::USD)),
                0.05,
                date!(2025 - 01 - 01),
                date!(2026 - 01 - 01),
                "USD-OIS",
            )
            .expect("bond")
            .compute("ufcf", "revenue - cs.interest_expense.total")
            .expect("formula")
            .with_meta("currency", serde_json::json!("USD"))
            .build()
            .expect("model");

        let result = CorporateAnalysisBuilder::new(model)
            .market(market)
            .as_of(as_of)
            .dcf(0.10, TerminalValueSpec::GordonGrowth { growth_rate: 0.02 })
            .net_debt_override(0.0)
            .cfads_node("ufcf")
            .interest_coverage_node("revenue")
            .checks(non_finite_suite())
            .analyze();

        assert!(
            result.is_ok(),
            "DCF analysis should reuse the as-of aware statement evaluation"
        );
    }

    fn sample_periods() -> Vec<Period> {
        vec![
            Period {
                id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
                start: date!(2025 - 01 - 01),
                end: date!(2025 - 04 - 01),
                is_actual: false,
            },
            Period {
                id: PeriodId::quarter(2025, 2).expect("valid period fixture"),
                start: date!(2025 - 04 - 01),
                end: date!(2025 - 07 - 01),
                is_actual: false,
            },
        ]
    }

    #[test]
    fn test_ltv_reference_path_broadcasts_scalar_ev() {
        let periods = sample_periods();
        let statement = StatementResult::new();
        let path = ltv_reference_path(&periods, &statement, None, Some(10_000_000.0))
            .expect("broadcast path");
        assert_eq!(
            path,
            vec![
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    10_000_000.0
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    10_000_000.0
                ),
            ]
        );
    }

    #[test]
    fn test_ltv_reference_path_reads_node_and_skips_missing() {
        let periods = sample_periods();
        let mut statement = StatementResult::new();
        let mut values = IndexMap::new();
        values.insert(
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            10_000_000.0,
        );
        values.insert(
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
            12_000_000.0,
        );
        statement
            .nodes
            .insert("enterprise_value".to_string(), values);

        let path = ltv_reference_path(&periods, &statement, Some("enterprise_value"), Some(1.0))
            .expect("node path");
        assert_eq!(
            path,
            vec![
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    10_000_000.0
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    12_000_000.0
                ),
            ]
        );

        statement
            .nodes
            .get_mut("enterprise_value")
            .expect("node")
            .shift_remove(&PeriodId::quarter(2025, 2).expect("valid period fixture"));
        let path = ltv_reference_path(&periods, &statement, Some("enterprise_value"), None)
            .expect("partial node path");
        assert_eq!(
            path,
            vec![(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                10_000_000.0
            )]
        );
    }

    #[test]
    fn test_ltv_value_node_on_builder_uses_per_period_values() {
        let as_of = date!(2025 - 01 - 01);
        let market = MarketContext::new().insert(flat_discount_curve(0.05, as_of, "USD-OIS"));
        let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let q2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let model = ModelBuilder::new("ltv-node")
            .periods("2025Q1..Q2", Some("2025Q1"))
            .expect("periods")
            .value(
                "enterprise_value",
                &[
                    (q1, AmountOrScalar::scalar(10_000_000.0)),
                    (q2, AmountOrScalar::scalar(8_000_000.0)),
                ],
            )
            .value(
                "cfads",
                &[
                    (q1, AmountOrScalar::scalar(1_000_000.0)),
                    (q2, AmountOrScalar::scalar(1_000_000.0)),
                ],
            )
            .value(
                "ebitda",
                &[
                    (q1, AmountOrScalar::scalar(1_500_000.0)),
                    (q2, AmountOrScalar::scalar(1_500_000.0)),
                ],
            )
            .availability_dates("enterprise_value", &[(q1, as_of)])
            .expect("availability")
            .availability_dates("cfads", &[(q1, as_of)])
            .expect("availability")
            .availability_dates("ebitda", &[(q1, as_of)])
            .expect("availability")
            .add_bond(
                "BOND-001",
                Money::from((4_000_000_i64, finstack_quant_core::currency::Currency::USD)),
                0.05,
                date!(2025 - 01 - 01),
                date!(2026 - 01 - 01),
                "USD-OIS",
            )
            .expect("bond")
            .with_meta("currency", serde_json::json!("USD"))
            .build()
            .expect("model");

        let analysis = CorporateAnalysisBuilder::new(model)
            .market(market)
            .as_of(as_of)
            .ltv_value_node("enterprise_value")
            .cfads_node("cfads")
            .interest_coverage_node("ebitda")
            .checks(non_finite_suite())
            .analyze()
            .expect("analysis");

        let metrics = analysis.credit.get("BOND-001").expect("bond credit");
        assert_eq!(metrics.ltv.len(), 2);
        let ev_q1 = 10_000_000.0;
        let ev_q2 = 8_000_000.0;
        let cs = analysis.statement.cs_cashflows.as_ref().expect("cs");
        let inst = cs.by_instrument.get("BOND-001").expect("instrument");
        let debt_q1 = inst.get(&q1).expect("q1 cf").debt_balance.amount();
        let debt_q2 = inst.get(&q2).expect("q2 cf").debt_balance.amount();
        assert!((metrics.ltv[0].1 - debt_q1 / ev_q1).abs() < 1e-12);
        assert!((metrics.ltv[1].1 - debt_q2 / ev_q2).abs() < 1e-12);
        assert!((metrics.ltv[0].1 - metrics.ltv[1].1).abs() > 1e-9);
    }
}
