//! Credit context metrics — coverage ratios derived from statement + capital structure data.

use std::collections::HashMap;

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Period, PeriodId};
use finstack_quant_core::money::Money;
use finstack_quant_statements::capital_structure::CapitalStructureCashflows;
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::analysis::units::{finite_ratio, require_currency};

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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
    /// LTV by period: current debt face divided by the selected same-date collateral value.
    /// A missing value leaves LTV unavailable for that period.
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
/// non-finite. Contradictory numeric/monetary numerator projections and
/// non-finite denominator sums or computed ratios also fail.
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
/// statement.set_series("cfads", IndexMap::from([(period.id, 250_000.0)]))?;
/// statement.set_series("ebitda", IndexMap::from([(period.id, 300_000.0)]))?;
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
///             opening_balance: None,
///             scheduled_principal: None,
///             mandatory_prepayment: None,
///             sweep_prepayment: None,
///             voluntary_prepayment: None,
///             interest_arrears: None,
///             principal_pending: None,
///             principal_arrears: None,
///             principal_advance_reserve: None,
///             scheduled_cash_interest_due: None,
///             scheduled_principal_due: None,
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

    let mut dscr = Vec::new();
    let mut dscr_total = Vec::new();
    let mut dscr_incl_fees = Vec::new();
    let mut interest_coverage = Vec::new();
    let mut ltv = Vec::new();
    let mut skipped_periods = Vec::new();

    for period in periods {
        for node in [numerators.cfads, numerators.interest_coverage] {
            require_currency(statement, node, &period.id, reporting_currency)?;
        }
        let Some(cf) = inst_data.get(&period.id) else {
            skipped_periods.push(period.id);
            continue;
        };
        cf.validate_currency_invariant()?;
        let instrument_currency = cf.interest_expense_cash.currency();
        if instrument_currency != reporting_currency {
            return Err(Error::CurrencyMismatch(
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
        let ratio = |numerator: f64, denominator: f64| -> Result<Option<f64>> {
            if !denominator.is_finite() {
                return Err(Error::eval(format!(
                    "Credit denominator for instrument '{instrument_id}' is non-finite in period {}",
                    period.id
                )));
            }
            if denominator <= 0.0 {
                return Ok(None);
            }
            finite_ratio(numerator, denominator).map(Some)
        };

        if let Some(&reference_value) = reference_by_period.get(&period.id) {
            if !reference_value.is_finite() {
                return Err(Error::eval(format!(
                    "Credit context LTV reference for instrument '{instrument_id}' is \
                     non-finite in period {}",
                    period.id
                )));
            }
            if let Some(value) = ratio(balance, reference_value)? {
                ltv.push((period.id, value));
            }
        }

        let coverage_counts = (
            dscr.len(),
            dscr_total.len(),
            dscr_incl_fees.len(),
            interest_coverage.len(),
        );
        if let Some(cfads) = crate::cell_float(statement, numerators.cfads, &period.id) {
            if !cfads.is_finite() {
                return Err(Error::eval(format!(
                    "Credit context CFADS node '{}' is non-finite in period {}",
                    numerators.cfads, period.id
                )));
            }
            let debt_service_cash = interest_cash + principal;
            if let Some(value) = ratio(cfads, debt_service_cash)? {
                dscr.push((period.id, value));
            }
            let debt_service_total = interest_total + principal;
            if let Some(value) = ratio(cfads, debt_service_total)? {
                dscr_total.push((period.id, value));
            }
            let debt_service_incl_fees = interest_cash + principal + fees;
            if let Some(value) = ratio(cfads, debt_service_incl_fees)? {
                dscr_incl_fees.push((period.id, value));
            }
        }

        if let Some(numerator) =
            crate::cell_float(statement, numerators.interest_coverage, &period.id)
        {
            if !numerator.is_finite() {
                return Err(Error::eval(format!(
                    "Credit context interest-coverage node '{}' is non-finite in period {}",
                    numerators.interest_coverage, period.id
                )));
            }
            if let Some(value) = ratio(numerator, interest_total)? {
                interest_coverage.push((period.id, value));
            }
        }
        if coverage_counts.0 == dscr.len()
            || coverage_counts.1 == dscr_total.len()
            || coverage_counts.2 == dscr_incl_fees.len()
            || coverage_counts.3 == interest_coverage.len()
        {
            skipped_periods.push(period.id);
        }
    }

    let dscr_min = dscr.iter().map(|(_, v)| *v).reduce(f64::min);
    let dscr_total_min = dscr_total.iter().map(|(_, v)| *v).reduce(f64::min);
    let dscr_incl_fees_min = dscr_incl_fees.iter().map(|(_, v)| *v).reduce(f64::min);
    let interest_coverage_min = interest_coverage.iter().map(|(_, v)| *v).reduce(f64::min);

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

/// The measure to calculate for an identified borrowing and reporting period.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CreditMeasure {
    /// Cash flow available for debt service divided by scheduled cash interest and principal due.
    CashDscr,
    /// Cash flow available for debt service divided by scheduled debt service and recognized PIK.
    CashAndPikDscr,
    /// Cash flow available for debt service divided by scheduled debt service and selected cash fees.
    CashDscrWithFees,
    /// Elected earnings divided by separately evidenced recognized interest including PIK.
    InterestCoverage,
    /// Closing economic face divided by a matched positive collateral valuation.
    LoanToValue,
    /// Closing economic face divided by an explicitly elected enterprise valuation at the same cut.
    DebtToEnterpriseValue,
}

/// Identified collateral or enterprise value matched to a particular closing date and debt scope.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreditReferenceValue {
    /// Basis supported by the evidence. Use LoanToValue for collateral and DebtToEnterpriseValue for enterprise value.
    pub measure: CreditMeasure,
    /// Reported calendar close matched to an exact model period.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: finstack_quant_core::dates::Date,
    /// Positive value of the specified collateral or enterprise in reporting currency.
    pub amount: finstack_quant_core::money::Money,
    /// Identifier of the underlying valuation and its direct scope evidence.
    pub source_id: String,
}

/// Identified statement-run input and borrower measures.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreditContextSpec {
    /// Exact borrowing identifier carried by the verified capital result.
    pub borrowing_id: String,
    /// Expected statement and capital run identity.
    pub run_id: String,
    /// Reporting currency for all financial operands.
    pub currency: Currency,
    /// Explicit unique reporting periods from that statement run.
    pub periods: Vec<PeriodId>,
    /// Explicit nonempty list of distinct measures in reporting order.
    pub measures: Vec<CreditMeasure>,
    /// Same-currency cash-flow numerator for every requested DSCR.
    pub cfads_node: Option<String>,
    /// Same-currency earnings numerator used for interest coverage.
    pub earnings_node: Option<String>,
    /// Verified recognized interest including PIK on the same borrowing and period basis.
    pub recognized_interest_node: Option<String>,
    /// Named cash-fee amount required by the fee-inclusive measure.
    pub cash_fees_node: Option<String>,
    /// Explicit borrowing-scoped valuations, at most one date for each elected reference basis.
    pub reference_values: Vec<CreditReferenceValue>,
}

