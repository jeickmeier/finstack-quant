//! Stateful `Performance` orchestrator over the analytics building blocks.
//!
//! Implementation is split across sibling files (`scalar`, `rolling`,
//! `benchmark`, `aggregation`); each adds `impl Performance` methods.
//! Public re-exports happen from `lib.rs`.

use std::collections::HashSet;

use finstack_quant_core::dates::{Date, PeriodKind};

use super::drawdown::to_drawdown_series;
use super::returns::pairwise_returns;

pub use panel::{PanelSeriesKind, PerformancePanel};
pub use scalar::validate_summary;

mod aggregation;
mod benchmark;
mod panel;
mod rolling;
mod scalar;
mod serialization;

/// A price panel from the five values a test supplies.
#[cfg(test)]
pub(crate) fn prices_panel(
    dates: Vec<Date>,
    series: Vec<Vec<f64>>,
    ticker_names: Vec<String>,
    benchmark_ticker: Option<&str>,
    frequency: PeriodKind,
) -> PerformancePanel {
    PerformancePanel {
        dates,
        series,
        kind: PanelSeriesKind::Prices,
        ticker_names,
        benchmark_ticker: benchmark_ticker.map(str::to_owned),
        frequency: Some(frequency),
    }
}

/// A simple-return panel from the five values a test supplies.
#[cfg(test)]
pub(crate) fn returns_panel(
    dates: Vec<Date>,
    series: Vec<Vec<f64>>,
    ticker_names: Vec<String>,
    benchmark_ticker: Option<&str>,
    frequency: PeriodKind,
) -> PerformancePanel {
    PerformancePanel {
        kind: PanelSeriesKind::Returns,
        ..prices_panel(dates, series, ticker_names, benchmark_ticker, frequency)
    }
}

/// Central performance analytics engine.
///
/// Holds pre-computed returns, drawdowns, and benchmark data for a universe of
/// tickers. Methods delegate to the pure-function sub-modules.
///
/// The facade follows one shape convention throughout: scalar methods return
/// one value per ticker in the same order as `ticker_names`, while per-ticker
/// rolling and episode methods take a zero-based ticker index.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_analytics::{PanelSeriesKind, Performance, PerformancePanel};
/// use finstack_quant_core::dates::{Date, PeriodKind};
/// use time::Month;
///
/// let dates: Vec<Date> = (1..=6)
///     .map(|d| Date::from_calendar_date(2025, Month::January, d).unwrap())
///     .collect();
/// let benchmark = vec![100.0, 101.0, 99.0, 102.0, 101.0, 103.0];
/// let portfolio = vec![100.0, 103.0, 100.0, 104.0, 102.0, 106.0];
///
/// let mut perf = Performance::new(&PerformancePanel {
///     dates,
///     series: vec![benchmark, portfolio],
///     kind: PanelSeriesKind::Prices,
///     ticker_names: vec!["SPY".to_string(), "ALPHA".to_string()],
///     benchmark_ticker: Some("SPY".to_string()),
///     frequency: Some(PeriodKind::Daily),
/// })?;
///
/// let sharpe = perf.sharpe(0.02)?;
/// let beta = perf.beta()?;
/// let rolling = perf.rolling_sharpe(1, 3, 0.02)?;
/// assert_eq!(sharpe.len(), 2);
/// assert_eq!(beta.len(), 2);
/// assert_eq!(rolling.values.len(), 3);
///
/// perf.reset_date_range(
///     Date::from_calendar_date(2025, Month::January, 3).unwrap(),
///     Date::from_calendar_date(2025, Month::January, 6).unwrap(),
/// )?;
/// assert_eq!(perf.cagr(finstack_quant_analytics::CagrDayCount::default(), finstack_quant_core::dates::DayCountContext::default())?.len(), 2);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "serialization::PerformanceState")]
pub struct Performance {
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    price_dates: Vec<Date>,
    returns: Vec<Vec<f64>>,
    return_spans: Vec<TickerSpan>,
    ticker_names: Vec<String>,
    benchmark_idx: Option<usize>,
    #[serde(skip_serializing)]
    drawdowns: Vec<Vec<f64>>,
    #[serde(skip_serializing)]
    active_window_drawdowns: Option<Vec<Vec<f64>>>,
    frequency: Option<PeriodKind>,
    start_idx: usize,
    end_idx: usize,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct TickerSpan {
    start: usize,
    end: usize,
}

impl TickerSpan {
    fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    fn intersect(self, other: Self) -> Self {
        let start = self.start.max(other.start);
        let end = self.end.min(other.end).max(start);
        Self { start, end }
    }

    fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

fn invalid_return_series(
    ticker: impl Into<String>,
    index: usize,
    reason: impl Into<String>,
) -> finstack_quant_core::InputError {
    finstack_quant_core::InputError::InvalidReturnSeries {
        ticker: ticker.into(),
        index,
        reason: reason.into(),
    }
}

/// Zero-based column of `ticker` in `ticker_names`.
///
/// Returns [`finstack_quant_core::InputError::NotFound`], naming the loaded
/// tickers, when `ticker` is absent.
fn ticker_index(ticker_names: &[String], ticker: &str) -> finstack_quant_core::Result<usize> {
    ticker_names
        .iter()
        .position(|name| name == ticker)
        .ok_or_else(|| {
            finstack_quant_core::InputError::NotFound {
                id: format!("ticker '{ticker}' (loaded: {ticker_names:?})"),
            }
            .into()
        })
}

fn validate_ticker_names(ticker_names: &[String]) -> finstack_quant_core::Result<()> {
    let mut names = HashSet::with_capacity(ticker_names.len());
    for (index, name) in ticker_names.iter().enumerate() {
        if name.trim().is_empty() {
            return Err(invalid_return_series(name, index, "ticker name must not be blank").into());
        }
        if !names.insert(name) {
            return Err(invalid_return_series(name, index, "duplicate ticker name").into());
        }
    }
    Ok(())
}

/// Reject empty, duplicate, or non-monotonic date inputs at construction.
///
/// Several lookback and aggregation paths use [`slice::partition_point`] on
/// the internal date grid; both rely on strictly ascending order. Validating
/// once at the boundary keeps the rest of the crate slice-only.
fn validate_strictly_ascending_dates(dates: &[Date]) -> finstack_quant_core::Result<()> {
    for (idx, pair) in dates.windows(2).enumerate() {
        let (prev, next) = (pair[0], pair[1]);
        if next <= prev {
            let reason = if next == prev {
                format!("duplicate date {next} at index {} and {}", idx, idx + 1)
            } else {
                format!(
                    "dates not strictly ascending: index {} ({prev}) is not before index {} ({next})",
                    idx,
                    idx + 1
                )
            };
            tracing::debug!(
                index = idx,
                ?prev,
                ?next,
                reason = "non_monotonic_dates",
                "rejecting Performance construction"
            );
            return Err(invalid_return_series("<panel>", idx + 1, reason).into());
        }
    }
    Ok(())
}

/// Shape checks shared by both panel kinds of [`Performance::new`]:
/// non-empty columns and dates, strictly ascending dates, and one ticker name
/// per column. `what` names the value columns (`"prices"` / `"returns"`) in
/// the error text.
fn check_panel_inputs(
    dates: &[Date],
    n_columns: usize,
    n_names: usize,
    what: &str,
) -> finstack_quant_core::Result<()> {
    if n_columns == 0 || dates.len() < 2 {
        return Err(
            invalid_return_series("<panel>", 0, format!("{what} or dates is empty")).into(),
        );
    }
    validate_strictly_ascending_dates(dates)?;
    if n_names != n_columns {
        return Err(invalid_return_series(
            "<panel>",
            0,
            format!("ticker_names.len() = {n_names} does not match {what}.len() = {n_columns}"),
        )
        .into());
    }
    Ok(())
}

fn price_column_to_returns(
    col: &[f64],
    ticker: &str,
    expected_price_len: usize,
) -> finstack_quant_core::Result<(Vec<f64>, TickerSpan)> {
    if col.len() != expected_price_len {
        return Err(invalid_return_series(
            ticker,
            col.len(),
            "price column length must equal valuation-grid length",
        )
        .into());
    }
    let Some(first) = col.iter().position(|value| !value.is_nan()) else {
        return Err(invalid_return_series(ticker, 0, "price column has no observations").into());
    };
    let last = col
        .iter()
        .rposition(|value| !value.is_nan())
        .unwrap_or(first);
    if first == last {
        return Err(invalid_return_series(
            ticker,
            first,
            "price column needs an opening and closing observation",
        )
        .into());
    }
    for (index, &value) in col[first..=last].iter().enumerate() {
        let absolute = first + index;
        if !value.is_finite() || value < 0.0 || (value == 0.0 && absolute != last) {
            return Err(invalid_return_series(
                ticker,
                absolute,
                "price must be finite and positive, except an actual final zero wealth observation",
            )
            .into());
        }
    }
    let returns = pairwise_returns(&col[first..=last])?;
    Ok((returns, TickerSpan::new(first, last)))
}

fn return_column_to_span(
    col: Vec<f64>,
    ticker: &str,
    expected_len: usize,
) -> finstack_quant_core::Result<(Vec<f64>, TickerSpan)> {
    if col.len() != expected_len {
        return Err(invalid_return_series(
            ticker,
            col.len(),
            "return column length must equal valuation-grid length minus one",
        )
        .into());
    }
    let Some(first) = col.iter().position(|value| !value.is_nan()) else {
        return Err(invalid_return_series(ticker, 0, "return column has no observations").into());
    };
    let last = col
        .iter()
        .rposition(|value| !value.is_nan())
        .unwrap_or(first);
    for (index, &value) in col[first..=last].iter().enumerate() {
        if !value.is_finite() || value < -1.0 || (value <= -1.0 && first + index != last) {
            return Err(invalid_return_series(ticker, first + index, "returns must be finite and at least -1, with wipeout allowed only as the final observation").into());
        }
    }
    Ok((col[first..=last].to_vec(), TickerSpan::new(first, last + 1)))
}

fn local_range(span: TickerSpan, global: TickerSpan) -> core::ops::Range<usize> {
    let start = global.start.saturating_sub(span.start).min(span.len());
    let end = global
        .end
        .saturating_sub(span.start)
        .min(span.len())
        .max(start);
    start..end
}

impl Performance {
    fn global_active_span(&self) -> TickerSpan {
        TickerSpan::new(self.start_idx, self.end_idx.min(self.get_dates().len()))
    }

