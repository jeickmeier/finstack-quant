//! Credit context metrics — coverage ratios derived from statement + capital structure data.

use std::collections::HashMap;

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Period, PeriodId};
use finstack_quant_statements::capital_structure::CapitalStructureCashflows;
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::types::NodeValueType;
use finstack_quant_statements::{Error, Result};
use serde::{Deserialize, Serialize};

/// Per-instrument credit context metrics derived from statement data.
///
/// Ratios are plain scalars: `2.0` means `2.0x` coverage and `0.40` means
/// `40%` loan-to-value.
///
/// DSCR is reported in three flavours, all using cash flow available for debt
/// service (`cfads_node`) rather than EBITDA:
///
/// - [`CreditContextMetrics::dscr`] / [`CreditContextMetrics::dscr_min`]:
///   CFADS divided by cash interest plus principal.
/// - [`CreditContextMetrics::dscr_total`] /
///   [`CreditContextMetrics::dscr_total_min`]: CFADS divided by total interest
///   (cash plus PIK) and principal.
/// - [`CreditContextMetrics::dscr_incl_fees`] /
///   [`CreditContextMetrics::dscr_incl_fees_min`]: CFADS divided by cash
///   interest, principal, and fees.
///
/// Interest coverage uses a separate `interest_coverage_node`, normally EBITDA
/// or EBIT, so changing the DSCR numerator cannot silently redefine interest
/// coverage.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreditContextMetrics {
    /// Cash DSCR by period: `CFADS / (interest_cash + principal)`.
    pub dscr: Vec<(PeriodId, f64)>,
    /// Total DSCR by period: `CFADS / (interest_total + principal)`.
    pub dscr_total: Vec<(PeriodId, f64)>,
    /// Fee-inclusive cash DSCR by period:
    /// `CFADS / (interest_cash + principal + fees)`.
    pub dscr_incl_fees: Vec<(PeriodId, f64)>,
    /// Interest coverage by period:
    /// `interest_coverage_node_value / interest_expense_total`.
    pub interest_coverage: Vec<(PeriodId, f64)>,
    /// LTV by period: `debt_balance[t] / reference_value[t]`.
    ///
    /// A scalar DCF enterprise value is broadcast as the same denominator
    /// on every requested period (current valuation versus forward debt,
    /// not a rolled enterprise-value path). A per-period statement node
    /// supplies a varying value path; a missing period omits LTV for that
    /// period only.
    pub ltv: Vec<(PeriodId, f64)>,
    /// Minimum cash DSCR across all periods.
    pub dscr_min: Option<f64>,
    /// Minimum total DSCR across all periods.
    pub dscr_total_min: Option<f64>,
    /// Minimum fee-inclusive cash DSCR across all periods.
    pub dscr_incl_fees_min: Option<f64>,
    /// Minimum interest coverage across all periods.
    pub interest_coverage_min: Option<f64>,
    /// Periods requested but excluded from one or more coverage series.
    ///
    /// Missing cashflows, missing numerator values, and non-positive
    /// denominators exclude a period. Non-finite and cross-currency inputs are
    /// errors rather than skipped observations.
    #[serde(default)]
    pub skipped_periods: Vec<PeriodId>,
}

/// Statement nodes used as credit-ratio numerators.
#[derive(Debug, Clone, Copy)]
pub struct CreditNumeratorNodes<'a> {
    /// Cash flow available for debt service, used for every DSCR variant.
    pub cfads: &'a str,
    /// EBITDA, EBIT, or another earnings measure used for interest coverage.
    pub interest_coverage: &'a str,
}

