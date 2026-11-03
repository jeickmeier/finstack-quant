//! Pre-built check suites for common financial model patterns.
//!
//! Each factory function accepts a typed mapping and returns a ready-to-run
//! [`CheckSuite`] with the appropriate structural, data-quality, and
//! credit-reasonableness checks.

use finstack_quant_statements::checks::builtins::{
    BalanceSheetArticulation, CashReconciliation, MissingValueCheck, NonFiniteCheck,
    RetainedEarningsReconciliation,
};
use finstack_quant_statements::checks::{CheckSuite, PeriodScope, Severity, SignConventionPolicy};

use super::consistency::{EffectiveTaxRateCheck, GrowthRateConsistency, WorkingCapitalConsistency};
use super::credit::{
    CoverageFloorCheck, FcfSignCheck, LeverageRangeCheck, LiquidityRunwayCheck, TrendCheck,
};
use super::reconciliation::{
    CapexReconciliation, DepreciationReconciliation, DividendReconciliation,
    InterestExpenseReconciliation,
};
use super::{CreditMapping, ThreeStatementMapping, TrendDirection};

/// Effective-tax-rate band outside which the suite raises an informational
/// finding. Widened beyond any single jurisdiction's statutory rate so that it
/// flags model errors — a negative provision on positive pre-tax income, or a
/// rate no jurisdiction levies — rather than opining on tax planning.
const ETR_PLAUSIBLE_RANGE: (f64, f64) = (0.0, 0.50);

/// Period-over-period growth bounds for the plausibility sweep.
///
/// Deliberately wide: the purpose is catching mechanical model faults (unit
/// errors, dropped links, sign flips), not judging whether a forecast is
/// ambitious. A doubling or a halving in one period clears the bar for review.
const MAX_PERIOD_GROWTH: f64 = 1.00;
const MAX_PERIOD_DECLINE: f64 = -0.50;

/// Liquidity-runway thresholds in months. The warning level matches the
/// twelve-month horizon over which going-concern is conventionally assessed;
/// the error level marks runway short enough to be an immediate credit event.
const RUNWAY_WARN_MONTHS: f64 = 12.0;
const RUNWAY_ERROR_MONTHS: f64 = 6.0;

// Three-statement suite