    fn active_span_for_ticker(&self, ticker_idx: usize) -> TickerSpan {
        self.return_spans
            .get(ticker_idx)
            .copied()
            .unwrap_or_else(|| TickerSpan::new(0, 0))
            .intersect(self.global_active_span())
    }

    fn active_two_ticker_span(&self, lhs_idx: usize, rhs_idx: usize) -> TickerSpan {
        self.active_span_for_ticker(lhs_idx)
            .intersect(self.active_span_for_ticker(rhs_idx))
    }

    fn common_active_span(&self) -> TickerSpan {
        let n = self.ticker_names.len();
        let mut span = self.active_span_for_ticker(0);
        for i in 1..n {
            span = span.intersect(self.active_span_for_ticker(i));
        }
        span
    }

    fn returns_for_span(&self, ticker_idx: usize, global: TickerSpan) -> &[f64] {
        let Some(series) = self.returns.get(ticker_idx) else {
            return &[];
        };
        let Some(span) = self.return_spans.get(ticker_idx).copied() else {
            return &[];
        };
        if global.is_empty() {
            return &[];
        }
        let range = local_range(span, global);
        &series[range]
    }

    fn active_two_ticker_returns(&self, lhs_idx: usize, rhs_idx: usize) -> (&[f64], &[f64]) {
        let span = self.active_two_ticker_span(lhs_idx, rhs_idx);
        (
            self.returns_for_span(lhs_idx, span),
            self.returns_for_span(rhs_idx, span),
        )
    }

    fn dates_for_span(&self, span: TickerSpan) -> &[Date] {
        let dates = self.get_dates();
        let end = span.end.min(dates.len());
        &dates[span.start.min(end)..end]
    }

    fn active_holding_period_for_ticker(&self, ticker_idx: usize) -> Option<(Date, Date)> {
        let span = self.active_span_for_ticker(ticker_idx);
        if span.is_empty() || self.price_dates.len() < 2 {
            return None;
        }
        let end_idx = span.end.min(self.price_dates.len() - 1);
        let start_idx = span.start.min(end_idx);
        if start_idx >= end_idx {
            None
        } else {
            Some((self.price_dates[start_idx], self.price_dates[end_idx]))
        }
    }

