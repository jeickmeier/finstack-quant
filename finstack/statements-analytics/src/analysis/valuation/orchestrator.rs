//! Corporate analysis orchestrator.
//!
//! Provides [`CorporateAnalysisBuilder`] --- a fluent API that coordinates
//! statement evaluation, credit instrument pricing, and equity valuation
//! in a single pipeline.

use crate::analysis::credit::{compute_credit_context, CreditContextMetrics};
use crate::analysis::valuation::corporate::{CorporateValuationResult, DcfOptions};
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_statements::error::Result;
use finstack_statements::evaluator::StatementResult;
use finstack_statements::types::FinancialModelSpec;
use finstack_valuations::instruments::equity::dcf_equity::TerminalValueSpec;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Unified analysis result combining statement, equity, and credit perspectives.
///
/// This is the highest-level analysis envelope in the crate. Monetary outputs
/// remain in the evaluated model currency, while coverage/leverage metrics are
/// plain scalar ratios.
#[derive(Debug, Clone)]
pub struct CorporateAnalysis {
    /// Full statement evaluation (all nodes, all periods)
    pub statement: StatementResult,
    /// Equity valuation result (if DCF was configured)
    pub equity: Option<CorporateValuationResult>,
    /// Per-instrument credit analysis
    pub credit: IndexMap<String, CreditInstrumentAnalysis>,
}

/// Credit analysis for a single instrument.
///
/// This currently exposes statement-derived credit context only, leaving room
/// for future spread, rating, or recovery analytics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditInstrumentAnalysis {
    /// Coverage and leverage metrics from statement context
    pub coverage: CreditContextMetrics,
    /// `true` when a DCF enterprise value was computed but suppressed as
    /// the LTV reference because it was non-positive. LTV-style metrics
    /// in [`coverage`](Self::coverage) are then computed without an EV
    /// reference and should be interpreted accordingly.
    #[serde(default)]
    pub ev_suppressed_non_positive: bool,
}

/// Equity valuation mode.
enum EquityMode {
    Dcf {
        wacc: f64,
        terminal_value: TerminalValueSpec,
        ufcf_node: String,
        net_debt_override: Option<f64>,
        dcf_options: DcfOptions,
    },
}

/// Builder for corporate analysis.
///
/// This builder is intended for "single button" analysis workflows where one
/// evaluated statement model should feed both equity and credit views.
///
/// # Example
///
/// ```ignore
/// use finstack_statements_analytics::analysis::CorporateAnalysisBuilder;
/// use finstack_statements::builder::ModelBuilder;
/// use finstack_core::dates::PeriodId;
/// use finstack_statements::types::AmountOrScalar;
/// use finstack_valuations::instruments::equity::dcf_equity::TerminalValueSpec;
///
/// # fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
/// let model = ModelBuilder::new("demo")
///     .periods("2025Q1..Q4", None)?
///     .value("ufcf", &[
///         (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100_000.0)),
///     ])
///     .with_meta("currency", serde_json::json!("USD"))
///     .build()?;
///
/// let _result = CorporateAnalysisBuilder::new(model)
///     .dcf(0.10, TerminalValueSpec::GordonGrowth { growth_rate: 0.02 })
///     .analyze()?;
/// # Ok(())
/// # }
/// ```
pub struct CorporateAnalysisBuilder {
    model: FinancialModelSpec,
    market: Option<MarketContext>,
    as_of: Option<Date>,
    equity_mode: Option<EquityMode>,
    coverage_node: String,
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
    /// A builder with no market context, no DCF valuation, and `"ebitda"` as
    /// the default credit coverage node.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_statements::builder::ModelBuilder;
    /// use finstack_statements_analytics::analysis::CorporateAnalysisBuilder;
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
            equity_mode: None,
            coverage_node: "ebitda".to_string(),
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