/// Build a check suite for a three-statement financial model.
///
/// Includes:
/// - **BalanceSheetArticulation** (always)
/// - **RetainedEarningsReconciliation** (always)
/// - **CashReconciliation** (if `total_cf_node` is provided)
/// - **DepreciationReconciliation** (if `ppe_node`, `depreciation_node`, and
///   `capex_node` are all provided)
/// - **CapexReconciliation** (if `capex_node` and at least one of
///   `ppe_additions_node` / `intangible_additions_node` are provided)
/// - **DividendReconciliation** (if `dividends_node` and
///   `dividends_equity_node` are both provided)
/// - **InterestExpenseReconciliation** (if `interest_expense_node` is provided
///   along with either `cs_interest_node` or non-empty `debt_balance_nodes`)
/// - **WorkingCapitalConsistency** (if `wc_change_cf_node` and at least one
///   current-asset or current-liability node are provided)
/// - **EffectiveTaxRateCheck** (if `tax_expense_node` and `pretax_income_node`
///   are both provided; informational severity only)
/// - **GrowthRateConsistency** over all mapping nodes (always)
/// - **NonFiniteCheck** over all mapping nodes
/// - **MissingValueCheck** for required nodes
///
/// # Arguments
///
/// * `mapping` - Three-statement node mapping that identifies balance-sheet,
///   cash-flow, and optional depreciation-reconciliation source nodes.
pub fn three_statement_checks(mapping: ThreeStatementMapping) -> CheckSuite {
    let mut builder = CheckSuite::builder("Three-Statement Checks")
        .description("Structural and data-quality checks for a three-statement model");

    // Balance sheet identity.
    builder = builder.add_check(BalanceSheetArticulation {
        assets_nodes: mapping.assets_nodes.clone(),
        liabilities_nodes: mapping.liabilities_nodes.clone(),
        equity_nodes: mapping.equity_nodes.clone(),
        tolerance: None,
    });

    // Retained earnings reconciliation.
    builder = builder.add_check(RetainedEarningsReconciliation {
        retained_earnings_node: mapping.retained_earnings_node.clone(),
        net_income_node: mapping.net_income_node.clone(),
        dividends_node: mapping.dividends_node.clone(),
        other_adjustments: vec![],
        tolerance: None,
        dividends_sign_convention: SignConventionPolicy::default(),
    });

    // Cash reconciliation (requires total_cf_node).
    if let Some(ref total_cf_node) = mapping.total_cf_node {
        builder = builder.add_check(CashReconciliation {
            cash_balance_node: mapping.cash_node.clone(),
            total_cash_flow_node: total_cf_node.clone(),
            cfo_node: mapping.cfo_node.clone(),
            cfi_node: mapping.cfi_node.clone(),
            cff_node: mapping.cff_node.clone(),
            tolerance: None,
        });
    }

    // Depreciation reconciliation (requires ppe + depreciation + capex).
    if let (Some(ppe), Some(dep), Some(capex)) = (
        &mapping.ppe_node,
        &mapping.depreciation_node,
        &mapping.capex_node,
    ) {
        builder = builder.add_check(DepreciationReconciliation {
            depreciation_expense_node: dep.clone(),
            ppe_node: ppe.clone(),
            capex_node: capex.clone(),
            disposals_node: None,
            tolerance: None,
            sign_convention: SignConventionPolicy::default(),
        });
    }

    // Capex reconciliation (requires capex plus at least one addition node —
    // the check is inert without a component to reconcile against).
    if let Some(ref capex) = mapping.capex_node
        && (mapping.ppe_additions_node.is_some() || mapping.intangible_additions_node.is_some())
    {
        builder = builder.add_check(CapexReconciliation {
            capex_cf_node: capex.clone(),
            ppe_additions_node: mapping.ppe_additions_node.clone(),
            intangible_additions_node: mapping.intangible_additions_node.clone(),
            tolerance: None,
            sign_convention: SignConventionPolicy::default(),
        });
    }

    // Dividend reconciliation (cash-flow dividends vs the equity roll-forward).
    if let (Some(div_cf), Some(div_eq)) = (&mapping.dividends_node, &mapping.dividends_equity_node)
    {
        builder = builder.add_check(DividendReconciliation {
            dividends_cf_node: div_cf.clone(),
            dividends_equity_node: div_eq.clone(),
            tolerance: None,
            sign_convention: SignConventionPolicy::default(),
        });
    }

    // Interest reconciliation (needs either a capital-structure interest node
    // to compare against, or debt balances to imply a rate from).
    if let Some(ref interest) = mapping.interest_expense_node
        && (mapping.cs_interest_node.is_some() || !mapping.debt_balance_nodes.is_empty())
    {
        builder = builder.add_check(InterestExpenseReconciliation {
            interest_expense_node: interest.clone(),
            debt_balance_nodes: mapping.debt_balance_nodes.clone(),
            cs_interest_node: mapping.cs_interest_node.clone(),
            tolerance_pct: None,
        });
    }

    // Working-capital consistency (CFS movement vs the balance-sheet delta).
    if let Some(ref wc_cf) = mapping.wc_change_cf_node
        && (!mapping.current_assets_nodes.is_empty()
            || !mapping.current_liabilities_nodes.is_empty())
    {
        builder = builder.add_check(WorkingCapitalConsistency {
            wc_change_cf_node: wc_cf.clone(),
            current_assets_nodes: mapping.current_assets_nodes.clone(),
            current_liabilities_nodes: mapping.current_liabilities_nodes.clone(),
            tolerance: None,
        });
    }

    // Effective tax rate plausibility (informational only).
    if let (Some(tax), Some(pretax)) = (&mapping.tax_expense_node, &mapping.pretax_income_node) {
        builder = builder.add_check(EffectiveTaxRateCheck {
            tax_expense_node: tax.clone(),
            pretax_income_node: pretax.clone(),
            expected_range: ETR_PLAUSIBLE_RANGE,
        });
    }

    // Growth-rate plausibility across every mapped node.
    builder = builder.add_check(GrowthRateConsistency {
        nodes: mapping.all_nodes(),
        max_period_growth_pct: MAX_PERIOD_GROWTH,
        max_decline_pct: MAX_PERIOD_DECLINE,
        scope: PeriodScope::AllPeriods,
    });

    // Data quality: NaN/Inf detection.
    builder = builder.add_check(NonFiniteCheck {
        nodes: mapping.all_nodes(),
    });

    // Data quality: required nodes present.
    let required = vec![
        mapping.cash_node,
        mapping.retained_earnings_node,
        mapping.net_income_node,
    ];
    builder = builder.add_check(MissingValueCheck {
        required_nodes: required,
        scope: PeriodScope::AllPeriods,
    });

    builder.build()
}

