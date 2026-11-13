//! Debt stack by seniority: balances, leverage and coverage through each payment
//! class, and the maturity wall, from an evaluated capital structure.

use std::collections::{BTreeMap, HashMap};

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, PeriodId};
use finstack_quant_core::math::summation::neumaier_sum;
use finstack_quant_core::money::Money;
use finstack_quant_core::table::{TableColumn, TableColumnData, TableEnvelope};
use finstack_quant_statements::capital_structure::CapitalStructureCashflows;
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::types::{FinancialModelSpec, FinancialStatementInstrument};
use finstack_quant_statements::{Error, Result};

use crate::analysis::units::{finite_ratio, require_currency};
use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
use serde::{Deserialize, Serialize};

/// Request for [`capital_structure_summary`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CapitalStructureSummaryConfig {
    /// Reporting period: balances are read at it and the LTM window ends at it.
    pub period: PeriodId,
    /// Statement node holding EBITDA for each single period (not already
    /// trailing). Never inferred from a conventional name.
    pub ebitda_node: String,
    /// Statement node holding the cash balance at the end of each period.
    /// Omitted, the summary has no cash, net debt or net leverage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cash_node: Option<String>,
}

/// Instrument type of a tranche in the debt stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum DebtInstrumentKind {
    /// Corporate or government bond.
    Bond,
    /// Convertible bond, counted as debt at its balance.
    ConvertibleBond,
    /// Term loan.
    TermLoan,
    /// Revolving credit facility, counted at its drawn balance.
    RevolvingCredit,
}

impl DebtInstrumentKind {
    /// Wire name of the kind (`bond`, `convertible_bond`, `term_loan` or
    /// `revolving_credit`), the serde spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bond => "bond",
            Self::ConvertibleBond => "convertible_bond",
            Self::TermLoan => "term_loan",
            Self::RevolvingCredit => "revolving_credit",
        }
    }
}

/// One debt instrument's row in a [`CapitalStructureSummary`].
///
/// Tranches in one payment class are pari passu, so the figures "through this
/// class" are the same on every tranche of the class: they count the whole
/// class and every class senior to it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CapitalStructureTranche {
    /// Instrument identifier in the capital structure.
    pub id: String,
    /// Instrument type.
    pub kind: DebtInstrumentKind,
    /// Seniority rank of the payment class; `0` is most senior. Every tranche
    /// has rank `0` when the waterfall declares no payment classes.
    pub rank: u32,
    /// Identifier of the payment class; `None` when the waterfall declares no
    /// payment classes.
    pub payment_class: Option<String>,
    /// Debt balance at the end of the reporting period. For a revolver this is
    /// the drawn balance, not the commitment.
    pub balance: Money,
    /// `balance` over total debt as a decimal (`0.25` means 25%); `None` when
    /// total debt is not positive.
    pub share_of_debt: Option<f64>,
    /// Final contractual maturity of the instrument.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub maturity: Date,
    /// Total balance of this payment class and every class senior to it.
    pub cumulative_debt: Money,
    /// `cumulative_debt` over LTM EBITDA, in turns (`3.5` means 3.5x); `None`
    /// when LTM EBITDA is partial or not positive.
    pub leverage: Option<f64>,
    /// LTM EBITDA over the LTM interest of this class and every class senior to
    /// it, in turns; `None` when LTM EBITDA is partial or not positive, or that
    /// interest is unavailable or not positive.
    pub interest_coverage: Option<f64>,
    /// This tranche's cash plus PIK interest expense summed over the LTM window;
    /// `None` when the window is short or the instrument has no cashflows in
    /// one of its periods.
    pub ltm_interest: Option<Money>,
}

/// Outstanding balance falling due in one calendar year.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MaturityWallBucket {
    /// Calendar year of the final maturities in this bucket.
    pub year: i32,
    /// Balance at the reporting period of the tranches maturing in `year`.
    pub amount: Money,
    /// `amount` over total debt as a decimal; `None` when total debt is not
    /// positive.
    pub share_of_debt: Option<f64>,
}

/// Debt stack by seniority at one period. Returned by
/// [`capital_structure_summary`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CapitalStructureSummary {
    /// Reporting period.
    pub as_of: PeriodId,
    /// Currency every tranche and monetary node is in.
    pub currency: Currency,
    /// Sum of tranche balances at the reporting period.
    pub total_debt: Money,
    /// Cash balance at the reporting period; `None` without a `cash_node`.
    pub cash: Option<Money>,
    /// `total_debt` less `cash`; `None` without a `cash_node`. Negative when
    /// cash exceeds debt.
    pub net_debt: Option<Money>,
    /// EBITDA summed over the LTM window; `None` when the window is short. It
    /// is reported when zero or negative, though the ratios are then `None`.
    pub ltm_ebitda: Option<Money>,
    /// Periods a full LTM window holds: 4 for quarters, 12 for months, and for a
    /// daily model the model's days in the trailing twelve months plus any
    /// calendar days of that year before the model's first day.
    pub ltm_periods_required: u32,
    /// Periods of the window that are in the model and hold an EBITDA value.
    pub ltm_periods_found: u32,
    /// `total_debt` over LTM EBITDA, in turns; `None` when LTM EBITDA is
    /// partial or not positive.
    pub total_leverage: Option<f64>,
    /// `net_debt` over LTM EBITDA, in turns; `None` without a `cash_node` or
    /// when LTM EBITDA is partial or not positive. Negative when cash exceeds
    /// debt.
    pub net_leverage: Option<f64>,
    /// LTM EBITDA over the LTM interest of every tranche, in turns; `None` when
    /// LTM EBITDA is partial or not positive, or that interest is unavailable or
    /// not positive.
    pub total_coverage: Option<f64>,
    /// Tranches by rank, then model declaration order.
    pub tranches: Vec<CapitalStructureTranche>,
    /// Non-zero balances by calendar year of final maturity, ascending. Uses the
    /// balance at the reporting period and ignores any scheduled amortization
    /// after it.
    pub maturity_wall: Vec<MaturityWallBucket>,
}