    /// Include the wealth baseline when locating episode peaks and durations.
    fn active_drawdown_path(&self, ticker_idx: usize) -> (Vec<f64>, &[Date]) {
        let dd = self.active_drawdown_values(ticker_idx);
        if dd.is_empty() {
            return (Vec::new(), &[]);
        }
        let span = self.active_span_for_ticker(ticker_idx);
        let mut path = Vec::with_capacity(dd.len() + 1);
        path.push(0.0);
        path.extend_from_slice(dd);
        (path, &self.price_dates[span.start..=span.end])
    }
}

impl Performance {
    /// Construct from a dated price or return panel.
    ///
    /// Both kinds use the caller's actual strictly increasing valuation grid.
    /// Prices contain one level per valuation date; returns contain one value per
    /// adjacent interval, hence one fewer value than dates. No initial date,
    /// frequency or benchmark is inferred. Each ticker has a contiguous observed
    /// span with missing observations allowed only at either edge. A terminal
    /// `-1` return or final zero wealth level ends that ticker's observations.
    ///
    /// # Arguments
    ///
    /// * `panel` - Actual valuation dates, ticker-major price or decimal-return
    ///   columns, unique nonblank identities, and optional explicit benchmark and
    ///   observation frequency. Return provenance and interval alignment are the caller's responsibility.
    ///
    /// # Errors
    ///
    /// Rejects malformed grids, shapes, names, missing spans, nonfinite economic
    /// observations, returns below `-1`, observations after wipeout and unknown
    /// benchmarks. Unrepresentable price-derived interval returns fail explicitly.
    ///
    /// # Tracing
    ///
    /// Emits a `debug`-level `tracing` span named `Performance::new` with
    /// `n_tickers`, `n_dates`, `kind` and `frequency` fields.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_analytics::{PanelSeriesKind, Performance, PerformancePanel};
    /// use finstack_quant_core::dates::{Date, PeriodKind};
    /// use time::Month;
    ///
    /// let dates: Vec<Date> = (1..=10)
    ///     .map(|d| Date::from_calendar_date(2025, Month::January, d))
    ///     .collect::<Result<_, _>>()?;
    /// let perf = Performance::new(&PerformancePanel {
    ///     dates,
    ///     series: vec![(0..10).map(|i| 100.0 + i as f64).collect()],
    ///     kind: PanelSeriesKind::Prices,
    ///     ticker_names: vec!["SPY".into()],
    ///     benchmark_ticker: None,
    ///     frequency: Some(PeriodKind::Daily),
    /// })?;
    /// assert_eq!(perf.get_ticker_names(), &["SPY"]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[tracing::instrument(level = "debug", skip(panel), fields(n_tickers = panel.series.len(), n_dates = panel.dates.len(), kind = ?panel.kind, frequency = ?panel.frequency))]
    pub fn new(panel: &PerformancePanel) -> finstack_quant_core::Result<Self> {
        let frequency = panel.frequency;
        let what = match panel.kind {
            PanelSeriesKind::Prices => "prices",
            PanelSeriesKind::Returns => "returns",
        };
        check_panel_inputs(
            &panel.dates,
            panel.series.len(),
            panel.ticker_names.len(),
            what,
        )?;

        validate_ticker_names(&panel.ticker_names)?;
        if let Some(benchmark) = panel.benchmark_ticker.as_deref() {
            ticker_index(&panel.ticker_names, benchmark)?;
        }

        let mut returns: Vec<Vec<f64>> = Vec::with_capacity(panel.series.len());
        let mut return_spans: Vec<TickerSpan> = Vec::with_capacity(panel.series.len());
        for (ticker, col) in panel.ticker_names.iter().zip(&panel.series) {
            let (column, span) = match panel.kind {
                PanelSeriesKind::Prices => price_column_to_returns(col, ticker, panel.dates.len())?,
                PanelSeriesKind::Returns => {
                    return_column_to_span(col.clone(), ticker, panel.dates.len() - 1)?
                }
            };
            returns.push(column);
            return_spans.push(span);
        }
        let price_dates = panel.dates.clone();

        Self::assemble(
            price_dates,
            returns,
            return_spans,
            panel.ticker_names.clone(),
            panel.benchmark_ticker.as_deref(),
            frequency,
        )
    }