// Credit underwriting suite

/// Build a check suite for credit underwriting analysis.
///
/// Includes:
/// - **NonFiniteCheck** over the credit ratio inputs (always)
/// - **LeverageRangeCheck** (always)
/// - **CoverageFloorCheck** (always)
/// - **FcfSignCheck** (if `fcf_node` is provided)
/// - **LiquidityRunwayCheck** (if `cash_node` and `cash_burn_node` are both
///   provided)
/// - **TrendCheck** on leverage (decreasing is good) and coverage
///   (increasing is good), both with 3-period lookback
///
/// # Arguments
///
/// * `mapping` - Credit-analysis node mapping that identifies debt, EBITDA,
///   interest, and optional free-cash-flow inputs plus warning thresholds.
pub fn credit_underwriting_checks(mapping: CreditMapping) -> CheckSuite {
    let warn = mapping.leverage_warn.unwrap_or((0.0, 6.0));
    let cov_warn = mapping.coverage_min_warn.unwrap_or(1.5);

    let mut builder = CheckSuite::builder("Credit Underwriting Checks")
        .description("Leverage, coverage, cash-flow, and trend checks for credit analysis");

    // Data quality: NaN/Inf detection on credit ratio inputs. A divide-by-zero
    // in a covenant ratio (e.g. EBITDA / 0 interest) yields NaN that would
    // otherwise pass silently through leverage and coverage checks.
    let mut credit_nodes = vec![
        mapping.debt_node.clone(),
        mapping.ebitda_node.clone(),
        mapping.interest_expense_node.clone(),
    ];
    if let Some(ref fcf_node) = mapping.fcf_node {
        credit_nodes.push(fcf_node.clone());
    }
    builder = builder.add_check(NonFiniteCheck {
        nodes: credit_nodes,
    });

    builder = builder.add_check(LeverageRangeCheck {
        debt_node: mapping.debt_node.clone(),
        ebitda_node: mapping.ebitda_node.clone(),
        warn_range: warn,
        error_range: (0.0, 10.0),
    });

    builder = builder.add_check(CoverageFloorCheck {
        numerator_node: mapping.ebitda_node.clone(),
        denominator_node: mapping.interest_expense_node.clone(),
        min_warning: cov_warn,
        min_error: 1.0,
    });

    if let Some(ref fcf_node) = mapping.fcf_node {
        builder = builder.add_check(FcfSignCheck {
            fcf_node: fcf_node.clone(),
            consecutive_negative_warning: 2,
            consecutive_negative_error: 4,
        });
    }

    // Liquidity runway (requires both a cash balance and a burn rate).
    if let (Some(cash), Some(burn)) = (&mapping.cash_node, &mapping.cash_burn_node) {
        builder = builder.add_check(LiquidityRunwayCheck {
            cash_node: cash.clone(),
            cash_burn_node: burn.clone(),
            min_months_warning: RUNWAY_WARN_MONTHS,
            min_months_error: RUNWAY_ERROR_MONTHS,
        });
    }

    // Trend checks — use EBITDA as a proxy for coverage direction and
    // debt/EBITDA leverage direction.
    builder = builder.add_check(TrendCheck {
        node: mapping.ebitda_node.clone(),
        direction: TrendDirection::IncreasingIsGood,
        lookback_periods: 3,
        severity: Severity::Warning,
    });

    builder = builder.add_check(TrendCheck {
        node: mapping.debt_node,
        direction: TrendDirection::DecreasingIsGood,
        lookback_periods: 3,
        severity: Severity::Warning,
    });

    builder.build()
}

// LBO suite

/// Build a combined check suite for an LBO model.
///
/// Merges [`three_statement_checks`] and [`credit_underwriting_checks`],
/// overriding the leverage warning range to `(0.0, 8.0)` to accommodate
/// the higher leverage typical in buyouts.
///
/// # Arguments
///
/// * `mapping` - Three-statement node mapping used to construct structural and
///   data-quality checks.
/// * `credit` - Credit-underwriting node mapping; its leverage warning range
///   is replaced with the LBO-specific range.
pub fn lbo_model_checks(mapping: ThreeStatementMapping, credit: CreditMapping) -> CheckSuite {
    let ts_suite = three_statement_checks(mapping);

    let credit_override = CreditMapping {
        leverage_warn: Some((0.0, 8.0)),
        ..credit
    };
    let credit_suite = credit_underwriting_checks(credit_override);

    ts_suite.merge(credit_suite)
}