/// Typed result for one measure and period, retaining reasons for missing or zero debt service.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum CreditMeasureValue {
    /// Complete monetary operands and calculated ratio.
    Measured {
        /// Signed numerator in reporting currency.
        numerator: finstack_quant_core::money::Money,
        /// Positive denominator in reporting currency.
        denominator: finstack_quant_core::money::Money,
        /// Unitless ratio, where 1.5 represents 1.5x or 150%.
        ratio: f64,
    },
    /// The specifically measured denominator was exactly zero.
    Inapplicable,
    /// One or more required inputs were unavailable.
    Unavailable {
        /// Specific input or exposure admission evidence that is missing.
        reason: String,
    },
}

/// Per-period calculation evidence for one of the elected credit measures.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreditMeasureRow {
    /// Elected reporting period.
    pub period: PeriodId,
    /// Contractual reporting date that closes the period.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: finstack_quant_core::dates::Date,
    /// Elected measure for this row.
    pub measure: CreditMeasure,
    /// Source valuation identifier for a collateral or enterprise denominator.
    pub reference_source_id: Option<String>,
    /// Ratio, absence or explicit inapplicability.
    pub value: CreditMeasureValue,
}

/// Coverage summary for one elected measure.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreditMeasureCoverage {
    /// Elected measure summarized.
    pub measure: CreditMeasure,
    /// Number of measured periods.
    #[cfg_attr(feature = "json-schema", schemars(with = "u32"))]
    pub measured: usize,
    /// Number of verified periods where the measure had zero debt service.
    #[cfg_attr(feature = "json-schema", schemars(with = "u32"))]
    pub inapplicable: usize,
    /// Number of requested periods where applicable coverage could not be verified.
    #[cfg_attr(feature = "json-schema", schemars(with = "u32"))]
    pub unavailable: usize,
    /// Minimum among the available observed periods.
    pub observed_minimum: Option<f64>,
    /// Minimum of the eligible path, present only when every applicable period is measured.
    pub full_path_minimum: Option<f64>,
}