    /// Validate return columns, build per-ticker drawdown caches, and finalize state.
    ///
    /// Shared by both [`PanelSeriesKind`]s of [`Self::new`]: a price panel
    /// arrives here with its pre-computed simple returns.
    fn assemble(
        price_dates: Vec<Date>,
        returns: Vec<Vec<f64>>,
        return_spans: Vec<TickerSpan>,
        ticker_names: Vec<String>,
        benchmark_ticker: Option<&str>,
        frequency: Option<PeriodKind>,
    ) -> finstack_quant_core::Result<Self> {
        if price_dates.len() < 2 || returns.is_empty() {
            return Err(invalid_return_series("<panel>", 0, "returns or dates is empty").into());
        }
        validate_strictly_ascending_dates(&price_dates)?;
        let end_idx = price_dates.len() - 1;
        if ticker_names.len() != returns.len() {
            return Err(invalid_return_series(
                "<panel>",
                0,
                format!(
                    "ticker_names.len() = {} does not match returns.len() = {}",
                    ticker_names.len(),
                    returns.len()
                ),
            )
            .into());
        }

        validate_ticker_names(&ticker_names)?;

        let benchmark_idx = match benchmark_ticker {
            Some(name) => Some(ticker_index(&ticker_names, name)?),
            None => None,
        };

        if return_spans.len() != returns.len() {
            return Err(invalid_return_series(
                "<panel>",
                0,
                format!(
                    "return_spans.len() = {} does not match returns.len() = {}",
                    return_spans.len(),
                    returns.len()
                ),
            )
            .into());
        }

        let mut all_drawdowns: Vec<Vec<f64>> = Vec::with_capacity(returns.len());
        for ((col, span), ticker) in returns.iter().zip(&return_spans).zip(&ticker_names) {
            if span.start >= span.end || span.end > end_idx || span.len() != col.len() {
                return Err(invalid_return_series(
                    ticker,
                    span.start,
                    "return span does not match its column or date grid",
                )
                .into());
            }
            for (index, &value) in col.iter().enumerate() {
                if !value.is_finite() || value < -1.0 || (value <= -1.0 && index + 1 != col.len()) {
                    return Err(invalid_return_series(
                        ticker,
                        span.start + index,
                        "returns must be finite and at least -1; a wipeout must be the final observation",
                    )
                    .into());
                }
            }
            let dd = to_drawdown_series(col)?;
            all_drawdowns.push(dd);
        }

        Ok(Self {
            price_dates,
            returns,
            return_spans,
            ticker_names,
            benchmark_idx,
            drawdowns: all_drawdowns,
            active_window_drawdowns: None,
            frequency,
            start_idx: 0,
            end_idx,
        })
    }

    /// Restrict all subsequent analytics to the `[start, end]` date window.
    ///
    /// Finds the index boundaries in the internal date vector using binary
    /// search and stores them as `start_idx`/`end_idx`. All `active_*`
    /// accessors respect this range until it is changed again.
    ///
    /// # Drawdown semantics on a windowed range
    ///
    /// Drawdown caches are **rebuilt from scratch** within the new window:
    /// the peak watermark is reset to the valuation immediately before the
    /// first included return, so any drawdown that began before `start` is *not* carried
    /// over. As a consequence:
    ///
    /// - [`Self::max_drawdown`], [`Self::mean_drawdown`],
    ///   [`Self::get_drawdown_series`], [`Self::drawdown_details`],
    ///   [`Self::ulcer_index`], [`Self::pain_index`], [`Self::cdar`],
    ///   [`Self::recovery_factor`], [`Self::sterling_ratio`],
    ///   [`Self::burke_ratio`], [`Self::martin_ratio`],
    ///   [`Self::pain_ratio`], [`Self::calmar`], and
    ///   [`Self::max_drawdown_duration`] all reflect drawdowns measured
    ///   from the baseline immediately before the first return in `[start, end]`.
    /// - To preserve a watermark from before `start`, call these methods on
    ///   the un-windowed `Performance` first or fork the instance.
    ///
    /// # Arguments
    ///
    /// * `start` - First date to include (inclusive).
    /// * `end`   - Last return-ending date to include (inclusive); must not precede `start`.
    ///
    /// # Errors
    ///
    /// Reversed bounds or a numerical cache failure leave the previous selection unchanged.
    pub fn reset_date_range(&mut self, start: Date, end: Date) -> finstack_quant_core::Result<()> {
        if start > end {
            return Err(finstack_quant_core::Error::Validation(
                "active window start must not be after end".into(),
            ));
        }
        let start_idx = self.get_dates().partition_point(|&d| d < start);
        let end_idx = self.get_dates().partition_point(|&d| d <= end);
        let next = TickerSpan::new(start_idx, end_idx);
        let cache = self.build_active_drawdown_cache(next)?;
        self.start_idx = start_idx;
        self.end_idx = end_idx;
        self.active_window_drawdowns = cache;
        Ok(())
    }

