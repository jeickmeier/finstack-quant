//! Adjusted Net Debt bridge for credit analysis.
//!
//! # Motivation
//!
//! Raw total debt understates leverage for issuers with material
//! operating-lease obligations, defined-benefit pension deficits, or
//! off-balance-sheet financing. Credit analysts correct for these by
//! computing **Adjusted Net Debt**:
//!
//! ```text
//! Adjusted Net Debt =   Total Debt
//!                     − Cash & Cash Equivalents
//!                     − Marketable Securities
//!                     + Capitalized Operating Leases
//!                     + Pension Deficit (unfunded defined benefit)
//!                     + Other Debt-Like Obligations
//! ```
//!
//! This is the canonical denominator for rating-agency leverage
//! metrics (Moody's "adjusted leverage", S&P "fully adjusted debt", Fitch
//! "FCF after rent"). Using it instead of raw balance-sheet debt can
//! move the Debt/EBITDA ratio by 1.0x or more on lease-heavy or
//! pension-heavy issuers.
//!
//! # Design
//!
//! [`AdjustedNetDebtSpec`] declares which financial-model nodes supply
//! each component. The computation is period-wise: for every
//! [`PeriodId`] present in the evaluated [`StatementResult`], it reads
//! the configured nodes, subtracts liquid assets, adds debt-like
//! obligations, and returns the adjusted figure. Missing optional
//! nodes default to zero so minimal specs (just total debt and cash)
//! can produce a "Net Debt" figure without listing every off-balance-
//! sheet item.
//!
//! # Relationship to covenants
//!
//! The covenant engine ([`finstack_quant_covenants`]) accepts any
//! scalar time-series via
//! [`ModelTimeSeries`](finstack_quant_covenants::ModelTimeSeries);
//! plugging the output of [`AdjustedNetDebtSpec::compute_series`] into a
//! covenant with the
//! [`CovenantType::MaxDebtToEBITDA`](finstack_quant_covenants::CovenantType::MaxDebtToEBITDA)
//! variant produces a fully-adjusted
//! leverage forecast without changing the covenant engine itself.
//!
//! A full `AdjustedNetDebt`-typed node in `FinancialModelSpec` is
//! still follow-up work, but consumers that already have the component
//! nodes in their model can pipe them through this helper to produce a
//! consistent adjusted-debt series.
//!
//! # Limitations
//!
//! Several rating-agency refinements are **not** modeled here and must be
//! pre-applied by the caller in the component nodes if desired:
//!
//! - **Restricted-cash haircut**: all cash supplied via the cash node is
//!   netted in full. Agencies typically exclude (or haircut) restricted or
//!   trapped cash; pass an already-haircut cash node to replicate that.
//! - **Pension tax-effecting**: the pension-deficit node is added gross.
//!   S&P/Moody's commonly tax-effect the unfunded obligation (deficit ×
//!   (1 − tax rate)); supply a tax-effected node to match.
//! - **Lease capitalization helper**: there is no built-in multiple-of-rent
//!   or discounted-lease capitalization; the capitalized-leases node must
//!   already contain the capitalized amount (e.g. 8× rent or the IFRS 16
//!   lease liability).

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::types::NodeId;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Specification of the balance-sheet nodes that compose Adjusted Net Debt.
///
/// Construct via [`AdjustedNetDebtSpec::builder`] or the direct struct
/// literal. All component nodes are looked up in the evaluated
/// [`StatementResult`] at each period; missing optional nodes default
/// to zero so callers with partial models can still produce a defined
/// "Net Debt = Debt − Cash" figure.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::PeriodId;
/// use finstack_quant_statements::builder::ModelBuilder;
/// use finstack_quant_statements::evaluator::Evaluator;
/// use finstack_quant_statements::types::AmountOrScalar;
/// use finstack_quant_statements_analytics::analysis::AdjustedNetDebtSpec;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let period = PeriodId::quarter(2025, 1);
/// let model = ModelBuilder::new("credit-model")
///     .periods("2025Q1..Q1", None)?
///     .value("debt", &[(period, AmountOrScalar::scalar(1_000.0))])
///     .value("cash", &[(period, AmountOrScalar::scalar(200.0))])
///     .build()?;
/// let mut evaluator = Evaluator::new();
/// let results = evaluator.evaluate(&model)?;
///
/// let spec = AdjustedNetDebtSpec::builder("debt".into())
///     .cash("cash".into())
///     .build();
///
/// assert_eq!(spec.compute(&results, &period), Some(800.0));
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjustedNetDebtSpec {
    /// Total debt node (mandatory).
    pub total_debt_node: NodeId,
    /// Cash and cash equivalents (subtracted). Optional.
    pub cash_node: Option<NodeId>,
    /// Marketable securities / short-term investments (subtracted). Optional.
    pub marketable_securities_node: Option<NodeId>,
    /// Capitalized operating-lease debt (added). Optional. Per IFRS 16 /
    /// ASC 842, on-balance-sheet lease liabilities are already in
    /// `total_debt_node`; this component is for residual off-balance-
    /// sheet lease adjustments rating agencies capitalize.
    pub operating_lease_debt_node: Option<NodeId>,
    /// Unfunded defined-benefit pension obligation (added). Optional.
    pub pension_deficit_node: Option<NodeId>,
    /// Additional debt-like obligations to be added (guarantees, preferred
    /// stock at rating-agency haircut, etc.).
    #[serde(default)]
    pub other_additions: Vec<NodeId>,
    /// Additional subtractions beyond cash / marketable securities
    /// (restricted cash that becomes available, etc.).
    #[serde(default)]
    pub other_subtractions: Vec<NodeId>,
}