/// Dated credit measurements and coverage for an identified borrowing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreditContextReport {
    /// Borrowing whose obligations and balance were measured.
    pub borrowing_id: String,
    /// Verified statement run that produced both cash flows and period metadata.
    pub run: finstack_quant_statements::evaluator::RunProvenance,
    /// Same currency for all reported monetary operands.
    pub currency: Currency,
    /// All elected period and measure pairs, including unavailable rows.
    pub rows: Vec<CreditMeasureRow>,
    /// Coverage counts and minima in requested measure order.
    pub coverage: Vec<CreditMeasureCoverage>,
}

fn absent(reason: impl Into<String>) -> CreditMeasureValue {
    CreditMeasureValue::Unavailable {
        reason: reason.into(),
    }
}

fn credit_money(
    results: &StatementResult,
    node: Option<&str>,
    period: &PeriodId,
    currency: Currency,
) -> Result<Option<Money>> {
    let Some(node) = node else {
        return Err(Error::invalid_input(
            "An explicitly selected credit measure requires a named monetary node",
        ));
    };
    if !matches!(
        results.node_value_types.get(node),
        Some(finstack_quant_statements::types::NodeValueType::Monetary { .. })
    ) {
        return Err(Error::invalid_input(format!(
            "Credit measure node '{node}' must be identified monetary data"
        )));
    }
    require_currency(results, node, period, currency)?;
    Ok(results.get_money(node, period))
}

