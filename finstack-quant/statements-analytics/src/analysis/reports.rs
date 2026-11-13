//! Convenience reporting for financial statements: ASCII P&L summaries and
//! structured credit-assessment views over evaluated statement results.
//!
//! # Examples
//!
//! ```no_run
//! use finstack_quant_statements_analytics::analysis::PLSummaryReport;
//! use finstack_quant_statements::evaluator::StatementResult;
//! use finstack_quant_core::dates::PeriodId;
//!
//! # let results: StatementResult = unimplemented!("evaluate a model to obtain StatementResult");
//! let line_items = vec!["revenue", "cogs"];
//! let periods = vec![PeriodId::quarter(2025, 1).expect("valid period fixture")];
//! let report = PLSummaryReport::new(&results, line_items, periods);
//! println!("{report}");
//! ```

use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::table::{TableColumn, TableColumnData, TableColumnRole, TableEnvelope};
use finstack_quant_statements::evaluator::StatementResult;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Alignment {
    #[default]
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub(crate) struct TableBuilder {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    alignment: Vec<Alignment>,
}

impl TableBuilder {
    pub fn new() -> Self {
        Self {
            headers: Vec::new(),
            rows: Vec::new(),
            alignment: Vec::new(),
        }
    }

    pub fn add_header(&mut self, name: impl Into<String>) {
        self.headers.push(name.into());
        self.alignment.push(Alignment::Left);
    }

    pub fn add_header_with_alignment(&mut self, name: impl Into<String>, alignment: Alignment) {
        self.headers.push(name.into());
        self.alignment.push(alignment);
    }

    pub fn add_row(&mut self, cells: Vec<String>) {
        self.rows.push(cells);
    }

    pub fn build(&self) -> String {
        if self.headers.is_empty() {
            return String::new();
        }

        let mut output = String::new();
        let widths = self.calculate_column_widths();

        self.write_border(&mut output, &widths, "┌", "┬", "┐");
        output.push('\n');

        output.push('│');
        for (i, header) in self.headers.iter().enumerate() {
            let width = widths[i];
            let aligned = self.align_text(header, width, self.alignment[i]);
            output.push_str(&format!(" {aligned} │"));
        }
        output.push('\n');

        self.write_border(&mut output, &widths, "├", "┼", "┤");
        output.push('\n');

        for row in &self.rows {
            output.push('│');
            for (i, cell) in row.iter().enumerate() {
                if i < widths.len() {
                    let width = widths[i];
                    let align = if i < self.alignment.len() {
                        self.alignment[i]
                    } else {
                        Alignment::Left
                    };
                    let aligned = self.align_text(cell, width, align);
                    output.push_str(&format!(" {aligned} │"));
                }
            }
            output.push('\n');
        }

        self.write_border(&mut output, &widths, "└", "┴", "┘");

        output
    }

    fn calculate_column_widths(&self) -> Vec<usize> {
        let mut widths = self.headers.iter().map(|h| h.len()).collect::<Vec<_>>();

        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < widths.len() {
                    widths[i] = widths[i].max(cell.len());
                }
            }
        }

        widths
    }

    fn align_text(&self, text: &str, width: usize, alignment: Alignment) -> String {
        let text_len = text.len();
        if text_len >= width {
            return text.to_string();
        }

        let padding = width - text_len;

        match alignment {
            Alignment::Left => format!("{}{}", text, " ".repeat(padding)),
            Alignment::Right => format!("{}{}", " ".repeat(padding), text),
        }
    }

    fn write_border(
        &self,
        output: &mut String,
        widths: &[usize],
        left: &str,
        middle: &str,
        right: &str,
    ) {
        output.push_str(left);
        for (i, &width) in widths.iter().enumerate() {
            output.push_str(&"─".repeat(width + 2));
            if i < widths.len() - 1 {
                output.push_str(middle);
            }
        }
        output.push_str(right);
    }
}