impl CapitalStructureSummary {
    /// Export one row per tranche, in [`tranches`](Self::tranches) order, as a
    /// [`TableEnvelope`].
    ///
    /// Columns: `id` (string), `kind` (`bond`, `convertible_bond`, `term_loan`
    /// or `revolving_credit`), `rank` (integer), `payment_class` (nullable
    /// string), `balance` (float), `share_of_debt` (nullable float, decimal),
    /// `maturity` (ISO date string), `cumulative_debt` (float), `leverage` and
    /// `interest_coverage` (nullable float, turns) and `ltm_interest`
    /// (nullable float). Amounts are in [`currency`](Self::currency). The
    /// columns remain when there are no tranches.
    #[must_use]
    pub fn to_table(&self) -> TableEnvelope {
        let rows = &self.tranches;
        let nullable = |f: fn(&CapitalStructureTranche) -> Option<f64>| {
            TableColumnData::NullableFloat64(rows.iter().map(f).collect())
        };
        TableEnvelope {
            row_count: rows.len(),
            columns: vec![
                TableColumn::new(
                    "id",
                    TableColumnData::String(rows.iter().map(|t| t.id.clone()).collect()),
                ),
                TableColumn::new(
                    "kind",
                    TableColumnData::String(
                        rows.iter().map(|t| t.kind.as_str().to_string()).collect(),
                    ),
                ),
                TableColumn::new(
                    "rank",
                    TableColumnData::Int64(rows.iter().map(|t| i64::from(t.rank)).collect()),
                ),
                TableColumn::new(
                    "payment_class",
                    TableColumnData::NullableString(
                        rows.iter().map(|t| t.payment_class.clone()).collect(),
                    ),
                ),
                TableColumn::new(
                    "balance",
                    TableColumnData::Float64(rows.iter().map(|t| t.balance.amount()).collect()),
                ),
                TableColumn::new("share_of_debt", nullable(|t| t.share_of_debt)),
                TableColumn::new(
                    "maturity",
                    TableColumnData::String(rows.iter().map(|t| t.maturity.to_string()).collect()),
                ),
                TableColumn::new(
                    "cumulative_debt",
                    TableColumnData::Float64(
                        rows.iter().map(|t| t.cumulative_debt.amount()).collect(),
                    ),
                ),
                TableColumn::new("leverage", nullable(|t| t.leverage)),
                TableColumn::new("interest_coverage", nullable(|t| t.interest_coverage)),
                TableColumn::new(
                    "ltm_interest",
                    nullable(|t| t.ltm_interest.map(|interest| interest.amount())),
                ),
            ],
            metadata: Default::default(),
        }
    }
}

/// One tranche's contract terms and seniority, before reading cashflows.
struct StackEntry<'a> {
    id: &'a str,
    kind: DebtInstrumentKind,
    maturity: Date,
    rank: u32,
    payment_class: Option<&'a str>,
}