/// Measure explicitly selected credit ratios over verified statement periods.
///
/// # Arguments
/// * `results` - Point-in-time statements containing the selected borrowing's capital cashflows.
/// * `spec` - Borrowing and run IDs, reporting currency, requested periods and ratio sources.
///
/// # Errors
/// Rejects conflicting run identity, invalid scopes, duplicate periods and
/// measures, mismatched units, and invalid values or ratio operands. Missing
/// observations on otherwise admitted rows are reported as unavailable.
pub fn evaluate_credit_context(
    results: &StatementResult,
    spec: &CreditContextSpec,
) -> Result<CreditContextReport> {
    use std::collections::{BTreeMap, BTreeSet};

    let run = results
        .provenance
        .as_ref()
        .filter(|run| {
            run.run_id == spec.run_id
                && run.source_snapshot.is_some()
                && matches!(
                    run.observation_cutoff,
                    finstack_quant_statements::evaluator::ObservationCutoff::Through(_)
                )
        })
        .ok_or_else(|| {
            Error::invalid_input(
                "Credit context requires its identified point-in-time statement run",
            )
        })?;
    let cashflows = results
        .cs_cashflows
        .as_ref()
        .and_then(|book| book.by_instrument.get(&spec.borrowing_id))
        .ok_or_else(|| {
            Error::invalid_input(format!(
                "Borrowing '{}' is absent from the admitted capital structure",
                spec.borrowing_id
            ))
        })?;
    let distinct_periods: BTreeSet<_> = spec.periods.iter().copied().collect();
    let distinct_measures: BTreeSet<_> = spec.measures.iter().copied().collect();
    if spec.periods.is_empty()
        || distinct_periods.len() != spec.periods.len()
        || spec.measures.is_empty()
        || distinct_measures.len() != spec.measures.len()
    {
        return Err(Error::invalid_input(
            "Credit context requires nonempty, unique period and measure selections",
        ));
    }
    let dates = spec
        .periods
        .iter()
        .map(|id| {
            results
                .get_periods()
                .iter()
                .find(|period| period.id == *id)
                .and_then(|period| period.end.previous_day())
                .map(|date| (*id, date))
                .ok_or_else(|| {
                    Error::invalid_input(format!("Credit period {id} is not in the statement run"))
                })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut references = BTreeMap::new();
    for reference in &spec.reference_values {
        if reference.source_id.trim().is_empty()
            || reference.amount.currency() != spec.currency
            || reference.amount.amount_decimal() <= rust_decimal::Decimal::ZERO
            || !matches!(
                reference.measure,
                CreditMeasure::LoanToValue | CreditMeasure::DebtToEnterpriseValue
            )
            || !distinct_measures.contains(&reference.measure)
            || !dates.iter().any(|(_, date)| *date == reference.date)
            || references
                .insert((reference.measure, reference.date), reference)
                .is_some()
        {
            return Err(Error::invalid_input(
                "Credit reference valuation needs unique admitted scope, close, currency and positive value",
            ));
        }
    }
    let mut rows = Vec::new();
    let mut coverage = Vec::with_capacity(spec.measures.len());
    for &measure in &spec.measures {
        let mut measured = 0usize;
        let mut unavailable = 0usize;
        let mut inapplicable = 0usize;
        let mut observed_minimum: Option<f64> = None;
        for &(period, date) in &dates {
            let reference = references.get(&(measure, date));
            let value = match cashflows.get(&period) {
                Some(cashflow) => {
                    cashflow.validate_currency_invariant()?;
                    if cashflow.debt_balance.currency() != spec.currency {
                        return Err(Error::CurrencyMismatch(
                            spec.currency,
                            cashflow.debt_balance.currency(),
                        ));
                    }
                    measure_credit_row(
                        results,
                        spec,
                        &period,
                        cashflow,
                        measure,
                        reference.map(|entry| entry.amount),
                    )?
                }
                None => absent("selected borrowing has no period cashflow evidence"),
            };
            match value {
                CreditMeasureValue::Measured { ratio, .. } => {
                    measured += 1;
                    observed_minimum = Some(observed_minimum.map_or(ratio, |min| min.min(ratio)));
                }
                CreditMeasureValue::Inapplicable => inapplicable += 1,
                CreditMeasureValue::Unavailable { .. } => unavailable += 1,
            }
            rows.push(CreditMeasureRow {
                period,
                date,
                measure,
                reference_source_id: reference.map(|entry| entry.source_id.clone()),
                value,
            });
        }
        coverage.push(CreditMeasureCoverage {
            measure,
            measured,
            unavailable,
            inapplicable,
            observed_minimum,
            full_path_minimum: if unavailable == 0 {
                observed_minimum
            } else {
                None
            },
        });
    }
    Ok(CreditContextReport {
        borrowing_id: spec.borrowing_id.clone(),
        run: run.clone(),
        currency: spec.currency,
        rows,
        coverage,
    })
}

fn measure_credit_row(
    results: &StatementResult,
    spec: &CreditContextSpec,
    period: &PeriodId,
    cashflow: &finstack_quant_statements::capital_structure::CashflowBreakdown,
    measure: CreditMeasure,
    reference: Option<Money>,
) -> Result<CreditMeasureValue> {
    let amount = |node: Option<&str>| credit_money(results, node, period, spec.currency);
    let (numerator, denominator) = match measure {
        CreditMeasure::CashDscr
        | CreditMeasure::CashAndPikDscr
        | CreditMeasure::CashDscrWithFees => {
            let (Some(interest), Some(principal)) = (
                cashflow.scheduled_cash_interest_due,
                cashflow.scheduled_principal_due,
            ) else {
                return Ok(absent(
                    "contractual cash interest or principal due is unavailable",
                ));
            };
            let due = interest.checked_add(principal)?;
            let supplemental = match measure {
                CreditMeasure::CashAndPikDscr => Some(cashflow.interest_expense_pik),
                CreditMeasure::CashDscrWithFees => amount(spec.cash_fees_node.as_deref())?,
                _ => Some(Money::zero(spec.currency)),
            };
            (
                amount(spec.cfads_node.as_deref())?,
                supplemental
                    .map(|supplemental| due.checked_add(supplemental))
                    .transpose()?,
            )
        }
        CreditMeasure::InterestCoverage => (
            amount(spec.earnings_node.as_deref())?,
            amount(spec.recognized_interest_node.as_deref())?,
        ),
        CreditMeasure::LoanToValue | CreditMeasure::DebtToEnterpriseValue => {
            (Some(cashflow.debt_balance), reference)
        }
    };
    let (Some(numerator), Some(denominator)) = (numerator, denominator) else {
        return Ok(absent(
            "required credit ratio operand is unavailable on this close",
        ));
    };
    if denominator.amount_decimal() < rust_decimal::Decimal::ZERO {
        return Err(Error::invalid_input(
            "Credit measure denominator cannot be negative",
        ));
    }
    if denominator.amount_decimal().is_zero() {
        return Ok(CreditMeasureValue::Inapplicable);
    }
    let ratio = finite_ratio(numerator.amount(), denominator.amount())?;
    Ok(CreditMeasureValue::Measured {
        numerator,
        denominator,
        ratio,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
    use finstack_quant_statements::capital_structure::CashflowBreakdown;
    use indexmap::IndexMap;

    #[test]
    fn checked_credit_dscr_uses_scheduled_due_and_preserves_incomplete_path() {
        use finstack_quant_core::market_data::context::MarketContext;
        use finstack_quant_statements::builder::ModelBuilder;
        use finstack_quant_statements::capital_structure::CashflowBreakdown;
        use finstack_quant_statements::evaluator::{Evaluator, ObservationCutoff};
        use time::macros::date;

        let first = PeriodId::quarter(2025, 1).unwrap();
        let next = PeriodId::quarter(2025, 2).unwrap();
        let valuation = date!(2024 - 12 - 31);
        let usd = |x| Money::from((x, Currency::USD));
        let mut builder = ModelBuilder::new("borrower");
        builder.periods("2025Q1..Q2", None).unwrap();
        builder.source_snapshot("admitted-source");
        builder.value_money("free_cash", &[(first, usd(12_i64)), (next, usd(12))]);
        let model = builder.build().unwrap();
        let mut results = Evaluator::new()
            .evaluate_with_market(
                &model,
                &MarketContext::new(),
                valuation,
                ObservationCutoff::Through(valuation),
            )
            .unwrap();
        let mut cf = CashflowBreakdown::with_currency(Currency::USD);
        cf.interest_expense_cash = usd(1);
        cf.principal_payment = usd(50);
        cf.scheduled_cash_interest_due = Some(usd(2));
        cf.scheduled_principal_due = Some(usd(4));
        cf.debt_balance = usd(60);
        let mut book = CapitalStructureCashflows::new();
        book.by_instrument
            .entry("term".to_owned())
            .or_default()
            .insert(first, cf.clone());
        cf.scheduled_principal_due = None;
        book.by_instrument
            .entry("term".to_owned())
            .or_default()
            .insert(next, cf);
        results.cs_cashflows = Some(book);
        let report = evaluate_credit_context(
            &results,
            &CreditContextSpec {
                borrowing_id: "term".to_owned(),
                run_id: results.provenance.as_ref().unwrap().run_id.clone(),
                currency: Currency::USD,
                periods: vec![first, next],
                measures: vec![
                    CreditMeasure::CashDscr,
                    CreditMeasure::DebtToEnterpriseValue,
                ],
                cfads_node: Some("free_cash".to_owned()),
                earnings_node: None,
                recognized_interest_node: None,
                cash_fees_node: None,
                reference_values: vec![CreditReferenceValue {
                    measure: CreditMeasure::DebtToEnterpriseValue,
                    date: date!(2025 - 03 - 31),
                    amount: usd(150),
                    source_id: "independent-matched-ev".to_owned(),
                }],
            },
        )
        .unwrap();
        assert!(matches!(
            report.rows[0].value,
            CreditMeasureValue::Measured { ratio: 2.0, .. }
        ));
        assert_eq!(report.coverage[0].observed_minimum, Some(2.0));
        assert_eq!(report.coverage[0].full_path_minimum, None);
        assert_eq!(report.coverage[1].observed_minimum, Some(0.4));
        assert!(matches!(
            report.rows[3].value,
            CreditMeasureValue::Unavailable { .. }
        ));
    }

    #[test]
    fn rejects_foreign_principal_fees_and_debt_balance() {
        for field in ["principal", "fees", "balance"] {
            let (result, mut cs, periods) = make_result_and_cs();
            let cashflow = cs
                .by_instrument
                .get_mut("BOND-001")
                .unwrap()
                .get_mut(&periods[0].id)
                .unwrap();
            let foreign = Money::from((100_i64, Currency::EUR));
            match field {
                "principal" => cashflow.principal_payment = foreign,
                "fees" => cashflow.fees = foreign,
                _ => cashflow.debt_balance = foreign,
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
            .unwrap_err();
            assert!(
                error.to_string().contains("Currency mismatch"),
                "{field}: {error}"
            );
        }
    }

    #[test]
    fn finite_inputs_cannot_produce_infinite_credit_ratios() {
        let (mut result, mut cs, periods) = make_result_and_cs();
        let calculate = |result: &StatementResult,
                         cs: &CapitalStructureCashflows,
                         refs: Option<&[(PeriodId, f64)]>| {
            compute_credit_context(
                result,
                cs,
                "BOND-001",
                CreditNumeratorNodes {
                    cfads: "cfads",
                    interest_coverage: "ebitda",
                },
                Currency::USD,
                &periods,
                refs,
            )
        };
        assert!(calculate(&result, &cs, Some(&[(periods[0].id, 1e-320)])).is_err());
        result.cells.get_mut("cfads").unwrap().insert(
            periods[0].id,
            finstack_quant_statements::evaluator::ResultCell::scalar(f64::MAX).expect("finite"),
        );
        let cf = cs
            .by_instrument
            .get_mut("BOND-001")
            .unwrap()
            .get_mut(&periods[0].id)
            .unwrap();
        cf.interest_expense_cash = Money::new(0.1, Currency::USD).unwrap();
        cf.principal_payment = Money::from((0_i64, Currency::USD));
        assert!(calculate(&result, &cs, None).is_err());
    }

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
        result
            .set_series("ebitda", ebitda_map)
            .expect("finite fixture values");

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
        result
            .set_series("cfads", cfads_map)
            .expect("finite fixture values");

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
                    opening_balance: None,
                    scheduled_principal: None,
                    mandatory_prepayment: None,
                    sweep_prepayment: None,
                    voluntary_prepayment: None,
                    interest_arrears: None,
                    principal_pending: None,
                    principal_arrears: None,
                    principal_advance_reserve: None,
                    scheduled_cash_interest_due: None,
                    scheduled_principal_due: None,
                    fee_rebates: None,
                    repayment_refunds: None,
                    funding_proceeds: None,
                    funding_principal: None,
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
        if let Some(ebitda) = result.cells.get_mut("ebitda") {
            ebitda.shift_remove(&PeriodId::quarter(2025, 2).expect("valid period fixture"));
        }
        if let Some(cfads) = result.cells.get_mut("cfads") {
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
        if let Some(ebitda) = result.cells.get_mut("ebitda") {
            ebitda.shift_remove(&PeriodId::quarter(2025, 2).expect("valid period fixture"));
        }
        if let Some(cfads) = result.cells.get_mut("cfads") {
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
        result.cells.get_mut("cfads").expect("cfads").insert(
            periods[0].id,
            finstack_quant_statements::evaluator::ResultCell::unavailable(
                finstack_quant_statements::evaluator::UnavailableReason::MissingInput,
            ),
        );

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