impl Default for TableBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// P&L summary report.
///
/// # Examples
///
/// ```rust
/// # use finstack_quant_statements::builder::ModelBuilder;
/// # use finstack_quant_statements::evaluator::Evaluator;
/// # use finstack_quant_statements_analytics::analysis::PLSummaryReport;
/// # use finstack_quant_core::dates::PeriodId;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
/// # let model = {
/// #     let mut model_builder = ModelBuilder::new("demo");
/// #     model_builder.periods("2025Q1..Q2", None)?;
/// #     model_builder.compute("revenue", "100000")?;
/// #     model_builder.compute("cogs", "40000")?;
/// #     model_builder.build()?
/// # };
/// # let mut evaluator = Evaluator::new();
/// # let results = evaluator.evaluate(&model)?;
/// let report = PLSummaryReport::new(&results, vec!["revenue", "cogs"], vec![period]);
/// println!("{report}");
/// # Ok(())
/// # }
/// ```
pub struct PLSummaryReport<'a> {
    results: &'a StatementResult,
    line_items: Vec<String>,
    periods: Vec<PeriodId>,
}

impl<'a> PLSummaryReport<'a> {
    /// Create a new P&L summary report.
    ///
    /// # Arguments
    ///
    /// * `results` - Evaluation results
    /// * `line_items` - Node IDs to include
    /// * `periods` - Periods to display
    pub fn new(
        results: &'a StatementResult,
        line_items: Vec<impl Into<String>>,
        periods: Vec<PeriodId>,
    ) -> Self {
        Self {
            results,
            line_items: line_items.into_iter().map(|s| s.into()).collect(),
            periods,
        }
    }
}

impl PLSummaryReport<'_> {
    /// Export the report as a long table: one row per (line item, period).
    ///
    /// Columns: `line_item` (Utf8), `period` (Utf8), and `value`
    /// (nullable Float64). Missing line items are `null` rather than `0.0`,
    /// mirroring the `-` cells in the text rendering. Values are in the node's
    /// own units (monetary nodes are unwrapped to their decimal amount).
    ///
    /// # Errors
    ///
    /// Returns an error if the table envelope cannot be assembled (column
    /// length mismatch), which cannot happen for the columns built here.
    pub fn to_table(&self) -> finstack_quant_core::Result<TableEnvelope> {
        let capacity = self.line_items.len() * self.periods.len();
        let mut line_items = Vec::with_capacity(capacity);
        let mut periods = Vec::with_capacity(capacity);
        let mut values = Vec::with_capacity(capacity);
        for line_item in &self.line_items {
            for period in &self.periods {
                line_items.push(line_item.clone());
                periods.push(period.to_string());
                values.push(self.results.get(line_item, period));
            }
        }
        let columns = vec![
            TableColumn::new("line_item", TableColumnData::String(line_items))
                .with_role(TableColumnRole::Dimension),
            TableColumn::new("period", TableColumnData::String(periods))
                .with_role(TableColumnRole::Index),
            TableColumn::new("value", TableColumnData::NullableFloat64(values))
                .with_role(TableColumnRole::Measure),
        ];
        let mut metadata = IndexMap::new();
        metadata.insert("layout".to_string(), serde_json::json!("long"));
        metadata.insert("source".to_string(), serde_json::json!("pl_summary_report"));
        TableEnvelope::new(columns, metadata)
    }
}

impl fmt::Display for PLSummaryReport<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut table = TableBuilder::new();

        table.add_header("Line Item");
        for period in &self.periods {
            table.add_header_with_alignment(period.to_string(), Alignment::Right);
        }

        for line_item in &self.line_items {
            let mut row = vec![line_item.clone()];
            for period in &self.periods {
                // Render missing line items as "-" rather than a misleading
                // 0.00 (zero is a legitimate value, absence is not).
                let cell = match self.results.get(line_item, period) {
                    Some(value) => format!("{:.2}", value),
                    None => "-".to_string(),
                };
                row.push(cell);
            }
            table.add_row(row);
        }

        write!(formatter, "P&L Summary\n\n{}", table.build())
    }
}