/// Summarize the debt stack by seniority at one period.
///
/// Only bonds, convertible bonds, term loans and revolvers are debt; swaps,
/// cap/floors and swaptions are left out of balances and interest, including
/// when a payment class names them. Seniority is the rank of the waterfall's
/// payment classes. A model with no waterfall or no payment classes has one
/// implicit class: every tranche has rank `0` and no class id.
///
/// LTM EBITDA and LTM interest sum the twelve calendar months of model periods
/// ending at `period` given by [`ltm_window`](finstack_quant_core::dates::ltm_window),
/// the slots `ttm()` reads in a formula: 4 quarters or 12 months, and on a daily
/// model every calendar or business day of the trailing year. A window with a slot missing from the model
/// or a slot where the EBITDA node has no value is never annualised or
/// partially summed: LTM EBITDA and every ratio are `None`, and
/// [`CapitalStructureSummary::ltm_periods_found`] says how many slots held. A
/// zero or negative LTM EBITDA also gives `None` ratios, not a negative multiple.
/// Interest is cash plus PIK, as in
/// [`compute_credit_context`](crate::analysis::compute_credit_context). Where an
/// instrument has no cashflows in an earlier slot, its LTM interest and the
/// coverage through its class and every class junior to it are `None`.
///
/// # Arguments
///
/// * `results` - Evaluated statement results holding `cs_cashflows` and both
///   configured nodes. Balances and interest are in each instrument's native
///   currency; a scalar EBITDA or cash node is read as an amount in the debt
///   currency.
/// * `model` - Model whose `capital_structure` gives the instruments, their
///   declaration order and the payment classes, and whose `periods` bound the
///   LTM window.
/// * `config` - Reporting period and the statement nodes for single-period
///   EBITDA and, optionally, the period-end cash balance.
///
/// # Returns
///
/// The summary with tranches by rank and declaration order, the ratios through
/// each class and the maturity wall.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] when the model has no capital structure, or no
/// debt instruments once hedges are excluded; an instrument is neither one of the
/// four debt kinds nor a hedge; `cs_cashflows` is absent; `period`
/// is not in the model; a configured node is not in `results`, or the cash node
/// has no value at `period`; an instrument has no cashflows at `period` or is in
/// no class while payment classes are declared; a consumed value or ratio is
/// non-finite; tranches are in more than one currency (no FX is applied); or the
/// EBITDA or cash node is monetary in a currency other than the debt's.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::PeriodId;
/// use finstack_quant_core::money::Money;
/// use finstack_quant_statements::builder::ModelBuilder;
/// use finstack_quant_statements::capital_structure::{
///     CapitalStructureCashflows, CashflowBreakdown,
/// };
/// use finstack_quant_statements::evaluator::StatementResult;
/// use finstack_quant_statements_analytics::analysis::{
///     capital_structure_summary, CapitalStructureSummaryConfig,
/// };
/// use indexmap::IndexMap;
/// use time::macros::date;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut builder = ModelBuilder::new("borrower");
/// builder.periods("2025Q1..2025Q4", None)?;
/// builder.add_bond(
///     "SENIOR",
///     Money::new(1_000.0, Currency::USD)?,
///     0.05,
///     date!(2025 - 01 - 01),
///     date!(2030 - 01 - 01),
///     "USD-OIS",
/// )?;
/// let model = builder.build()?;
///
/// let quarters = (1..=4)
///     .map(|quarter| PeriodId::quarter(2025, quarter))
///     .collect::<Result<Vec<_>, _>>()?;
/// let row = CashflowBreakdown {
///     interest_expense_cash: Money::new(12.5, Currency::USD)?,
///     debt_balance: Money::new(1_000.0, Currency::USD)?,
///     ..CashflowBreakdown::with_currency(Currency::USD)
/// };
/// let mut cashflows = CapitalStructureCashflows::new();
/// cashflows.by_instrument.insert(
///     "SENIOR".to_string(),
///     quarters.iter().map(|id| (*id, row.clone())).collect(),
/// );
/// let mut results = StatementResult::new();
/// results.cs_cashflows = Some(cashflows);
/// results.nodes.insert(
///     "ebitda".to_string(),
///     quarters.iter().map(|id| (*id, 50.0)).collect::<IndexMap<_, _>>(),
/// );
///
/// let summary = capital_structure_summary(
///     &results,
///     &model,
///     &CapitalStructureSummaryConfig {
///         period: quarters[3],
///         ebitda_node: "ebitda".to_string(),
///         cash_node: None,
///     },
/// )?;
/// assert_eq!(summary.total_leverage, Some(5.0)); // 1,000 / (4 * 50)
/// assert_eq!(summary.total_coverage, Some(4.0)); // 200 / (4 * 12.5)
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// - Coverage and leverage interpretation:
///   `docs/REFERENCES.md#tuckman-serrat-fixed-income`
pub fn capital_structure_summary(
    results: &StatementResult,
    model: &FinancialModelSpec,
    config: &CapitalStructureSummaryConfig,
) -> Result<CapitalStructureSummary> {
    let period = config.period;
    let stack = debt_stack(model)?;
    let cashflows = results.cs_cashflows.as_ref().ok_or_else(|| {
        Error::invalid_input(
            "Capital structure summary needs results with cs_cashflows; evaluate the model with \
             its capital structure",
        )
    })?;
    if !model.periods.iter().any(|p| p.id == period) {
        return Err(Error::invalid_input(format!(
            "Capital structure summary period {period} is not in the model"
        )));
    }
    for node in std::iter::once(&config.ebitda_node).chain(&config.cash_node) {
        if !results.nodes.contains_key(node) {
            return Err(Error::invalid_input(format!(
                "Capital structure summary node '{node}' is not in the results"
            )));
        }
    }

    let balances = stack
        .iter()
        .map(|entry| balance_at(cashflows, entry.id, period))
        .collect::<Result<Vec<_>>>()?;
    let currency = single_currency(&stack, &balances)?;
    let mut total_debt = Money::zero(currency);
    for balance in &balances {
        total_debt = total_debt.checked_add(*balance)?;
    }

    let ltm = finstack_quant_core::dates::ltm_window(period, &model.periods)
        .map_err(|error| Error::invalid_input(error.to_string()))?;
    let required = ltm.required();
    let window: Vec<PeriodId> = ltm
        .slots
        .into_iter()
        .filter(|slot| model.periods.iter().any(|p| p.id == *slot))
        .collect();
    let full_window = window.len() == required;

    let ebitda_values = node_values(results, &config.ebitda_node, &window, currency)?;
    let found = ebitda_values.len();
    let ltm_ebitda_amount = (found == required)
        .then(|| finite_sum(&ebitda_values, &config.ebitda_node))
        .transpose()?;
    let ltm_ebitda = ltm_ebitda_amount
        .map(|amount| Money::new(amount, currency))
        .transpose()?;
    // Ratios need a positive LTM EBITDA; a negative one is no multiple.
    let earnings = ltm_ebitda_amount.filter(|amount| *amount > 0.0);

    let cash = config
        .cash_node
        .as_deref()
        .map(|node| cash_at(results, node, period, currency))
        .transpose()?;
    let net_debt = cash
        .map(|cash| {
            total_debt
                .checked_sub(cash)
                .map_err(|error| Error::eval(error.to_string()))
        })
        .transpose()?;

    let interests = stack
        .iter()
        .map(|entry| {
            if !full_window {
                return Ok(None);
            }
            ltm_interest(cashflows, entry.id, &window, currency)
        })
        .collect::<Result<Vec<_>>>()?;

    let rows: Vec<(&StackEntry<'_>, Money, Option<Money>)> = stack
        .iter()
        .zip(balances.iter().copied())
        .zip(interests)
        .map(|((entry, balance), interest)| (entry, balance, interest))
        .collect();
    let mut tranches = Vec::with_capacity(stack.len());
    let mut cumulative_debt = Money::zero(currency);
    let mut cumulative_interest = Some(Money::zero(currency));
    for rank_rows in rows.chunk_by(|a, b| a.0.rank == b.0.rank) {
        for (_, balance, interest) in rank_rows {
            cumulative_debt = cumulative_debt.checked_add(*balance)?;
            cumulative_interest = match (cumulative_interest, interest) {
                (Some(sum), Some(interest)) => Some(sum.checked_add(*interest)?),
                _ => None,
            };
        }
        let leverage = earnings
            .map(|ebitda| finite_ratio(cumulative_debt.amount(), ebitda))
            .transpose()?;
        let interest_coverage = coverage(earnings, cumulative_interest)?;
        for (entry, balance, interest) in rank_rows {
            tranches.push(CapitalStructureTranche {
                id: entry.id.to_string(),
                kind: entry.kind,
                rank: entry.rank,
                payment_class: entry.payment_class.map(str::to_string),
                balance: *balance,
                share_of_debt: share(*balance, total_debt)?,
                maturity: entry.maturity,
                cumulative_debt,
                leverage,
                interest_coverage,
                ltm_interest: *interest,
            });
        }
    }

    Ok(CapitalStructureSummary {
        as_of: period,
        currency,
        total_debt,
        cash,
        net_debt,
        ltm_ebitda,
        ltm_periods_required: u32::try_from(required)
            .map_err(|_| Error::invalid_input("LTM window is too long"))?,
        ltm_periods_found: found as u32,
        total_leverage: earnings
            .map(|ebitda| finite_ratio(total_debt.amount(), ebitda))
            .transpose()?,
        net_leverage: earnings
            .zip(net_debt)
            .map(|(ebitda, net_debt)| finite_ratio(net_debt.amount(), ebitda))
            .transpose()?,
        total_coverage: coverage(earnings, cumulative_interest)?,
        maturity_wall: maturity_wall(&tranches, total_debt)?,
        tranches,
    })
}

/// Debt instruments with their rank, ordered by rank then declaration order.
fn debt_stack(model: &FinancialModelSpec) -> Result<Vec<StackEntry<'_>>> {
    let capital_structure = model.capital_structure.as_ref().ok_or_else(|| {
        Error::invalid_input(format!(
            "Capital structure summary: model '{}' has no capital structure",
            model.id
        ))
    })?;
    let classes = capital_structure
        .waterfall
        .as_ref()
        .map_or(&[][..], |waterfall| &waterfall.payment_classes);
    let class_of: HashMap<&str, (u32, &str)> = classes
        .iter()
        .flat_map(|class| {
            class
                .instrument_ids
                .iter()
                .map(move |id| (id.as_str(), (class.rank, class.id.as_str())))
        })
        .collect();

    let mut stack = Vec::with_capacity(capital_structure.debt_instruments.len());
    for debt in &capital_structure.debt_instruments {
        // The enum is non-exhaustive: a later instrument kind must be classified here.
        let (kind, maturity) = match &debt.spec {
            FinancialStatementInstrument::Bond(bond) => (DebtInstrumentKind::Bond, bond.maturity),
            FinancialStatementInstrument::ConvertibleBond(bond) => {
                (DebtInstrumentKind::ConvertibleBond, bond.maturity)
            }
            FinancialStatementInstrument::TermLoan(loan) => {
                (DebtInstrumentKind::TermLoan, loan.maturity)
            }
            FinancialStatementInstrument::RevolvingCredit(revolver) => {
                (DebtInstrumentKind::RevolvingCredit, revolver.maturity)
            }
            FinancialStatementInstrument::InterestRateSwap(_)
            | FinancialStatementInstrument::CapFloor(_)
            | FinancialStatementInstrument::Swaption(_) => continue,
            other => {
                return Err(Error::invalid_input(format!(
                    "Capital structure summary: instrument '{}' is a '{}', which is neither \
                     debt nor a hedge",
                    debt.id,
                    InstrumentJson::from(other.clone()).type_tag()
                )));
            }
        };
        let (rank, payment_class) = if classes.is_empty() {
            (0, None)
        } else {
            let (rank, class) = class_of.get(debt.id.as_str()).ok_or_else(|| {
                Error::invalid_input(format!(
                    "Capital structure summary: instrument '{}' is in no payment class, so it \
                     has no seniority",
                    debt.id
                ))
            })?;
            (*rank, Some(*class))
        };
        stack.push(StackEntry {
            id: &debt.id,
            kind,
            maturity,
            rank,
            payment_class,
        });
    }
    if stack.is_empty() {
        return Err(Error::invalid_input(format!(
            "Capital structure summary: model '{}' has no debt instruments once swaps, cap/floors \
             and swaptions are excluded",
            model.id
        )));
    }
    stack.sort_by_key(|entry| entry.rank);
    Ok(stack)
}

/// Debt balance of an instrument at the reporting period.
fn balance_at(
    cashflows: &CapitalStructureCashflows,
    instrument_id: &str,
    period: PeriodId,
) -> Result<Money> {
    cashflows
        .by_instrument
        .get(instrument_id)
        .and_then(|rows| rows.get(&period))
        .map(|row| row.debt_balance)
        .ok_or_else(|| {
            Error::invalid_input(format!(
                "Capital structure summary: instrument '{instrument_id}' has no cashflows in \
                 period {period}"
            ))
        })
}

