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
use std::fmt::{self, Write as FmtWrite};

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

    #[allow(clippy::expect_used)] // write! to String is infallible
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
            write!(&mut output, " {} │", aligned).expect("writing to String cannot fail");
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
                    write!(&mut output, " {} │", aligned).expect("writing to String cannot fail");
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
/// # let model = ModelBuilder::new("demo")
/// #     .periods("2025Q1..Q2", None)?
/// #     .compute("revenue", "100000")?
/// #     .compute("cogs", "40000")?
/// #     .build()?;
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
        TableEnvelope::new_with_metadata(columns, metadata)
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
/// Returns `None` unless a full window (`periods_per_year` of `at`, e.g. 4
/// quarters or 1 annual period) of finite values is available at or before
/// `at`. Incomplete windows (for example Q1 of a quarterly model) are skipped
/// rather than annualized from a partial year.
///
/// # Arguments
///
/// * `results` - Evaluated statement nodes to read. The named node must exist
///   and contribute a value at each period in the trailing window.
/// * `node_id` - Statement node whose period values are summed (typically a
///   flow such as EBITDA or interest expense), in the model's reporting units.
/// * `at` - Inclusive window end. Window length is `at.kind().periods_per_year()`.
pub(crate) fn trailing_sum_at(
    results: &StatementResult,
    node_id: &str,
    at: &PeriodId,
) -> Option<f64> {
    let mut period = *at;
    let mut total = 0.0;
    for index in 0..at.kind().periods_per_year() {
        let value = results.get(node_id, &period)?;
        if !value.is_finite() {
            return None;
        }
        total += value;
        if index + 1 < at.kind().periods_per_year() {
            period = period.prev().ok()?;
        }
    }
    total.is_finite().then_some(total)
}

/// Leverage ratio (total debt / TTM EBITDA) at `at`. `None` if inputs missing
/// or TTM EBITDA is zero.
fn leverage_at(results: &StatementResult, at: &PeriodId) -> Option<f64> {
    let debt = results.get("total_debt", at)?;
    let ebitda = trailing_sum_at(results, "ebitda", at)?;
    (ebitda != 0.0).then_some(debt / ebitda)
}

/// Interest coverage (TTM EBITDA / TTM interest expense) at `at`. `None` if
/// inputs missing or TTM interest is zero.
fn interest_coverage_at(results: &StatementResult, at: &PeriodId) -> Option<f64> {
    let ebitda = trailing_sum_at(results, "ebitda", at)?;
    let interest = trailing_sum_at(results, "interest_expense", at)?;
    (interest != 0.0).then_some(ebitda / interest)
}

/// One period's structured credit metrics. Each metric is `None` when it
/// cannot be computed for that period (e.g. an incomplete TTM window).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
/// # use finstack_quant_core::dates::PeriodId;
/// # use finstack_quant_statements_analytics::analysis::CreditAssessment;
/// # let results = StatementResult::new();
/// let assessment = CreditAssessment::compute(&results, PeriodId::quarter(2025, 4).expect("valid period fixture"));
/// assert_eq!(assessment.period, "2025Q4");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// The series spans every period (≤ `period`) present on any of the driver
    /// nodes (`ebitda`, `total_debt`, `interest_expense`, `free_cash_flow`),
    /// in ascending order.
    pub fn compute(results: &StatementResult, period: PeriodId) -> Self {
        let mut periods: std::collections::BTreeSet<PeriodId> = std::collections::BTreeSet::new();
        for node in ["ebitda", "total_debt", "interest_expense", "free_cash_flow"] {
            if let Some(series) = results.get_node(node) {
                for (observed, _) in series.iter() {
                    if *observed <= period {
                        periods.insert(*observed);
                    }
                }
            }
        }

        let series: Vec<CreditAssessmentPoint> = periods
            .iter()
            .map(|period| CreditAssessmentPoint {
                period: period.to_string(),
                leverage_ratio: leverage_at(results, period),
                interest_coverage: interest_coverage_at(results, period),
                free_cash_flow: results.get("free_cash_flow", period),
            })
            .collect();

        Self {
            period: period.to_string(),
            leverage_ratio: leverage_at(results, &period),
            interest_coverage: interest_coverage_at(results, &period),
            free_cash_flow: results.get("free_cash_flow", &period),
            series,
        }
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
/// # let model = ModelBuilder::new("demo")
/// #     .periods("2025Q1..Q2", None)?
/// #     .compute("revenue", "100000")?
/// #     .build()?;
/// # let mut evaluator = Evaluator::new();
/// # let results = evaluator.evaluate(&model)?;
/// let report = CreditAssessmentReport::new(&results, period);
/// println!("{report}");
/// # Ok(())
/// # }
/// ```
pub struct CreditAssessmentReport<'a> {
    results: &'a StatementResult,
    period: PeriodId,
}

impl<'a> CreditAssessmentReport<'a> {
    /// Create a new credit assessment report.
    pub fn new(results: &'a StatementResult, period: PeriodId) -> Self {
        Self { results, period }
    }

    fn calculate_leverage_ratio(&self) -> Option<f64> {
        leverage_at(self.results, &self.period)
    }

    fn calculate_interest_coverage(&self) -> Option<f64> {
        interest_coverage_at(self.results, &self.period)
    }
}

impl fmt::Display for CreditAssessmentReport<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut output = format!("Credit Assessment as of {}\n\n", self.period);

        if let Some(leverage) = self.calculate_leverage_ratio() {
            output.push_str(&format!("Total Debt / TTM EBITDA:    {:.2}x\n", leverage));
        }

        if let Some(coverage) = self.calculate_interest_coverage() {
            output.push_str(&format!("TTM EBITDA / TTM Interest:  {:.2}x\n", coverage));
        }

        if let Some(fcf) = self.results.get("free_cash_flow", &self.period) {
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

        let period = PeriodId::quarter(2025, 4).expect("valid period fixture");
        let assessment = CreditAssessment::compute(&results, period);

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

        let assessment = CreditAssessment::compute(
            &results,
            PeriodId::quarter(2025, 4).expect("valid period fixture"),
        );

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

        let report = CreditAssessmentReport::new(
            &results,
            PeriodId::quarter(2025, 4).expect("valid period fixture"),
        );

        assert_eq!(report.calculate_leverage_ratio(), Some(3.0));
        assert_eq!(report.calculate_interest_coverage(), Some(10.0));
    }
}