impl AdjustedNetDebtSpec {
    /// Start a fluent builder configured with the mandatory total-debt node.
    ///
    /// # Arguments
    ///
    /// * `total_debt_node` - Node id containing total balance-sheet debt.
    ///
    /// # Returns
    ///
    /// A builder initialized with the mandatory debt node and no optional
    /// adjustments.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_statements_analytics::analysis::AdjustedNetDebtSpec;
    ///
    /// let spec = AdjustedNetDebtSpec::builder("debt".into())
    ///     .cash("cash".into())
    ///     .build();
    ///
    /// assert_eq!(spec.total_debt_node, "debt");
    /// ```
    #[must_use]
    pub fn builder(total_debt_node: NodeId) -> AdjustedNetDebtSpecBuilder {
        AdjustedNetDebtSpecBuilder {
            spec: AdjustedNetDebtSpec {
                total_debt_node,
                cash_node: None,
                marketable_securities_node: None,
                operating_lease_debt_node: None,
                pension_deficit_node: None,
                other_additions: Vec::new(),
                other_subtractions: Vec::new(),
            },
        }
    }

    /// Compute the adjusted net debt at a single period.
    ///
    /// Returns `None` if the mandatory `total_debt_node` is missing at
    /// the requested period — the metric is undefined without a debt
    /// anchor. Returns `None` as well when a *configured* component node
    /// (cash, leases, pension, explicit additions/subtractions) is absent
    /// from the results: a configured-but-missing node signals an
    /// unresolved identifier, and silently treating it as zero would
    /// understate adjusted leverage. Components left unconfigured
    /// (`None` / empty list) contribute zero, as intended.
    ///
    /// # Arguments
    ///
    /// * `results` - Evaluated statement result containing the configured
    ///   component nodes.
    /// * `period` - Period at which to compute adjusted net debt.
    ///
    /// # Returns
    ///
    /// `Some(adjusted_net_debt)` when the mandatory debt node is present, or
    /// `None` when total debt is missing for the requested period.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::PeriodId;
    /// use finstack_quant_statements::builder::ModelBuilder;
    /// use finstack_quant_statements::evaluator::Evaluator;
    /// use finstack_quant_statements::types::AmountOrScalar;
    /// use finstack_quant_statements_analytics::analysis::AdjustedNetDebtSpec;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let period = PeriodId::quarter(2025, 1);
    /// let model = ModelBuilder::new("credit-model")
    ///     .periods("2025Q1..Q1", None)?
    ///     .value("debt", &[(period, AmountOrScalar::scalar(1_000.0))])
    ///     .value("cash", &[(period, AmountOrScalar::scalar(150.0))])
    ///     .value("leases", &[(period, AmountOrScalar::scalar(50.0))])
    ///     .build()?;
    /// let mut evaluator = Evaluator::new();
    /// let results = evaluator.evaluate(&model)?;
    /// let spec = AdjustedNetDebtSpec::builder("debt".into())
    ///     .cash("cash".into())
    ///     .operating_lease_debt("leases".into())
    ///     .build();
    ///
    /// assert_eq!(spec.compute(&results, &period), Some(900.0));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # References
    ///
    /// - Rating-agency adjusted leverage convention: `docs/REFERENCES.md#tuckman-serrat-fixed-income`
    pub fn compute(&self, results: &StatementResult, period: &PeriodId) -> Option<f64> {
        let debt = results.get(self.total_debt_node.as_str(), period)?;

        // A configured component (`Some`) that the results cannot resolve
        // yields `None` so the whole period is omitted; an unconfigured
        // component (`None`) contributes zero.
        let pull = |node: &Option<NodeId>| -> Option<f64> {
            match node {
                None => Some(0.0),
                Some(n) => results.get(n.as_str(), period),
            }
        };
        // Every node in an explicit additions/subtractions list must resolve;
        // a missing entry collapses the sum to `None`.
        let sum_required = |nodes: &[NodeId]| -> Option<f64> {
            nodes.iter().map(|n| results.get(n.as_str(), period)).sum()
        };

        let cash = pull(&self.cash_node)?;
        let marketable = pull(&self.marketable_securities_node)?;
        let leases = pull(&self.operating_lease_debt_node)?;
        let pension = pull(&self.pension_deficit_node)?;
        let other_add = sum_required(&self.other_additions)?;
        let other_sub = sum_required(&self.other_subtractions)?;

        Some(debt - cash - marketable + leases + pension + other_add - other_sub)
    }