    /// Designate a different ticker as the benchmark for all subsequent analytics.
    ///
    /// Updates `benchmark_idx`; all benchmark-aware accessors derive their
    /// series from `returns[benchmark_idx]` / `drawdowns[benchmark_idx]`
    /// (or the active windowed-drawdown cache when a date range is set).
    ///
    /// # Arguments
    ///
    /// * `ticker` - Name of the ticker to use as benchmark. Must match one
    ///   of the names provided at construction time.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::InputError::NotFound`] when `ticker` is not
    /// among the loaded ticker names; the benchmark is left unchanged.
    pub fn reset_bench_ticker(&mut self, ticker: &str) -> finstack_quant_core::Result<()> {
        self.benchmark_idx = Some(self.get_ticker_index(ticker)?);
        Ok(())
    }

    fn using_full_range(&self) -> bool {
        self.start_idx == 0 && self.end_idx >= self.get_dates().len()
    }

    fn build_active_drawdown_cache(
        &self,
        active: TickerSpan,
    ) -> finstack_quant_core::Result<Option<Vec<Vec<f64>>>> {
        if active.start == 0 && active.end == self.get_dates().len() {
            return Ok(None);
        }
        (0..self.returns.len())
            .map(|ticker_idx| {
                let span = self.return_spans[ticker_idx].intersect(active);
                to_drawdown_series(self.returns_for_span(ticker_idx, span))
            })
            .collect::<finstack_quant_core::Result<Vec<_>>>()
            .map(Some)
    }

    fn refresh_active_drawdown_cache(&mut self) -> finstack_quant_core::Result<()> {
        self.active_window_drawdowns =
            self.build_active_drawdown_cache(self.global_active_span())?;
        Ok(())
    }