/// Trailing-twelve-month sum of `node_id` ending at (and including) `at`.
///
/// Sums the window of [`ltm_window`](finstack_quant_core::dates::ltm_window)
/// over the results' periods: 1, 2, 4, 12 or 52 periods for annual,
/// semi-annual, quarterly, monthly and weekly models (Gregorian or fiscal), and
/// every model day in the twelve calendar months ending at `at` for a daily
/// calendar-day or business-day model. Requires a complete window of finite
/// values in consistent units; incomplete windows are unavailable rather than
/// annualized from partial data.
///
/// # Arguments
///
/// * `results` - Evaluated statement nodes and their periods. The named node
///   must exist and contribute a value at each period in the trailing window.
/// * `node_id` - Statement node whose period values are summed (typically a
///   flow such as EBITDA or interest expense), in the model's reporting units.
/// * `at` - Inclusive reporting-period end.
///
/// # Errors
///
/// Returns an invalid-input error when the results carry no period plan,
/// and propagates a malformed trailing window (`at` absent from the plan, a
/// weekly `at`, or a stepping/date-domain failure) instead of reporting it as
/// unavailable (record CORE-CONVENTIONS-014 #12). An incomplete window or a
/// missing, non-finite or unit-inconsistent value is `Ok(None)`.
pub(crate) fn trailing_sum_at(
    results: &StatementResult,
    node_id: &str,
    at: &PeriodId,
) -> finstack_quant_statements::Result<Option<f64>> {
    // A node with no value at the target has no trailing sum, whatever the
    // plan; only a window over present data needs the plan.
    if results.get(node_id, at).is_none() {
        return Ok(None);
    }
    let Ok(unit) = super::units::node_unit_at(results, node_id, at) else {
        return Ok(None);
    };
    let plan = results.periods.as_ref().ok_or_else(|| {
        finstack_quant_statements::Error::invalid_input(
            "trailing sums need the statement results' period plan",
        )
    })?;
    let window = finstack_quant_core::dates::ltm_window(*at, plan)?;
    if !window.is_complete() {
        return Ok(None);
    }
    let mut total = 0.0;
    for period in &window.slots {
        if super::units::node_unit_at(results, node_id, period).ok() != Some(unit) {
            return Ok(None);
        }
        let Some(value) = results.get(node_id, period) else {
            return Ok(None);
        };
        if !value.is_finite() {
            return Ok(None);
        }
        total += value;
    }
    Ok(total.is_finite().then_some(total))
}

/// Leverage ratio (total debt / TTM EBITDA) at `at`. `None` if inputs missing
/// or TTM EBITDA is zero.
fn leverage_at(
    results: &StatementResult,
    at: &PeriodId,
) -> finstack_quant_statements::Result<Option<f64>> {
    if super::units::validate_matching_units(results, "total_debt", "ebitda", at).is_err() {
        return Ok(None);
    }
    let Some(debt) = results.get("total_debt", at) else {
        return Ok(None);
    };
    let Some(ebitda) = trailing_sum_at(results, "ebitda", at)? else {
        return Ok(None);
    };
    let ratio = debt / ebitda;
    Ok((debt.is_finite() && ebitda > 0.0 && ratio.is_finite()).then_some(ratio))
}

/// Interest coverage (TTM EBITDA / TTM interest expense) at `at`. `None` if
/// inputs missing or TTM interest is zero.
fn interest_coverage_at(
    results: &StatementResult,
    at: &PeriodId,
) -> finstack_quant_statements::Result<Option<f64>> {
    if super::units::validate_matching_units(results, "ebitda", "interest_expense", at).is_err() {
        return Ok(None);
    }
    let (Some(ebitda), Some(interest)) = (
        trailing_sum_at(results, "ebitda", at)?,
        trailing_sum_at(results, "interest_expense", at)?,
    ) else {
        return Ok(None);
    };
    let ratio = ebitda / interest;
    Ok((interest > 0.0 && ratio.is_finite()).then_some(ratio))
}

/// One period's structured credit metrics. Each metric is `None` when it
/// cannot be computed for that period (e.g. an incomplete TTM window, mixed
/// scalar/monetary representations, or different input currencies).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CreditAssessmentPoint {
    /// Period identifier rendered as a string (e.g. `"2025Q4"`).
    pub period: String,
    /// Total debt / TTM EBITDA.
    pub leverage_ratio: Option<f64>,
    /// TTM EBITDA / TTM interest expense.
    pub interest_coverage: Option<f64>,
    /// Free cash flow at this period.
    pub free_cash_flow: Option<f64>,
}