    /// Set the as-of date for valuation.
    ///
    /// The date controls market-context lookups during evaluation. It does not
    /// change the model's discrete period grid.
    ///
    /// # Arguments
    ///
    /// * `date` - Valuation date for market-context lookups.
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
    /// ```ignore
    /// use finstack_statements::builder::ModelBuilder;
    /// use finstack_statements_analytics::analysis::CorporateAnalysisBuilder;
    /// use finstack_valuations::instruments::equity::dcf_equity::TerminalValueSpec;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let model = ModelBuilder::new("demo").periods("2025Q1..Q4", None)?.build()?;
    /// let _builder = CorporateAnalysisBuilder::new(model)
    ///     .dcf(0.10, TerminalValueSpec::GordonGrowth { growth_rate: 0.02 });
    /// # Ok(())
    /// # }
    /// ```
    pub fn dcf(mut self, wacc: f64, terminal_value: TerminalValueSpec) -> Self {
        self.equity_mode = Some(EquityMode::Dcf {
            wacc,
            terminal_value,
            ufcf_node: "ufcf".to_string(),
            net_debt_override: None,
            dcf_options: DcfOptions::default(),
        });
        self
    }

    /// Configure DCF equity valuation with custom options.
    ///
    /// `wacc` uses decimal form, so `0.10` means `10%`.
    ///
    /// # Arguments
    ///
    /// * `wacc` - Weighted-average cost of capital in decimal form.
    /// * `terminal_value` - Terminal value methodology for the DCF bridge.
    /// * `options` - DCF extraction and discounting options.
    ///
    /// # Returns
    ///
    /// The updated builder with equity valuation enabled.
    pub fn dcf_with_options(
        mut self,
        wacc: f64,
        terminal_value: TerminalValueSpec,
        options: DcfOptions,
    ) -> Self {
        self.equity_mode = Some(EquityMode::Dcf {
            wacc,
            terminal_value,
            ufcf_node: "ufcf".to_string(),
            net_debt_override: None,
            dcf_options: options,
        });
        self
    }

    /// Override the UFCF node name (default: "ufcf").
    ///
    /// Must be called after [`Self::dcf`] or [`Self::dcf_with_options`]; has no
    /// effect otherwise.
    ///
    /// # Arguments
    ///
    /// * `node` - Node id containing unlevered free cash flow.
    ///
    /// # Returns
    ///
    /// The updated builder. If DCF has not been configured, the builder is
    /// returned unchanged.
    pub fn dcf_node(mut self, node: &str) -> Self {
        if let Some(EquityMode::Dcf {
            ref mut ufcf_node, ..
        }) = self.equity_mode
        {
            *ufcf_node = node.to_string();
        }
        self
    }