/// Compute credit context metrics for one instrument.
///
/// DSCR uses a cash-flow numerator while interest coverage uses a separate
/// earnings numerator. All monetary inputs must already share
/// `reporting_currency`; this function rejects rather than implicitly converts
/// a mixed-currency ratio.
///
/// # Arguments
///
/// * `statement` - Evaluated statement results containing both numerator nodes
/// * `cs_cashflows` - Capital-structure cashflows from statement evaluation
/// * `instrument_id` - Instrument whose debt service and balance are measured
/// * `numerators` - Separate CFADS and interest-coverage statement node ids
/// * `reporting_currency` - Currency shared by statement amounts, LTV
///   reference values, and instrument cashflows
/// * `periods` - Ordered reporting periods over which metrics are calculated
/// * `reference_values` - Optional per-period LTV denominators in
///   `reporting_currency`; missing or non-positive entries omit LTV only
///
/// # Returns
///
/// Returns the requested coverage and leverage series. An absent instrument
/// returns an empty [`CreditContextMetrics`].
///
/// # Errors
///
/// Returns an error when either numerator node or instrument cashflows use a
/// currency different from `reporting_currency`, or any consumed value is
/// non-finite.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::{Period, PeriodId};
/// use finstack_quant_core::money::Money;
/// use finstack_quant_statements::capital_structure::{
///     CapitalStructureCashflows, CashflowBreakdown,
/// };
/// use finstack_quant_statements::evaluator::StatementResult;
/// use finstack_quant_statements_analytics::analysis::{
///     compute_credit_context, CreditNumeratorNodes,
/// };
/// use indexmap::IndexMap;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let period = Period {
///     id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
///     start: time::macros::date!(2025 - 01 - 01),
///     end: time::macros::date!(2025 - 04 - 01),
///     is_actual: false,
/// };
/// let mut statement = StatementResult::new();
/// statement.nodes.insert(
///     "cfads".to_string(),
///     IndexMap::from([(period.id, 250_000.0)]),
/// );
/// statement.nodes.insert(
///     "ebitda".to_string(),
///     IndexMap::from([(period.id, 300_000.0)]),
/// );
/// let mut cashflows = CapitalStructureCashflows::new();
/// cashflows.by_instrument.insert(
///     "TLB".to_string(),
///     IndexMap::from([(
///         period.id,
///         CashflowBreakdown {
///             interest_expense_cash: Money::from((50_000_i64, Currency::USD)),
///             interest_income_cash: None,
///             interest_expense_pik: Money::from((0_i64, Currency::USD)),
///             principal_payment: Money::from((100_000_i64, Currency::USD)),
///             fees: Money::from((0_i64, Currency::USD)),
///             debt_balance: Money::from((4_000_000_i64, Currency::USD)),
///             accrued_interest: Money::from((0_i64, Currency::USD)),
///         },
///     )]),
/// );
///
/// let metrics = compute_credit_context(
///     &statement,
///     &cashflows,
///     "TLB",
///     CreditNumeratorNodes {
///         cfads: "cfads",
///         interest_coverage: "ebitda",
///     },
///     Currency::USD,
///     std::slice::from_ref(&period),
///     Some(&[(period.id, 10_000_000.0)]),
/// )?;
/// assert_eq!(metrics.dscr.len(), 1);
/// assert_eq!(metrics.interest_coverage.len(), 1);
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// - Coverage and leverage interpretation:
///   `docs/REFERENCES.md#tuckman-serrat-fixed-income`
pub fn compute_credit_context(
    statement: &StatementResult,
    cs_cashflows: &CapitalStructureCashflows,
    instrument_id: &str,
    numerators: CreditNumeratorNodes<'_>,
    reporting_currency: Currency,
    periods: &[Period],
    reference_values: Option<&[(PeriodId, f64)]>,
) -> Result<CreditContextMetrics> {
    let Some(inst_data) = cs_cashflows.by_instrument.get(instrument_id) else {
        return Ok(CreditContextMetrics::default());
    };

    let reference_by_period: HashMap<PeriodId, f64> =
        reference_values.unwrap_or(&[]).iter().copied().collect();
    for node_id in [numerators.cfads, numerators.interest_coverage] {
        if let Some(NodeValueType::Monetary { currency }) = statement.node_value_types.get(node_id)
        {
            if *currency != reporting_currency {
                return Err(Error::currency_mismatch(reporting_currency, *currency));
            }
        }
    }

    let mut dscr = Vec::new();
    let mut dscr_total = Vec::new();
    let mut dscr_incl_fees = Vec::new();
    let mut interest_coverage = Vec::new();
    let mut ltv = Vec::new();

    for period in periods {
        let Some(cf) = inst_data.get(&period.id) else {
            continue;
        };
        let instrument_currency = cf.interest_expense_cash.currency();
        if instrument_currency != reporting_currency {
            return Err(Error::currency_mismatch(
                reporting_currency,
                instrument_currency,
            ));
        }

        let interest_total = cf
            .interest_expense_total()
            .map_err(|error| Error::eval(error.to_string()))?
            .amount();
        let interest_cash = cf.interest_expense_cash.amount();
        let principal = cf.principal_payment.amount();
        let fees = cf.fees.amount();
        let balance = cf.debt_balance.amount();
        let debt_values = [interest_total, interest_cash, principal, fees, balance];
        if debt_values.iter().any(|value| !value.is_finite()) {
            return Err(Error::eval(format!(
                "Credit context for instrument '{instrument_id}' contains a non-finite \
                 cashflow value in period {}",
                period.id
            )));
        }

        if let Some(&reference_value) = reference_by_period.get(&period.id) {
            if !reference_value.is_finite() {
                return Err(Error::eval(format!(
                    "Credit context LTV reference for instrument '{instrument_id}' is \
                     non-finite in period {}",
                    period.id
                )));
            }
            if reference_value > 0.0 {
                ltv.push((period.id, balance / reference_value));
            }
        }

        if let Some(cfads) = statement.get(numerators.cfads, &period.id) {
            if !cfads.is_finite() {
                return Err(Error::eval(format!(
                    "Credit context CFADS node '{}' is non-finite in period {}",
                    numerators.cfads, period.id
                )));
            }
            let debt_service_cash = interest_cash + principal;
            if debt_service_cash > 0.0 {
                dscr.push((period.id, cfads / debt_service_cash));
            }
            let debt_service_total = interest_total + principal;
            if debt_service_total > 0.0 {
                dscr_total.push((period.id, cfads / debt_service_total));
            }
            let debt_service_incl_fees = interest_cash + principal + fees;
            if debt_service_incl_fees > 0.0 {
                dscr_incl_fees.push((period.id, cfads / debt_service_incl_fees));
            }
        }

        if let Some(numerator) = statement.get(numerators.interest_coverage, &period.id) {
            if !numerator.is_finite() {
                return Err(Error::eval(format!(
                    "Credit context interest-coverage node '{}' is non-finite in period {}",
                    numerators.interest_coverage, period.id
                )));
            }
            if interest_total > 0.0 {
                interest_coverage.push((period.id, numerator / interest_total));
            }
        }
    }

    let dscr_min = dscr.iter().map(|(_, v)| *v).reduce(f64::min);
    let dscr_total_min = dscr_total.iter().map(|(_, v)| *v).reduce(f64::min);
    let dscr_incl_fees_min = dscr_incl_fees.iter().map(|(_, v)| *v).reduce(f64::min);
    let interest_coverage_min = interest_coverage.iter().map(|(_, v)| *v).reduce(f64::min);

    // Surface coverage gaps: any requested period absent from at least one
    // of the coverage series was (partially) skipped and is excluded from
    // the min statistics above.
    let skipped_periods: Vec<PeriodId> = periods
        .iter()
        .map(|p| p.id)
        .filter(|pid| {
            !(dscr.iter().any(|(p, _)| p == pid)
                && dscr_total.iter().any(|(p, _)| p == pid)
                && dscr_incl_fees.iter().any(|(p, _)| p == pid)
                && interest_coverage.iter().any(|(p, _)| p == pid))
        })
        .collect();
    if !skipped_periods.is_empty() {
        tracing::warn!(
            instrument_id,
            skipped = skipped_periods.len(),
            requested = periods.len(),
            "credit context: some periods excluded from coverage metrics",
        );
    }

    Ok(CreditContextMetrics {
        dscr,
        dscr_total,
        dscr_incl_fees,
        interest_coverage,
        ltv,
        dscr_min,
        dscr_total_min,
        dscr_incl_fees_min,
        interest_coverage_min,
        skipped_periods,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
    use finstack_quant_statements::capital_structure::CashflowBreakdown;
    use indexmap::IndexMap;

    fn make_result_and_cs() -> (StatementResult, CapitalStructureCashflows, Vec<Period>) {
        let mut result = StatementResult::new();
        let periods = vec![
            Period {
                id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
                start: time::macros::date!(2025 - 01 - 01),
                end: time::macros::date!(2025 - 04 - 01),
                is_actual: false,
            },
            Period {
                id: PeriodId::quarter(2025, 2).expect("valid period fixture"),
                start: time::macros::date!(2025 - 04 - 01),
                end: time::macros::date!(2025 - 07 - 01),
                is_actual: false,
            },
        ];

        // EBITDA = 500k per quarter
        let mut ebitda_map = IndexMap::new();
        ebitda_map.insert(
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            500_000.0,
        );
        ebitda_map.insert(
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
            500_000.0,
        );
        result.nodes.insert("ebitda".to_string(), ebitda_map);

        // CFADS = 450k per quarter, distinct from EBITDA.
        let mut cfads_map = IndexMap::new();
        cfads_map.insert(
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            450_000.0,
        );
        cfads_map.insert(
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
            450_000.0,
        );
        result.nodes.insert("cfads".to_string(), cfads_map);

        // CS cashflows: Bond with 50k interest, 100k principal per period
        let mut cs = CapitalStructureCashflows::new();
        let mut inst_map = IndexMap::new();
        for p in &periods {
            inst_map.insert(
                p.id,
                CashflowBreakdown {
                    interest_expense_cash: Money::from((50_000_i64, Currency::USD)),
                    interest_income_cash: None,
                    interest_expense_pik: Money::from((0_i64, Currency::USD)),
                    principal_payment: Money::from((100_000_i64, Currency::USD)),
                    fees: Money::from((0_i64, Currency::USD)),
                    debt_balance: Money::from((4_000_000_i64, Currency::USD)),
                    accrued_interest: Money::from((0_i64, Currency::USD)),
                },
            );
        }
        cs.by_instrument.insert("BOND-001".to_string(), inst_map);
        (result, cs, periods)
    }

    fn constant_refs(periods: &[Period], value: f64) -> Vec<(PeriodId, f64)> {
        periods.iter().map(|p| (p.id, value)).collect()
    }

    fn metrics(
        result: &StatementResult,
        cs: &CapitalStructureCashflows,
        instrument_id: &str,
        periods: &[Period],
        reference_values: Option<&[(PeriodId, f64)]>,
    ) -> CreditContextMetrics {
        compute_credit_context(
            result,
            cs,
            instrument_id,
            CreditNumeratorNodes {
                cfads: "cfads",
                interest_coverage: "ebitda",
            },
            Currency::USD,
            periods,
            reference_values,
        )
        .expect("same-currency finite credit inputs")
    }

    #[test]
    fn test_dscr_computed_correctly() {
        let (result, mut cs, periods) = make_result_and_cs();
        for cf in cs
            .by_instrument
            .get_mut("BOND-001")
            .expect("instrument")
            .values_mut()
        {
            cf.fees = Money::from((25_000_i64, Currency::USD));
        }
        let metrics = metrics(&result, &cs, "BOND-001", &periods, None);

        // DSCR = 450k / (50k + 100k) = 3.0x
        assert_eq!(metrics.dscr.len(), 2);
        assert!((metrics.dscr[0].1 - 3.0).abs() < 0.01);
        assert_eq!(metrics.dscr_min, Some(3.0));

        // Fee-inclusive DSCR = 450k / (50k + 100k + 25k) ≈ 2.571x
        let expected_incl_fees = 450_000.0 / 175_000.0;
        assert_eq!(metrics.dscr_incl_fees.len(), 2);
        assert!((metrics.dscr_incl_fees[0].1 - expected_incl_fees).abs() < 0.01);
        assert!(
            (metrics
                .dscr_incl_fees_min
                .expect("dscr_incl_fees_min should be set")
                - expected_incl_fees)
                .abs()
                < 0.01
        );
    }

    #[test]
    fn test_interest_coverage_computed_correctly() {
        let (result, cs, periods) = make_result_and_cs();
        let metrics = metrics(&result, &cs, "BOND-001", &periods, None);

        // Interest coverage = 500k / 50k = 10x
        assert_eq!(metrics.interest_coverage.len(), 2);
        assert!((metrics.interest_coverage[0].1 - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_ltv_computed_when_reference_value_provided() {
        let (result, cs, periods) = make_result_and_cs();
        let refs = constant_refs(&periods, 10_000_000.0);
        let metrics = metrics(&result, &cs, "BOND-001", &periods, Some(refs.as_slice()));

        // LTV = 4M / 10M = 0.4
        assert_eq!(metrics.ltv.len(), 2);
        assert!((metrics.ltv[0].1 - 0.4).abs() < 0.01);
    }

    #[test]
    fn test_ltv_path_varies_with_debt() {
        let (result, mut cs, periods) = make_result_and_cs();
        let inst = cs.by_instrument.get_mut("BOND-001").expect("instrument");
        inst.get_mut(&periods[0].id).expect("q1").debt_balance =
            Money::from((4_000_000_i64, Currency::USD));
        inst.get_mut(&periods[1].id).expect("q2").debt_balance =
            Money::from((3_000_000_i64, Currency::USD));

        let refs = constant_refs(&periods, 10_000_000.0);
        let metrics = metrics(&result, &cs, "BOND-001", &periods, Some(refs.as_slice()));

        assert_eq!(metrics.ltv.len(), 2);
        assert!((metrics.ltv[0].1 - 0.4).abs() < 0.01);
        assert!((metrics.ltv[1].1 - 0.3).abs() < 0.01);
    }

    #[test]
    fn test_ltv_skips_period_missing_from_reference_path() {
        let (result, cs, periods) = make_result_and_cs();
        let refs = vec![(periods[0].id, 10_000_000.0)];
        let metrics = metrics(&result, &cs, "BOND-001", &periods, Some(refs.as_slice()));

        assert_eq!(metrics.ltv.len(), 1);
        assert_eq!(metrics.ltv[0].0, periods[0].id);
        assert!((metrics.ltv[0].1 - 0.4).abs() < 0.01);
    }

    #[test]
    fn test_missing_instrument_returns_empty() {
        let (result, cs, periods) = make_result_and_cs();
        let metrics = metrics(&result, &cs, "NONEXISTENT", &periods, None);
        assert!(metrics.dscr.is_empty());
        assert!(metrics.dscr_incl_fees.is_empty());
        assert!(metrics.interest_coverage.is_empty());
        assert!(metrics.dscr_min.is_none());
        assert!(metrics.dscr_incl_fees_min.is_none());
    }

    #[test]
    fn test_missing_coverage_period_is_skipped_not_treated_as_zero() {
        let (mut result, cs, periods) = make_result_and_cs();
        if let Some(ebitda) = result.nodes.get_mut("ebitda") {
            ebitda.shift_remove(&PeriodId::quarter(2025, 2).expect("valid period fixture"));
        }
        if let Some(cfads) = result.nodes.get_mut("cfads") {
            cfads.shift_remove(&PeriodId::quarter(2025, 2).expect("valid period fixture"));
        }

        let metrics = metrics(&result, &cs, "BOND-001", &periods, None);

        assert_eq!(metrics.dscr.len(), 1);
        assert_eq!(metrics.interest_coverage.len(), 1);
        assert_eq!(
            metrics.dscr[0].0,
            PeriodId::quarter(2025, 1).expect("valid period fixture")
        );
        assert!(
            (metrics.dscr_min.expect("dscr_min should be set") - metrics.dscr[0].1).abs() < 1e-12
        );
        // The dropped period must be surfaced rather than silently omitted.
        assert_eq!(
            metrics.skipped_periods,
            vec![PeriodId::quarter(2025, 2).expect("valid period fixture")]
        );
    }

    #[test]
    fn test_full_coverage_reports_no_skipped_periods() {
        let (result, cs, periods) = make_result_and_cs();
        let metrics = metrics(&result, &cs, "BOND-001", &periods, None);
        assert!(metrics.skipped_periods.is_empty());
    }

    #[test]
    fn test_missing_coverage_period_still_computes_ltv() {
        let (mut result, cs, periods) = make_result_and_cs();
        if let Some(ebitda) = result.nodes.get_mut("ebitda") {
            ebitda.shift_remove(&PeriodId::quarter(2025, 2).expect("valid period fixture"));
        }
        if let Some(cfads) = result.nodes.get_mut("cfads") {
            cfads.shift_remove(&PeriodId::quarter(2025, 2).expect("valid period fixture"));
        }

        let refs = constant_refs(&periods, 10_000_000.0);
        let metrics = metrics(&result, &cs, "BOND-001", &periods, Some(refs.as_slice()));

        assert_eq!(metrics.ltv.len(), 2);
        assert_eq!(
            metrics.ltv[0].0,
            PeriodId::quarter(2025, 1).expect("valid period fixture")
        );
        assert_eq!(
            metrics.ltv[1].0,
            PeriodId::quarter(2025, 2).expect("valid period fixture")
        );
        assert!((metrics.ltv[0].1 - 0.4).abs() < 0.01);
        assert!((metrics.ltv[1].1 - 0.4).abs() < 0.01);
    }

    #[test]
    fn test_cross_currency_instrument_is_rejected() {
        let (result, mut cs, periods) = make_result_and_cs();
        let instrument = cs.by_instrument.get_mut("BOND-001").expect("instrument");
        for cashflow in instrument.values_mut() {
            *cashflow = CashflowBreakdown::with_currency(Currency::EUR);
        }

        let error = compute_credit_context(
            &result,
            &cs,
            "BOND-001",
            CreditNumeratorNodes {
                cfads: "cfads",
                interest_coverage: "ebitda",
            },
            Currency::USD,
            &periods,
            None,
        )
        .expect_err("mixed-currency ratios must fail");
        assert!(error.to_string().contains("Currency mismatch"));
    }

    #[test]
    fn test_non_finite_numerator_is_rejected() {
        let (mut result, cs, periods) = make_result_and_cs();
        result
            .nodes
            .get_mut("cfads")
            .expect("cfads")
            .insert(periods[0].id, f64::NAN);

        let error = compute_credit_context(
            &result,
            &cs,
            "BOND-001",
            CreditNumeratorNodes {
                cfads: "cfads",
                interest_coverage: "ebitda",
            },
            Currency::USD,
            &periods,
            None,
        )
        .expect_err("non-finite ratios must fail");
        assert!(error.to_string().contains("non-finite"));
    }
}