    /// Compute the adjusted net debt series across every period present
    /// in `results` that has a [`Self::total_debt_node`] value.
    ///
    /// Returns an [`IndexMap<PeriodId, f64>`] preserving the period
    /// ordering from the evaluator. Periods where total debt is missing
    /// are omitted from the output.
    ///
    /// # Arguments
    ///
    /// * `results` - Evaluated statement result containing the configured
    ///   component nodes.
    ///
    /// # Returns
    ///
    /// A period-indexed adjusted-net-debt series. The series is empty when the
    /// mandatory total-debt node is absent from the result set.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::PeriodId;
    /// use finstack_quant_statements::builder::ModelBuilder;
    /// use finstack_quant_statements::evaluator::Evaluator;
    /// use finstack_quant_statements::types::AmountOrScalar;
    /// use finstack_quant_statements_analytics::analysis::AdjustedNetDebtSpec;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let q1 = PeriodId::quarter(2025, 1);
    /// let q2 = PeriodId::quarter(2025, 2);
    /// let model = ModelBuilder::new("credit-model")
    ///     .periods("2025Q1..Q2", None)?
    ///     .value("debt", &[
    ///         (q1, AmountOrScalar::scalar(1_000.0)),
    ///         (q2, AmountOrScalar::scalar(900.0)),
    ///     ])
    ///     .value("cash", &[
    ///         (q1, AmountOrScalar::scalar(100.0)),
    ///         (q2, AmountOrScalar::scalar(125.0)),
    ///     ])
    ///     .build()?;
    /// let mut evaluator = Evaluator::new();
    /// let results = evaluator.evaluate(&model)?;
    /// let spec = AdjustedNetDebtSpec::builder("debt".into())
    ///     .cash("cash".into())
    ///     .build();
    ///
    /// let series = spec.compute_series(&results);
    /// assert_eq!(series.get(&q1), Some(&900.0));
    /// assert_eq!(series.get(&q2), Some(&775.0));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # References
    ///
    /// - Rating-agency adjusted leverage convention: `docs/REFERENCES.md#tuckman-serrat-fixed-income`
    #[must_use]
    pub fn compute_series(&self, results: &StatementResult) -> IndexMap<PeriodId, f64> {
        let mut series = IndexMap::new();
        let Some(debt_periods) = results.nodes.get(self.total_debt_node.as_str()) else {
            return series;
        };
        for period in debt_periods.keys() {
            if let Some(value) = self.compute(results, period) {
                series.insert(*period, value);
            }
        }
        series
    }
}