/// Structured credit assessment: leverage, interest coverage, and free cash
/// flow at a `period` plus a per-period series for trend display.
///
/// This is the structured counterpart of [`CreditAssessmentReport`]; both share
/// the same TTM computation so the numbers always agree.
///
/// # Examples
///
/// ```rust
/// # use finstack_quant_statements::evaluator::StatementResult;
/// # use finstack_quant_core::dates::{Period, PeriodId};
/// # use finstack_quant_statements_analytics::analysis::CreditAssessment;
/// let q4 = PeriodId::quarter(2025, 4)?;
/// let mut results = StatementResult::new();
/// results.periods.get_or_insert_with(|| finstack_quant_core::dates::PeriodPlan::try_new(finstack_quant_core::dates::PeriodCalendar::Gregorian, Vec::new()).expect("valid period plan")).periods.push(Period {
///     id: q4,
///     start: time::macros::date!(2025 - 10 - 01),
///     end: time::macros::date!(2026 - 01 - 01),
///     is_actual: false,
/// });
/// results.nodes.entry("total_debt".into()).or_default().insert(q4, 300.0);
/// let assessment = CreditAssessment::compute(&results, q4)?;
/// assert_eq!(assessment.period, "2025Q4");
/// // A period the results do not contain is an error, not a null assessment.
/// assert!(CreditAssessment::compute(&results, PeriodId::annual(2025).expect("valid annual period")).is_err());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CreditAssessment {
    /// Assessment period rendered as a string (e.g. `"2025Q4"`).
    pub period: String,
    /// Leverage ratio at `period`.
    pub leverage_ratio: Option<f64>,
    /// Interest coverage at `period`.
    pub interest_coverage: Option<f64>,
    /// Free cash flow at `period`.
    pub free_cash_flow: Option<f64>,
    /// Per-period series (ascending) for trend display.
    pub series: Vec<CreditAssessmentPoint>,
}

impl CreditAssessment {
    /// Compute a structured credit assessment from statement results.
    ///
    /// The series spans every period of [`StatementResult::periods`] up to and
    /// including `period`, in timeline order. Ratios are unavailable for incompatible node types
    /// or currencies, incomplete trailing years, or non-positive denominators.
    /// Windows step through the results' period plan in its own calendar
    /// ([`finstack_quant_core::dates::ltm_window`]); a daily plan needs every
    /// calendar day of the trailing year, leap days included.
    ///
    /// # Arguments
    ///
    /// * `results` - Evaluated statement values with monetary/scalar metadata
    ///   preserved so ratios only compare like units and currencies.
    /// * `period` - Inclusive reporting period ending each trailing-year window.
    ///   It must be one of [`StatementResult::periods`], in the model's own
    ///   frequency: `2025` is the annual period, not
    ///   the last quarter of a quarterly model.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error when `period` is not one of
    /// [`StatementResult::periods`], so an unknown or wrong-frequency period never yields an
    /// all-null assessment or a series truncated at a foreign period, and
    /// the trailing-window error when the results' plan is weekly.
    pub fn compute(
        results: &StatementResult,
        period: PeriodId,
    ) -> finstack_quant_statements::Result<Self> {
        if !results.get_periods().iter().any(|p| p.id == period) {
            return Err(finstack_quant_statements::Error::invalid_input(format!(
                "Credit assessment period '{period}' is not a period of the statement results"
            )));
        }
        let periods = results
            .get_periods()
            .iter()
            .map(|p| p.id)
            .filter(|observed| *observed <= period);

        let series: Vec<CreditAssessmentPoint> = periods
            .map(|period| {
                Ok(CreditAssessmentPoint {
                    period: period.to_string(),
                    leverage_ratio: leverage_at(results, &period)?,
                    interest_coverage: interest_coverage_at(results, &period)?,
                    free_cash_flow: results
                        .get("free_cash_flow", &period)
                        .filter(|value| value.is_finite()),
                })
            })
            .collect::<finstack_quant_statements::Result<_>>()?;

        Ok(Self {
            period: period.to_string(),
            leverage_ratio: leverage_at(results, &period)?,
            interest_coverage: interest_coverage_at(results, &period)?,
            free_cash_flow: results
                .get("free_cash_flow", &period)
                .filter(|value| value.is_finite()),
            series,
        })
    }
}

/// Credit assessment report.
///
/// # Examples
///
/// ```rust
/// # use finstack_quant_statements::builder::ModelBuilder;
/// # use finstack_quant_statements::evaluator::Evaluator;
/// # use finstack_quant_statements_analytics::analysis::CreditAssessmentReport;
/// # use finstack_quant_core::dates::PeriodId;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
/// # let model = {
/// #     let mut model_builder = ModelBuilder::new("demo");
/// #     model_builder.periods("2025Q1..Q2", None)?;
/// #     model_builder.compute("revenue", "100000")?;
/// #     model_builder.build()?
/// # };
/// # let mut evaluator = Evaluator::new();
/// # let results = evaluator.evaluate(&model)?;
/// let report = CreditAssessmentReport::new(&results, period)?;
/// println!("{report}");
/// # Ok(())
/// # }
/// ```
pub struct CreditAssessmentReport {
    period: PeriodId,
    leverage_ratio: Option<f64>,
    interest_coverage: Option<f64>,
    free_cash_flow: Option<f64>,
}