    /// Override net debt for equity bridge calculation.
    ///
    /// Must be called after [`Self::dcf`] or [`Self::dcf_with_options`]; has no
    /// effect otherwise.
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
        if let Some(EquityMode::Dcf {
            net_debt_override: ref mut nd,
            ..
        }) = self.equity_mode
        {
            *nd = Some(net_debt);
        }
        self
    }

    /// Set the coverage node for credit metrics (default: "ebitda").
    ///
    /// The selected node is used as the numerator in DSCR and interest-coverage
    /// calculations.
    ///
    /// # Arguments
    ///
    /// * `node` - Statement node id to use as the coverage numerator.
    ///
    /// # Returns
    ///
    /// The updated builder.
    pub fn coverage_node(mut self, node: &str) -> Self {
        self.coverage_node = node.to_string();
        self
    }

    /// Execute the analysis pipeline.
    ///
    /// Steps:
    /// 1. Evaluate the financial statement model
    /// 2. Run equity valuation (if configured)
    /// 3. Compute credit context metrics for each capital structure instrument,
    ///    using enterprise value from step 2 as the LTV reference when available
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
    /// use finstack_core::dates::PeriodId;
    /// use finstack_statements::builder::ModelBuilder;
    /// use finstack_statements::types::AmountOrScalar;
    /// use finstack_statements_analytics::analysis::CorporateAnalysisBuilder;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let period = PeriodId::quarter(2025, 1);
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
        // Step 1: Evaluate statement
        let mut evaluator = finstack_statements::evaluator::Evaluator::new();
        let statement = match (self.market.as_ref(), self.as_of) {
            (Some(market), Some(as_of)) => {
                evaluator.evaluate_with_market(&self.model, market, as_of)?
            }
            _ => evaluator.evaluate(&self.model)?,
        };

        // Step 2: Equity valuation (if configured)
        let equity = match self.equity_mode {
            Some(EquityMode::Dcf {
                wacc,
                terminal_value,
                ufcf_node,
                net_debt_override,
                dcf_options,
            }) => {
                let (result, _trace) =
                    crate::analysis::valuation::corporate::evaluate_dcf_from_results_impl(
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
                    )
                    .map_err(|e| {
                        finstack_statements::error::Error::Eval(format!(
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

        let mut credit = IndexMap::new();
        if let Some(ref cs) = statement.cs_cashflows {
            for instrument_id in cs.by_instrument.keys() {
                let coverage = compute_credit_context(
                    &statement,
                    cs,
                    instrument_id,
                    &self.coverage_node,
                    &self.model.periods,
                    ev_for_ltv,
                );
                credit.insert(
                    instrument_id.clone(),
                    CreditInstrumentAnalysis {
                        coverage,
                        ev_suppressed_non_positive,
                    },
                );
            }
        }

        Ok(CorporateAnalysis {
            statement,
            equity,
            credit,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::dates::PeriodId;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::math::interp::InterpStyle;
    use finstack_core::money::Money;
    use finstack_statements::builder::ModelBuilder;
    use finstack_statements::types::AmountOrScalar;
    use time::macros::date;

    fn flat_discount_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
        let mut builder = DiscountCurve::builder(curve_id)
            .base_date(base_date)
            .day_count(finstack_core::dates::DayCount::Act360)
            .knots([
                (0.0, 1.0),
                (1.0, (-rate).exp()),
                (5.0, (-rate * 5.0).exp()),
                (10.0, (-rate * 10.0).exp()),
                (30.0, (-rate * 30.0).exp()),
            ]);

        if rate.abs() < 1e-10 || rate < 0.0 {
            builder = builder.interp(InterpStyle::Linear).validation(
                finstack_core::market_data::term_structures::ValidationMode::Raw {
                    allow_non_monotonic: true,
                    forward_floor: None,
                },
            );
        }

        builder.build().expect("valid flat discount curve")
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
                        PeriodId::quarter(2025, 1),
                        AmountOrScalar::scalar(1_000_000.0),
                    ),
                    (
                        PeriodId::quarter(2025, 2),
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
        assert!(result
            .statement
            .get("ebitda", &PeriodId::quarter(2025, 1))
            .is_some());
    }

    #[test]
    fn test_dcf_analysis() {
        let model = ModelBuilder::new("dcf-test")
            .periods("2025Q1..Q4", None)
            .expect("periods")
            .value(
                "ufcf",
                &[
                    (
                        PeriodId::quarter(2025, 1),
                        AmountOrScalar::scalar(100_000.0),
                    ),
                    (
                        PeriodId::quarter(2025, 2),
                        AmountOrScalar::scalar(110_000.0),
                    ),
                    (
                        PeriodId::quarter(2025, 3),
                        AmountOrScalar::scalar(120_000.0),
                    ),
                    (
                        PeriodId::quarter(2025, 4),
                        AmountOrScalar::scalar(130_000.0),
                    ),
                ],
            )
            .with_meta("currency", serde_json::json!("USD"))
            .build()
            .expect("model");

        let result = CorporateAnalysisBuilder::new(model)
            .dcf(0.10, TerminalValueSpec::GordonGrowth { growth_rate: 0.02 })
            .net_debt_override(50_000.0)
            .analyze()
            .expect("should succeed");

        assert!(result.equity.is_some());
        let equity = result.equity.as_ref().expect("equity should be present");
        assert!(equity.equity_value.amount() > 0.0);
        assert!(equity.enterprise_value.amount() > equity.equity_value.amount());
    }

    #[test]
    fn test_dcf_analysis_with_as_of_and_capital_structure_succeeds() {
        let as_of = date!(2025 - 01 - 01);
        let market = MarketContext::new().insert(flat_discount_curve(0.05, as_of, "USD-OIS"));
        let model = ModelBuilder::new("dcf-cs-test")
            .periods("2025Q1..Q2", Some("2025Q1"))
            .expect("periods")
            .value(
                "revenue",
                &[
                    (
                        PeriodId::quarter(2025, 1),
                        AmountOrScalar::scalar(1_000_000.0),
                    ),
                    (
                        PeriodId::quarter(2025, 2),
                        AmountOrScalar::scalar(1_100_000.0),
                    ),
                ],
            )
            .add_bond(
                "BOND-001",
                Money::new(1_000_000.0, finstack_core::currency::Currency::USD),
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
            .analyze();

        assert!(
            result.is_ok(),
            "DCF analysis should reuse the as-of aware statement evaluation"
        );
    }
}