/// Fluent builder for [`AdjustedNetDebtSpec`].
#[derive(Debug, Clone)]
pub struct AdjustedNetDebtSpecBuilder {
    spec: AdjustedNetDebtSpec,
}

impl AdjustedNetDebtSpecBuilder {
    /// Configure the cash / cash-equivalent node to subtract.
    ///
    /// # Arguments
    ///
    /// * `node` - Node id containing cash and cash equivalents.
    ///
    /// # Returns
    ///
    /// The updated builder.
    #[must_use]
    pub fn cash(mut self, node: NodeId) -> Self {
        self.spec.cash_node = Some(node);
        self
    }

    /// Configure the marketable-securities node to subtract.
    ///
    /// # Arguments
    ///
    /// * `node` - Node id containing marketable securities or short-term
    ///   investments.
    ///
    /// # Returns
    ///
    /// The updated builder.
    #[must_use]
    pub fn marketable_securities(mut self, node: NodeId) -> Self {
        self.spec.marketable_securities_node = Some(node);
        self
    }

    /// Configure the capitalized-operating-lease node to add.
    ///
    /// # Arguments
    ///
    /// * `node` - Node id containing capitalized operating lease debt.
    ///
    /// # Returns
    ///
    /// The updated builder.
    #[must_use]
    pub fn operating_lease_debt(mut self, node: NodeId) -> Self {
        self.spec.operating_lease_debt_node = Some(node);
        self
    }

    /// Configure the pension-deficit node to add.
    ///
    /// # Arguments
    ///
    /// * `node` - Node id containing the unfunded pension deficit.
    ///
    /// # Returns
    ///
    /// The updated builder.
    #[must_use]
    pub fn pension_deficit(mut self, node: NodeId) -> Self {
        self.spec.pension_deficit_node = Some(node);
        self
    }

    /// Append an additional debt-like addition node.
    ///
    /// # Arguments
    ///
    /// * `node` - Node id containing another debt-like obligation to add.
    ///
    /// # Returns
    ///
    /// The updated builder.
    #[must_use]
    pub fn add_other_addition(mut self, node: NodeId) -> Self {
        self.spec.other_additions.push(node);
        self
    }

    /// Append an additional subtraction node.
    ///
    /// # Arguments
    ///
    /// * `node` - Node id containing another liquid-asset or adjustment item
    ///   to subtract.
    ///
    /// # Returns
    ///
    /// The updated builder.
    #[must_use]
    pub fn add_other_subtraction(mut self, node: NodeId) -> Self {
        self.spec.other_subtractions.push(node);
        self
    }