    /// Resolve a ticker name to its zero-based column index.
    ///
    /// Hosts use this to accept either an index or a name wherever a
    /// per-ticker method takes `ticker_idx`.
    ///
    /// # Arguments
    ///
    /// * `ticker` - Ticker name exactly as supplied in `ticker_names` at
    ///   construction (case-sensitive).
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::InputError::NotFound`] when `ticker` is not
    /// among the loaded ticker names.
    pub fn get_ticker_index(&self, ticker: &str) -> finstack_quant_core::Result<usize> {
        ticker_index(&self.ticker_names, ticker)
    }

    /// Reject a tail `confidence` outside the open interval `(0.5, 1)`.
    ///
    /// Tail metrics (`value_at_risk`, `expected_shortfall`, `tail_ratio`,
    /// `cdar`, `parametric_var`, `cornish_fisher_var`, `modified_sharpe`)
    /// route through this guard so a confidence such as `95.0` or `1.0`
    /// surfaces as an explicit error rather than a `NaN` column.
    fn ensure_confidence(confidence: f64) -> finstack_quant_core::Result<()> {
        if confidence.is_finite() && confidence > 0.5 && confidence < 1.0 {
            Ok(())
        } else {
            Err(finstack_quant_core::Error::Validation(format!(
                "confidence must lie in the open interval (0.5, 1); got {confidence}"
            )))
        }
    }

    /// Reject a zero rolling `window`.
    ///
    /// A window longer than the active series still yields an empty
    /// [`crate::DatedSeries`] (there is simply no complete window); a zero
    /// window is a caller error and is rejected here.
    fn ensure_window(window: usize) -> finstack_quant_core::Result<()> {
        if window == 0 {
            Err(finstack_quant_core::Error::Validation(
                "rolling window must be at least 1 observation".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    /// Reject `ticker_idx` outside the loaded ticker columns.
    ///
    /// Per-ticker public methods route through this guard so an invalid index
    /// surfaces as an explicit error instead of silently producing an empty
    /// slice that downstream metrics turn into a plausible-looking `0.0`.
    fn ensure_ticker_idx(&self, ticker_idx: usize) -> finstack_quant_core::Result<()> {
        if ticker_idx >= self.ticker_names.len() {
            tracing::debug!(
                ticker_idx,
                n_tickers = self.ticker_names.len(),
                reason = "ticker_idx_out_of_range",
                "rejecting per-ticker analytics call"
            );
            return Err(invalid_return_series(
                format!("<idx={ticker_idx}>"),
                ticker_idx,
                format!(
                    "ticker_idx {ticker_idx} is out of range; loaded {} ticker(s)",
                    self.ticker_names.len()
                ),
            )
            .into());
        }
        Ok(())
    }

    fn active_returns(&self, ticker_idx: usize) -> &[f64] {
        self.returns_for_span(ticker_idx, self.active_span_for_ticker(ticker_idx))
    }

    /// Per-period simple returns for one ticker over the active window.
    ///
    /// The one-column form of [`Self::get_returns`]: the same values as its
    /// row `ticker_idx`, without copying the other tickers.
    ///
    /// The series is span-aware: on edge-ragged panels it excludes the
    /// leading/trailing missing observations for this ticker, so its length
    /// matches [`Self::get_active_dates_for_ticker`] rather than
    /// [`Self::get_active_dates`].
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::InputError::InvalidReturnSeries`] when
    /// `ticker_idx` is outside the loaded ticker columns.
    ///
    /// # Arguments
    ///
    /// * `ticker_idx` - Zero-based ticker column index, in
    ///   [`Self::get_ticker_names`] order.
    ///
    /// # Returns
    ///
    /// The ticker's simple (non-compounded) returns as decimal fractions,
    /// e.g. `0.01` for `+1%`, in date order.
    pub fn get_returns_for_ticker(
        &self,
        ticker_idx: usize,
    ) -> finstack_quant_core::Result<Vec<f64>> {
        self.ensure_ticker_idx(ticker_idx)?;
        Ok(self.active_returns(ticker_idx).to_vec())
    }

    /// Date slice corresponding to the currently active analysis window.
    pub fn get_active_dates(&self) -> &[Date] {
        self.dates_for_span(self.global_active_span())
    }

    /// Date slice corresponding to a ticker's currently active return series.
    ///
    /// On edge-ragged panels this may be shorter than [`Self::get_active_dates`]
    /// because leading/trailing missing observations are excluded per ticker.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::InputError::InvalidReturnSeries`] when
    /// `ticker_idx` is outside the loaded ticker columns.
    ///
    /// # Arguments
    ///
    /// * `ticker_idx` - Zero-based column index of the ticker in the loaded performance panel.
    pub fn get_active_dates_for_ticker(
        &self,
        ticker_idx: usize,
    ) -> finstack_quant_core::Result<&[Date]> {
        self.ensure_ticker_idx(ticker_idx)?;
        Ok(self.active_dates_for_ticker_unchecked(ticker_idx))
    }

    /// Exact return-ending dates shared by a ticker and the selected benchmark.
    ///
    /// # Arguments
    ///
    /// * `ticker_idx` - Zero-based ticker column in supplied order; both ticker and benchmark must be loaded.
    ///
    /// # Errors
    ///
    /// Returns a validation error for an invalid ticker index or an absent benchmark.
    pub fn get_paired_dates(&self, ticker_idx: usize) -> finstack_quant_core::Result<&[Date]> {
        self.ensure_ticker_idx(ticker_idx)?;
        let benchmark = self.benchmark()?;
        Ok(self.dates_for_span(self.active_two_ticker_span(ticker_idx, benchmark)))
    }

    fn active_dates_for_ticker_unchecked(&self, ticker_idx: usize) -> &[Date] {
        self.dates_for_span(self.active_span_for_ticker(ticker_idx))
    }