impl CreditAssessmentReport {
    /// Compute the credit assessment of `results` at `period`.
    ///
    /// # Arguments
    ///
    /// * `results` - Evaluated statements; trailing-twelve-month windows step
    ///   through their period plan, so results holding `ebitda` or
    ///   `interest_expense` must carry one.
    /// * `period` - Assessment period: point-in-time `total_debt` and
    ///   `free_cash_flow` are read here, and TTM windows end here.
    ///
    /// # Errors
    ///
    /// Returns an error when a TTM window cannot be formed: the results hold
    /// a flow node but no period plan, `period` is not in the plan, or the
    /// plan is weekly (record CORE-CONVENTIONS-014 #11, #12).
    pub fn new(
        results: &StatementResult,
        period: PeriodId,
    ) -> finstack_quant_statements::Result<Self> {
        Ok(Self {
            period,
            leverage_ratio: leverage_at(results, &period)?,
            interest_coverage: interest_coverage_at(results, &period)?,
            free_cash_flow: results.get("free_cash_flow", &period),
        })
    }
}

impl fmt::Display for CreditAssessmentReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut output = format!("Credit Assessment as of {}\n\n", self.period);

        if let Some(leverage) = self.leverage_ratio {
            output.push_str(&format!("Total Debt / TTM EBITDA:    {:.2}x\n", leverage));
        }

        if let Some(coverage) = self.interest_coverage {
            output.push_str(&format!("TTM EBITDA / TTM Interest:  {:.2}x\n", coverage));
        }

        if let Some(fcf) = self.free_cash_flow {
            output.push_str(&format!(
                "Free Cash Flow:             ${:.2}M\n",
                fcf / 1_000_000.0
            ));
        }

        formatter.write_str(&output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::PeriodKind;

    /// Record every period a node has a value in as a result period; only the
    /// ids matter to fixed-frequency windows. Fiscal fixtures here use an
    /// October 1 opening; unprefixed ids are Gregorian.
    fn record_periods(results: &mut StatementResult) {
        let mut ids: Vec<PeriodId> = results
            .nodes
            .values()
            .flat_map(|values| values.keys().copied())
            .collect();
        ids.sort();
        ids.dedup();
        let calendar = if ids.first().is_some_and(PeriodId::is_fiscal) {
            finstack_quant_core::dates::PeriodCalendar::Fiscal(
                finstack_quant_core::dates::FiscalConfig::new(10, 1).expect("valid fiscal start"),
            )
        } else {
            finstack_quant_core::dates::PeriodCalendar::Gregorian
        };
        results.periods = Some(
            finstack_quant_core::dates::PeriodPlan::try_new(
                calendar,
                ids.into_iter()
                    .map(|id| finstack_quant_core::dates::Period {
                        id,
                        start: time::macros::date!(2000 - 01 - 01),
                        end: time::macros::date!(2000 - 01 - 02),
                        is_actual: false,
                    })
                    .collect(),
            )
            .expect("valid period plan"),
        );
    }

    #[test]
    fn test_empty_table() {
        let table = TableBuilder::new();
        assert_eq!(table.build(), "");
    }

    #[test]
    fn test_simple_table() {
        let mut table = TableBuilder::new();
        table.add_header("Name");
        table.add_header("Value");
        table.add_row(vec!["Revenue".to_string(), "100".to_string()]);
        table.add_row(vec!["COGS".to_string(), "40".to_string()]);

        let output = table.build();
        assert!(output.contains("Name"));
        assert!(output.contains("Value"));
        assert!(output.contains("Revenue"));
        assert!(output.contains("COGS"));
        assert!(output.contains("┌"));
        assert!(output.contains("└"));
    }

    #[test]
    fn test_alignment() {
        let table = TableBuilder::new();
        assert_eq!(table.align_text("test", 10, Alignment::Left), "test      ");
        assert_eq!(table.align_text("test", 10, Alignment::Right), "      test");
    }

    #[test]
    fn test_column_width_calculation() {
        let mut table = TableBuilder::new();
        table.add_header("A");
        table.add_header("B");
        table.add_row(vec!["short".to_string(), "much longer text".to_string()]);

        let widths = table.calculate_column_widths();
        assert_eq!(widths[0], 5); // "short" is longer than "A"
        assert_eq!(widths[1], 16); // "much longer text" is longer than "B"
    }

    #[test]
    fn pl_summary_renders_dash_for_missing_values() {
        let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let q2 = PeriodId::quarter(2025, 2).expect("valid period fixture");

        let mut results = StatementResult::new();
        // revenue only exists in Q1; Q2 is missing (not zero).
        results
            .nodes
            .entry("revenue".to_string())
            .or_default()
            .insert(q1, 1234.5);

        let report = PLSummaryReport::new(&results, vec!["revenue"], vec![q1, q2]);
        let output = report.to_string();

        assert!(
            output.contains("1234.50"),
            "present value renders: {output}"
        );
        assert!(
            output.contains('-'),
            "missing value renders as dash: {output}"
        );
        assert!(
            !output.contains("0.00"),
            "missing value must not render as 0.00: {output}"
        );
    }

    #[test]
    fn credit_assessment_compute_matches_report_scalars() {
        let mut results = StatementResult::new();
        for (quarter, ebitda) in [(1, 10.0), (2, 20.0), (3, 30.0), (4, 40.0)] {
            results
                .nodes
                .entry("ebitda".to_string())
                .or_default()
                .insert(
                    PeriodId::quarter(2025, quarter).expect("valid period fixture"),
                    ebitda,
                );
        }
        for (quarter, interest) in [(1, 1.0), (2, 2.0), (3, 3.0), (4, 4.0)] {
            results
                .nodes
                .entry("interest_expense".to_string())
                .or_default()
                .insert(
                    PeriodId::quarter(2025, quarter).expect("valid period fixture"),
                    interest,
                );
        }
        results
            .nodes
            .entry("total_debt".to_string())
            .or_default()
            .insert(
                PeriodId::quarter(2025, 4).expect("valid period fixture"),
                300.0,
            );

        record_periods(&mut results);
        let period = PeriodId::quarter(2025, 4).expect("valid period fixture");
        let assessment = CreditAssessment::compute(&results, period).expect("known period");

        assert_eq!(assessment.period, period.to_string());
        assert_eq!(assessment.leverage_ratio, Some(3.0));
        assert_eq!(assessment.interest_coverage, Some(10.0));
        assert_eq!(assessment.free_cash_flow, None);
    }

    #[test]
    fn credit_assessment_series_covers_periods_with_ttm() {
        let mut results = StatementResult::new();
        for (quarter, ebitda) in [(1, 10.0), (2, 20.0), (3, 30.0), (4, 40.0)] {
            results
                .nodes
                .entry("ebitda".to_string())
                .or_default()
                .insert(
                    PeriodId::quarter(2025, quarter).expect("valid period fixture"),
                    ebitda,
                );
        }
        results
            .nodes
            .entry("total_debt".to_string())
            .or_default()
            .insert(
                PeriodId::quarter(2025, 4).expect("valid period fixture"),
                300.0,
            );

        record_periods(&mut results);
        let assessment = CreditAssessment::compute(
            &results,
            PeriodId::quarter(2025, 4).expect("valid period fixture"),
        )
        .expect("known period");

        let q4 = assessment
            .series
            .iter()
            .find(|p| {
                p.period
                    == PeriodId::quarter(2025, 4)
                        .expect("valid period fixture")
                        .to_string()
            })
            .expect("Q4 point present");
        assert_eq!(q4.leverage_ratio, Some(3.0));

        let q1 = assessment
            .series
            .iter()
            .find(|p| {
                p.period
                    == PeriodId::quarter(2025, 1)
                        .expect("valid period fixture")
                        .to_string()
            })
            .expect("Q1 point present");
        assert_eq!(q1.leverage_ratio, None);
    }

    #[test]
    fn credit_report_uses_ttm_ebitda_for_leverage() {
        let mut results = StatementResult::new();
        for (quarter, ebitda) in [(1, 10.0), (2, 20.0), (3, 30.0), (4, 40.0)] {
            results
                .nodes
                .entry("ebitda".to_string())
                .or_default()
                .insert(
                    PeriodId::quarter(2025, quarter).expect("valid period fixture"),
                    ebitda,
                );
        }
        results
            .nodes
            .entry("interest_expense".to_string())
            .or_default()
            .insert(
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                1.0,
            );
        results
            .nodes
            .entry("interest_expense".to_string())
            .or_default()
            .insert(
                PeriodId::quarter(2025, 2).expect("valid period fixture"),
                2.0,
            );
        results
            .nodes
            .entry("interest_expense".to_string())
            .or_default()
            .insert(
                PeriodId::quarter(2025, 3).expect("valid period fixture"),
                3.0,
            );
        results
            .nodes
            .entry("interest_expense".to_string())
            .or_default()
            .insert(
                PeriodId::quarter(2025, 4).expect("valid period fixture"),
                4.0,
            );
        results
            .nodes
            .entry("total_debt".to_string())
            .or_default()
            .insert(
                PeriodId::quarter(2025, 4).expect("valid period fixture"),
                300.0,
            );

        record_periods(&mut results);
        let report = CreditAssessmentReport::new(
            &results,
            PeriodId::quarter(2025, 4).expect("valid period fixture"),
        )
        .expect("valid window");

        assert_eq!(report.leverage_ratio, Some(3.0));
        assert_eq!(report.interest_coverage, Some(10.0));
    }

    #[test]
    fn fiscal_quarterly_credit_window_crosses_year_boundary() {
        let mut results = StatementResult::new();
        for label in ["FY2024Q3", "FY2024Q4", "FY2025Q1", "FY2025Q2"] {
            let period: PeriodId = label.parse().expect("fiscal quarter");
            results
                .nodes
                .entry("ebitda".into())
                .or_default()
                .insert(period, 1.0);
            results
                .nodes
                .entry("interest_expense".into())
                .or_default()
                .insert(period, 0.5);
        }
        let at: PeriodId = "FY2025Q2".parse().expect("fiscal quarter");
        results
            .nodes
            .entry("total_debt".into())
            .or_default()
            .insert(at, 4.0);
        record_periods(&mut results);
        let assessment = CreditAssessment::compute(&results, at).expect("known period");
        assert_eq!(assessment.leverage_ratio, Some(1.0));
        assert_eq!(assessment.interest_coverage, Some(2.0));
        results
            .nodes
            .get_mut("ebitda")
            .expect("ebitda")
            .swap_remove(&"FY2024Q4".parse::<PeriodId>().expect("quarter"));
        assert_eq!(
            CreditAssessment::compute(&results, at)
                .expect("known period")
                .leverage_ratio,
            None
        );
    }

    #[test]
    fn daily_credit_windows_use_calendar_years_including_leap_days() {
        for (year, days) in [(2024, 366), (2025, 365)] {
            let mut results = StatementResult::new();
            for ordinal in 1..=days {
                let period = PeriodId::day(year, ordinal).expect("day");
                let (start, end) = period.bounds().expect("bounds");
                results
                    .periods
                    .get_or_insert_with(|| {
                        finstack_quant_core::dates::PeriodPlan::try_new(
                            finstack_quant_core::dates::PeriodCalendar::Gregorian,
                            Vec::new(),
                        )
                        .expect("valid period plan")
                    })
                    .periods
                    .push(finstack_quant_core::dates::Period {
                        id: period,
                        start,
                        end,
                        is_actual: true,
                    });
                results
                    .nodes
                    .entry("ebitda".into())
                    .or_default()
                    .insert(period, 1.0);
            }
            let at = PeriodId::day(year, days).expect("last day");
            results
                .nodes
                .entry("total_debt".into())
                .or_default()
                .insert(at, f64::from(days));
            assert_eq!(
                CreditAssessment::compute(&results, at)
                    .expect("known period")
                    .leverage_ratio,
                Some(1.0)
            );
            // A gap near the beginning must not be replaced with a shorter
            // 252-observation financial-market annualization window.
            results
                .nodes
                .get_mut("ebitda")
                .expect("ebitda")
                .swap_remove(&PeriodId::day(year, 1).expect("day"));
            assert_eq!(
                CreditAssessment::compute(&results, at)
                    .expect("known period")
                    .leverage_ratio,
                None
            );
        }
    }

    #[test]
    fn business_day_credit_window_is_not_a_complete_daily_year() {
        use finstack_quant_core::dates::{Date, Period};
        use time::Month;
        // Two years of weekdays only: absent rows do not prove weekends or
        // holidays, so a business-day grid is never a complete daily LTM
        // (record CORE-CONVENTIONS-014 #11).
        let mut results = StatementResult::new();
        let mut date = Date::from_calendar_date(2024, Month::January, 1).expect("start");
        let last = Date::from_calendar_date(2025, Month::December, 31).expect("end");
        while date <= last {
            if date.weekday().number_days_from_monday() < 5 {
                let id = PeriodId::from_date(date, PeriodKind::Daily, None).expect("day");
                results
                    .periods
                    .get_or_insert_with(|| {
                        finstack_quant_core::dates::PeriodPlan::try_new(
                            finstack_quant_core::dates::PeriodCalendar::Gregorian,
                            Vec::new(),
                        )
                        .expect("valid period plan")
                    })
                    .periods
                    .push(Period {
                        id,
                        start: date,
                        end: date.next_day().expect("next day"),
                        is_actual: true,
                    });
                results
                    .nodes
                    .entry("ebitda".into())
                    .or_default()
                    .insert(id, 1.0);
            }
            date = date.next_day().expect("next day");
        }
        let at = PeriodId::from_date(last, PeriodKind::Daily, None).expect("day");
        assert_eq!(
            trailing_sum_at(&results, "ebitda", &at).expect("valid window"),
            None
        );
        // A year-end inside the first year of history has no full window.
        let early = PeriodId::from_date(
            Date::from_calendar_date(2024, Month::December, 30).expect("day"),
            PeriodKind::Daily,
            None,
        )
        .expect("day");
        assert_eq!(
            trailing_sum_at(&results, "ebitda", &early).expect("valid window"),
            None
        );
    }

    #[test]
    fn daily_rolling_year_handles_february_boundaries() {
        use finstack_quant_core::dates::Date;
        use time::Month;
        for (start, end, expected) in [
            (
                (2023, Month::February, 28),
                (2024, Month::February, 29),
                366.0,
            ),
            ((2023, Month::March, 1), (2024, Month::March, 1), 366.0),
            ((2024, Month::March, 1), (2025, Month::March, 1), 365.0),
        ] {
            let mut date = Date::from_calendar_date(start.0, start.1, start.2).expect("start");
            let end = Date::from_calendar_date(end.0, end.1, end.2).expect("end");
            let mut results = StatementResult::new();
            while date < end {
                let period =
                    PeriodId::from_date(date, PeriodKind::Daily, None).expect("daily period");
                results
                    .periods
                    .get_or_insert_with(|| {
                        finstack_quant_core::dates::PeriodPlan::try_new(
                            finstack_quant_core::dates::PeriodCalendar::Gregorian,
                            Vec::new(),
                        )
                        .expect("valid period plan")
                    })
                    .periods
                    .push(finstack_quant_core::dates::Period {
                        id: period,
                        start: date,
                        end: date.next_day().expect("next day"),
                        is_actual: true,
                    });
                results
                    .nodes
                    .entry("ebitda".into())
                    .or_default()
                    .insert(period, 1.0);
                date = date.next_day().expect("next day");
            }
            let at = PeriodId::from_date(
                end.previous_day().expect("last day"),
                PeriodKind::Daily,
                None,
            )
            .expect("daily period");
            assert_eq!(
                trailing_sum_at(&results, "ebitda", &at).expect("valid window"),
                Some(expected)
            );
        }
    }

    #[test]
    fn credit_windows_without_a_full_year_of_periods_are_unavailable() {
        // A daily window without a year of days is unavailable; a weekly
        // window is not twelve months, so it is an error (record
        // CORE-CONVENTIONS-014 #11, #12).
        for (label, weekly) in [
            ("2025W52", true),
            ("FY2025W52", true),
            ("FY2025D365", false),
        ] {
            let at: PeriodId = label.parse().expect("period");
            let mut results = StatementResult::new();
            results
                .nodes
                .entry("ebitda".into())
                .or_default()
                .insert(at, 1.0);
            record_periods(&mut results);
            let window = trailing_sum_at(&results, "ebitda", &at);
            if weekly {
                assert!(window.is_err(), "{label}");
            } else {
                assert_eq!(window.expect("valid window"), None, "{label}");
            }
        }
    }
}