    /// Finalize the spec.
    ///
    /// # Returns
    ///
    /// The completed [`AdjustedNetDebtSpec`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_statements_analytics::analysis::AdjustedNetDebtSpec;
    ///
    /// let spec = AdjustedNetDebtSpec::builder("debt".into())
    ///     .cash("cash".into())
    ///     .build();
    ///
    /// assert_eq!(spec.cash_node.as_ref().map(|node| node.as_str()), Some("cash"));
    /// ```
    #[must_use]
    pub fn build(self) -> AdjustedNetDebtSpec {
        self.spec
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_statements::builder::ModelBuilder;
    use finstack_quant_statements::evaluator::Evaluator;
    use finstack_quant_statements::types::AmountOrScalar;

    fn q(quarter: u8) -> PeriodId {
        PeriodId::quarter(2025, quarter)
    }

    fn s(v: f64) -> AmountOrScalar {
        AmountOrScalar::scalar(v)
    }

    /// The Adjusted Net Debt formula
    /// `Debt − Cash − MarketableSecurities + Leases + Pension` must
    /// produce the canonical number on a minimal test model.
    #[test]
    fn adjusted_net_debt_basic_formula() {
        let model = ModelBuilder::new("p1_17_basic")
            .periods("2025Q1..Q1", None)
            .unwrap()
            .value("debt", &[(q(1), s(1_000.0))])
            .value("cash", &[(q(1), s(200.0))])
            .value("ms", &[(q(1), s(50.0))])
            .value("leases", &[(q(1), s(80.0))])
            .value("pension", &[(q(1), s(120.0))])
            .build()
            .unwrap();

        let mut ev = Evaluator::new();
        let results = ev.evaluate(&model).unwrap();

        let spec = AdjustedNetDebtSpec::builder(NodeId::new("debt"))
            .cash(NodeId::new("cash"))
            .marketable_securities(NodeId::new("ms"))
            .operating_lease_debt(NodeId::new("leases"))
            .pension_deficit(NodeId::new("pension"))
            .build();

        let value = spec.compute(&results, &q(1)).expect("defined");
        // 1000 − 200 − 50 + 80 + 120 = 950
        assert!(
            (value - 950.0).abs() < 1e-9,
            "Adjusted Net Debt = 950, got {value}"
        );
    }

    /// Missing optional components must default to zero so a minimal
    /// spec (just debt) produces the raw debt figure, and a two-node
    /// spec produces standard "Net Debt" = Debt − Cash.
    #[test]
    fn adjusted_net_debt_optional_components_default_to_zero() {
        let model = ModelBuilder::new("p1_17_optional")
            .periods("2025Q1..Q1", None)
            .unwrap()
            .value("debt", &[(q(1), s(1_000.0))])
            .value("cash", &[(q(1), s(200.0))])
            .build()
            .unwrap();

        let mut ev = Evaluator::new();
        let results = ev.evaluate(&model).unwrap();

        // Just total debt: equals raw debt.
        let raw = AdjustedNetDebtSpec::builder(NodeId::new("debt"))
            .build()
            .compute(&results, &q(1))
            .unwrap();
        assert!((raw - 1_000.0).abs() < 1e-9);

        // Debt − Cash: equals classical Net Debt.
        let net = AdjustedNetDebtSpec::builder(NodeId::new("debt"))
            .cash(NodeId::new("cash"))
            .build()
            .compute(&results, &q(1))
            .unwrap();
        assert!((net - 800.0).abs() < 1e-9);
    }

    /// Without a total-debt value the metric is undefined — the method
    /// must return `None` rather than silently treating missing debt
    /// as zero.
    #[test]
    fn adjusted_net_debt_requires_total_debt() {
        let model = ModelBuilder::new("p1_17_missing")
            .periods("2025Q1..Q1", None)
            .unwrap()
            .value("cash", &[(q(1), s(200.0))])
            .build()
            .unwrap();

        let mut ev = Evaluator::new();
        let results = ev.evaluate(&model).unwrap();

        let spec = AdjustedNetDebtSpec::builder(NodeId::new("debt_missing_node"))
            .cash(NodeId::new("cash"))
            .build();

        assert!(
            spec.compute(&results, &q(1)).is_none(),
            "compute() must return None when total debt is missing"
        );
    }

    /// The series form iterates every period the total-debt node has a
    /// value in, preserving evaluator ordering.
    #[test]
    fn adjusted_net_debt_series_covers_all_debt_periods() {
        let model = ModelBuilder::new("p1_17_series")
            .periods("2025Q1..Q3", None)
            .unwrap()
            .value(
                "debt",
                &[(q(1), s(1_000.0)), (q(2), s(1_200.0)), (q(3), s(1_500.0))],
            )
            .value(
                "cash",
                &[(q(1), s(200.0)), (q(2), s(250.0)), (q(3), s(300.0))],
            )
            .build()
            .unwrap();

        let mut ev = Evaluator::new();
        let results = ev.evaluate(&model).unwrap();

        let series = AdjustedNetDebtSpec::builder(NodeId::new("debt"))
            .cash(NodeId::new("cash"))
            .build()
            .compute_series(&results);

        assert_eq!(series.len(), 3);
        assert!((series[&q(1)] - 800.0).abs() < 1e-9);
        assert!((series[&q(2)] - 950.0).abs() < 1e-9);
        assert!((series[&q(3)] - 1_200.0).abs() < 1e-9);
    }
}