    fn active_drawdown_values(&self, ticker_idx: usize) -> &[f64] {
        if self.using_full_range() {
            return self
                .drawdowns
                .get(ticker_idx)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
        }

        self.active_window_drawdowns
            .as_ref()
            .and_then(|drawdowns| drawdowns.get(ticker_idx))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn ann(&self) -> finstack_quant_core::Result<f64> {
        self.frequency.map(|frequency| f64::from(frequency.annualization_factor())).ok_or_else(|| {
            finstack_quant_core::Error::Validation("explicit observation frequency is required for annualization or cash conversion".into())
        })
    }

    fn benchmark(&self) -> finstack_quant_core::Result<usize> {
        self.benchmark_idx.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "an explicit benchmark ticker is required".into(),
            )
        })
    }

    /// Map a per-ticker closure over all tickers in column order.
    ///
    /// Centralises the `(0..n_tickers).map(..).collect()` idiom used
    /// throughout the scalar-metric API.
    #[inline]
    fn map_tickers<T, F>(&self, f: F) -> Vec<T>
    where
        F: FnMut(usize) -> T,
    {
        (0..self.ticker_names.len()).map(f).collect()
    }

    /// Map a closure over each ticker's overlapping `(portfolio, benchmark)` return slices.
    #[inline]
    fn map_pair_returns<T, F>(&self, mut f: F) -> finstack_quant_core::Result<Vec<T>>
    where
        F: FnMut(&[f64], &[f64]) -> finstack_quant_core::Result<T>,
    {
        let benchmark = self.benchmark()?;
        (0..self.ticker_names.len())
            .map(|i| {
                let (r, bench) = self.active_two_ticker_returns(i, benchmark);
                f(r, bench)
            })
            .collect()
    }

    /// Full return-aligned date vector (independent of any active window).
    ///
    /// Returns the date grid that pairs with each row of internal returns,
    /// covering the full constructed range. To get just the dates inside
    /// the currently selected analysis window, use [`Self::get_active_dates`].
    pub fn get_dates(&self) -> &[Date] {
        &self.price_dates[1..]
    }
    /// Ticker names in column order.
    ///
    /// # Returns
    ///
    /// The names supplied at construction time.
    pub fn get_ticker_names(&self) -> &[String] {
        &self.ticker_names
    }
    /// Index of the benchmark ticker.
    ///
    /// # Returns
    ///
    /// The zero-based index of the selected benchmark, or `None` when absent.
    pub fn get_benchmark_idx(&self) -> Option<usize> {
        self.benchmark_idx
    }
    /// Observation frequency.
    ///
    /// # Returns
    ///
    /// The explicitly supplied nominal observation frequency, or `None` when absent.
    pub fn get_frequency(&self) -> Option<PeriodKind> {
        self.frequency
    }
}

/// Lookback returns for each period horizon.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "serialization::LookbackReturnsState")]
#[cfg_attr(feature = "json-schema", schemars(!try_from, deny_unknown_fields))]
pub struct LookbackReturns {
    /// Date on which every lookback window ended: the requested date, else the
    /// last active date of the panel.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub ref_date: Date,
    /// Ticker names aligned with every per-ticker vector below.
    pub ticker_names: Vec<String>,
    /// Month-to-date compounded return per ticker.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64_seq")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::NonFiniteF64Wire>")
    )]
    pub mtd: Vec<f64>,
    /// Quarter-to-date compounded return per ticker.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64_seq")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::NonFiniteF64Wire>")
    )]
    pub qtd: Vec<f64>,
    /// Year-to-date compounded return per ticker.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64_seq")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::NonFiniteF64Wire>")
    )]
    pub ytd: Vec<f64>,
    /// Fiscal-year-to-date compounded return per ticker.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64_seq")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::NonFiniteF64Wire>")
    )]
    pub fytd: Vec<f64>,
    /// Explicit fiscal partition used for fiscal-year-to-date selection.
    pub fiscal_config: finstack_quant_core::dates::FiscalConfig,
    /// Observation count and actual holding boundaries for each month-to-date value.
    pub mtd_samples: Vec<crate::aggregation::LookbackSample>,
    /// Observation count and actual holding boundaries for each quarter-to-date value.
    pub qtd_samples: Vec<crate::aggregation::LookbackSample>,
    /// Observation count and actual holding boundaries for each year-to-date value.
    pub ytd_samples: Vec<crate::aggregation::LookbackSample>,
    /// Observation count and actual holding boundaries for each fiscal-year-to-date value.
    pub fytd_samples: Vec<crate::aggregation::LookbackSample>,
}