/// The one currency of the stack; mixed currencies have no implicit FX.
fn single_currency(stack: &[StackEntry<'_>], balances: &[Money]) -> Result<Currency> {
    let currency = balances[0].currency();
    for (entry, balance) in stack.iter().zip(balances) {
        if balance.currency() != currency {
            return Err(Error::invalid_input(format!(
                "Capital structure summary: tranche '{}' is in {} while '{}' is in {currency}; \
                 no implicit FX is applied, so all tranches must share one currency",
                entry.id,
                balance.currency(),
                stack[0].id,
            )));
        }
    }
    Ok(currency)
}

/// Finite values of a node across the slots that hold one, rejecting a monetary
/// node in another currency.
fn node_values(
    results: &StatementResult,
    node: &str,
    slots: &[PeriodId],
    currency: Currency,
) -> Result<Vec<f64>> {
    let mut values = Vec::with_capacity(slots.len());
    for slot in slots {
        require_currency(results, node, slot, currency)?;
        let Some(value) = results.get(node, slot) else {
            continue;
        };
        if !value.is_finite() {
            return Err(Error::invalid_input(format!(
                "Capital structure summary node '{node}' is non-finite in period {slot}"
            )));
        }
        values.push(value);
    }
    Ok(values)
}

/// Cash balance at the reporting period, as the node's typed amount when it has one.
fn cash_at(
    results: &StatementResult,
    node: &str,
    period: PeriodId,
    currency: Currency,
) -> Result<Money> {
    require_currency(results, node, &period, currency)?;
    let value = results.get(node, &period).ok_or_else(|| {
        Error::invalid_input(format!(
            "Capital structure summary cash node '{node}' has no value in period {period}"
        ))
    })?;
    if !value.is_finite() {
        return Err(Error::invalid_input(format!(
            "Capital structure summary node '{node}' is non-finite in period {period}"
        )));
    }
    match results.get_money(node, &period) {
        Some(amount) => Ok(amount),
        None => Ok(Money::new(value, currency)?),
    }
}

/// Cash plus PIK interest over the window; `None` if a slot has no cashflows.
fn ltm_interest(
    cashflows: &CapitalStructureCashflows,
    instrument_id: &str,
    window: &[PeriodId],
    currency: Currency,
) -> Result<Option<Money>> {
    let Some(rows) = cashflows.by_instrument.get(instrument_id) else {
        return Ok(None);
    };
    let mut total = Money::zero(currency);
    for slot in window {
        let Some(row) = rows.get(slot) else {
            return Ok(None);
        };
        row.validate_currency_invariant()
            .map_err(|error| Error::invalid_input(error.to_string()))?;
        if row.interest_expense_cash.currency() != currency {
            return Err(Error::invalid_input(format!(
                "Capital structure summary: instrument '{instrument_id}' has interest in {} in \
                 period {slot} while the debt is in {currency}; no implicit FX is applied",
                row.interest_expense_cash.currency(),
            )));
        }
        let interest = row
            .interest_expense_total()
            .map_err(|error| Error::invalid_input(error.to_string()))?;
        total = total.checked_add(interest)?;
    }
    Ok(Some(total))
}

/// Non-zero balances grouped by calendar year of final maturity, ascending.
fn maturity_wall(
    tranches: &[CapitalStructureTranche],
    total_debt: Money,
) -> Result<Vec<MaturityWallBucket>> {
    let mut by_year: BTreeMap<i32, Money> = BTreeMap::new();
    for tranche in tranches {
        if tranche.balance.amount() == 0.0 {
            continue;
        }
        let year = tranche.maturity.year();
        let amount = by_year
            .get(&year)
            .copied()
            .unwrap_or_else(|| Money::zero(tranche.balance.currency()));
        by_year.insert(year, amount.checked_add(tranche.balance)?);
    }
    by_year
        .into_iter()
        .map(|(year, amount)| {
            Ok(MaturityWallBucket {
                year,
                amount,
                share_of_debt: share(amount, total_debt)?,
            })
        })
        .collect()
}

/// Compensated sum over a window, which finite inputs can still overflow.
fn finite_sum(values: &[f64], node: &str) -> Result<f64> {
    let sum = neumaier_sum(values.iter().copied());
    if !sum.is_finite() {
        return Err(Error::invalid_input(format!(
            "Capital structure summary: LTM sum of node '{node}' is non-finite"
        )));
    }
    Ok(sum)
}

/// LTM EBITDA over LTM interest, when both are available and interest is positive.
fn coverage(earnings: Option<f64>, interest: Option<Money>) -> Result<Option<f64>> {
    earnings
        .zip(interest.map(|interest| interest.amount()))
        .filter(|(_, interest)| *interest > 0.0)
        .map(|(ebitda, interest)| finite_ratio(ebitda, interest))
        .transpose()
}

/// `part` as a decimal share of total debt; `None` when total debt is not positive.
fn share(part: Money, total: Money) -> Result<Option<f64>> {
    if total.amount() <= 0.0 {
        return Ok(None);
    }
    finite_ratio(part.amount(), total.amount()).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::{Period, PeriodKind, build_periods};
    use finstack_quant_statements::capital_structure::{
        CashflowBreakdown, PaymentClassSpec, WaterfallSpec,
    };
    use finstack_quant_statements::types::NodeValueType;
    use finstack_quant_statements::types::{CapitalStructureSpec, DebtInstrumentSpec};
    use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
    use finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleBond;
    use finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCredit;
    use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
    use finstack_quant_valuations::instruments::rates::irs::InterestRateSwap;
    use indexmap::IndexMap;
    use time::macros::date;

    const TOLERANCE: f64 = 1e-12;

    fn usd(amount: f64) -> Money {
        Money::new(amount, Currency::USD).expect("finite amount")
    }

    fn q(quarter: u8) -> PeriodId {
        PeriodId::quarter(2025, quarter).expect("valid quarter")
    }

    fn periods(range: &str) -> Vec<Period> {
        build_periods(range, None).expect("valid range").periods
    }

    fn bond(maturity: Date) -> FinancialStatementInstrument {
        let mut bond = Bond::example().expect("example bond");
        bond.maturity = maturity;
        FinancialStatementInstrument::Bond(bond)
    }

    fn term_loan(maturity: Date) -> FinancialStatementInstrument {
        let mut loan = TermLoan::example().expect("example loan");
        loan.maturity = maturity;
        FinancialStatementInstrument::TermLoan(loan)
    }

    fn revolver(maturity: Date) -> FinancialStatementInstrument {
        let mut revolver = RevolvingCredit::example().expect("example revolver");
        revolver.maturity = maturity;
        FinancialStatementInstrument::RevolvingCredit(revolver)
    }

    fn swap() -> FinancialStatementInstrument {
        FinancialStatementInstrument::InterestRateSwap(
            InterestRateSwap::example().expect("example swap"),
        )
    }

    fn class(id: &str, rank: u32, instrument_ids: &[&str]) -> PaymentClassSpec {
        PaymentClassSpec {
            id: id.to_string(),
            rank,
            instrument_ids: instrument_ids.iter().map(|id| id.to_string()).collect(),
        }
    }

    fn model_of(
        range: &str,
        debts: Vec<(&str, FinancialStatementInstrument)>,
        classes: Vec<PaymentClassSpec>,
    ) -> FinancialModelSpec {
        let mut model = FinancialModelSpec::new(
            "borrower",
            finstack_quant_core::dates::PeriodPlan::try_new(
                finstack_quant_core::dates::PeriodCalendar::Gregorian,
                periods(range),
            )
            .expect("valid period plan"),
        );
        model.capital_structure = Some(CapitalStructureSpec {
            debt_instruments: debts
                .into_iter()
                .map(|(id, spec)| DebtInstrumentSpec {
                    id: id.to_string(),
                    spec,
                })
                .collect(),
            meta: IndexMap::new(),
            reporting_currency: None,
            fx_policy: None,
            waterfall: Some(WaterfallSpec {
                payment_classes: classes,
                ..WaterfallSpec::default()
            }),
        });
        model
    }

    fn row(balance: f64, cash_interest: f64, pik_interest: f64) -> CashflowBreakdown {
        CashflowBreakdown {
            interest_expense_cash: usd(cash_interest),
            interest_expense_pik: usd(pik_interest),
            debt_balance: usd(balance),
            ..CashflowBreakdown::with_currency(Currency::USD)
        }
    }

    fn rows(
        quarters: &[PeriodId],
        balance: f64,
        cash_interest: f64,
        pik_interest: f64,
    ) -> IndexMap<PeriodId, CashflowBreakdown> {
        quarters
            .iter()
            .map(|id| (*id, row(balance, cash_interest, pik_interest)))
            .collect()
    }

    /// Quarterly model declared junior-first with a swap between the pair:
    ///
    /// | Instrument | Class | Balance | Quarterly interest      | LTM interest |
    /// | SUB        | 2L    | 420     | 8 cash + 2 PIK          | 40           |
    /// | TLA        | 1L    | 400     | 5 cash                  | 20           |
    /// | SWAP       | 1L    | 7       | 99 cash (hedge, ignored)| -            |
    /// | RCF        | 1L    | 100     | 1 cash                  | 4            |
    ///
    /// EBITDA 100, 110, 120, 130 sums to an LTM of 460; cash is 150 at Q4.
    fn fixture() -> (StatementResult, FinancialModelSpec) {
        let model = model_of(
            "2025Q1..2025Q4",
            vec![
                ("SUB", bond(date!(2031 - 03 - 15))),
                ("TLA", term_loan(date!(2029 - 06 - 30))),
                ("SWAP", swap()),
                ("RCF", revolver(date!(2029 - 01 - 15))),
            ],
            vec![
                class("2L", 1, &["SUB"]),
                class("1L", 0, &["TLA", "RCF", "SWAP"]),
            ],
        );
        let quarters = [q(1), q(2), q(3), q(4)];
        let mut cashflows = CapitalStructureCashflows::new();
        for (id, instrument_rows) in [
            ("SUB", rows(&quarters, 420.0, 8.0, 2.0)),
            ("TLA", rows(&quarters, 400.0, 5.0, 0.0)),
            ("SWAP", rows(&quarters, 7.0, 99.0, 0.0)),
            ("RCF", rows(&quarters, 100.0, 1.0, 0.0)),
        ] {
            cashflows
                .by_instrument
                .insert(id.to_string(), instrument_rows);
        }
        let mut results = StatementResult::new();
        results.cs_cashflows = Some(cashflows);
        results.nodes.insert(
            "ebitda".to_string(),
            quarters
                .iter()
                .copied()
                .zip([100.0, 110.0, 120.0, 130.0])
                .collect(),
        );
        results.nodes.insert(
            "cash".to_string(),
            quarters.iter().map(|id| (*id, 150.0)).collect(),
        );
        (results, model)
    }

    fn config(period: PeriodId, cash_node: Option<&str>) -> CapitalStructureSummaryConfig {
        CapitalStructureSummaryConfig {
            period,
            ebitda_node: "ebitda".to_string(),
            cash_node: cash_node.map(str::to_string),
        }
    }

    fn error_of(
        results: &StatementResult,
        model: &FinancialModelSpec,
        config: &CapitalStructureSummaryConfig,
    ) -> String {
        let error =
            capital_structure_summary(results, model, config).expect_err("summary must fail");
        assert!(
            matches!(error, Error::InvalidInput(_)),
            "validation error expected: {error:?}"
        );
        error.to_string()
    }

    fn assert_close(actual: Option<f64>, expected: f64) {
        let actual = actual.expect("value present");
        assert!(
            (actual - expected).abs() < TOLERANCE,
            "{actual} != {expected}"
        );
    }

    #[test]
    fn orders_by_rank_then_declaration_and_excludes_the_swap() {
        let (results, model) = fixture();
        let summary = capital_structure_summary(&results, &model, &config(q(4), Some("cash")))
            .expect("summary");
        let ids: Vec<_> = summary.tranches.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["TLA", "RCF", "SUB"]);
        assert_eq!(summary.tranches[0].kind, DebtInstrumentKind::TermLoan);
        assert_eq!(
            summary.tranches[1].kind,
            DebtInstrumentKind::RevolvingCredit
        );
        assert_eq!(summary.tranches[2].kind, DebtInstrumentKind::Bond);
        assert_eq!(summary.tranches[0].payment_class.as_deref(), Some("1L"));
        assert_eq!(summary.tranches[2].rank, 1);
        // 400 + 100 + 420, without the swap's 7.
        assert_eq!(summary.total_debt, usd(920.0));
        assert_eq!(summary.currency, Currency::USD);
        assert_eq!(summary.as_of, q(4));
    }

    #[test]
    fn ltm_sums_four_quarters_not_the_last_one() {
        let (results, model) = fixture();
        let summary = capital_structure_summary(&results, &model, &config(q(4), Some("cash")))
            .expect("summary");
        // 100 + 110 + 120 + 130 = 460; Q4 alone would be 130.
        assert_eq!(summary.ltm_ebitda, Some(usd(460.0)));
        assert_eq!(summary.ltm_periods_required, 4);
        assert_eq!(summary.ltm_periods_found, 4);
        // 920 / 460.
        assert_close(summary.total_leverage, 2.0);
        // (920 - 150) / 460.
        assert_eq!(summary.cash, Some(usd(150.0)));
        assert_eq!(summary.net_debt, Some(usd(770.0)));
        assert_close(summary.net_leverage, 770.0 / 460.0);
        // 460 / (40 + 20 + 4); the swap's 396 stays out.
        assert_close(summary.total_coverage, 7.1875);
    }

    /// Business-day model over 2024-2025 with one term loan (balance 261,
    /// interest 0.1 a day) and EBITDA of 1 a day.
    fn business_day_fixture() -> (StatementResult, FinancialModelSpec, Vec<PeriodId>) {
        let mut model = model_of(
            "2025Q1..2025Q1",
            vec![("TLA", term_loan(date!(2029 - 06 - 30)))],
            vec![class("1L", 0, &["TLA"])],
        );
        model.periods.periods.clear();
        let mut day = date!(2024 - 01 - 01);
        while day <= date!(2025 - 12 - 31) {
            if day.weekday().number_days_from_monday() < 5 {
                let id = PeriodId::from_date(day, PeriodKind::Daily, None).expect("day");
                model.periods.periods.push(Period {
                    id,
                    start: day,
                    end: day.next_day().expect("next day"),
                    is_actual: true,
                });
            }
            day = day.next_day().expect("next day");
        }
        let days: Vec<PeriodId> = model.periods.iter().map(|p| p.id).collect();
        let mut cashflows = CapitalStructureCashflows::new();
        cashflows
            .by_instrument
            .insert("TLA".to_string(), rows(&days, 261.0, 0.1, 0.0));
        let mut results = StatementResult::new();
        results.cs_cashflows = Some(cashflows);
        results.nodes.insert(
            "ebitda".to_string(),
            days.iter().map(|id| (*id, 1.0)).collect(),
        );
        (results, model, days)
    }

    #[test]
    fn daily_ltm_needs_every_calendar_day_of_the_year() {
        let (results, model, days) = business_day_fixture();
        let at = *days.last().expect("last day");
        let summary =
            capital_structure_summary(&results, &model, &config(at, None)).expect("summary");
        // A business-day grid holds 2025's 261 weekdays of its 365 calendar
        // days; absent rows do not prove weekends or holidays, so the window
        // is incomplete (record CORE-CONVENTIONS-014 #11).
        assert_eq!(summary.ltm_periods_required, 365);
        assert_eq!(summary.ltm_periods_found, 261);
        assert_eq!(summary.ltm_ebitda, None);
        assert_eq!(summary.total_leverage, None);
        assert_eq!(summary.tranches[0].ltm_interest, None);
    }

    #[test]
    fn daily_ltm_inside_the_first_year_is_short() {
        let (results, model, _) = business_day_fixture();
        let at = PeriodId::from_date(date!(2024 - 12 - 30), PeriodKind::Daily, None).expect("day");
        let summary =
            capital_structure_summary(&results, &model, &config(at, None)).expect("summary");
        assert_eq!(summary.ltm_ebitda, None);
        assert_eq!(summary.total_leverage, None);
        assert_eq!(summary.tranches[0].ltm_interest, None);
        assert!(summary.ltm_periods_found < summary.ltm_periods_required);
    }

    #[test]
    fn figures_through_a_class_are_shared_by_its_pari_passu_tranches() {
        let (results, model) = fixture();
        let summary =
            capital_structure_summary(&results, &model, &config(q(4), None)).expect("summary");
        let [tla, rcf, sub] = summary.tranches.as_slice() else {
            panic!("three tranches");
        };
        for senior in [tla, rcf] {
            // Through 1L: debt 400 + 100 = 500, interest 20 + 4 = 24.
            assert_eq!(senior.cumulative_debt, usd(500.0));
            assert_close(senior.leverage, 500.0 / 460.0);
            assert_close(senior.interest_coverage, 460.0 / 24.0);
        }
        // Through 2L: debt 920, interest 64 (cash plus PIK).
        assert_eq!(sub.cumulative_debt, usd(920.0));
        assert_close(sub.leverage, 2.0);
        assert_close(sub.interest_coverage, 460.0 / 64.0);
        assert_eq!(tla.ltm_interest, Some(usd(20.0)));
        assert_eq!(rcf.ltm_interest, Some(usd(4.0)));
        assert_eq!(sub.ltm_interest, Some(usd(40.0)));
        assert_close(tla.share_of_debt, 400.0 / 920.0);
        assert_close(rcf.share_of_debt, 100.0 / 920.0);
        assert_close(sub.share_of_debt, 420.0 / 920.0);
        assert_eq!(tla.maturity, date!(2029 - 06 - 30));
        // No cash node: no net figures.
        assert_eq!(summary.cash, None);
        assert_eq!(summary.net_debt, None);
        assert_eq!(summary.net_leverage, None);
    }

    #[test]
    fn maturity_wall_groups_by_final_maturity_year() {
        let (results, model) = fixture();
        let summary =
            capital_structure_summary(&results, &model, &config(q(4), None)).expect("summary");
        // TLA 400 and RCF 100 both fall due in 2029; SUB 420 in 2031.
        let wall = &summary.maturity_wall;
        assert_eq!(wall.len(), 2);
        assert_eq!((wall[0].year, wall[0].amount), (2029, usd(500.0)));
        assert_close(wall[0].share_of_debt, 500.0 / 920.0);
        assert_eq!((wall[1].year, wall[1].amount), (2031, usd(420.0)));
        assert_close(wall[1].share_of_debt, 420.0 / 920.0);
    }

    #[test]
    fn zero_balance_tranche_stays_listed_but_makes_no_wall_bucket() {
        let (mut results, model) = fixture();
        let cashflows = results.cs_cashflows.as_mut().expect("cashflows");
        cashflows.by_instrument.insert(
            "SUB".to_string(),
            rows(&[q(1), q(2), q(3), q(4)], 0.0, 8.0, 2.0),
        );
        let summary =
            capital_structure_summary(&results, &model, &config(q(4), None)).expect("summary");
        assert_eq!(summary.tranches.len(), 3);
        assert_eq!(summary.tranches[2].balance, usd(0.0));
        assert_eq!(summary.maturity_wall.len(), 1);
        assert_eq!(summary.maturity_wall[0].year, 2029);
    }

    #[test]
    fn a_window_short_of_a_year_gives_null_ratios_and_says_so() {
        let (results, model) = fixture();
        // Q3 has three quarters behind it in a model starting at 2025Q1. Scaling
        // up 330 or reading 120 as annual would both give a number.
        let summary = capital_structure_summary(&results, &model, &config(q(3), Some("cash")))
            .expect("summary");
        assert_eq!(summary.ltm_periods_required, 4);
        assert_eq!(summary.ltm_periods_found, 3);
        assert_eq!(summary.ltm_ebitda, None);
        assert_eq!(summary.total_leverage, None);
        assert_eq!(summary.net_leverage, None);
        assert_eq!(summary.total_coverage, None);
        for tranche in &summary.tranches {
            assert_eq!(tranche.leverage, None);
            assert_eq!(tranche.interest_coverage, None);
            assert_eq!(tranche.ltm_interest, None);
        }
        // Balances do not need the window.
        assert_eq!(summary.total_debt, usd(920.0));
        assert_eq!(summary.net_debt, Some(usd(770.0)));
    }

    #[test]
    fn a_missing_ebitda_value_in_the_window_gives_null_ratios() {
        let (mut results, model) = fixture();
        results
            .nodes
            .get_mut("ebitda")
            .expect("ebitda")
            .shift_remove(&q(2));
        let summary =
            capital_structure_summary(&results, &model, &config(q(4), None)).expect("summary");
        assert_eq!(summary.ltm_periods_found, 3);
        assert_eq!(summary.ltm_ebitda, None);
        assert_eq!(summary.total_leverage, None);
        assert_eq!(summary.tranches[0].leverage, None);
        // Interest rows are complete, so the tranche's own LTM interest stands.
        assert_eq!(summary.tranches[0].ltm_interest, Some(usd(20.0)));
        assert_eq!(summary.tranches[0].interest_coverage, None);
    }

    #[test]
    fn a_calendar_gap_in_the_model_is_a_short_window() {
        let (results, mut model) = fixture();
        // Four periods before 2026Q1 exist, but 2025Q4 is missing from the model:
        // the last four model periods span five quarters, which `ttm` also refuses.
        model.periods.periods.retain(|period| period.id != q(4));
        model.periods.periods.extend(periods("2026Q1..2026Q1"));
        let period = PeriodId::quarter(2026, 1).expect("valid quarter");
        let mut results = results;
        for instrument_rows in results
            .cs_cashflows
            .as_mut()
            .expect("cashflows")
            .by_instrument
            .values_mut()
        {
            let copy = instrument_rows[&q(3)].clone();
            instrument_rows.insert(period, copy);
        }
        results
            .nodes
            .get_mut("ebitda")
            .expect("ebitda")
            .insert(period, 140.0);
        let summary =
            capital_structure_summary(&results, &model, &config(period, None)).expect("summary");
        assert_eq!(summary.ltm_periods_found, 3);
        assert_eq!(summary.total_leverage, None);
    }

    #[test]
    fn an_annual_period_is_its_own_window() {
        let model = model_of(
            "2025..2025",
            vec![("TL", term_loan(date!(2030 - 01 - 01)))],
            vec![],
        );
        let year = PeriodId::annual(2025).expect("valid annual period");
        let mut results = StatementResult::new();
        let mut cashflows = CapitalStructureCashflows::new();
        cashflows
            .by_instrument
            .insert("TL".to_string(), rows(&[year], 300.0, 10.0, 0.0));
        results.cs_cashflows = Some(cashflows);
        results.nodes.insert(
            "ebitda".to_string(),
            [(year, 100.0)].into_iter().collect::<IndexMap<_, _>>(),
        );
        let summary =
            capital_structure_summary(&results, &model, &config(year, None)).expect("summary");
        assert_eq!(summary.ltm_periods_required, 1);
        assert_eq!(summary.ltm_periods_found, 1);
        assert_close(summary.total_leverage, 3.0);
        assert_close(summary.total_coverage, 10.0);
    }

    #[test]
    fn negative_or_zero_ltm_ebitda_is_reported_with_null_ratios() {
        for ebitda in [-25.0, 0.0] {
            let (mut results, model) = fixture();
            results.nodes.insert(
                "ebitda".to_string(),
                [q(1), q(2), q(3), q(4)]
                    .iter()
                    .map(|id| (*id, ebitda))
                    .collect(),
            );
            let summary = capital_structure_summary(&results, &model, &config(q(4), Some("cash")))
                .expect("summary");
            assert_eq!(summary.ltm_ebitda, Some(usd(4.0 * ebitda)));
            assert_eq!(summary.ltm_periods_found, 4);
            assert_eq!(summary.total_leverage, None);
            assert_eq!(summary.net_leverage, None);
            assert_eq!(summary.total_coverage, None);
            assert_eq!(summary.tranches[0].leverage, None);
            assert_eq!(summary.tranches[0].interest_coverage, None);
        }
    }

    #[test]
    fn coverage_is_null_when_interest_is_not_positive() {
        let (mut results, model) = fixture();
        let quarters = [q(1), q(2), q(3), q(4)];
        let cashflows = results.cs_cashflows.as_mut().expect("cashflows");
        cashflows
            .by_instrument
            .insert("TLA".to_string(), rows(&quarters, 400.0, 0.0, 0.0));
        cashflows
            .by_instrument
            .insert("RCF".to_string(), rows(&quarters, 100.0, 0.0, 0.0));
        let summary =
            capital_structure_summary(&results, &model, &config(q(4), None)).expect("summary");
        // Through 1L there is no interest; through 2L the bond's 40 covers.
        assert_eq!(summary.tranches[0].interest_coverage, None);
        assert_close(summary.tranches[2].interest_coverage, 460.0 / 40.0);
        assert_close(summary.total_coverage, 460.0 / 40.0);
    }

    #[test]
    fn a_trailing_gap_in_a_tranches_rows_nulls_coverage_from_its_class_down() {
        let (mut results, model) = fixture();
        results
            .cs_cashflows
            .as_mut()
            .expect("cashflows")
            .by_instrument
            .get_mut("RCF")
            .expect("revolver")
            .shift_remove(&q(1));
        let summary =
            capital_structure_summary(&results, &model, &config(q(4), None)).expect("summary");
        assert_eq!(summary.tranches[1].ltm_interest, None);
        assert_eq!(summary.tranches[0].ltm_interest, Some(usd(20.0)));
        for tranche in &summary.tranches {
            assert_eq!(tranche.interest_coverage, None);
            assert!(tranche.leverage.is_some());
        }
        assert_eq!(summary.total_coverage, None);
    }

    #[test]
    fn no_payment_classes_means_one_implicit_class() {
        let (results, mut model) = fixture();
        model
            .capital_structure
            .as_mut()
            .expect("capital structure")
            .waterfall = None;
        let summary =
            capital_structure_summary(&results, &model, &config(q(4), None)).expect("summary");
        // Declaration order, one rank, no class id, the figures of the whole stack.
        let ids: Vec<_> = summary.tranches.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["SUB", "TLA", "RCF"]);
        for tranche in &summary.tranches {
            assert_eq!(tranche.rank, 0);
            assert_eq!(tranche.payment_class, None);
            assert_eq!(tranche.cumulative_debt, usd(920.0));
            assert_close(tranche.leverage, 2.0);
            assert_close(tranche.interest_coverage, 7.1875);
        }

        let (results, mut model) = fixture();
        model
            .capital_structure
            .as_mut()
            .and_then(|structure| structure.waterfall.as_mut())
            .expect("waterfall")
            .payment_classes
            .clear();
        let empty =
            capital_structure_summary(&results, &model, &config(q(4), None)).expect("summary");
        assert_eq!(empty, summary);
    }

    #[test]
    fn a_convertible_is_debt_at_its_own_maturity() {
        let (mut results, mut model) = fixture();
        let mut convertible = ConvertibleBond::example().expect("example convertible");
        convertible.maturity = date!(2028 - 12 - 01);
        let structure = model.capital_structure.as_mut().expect("capital structure");
        structure.debt_instruments.push(DebtInstrumentSpec {
            id: "CB".to_string(),
            spec: FinancialStatementInstrument::ConvertibleBond(convertible),
        });
        structure.waterfall = None;
        results
            .cs_cashflows
            .as_mut()
            .expect("cashflows")
            .by_instrument
            .insert(
                "CB".to_string(),
                rows(&[q(1), q(2), q(3), q(4)], 80.0, 0.0, 0.0),
            );
        let summary =
            capital_structure_summary(&results, &model, &config(q(4), None)).expect("summary");
        let cb = summary.tranches.last().expect("convertible");
        assert_eq!(cb.kind, DebtInstrumentKind::ConvertibleBond);
        assert_eq!(cb.maturity, date!(2028 - 12 - 01));
        assert_eq!(summary.maturity_wall[0].year, 2028);
    }

    #[test]
    fn a_monetary_ebitda_in_the_debt_currency_is_accepted() {
        let (mut results, model) = fixture();
        results.monetary_nodes.insert(
            "ebitda".to_string(),
            [
                (q(1), usd(100.0)),
                (q(2), usd(110.0)),
                (q(3), usd(120.0)),
                (q(4), usd(130.0)),
            ]
            .into_iter()
            .collect(),
        );
        let summary =
            capital_structure_summary(&results, &model, &config(q(4), None)).expect("summary");
        assert_close(summary.total_leverage, 2.0);
    }

    #[test]
    fn rejects_a_model_without_a_capital_structure() {
        let (results, mut model) = fixture();
        model.capital_structure = None;
        let message = error_of(&results, &model, &config(q(4), None));
        assert!(message.contains("no capital structure"), "{message}");
    }

    #[test]
    fn rejects_a_stack_of_only_hedges() {
        let (results, _) = fixture();
        let model = model_of("2025Q1..2025Q4", vec![("SWAP", swap())], vec![]);
        let message = error_of(&results, &model, &config(q(4), None));
        assert!(message.contains("no debt instruments"), "{message}");
    }

    #[test]
    fn rejects_results_without_capital_structure_cashflows() {
        let (mut results, model) = fixture();
        results.cs_cashflows = None;
        let message = error_of(&results, &model, &config(q(4), None));
        assert!(message.contains("cs_cashflows"), "{message}");
    }

    #[test]
    fn rejects_a_period_outside_the_model() {
        let (results, model) = fixture();
        let period = PeriodId::quarter(2030, 1).expect("valid quarter");
        let message = error_of(&results, &model, &config(period, None));
        assert!(message.contains("not in the model"), "{message}");
    }

    #[test]
    fn rejects_nodes_missing_from_the_results() {
        let (results, model) = fixture();
        let mut request = config(q(4), None);
        request.ebitda_node = "nope".to_string();
        let message = error_of(&results, &model, &request);
        assert!(message.contains("'nope'"), "{message}");
        let message = error_of(&results, &model, &config(q(4), Some("missing_cash")));
        assert!(message.contains("'missing_cash'"), "{message}");
    }

    #[test]
    fn rejects_a_cash_node_with_no_value_at_the_period() {
        let (mut results, model) = fixture();
        results
            .nodes
            .get_mut("cash")
            .expect("cash")
            .shift_remove(&q(4));
        let message = error_of(&results, &model, &config(q(4), Some("cash")));
        assert!(message.contains("no value in period"), "{message}");
    }

    #[test]
    fn rejects_an_instrument_with_no_row_at_the_period() {
        let (mut results, model) = fixture();
        results
            .cs_cashflows
            .as_mut()
            .expect("cashflows")
            .by_instrument
            .get_mut("TLA")
            .expect("term loan")
            .shift_remove(&q(4));
        let message = error_of(&results, &model, &config(q(4), None));
        assert!(message.contains("'TLA'"), "{message}");
    }

    #[test]
    fn rejects_an_instrument_in_no_class_when_classes_are_declared() {
        let (results, mut model) = fixture();
        model
            .capital_structure
            .as_mut()
            .and_then(|structure| structure.waterfall.as_mut())
            .expect("waterfall")
            .payment_classes = vec![class("1L", 0, &["TLA", "RCF"])];
        let message = error_of(&results, &model, &config(q(4), None));
        assert!(message.contains("'SUB'"), "{message}");
    }

    #[test]
    fn rejects_non_finite_ebitda_and_cash() {
        let (mut results, model) = fixture();
        results
            .nodes
            .get_mut("ebitda")
            .expect("ebitda")
            .insert(q(2), f64::NAN);
        let message = error_of(&results, &model, &config(q(4), None));
        assert!(message.contains("non-finite"), "{message}");

        let (mut results, model) = fixture();
        results
            .nodes
            .get_mut("cash")
            .expect("cash")
            .insert(q(4), f64::INFINITY);
        let message = error_of(&results, &model, &config(q(4), Some("cash")));
        assert!(message.contains("non-finite"), "{message}");
    }

    #[test]
    fn finite_inputs_cannot_produce_an_infinite_ratio() {
        let (mut results, model) = fixture();
        results.nodes.insert(
            "ebitda".to_string(),
            [q(1), q(2), q(3), q(4)]
                .iter()
                .map(|id| (*id, 1e-320))
                .collect(),
        );
        // A subnormal EBITDA is rejected when it is admitted as Money (it is
        // outside the Decimal domain), before any ratio can become infinite.
        let error = capital_structure_summary(&results, &model, &config(q(4), None))
            .expect_err("summary must fail");
        assert!(
            error.to_string().contains("Decimal conversion overflow"),
            "{error}"
        );
    }

    #[test]
    fn rejects_tranches_in_more_than_one_currency() {
        let (mut results, model) = fixture();
        results
            .cs_cashflows
            .as_mut()
            .expect("cashflows")
            .by_instrument
            .insert(
                "SUB".to_string(),
                [(q(4), CashflowBreakdown::with_currency(Currency::EUR))]
                    .into_iter()
                    .collect(),
            );
        let message = error_of(&results, &model, &config(q(4), None));
        assert!(message.contains("no implicit FX"), "{message}");
    }

    #[test]
    fn rejects_trailing_interest_in_another_currency() {
        let (mut results, model) = fixture();
        results
            .cs_cashflows
            .as_mut()
            .expect("cashflows")
            .by_instrument
            .get_mut("SUB")
            .expect("bond")
            .insert(q(1), CashflowBreakdown::with_currency(Currency::EUR));
        let message = error_of(&results, &model, &config(q(4), None));
        assert!(message.contains("no implicit FX"), "{message}");
    }

    #[test]
    fn rejects_monetary_nodes_in_another_currency() {
        for node in ["ebitda", "cash"] {
            let (mut results, model) = fixture();
            results.node_value_types.insert(
                node.to_string(),
                NodeValueType::Monetary {
                    currency: Currency::EUR,
                },
            );
            results.monetary_nodes.insert(
                node.to_string(),
                [q(1), q(2), q(3), q(4)]
                    .iter()
                    .map(|id| {
                        let value = results.get(node, id).expect("fixture value");
                        (
                            *id,
                            Money::new(value, Currency::EUR).expect("finite amount"),
                        )
                    })
                    .collect(),
            );
            let message = error_of(&results, &model, &config(q(4), Some("cash")));
            assert!(message.contains("no implicit FX"), "{node}: {message}");
        }
    }

    #[test]
    fn config_rejects_unknown_fields_and_defaults_cash_to_none() {
        let config: CapitalStructureSummaryConfig =
            serde_json::from_str(r#"{"period": "2025Q4", "ebitda_node": "ebitda"}"#)
                .expect("valid config");
        assert_eq!(config.period, q(4));
        assert_eq!(config.cash_node, None);
        assert!(
            serde_json::from_str::<CapitalStructureSummaryConfig>(
                r#"{"period": "2025Q4", "ebitda_node": "ebitda", "cash": "cash"}"#
            )
            .is_err()
        );
    }

    #[test]
    fn summary_round_trips_through_its_wire_form() {
        let (results, model) = fixture();
        let summary = capital_structure_summary(&results, &model, &config(q(4), Some("cash")))
            .expect("summary");
        let text = serde_json::to_string(&summary).expect("serialize");
        let back: CapitalStructureSummary = serde_json::from_str(&text).expect("deserialize");
        assert_eq!(back, summary);
        let wire: serde_json::Value = serde_json::from_str(&text).expect("json");
        assert_eq!(wire["as_of"], "2025Q4");
        assert_eq!(wire["tranches"][0]["kind"], "term_loan");
        assert_eq!(wire["tranches"][0]["maturity"], "2029-06-30");
    }
}
