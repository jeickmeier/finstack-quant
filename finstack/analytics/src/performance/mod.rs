//! Stateful `Performance` orchestrator over the analytics building blocks.
//!
//! Implementation is split across sibling files (`scalar`, `rolling`,
//! `benchmark`, `aggregation`); each adds `impl Performance` methods.
//! Public re-exports happen from `lib.rs`.

use crate::dates::{Date, Duration, PeriodKind};

use super::drawdown::to_drawdown_series;
use super::returns::{clean_returns, simple_returns};

mod aggregation;
mod benchmark;
mod rolling;
mod scalar;

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
/// use finstack_analytics::Performance;
/// use finstack_core::dates::{Date, Month, PeriodKind};
///
/// let dates: Vec<Date> = (1..=6)
///     .map(|d| Date::from_calendar_date(2025, Month::January, d).unwrap())
///     .collect();
/// let benchmark = vec![100.0, 101.0, 99.0, 102.0, 101.0, 103.0];
/// let portfolio = vec![100.0, 103.0, 100.0, 104.0, 102.0, 106.0];
///
/// let mut perf = Performance::new(
///     dates,
///     vec![benchmark, portfolio],
///     vec!["SPY".to_string(), "ALPHA".to_string()],
///     Some("SPY"),
///     PeriodKind::Daily,
/// )?;
///
/// let sharpe = perf.sharpe(0.02);
/// let beta = perf.beta();
/// let rolling = perf.rolling_sharpe(1, 3, 0.02)?;
/// assert_eq!(sharpe.len(), 2);
/// assert_eq!(beta.len(), 2);
/// assert_eq!(rolling.values.len(), 3);
///
/// perf.reset_date_range(
///     Date::from_calendar_date(2025, Month::January, 3).unwrap(),
///     Date::from_calendar_date(2025, Month::January, 6).unwrap(),
/// );
/// assert_eq!(perf.cagr()?.len(), 2);
/// # Ok::<(), finstack_core::Error>(())
/// ```
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Performance {
    price_dates: Vec<Date>,
    dates: Vec<Date>,
    returns: Vec<Vec<f64>>,
    ticker_names: Vec<String>,
    benchmark_idx: usize,
    drawdowns: Vec<Vec<f64>>,
    active_window_drawdowns: Option<Vec<Vec<f64>>>,
    freq: PeriodKind,
    start_idx: usize,
    end_idx: usize,
}

fn invalid_return_series(
    ticker: impl Into<String>,
    index: usize,
    reason: impl Into<String>,
) -> crate::error::InputError {
    crate::error::InputError::InvalidReturnSeries {
        ticker: ticker.into(),
        index,
        reason: reason.into(),
    }
}

/// Reject empty, duplicate, or non-monotonic date inputs at construction.
///
/// Several lookback and aggregation paths use [`slice::partition_point`] on
/// the internal date grid; both rely on strictly ascending order. Validating
/// once at the boundary keeps the rest of the crate slice-only.
fn validate_strictly_ascending_dates(dates: &[Date]) -> crate::Result<()> {
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

fn clean_return_column(
    mut col: Vec<f64>,
    ticker: &str,
    expected_len: usize,
) -> crate::Result<Vec<f64>> {
    if col.len() != expected_len {
        return Err(invalid_return_series(
            ticker,
            col.len().min(expected_len),
            format!(
                "column length {} does not match return-date grid length {}",
                col.len(),
                expected_len
            ),
        )
        .into());
    }

    clean_returns(&mut col, ticker);
    if col.len() != expected_len {
        return Err(invalid_return_series(
            ticker,
            col.len(),
            format!(
                "{} trailing NaN/non-finite rows stripped; series no longer aligned with date grid (length {} vs expected {})",
                expected_len - col.len(),
                col.len(),
                expected_len
            ),
        )
        .into());
    }

    if let Some((index, value)) = col
        .iter()
        .enumerate()
        .find(|(_, &v)| !v.is_finite() || v <= -1.0)
    {
        let reason = if !value.is_finite() {
            format!("non-finite return ({value})")
        } else {
            format!("return <= -1.0 ({value}); total wipeout makes downstream metrics meaningless")
        };
        return Err(invalid_return_series(ticker, index, reason).into());
    }

    Ok(col)
}

impl Performance {
    /// Construct from a price matrix (columns = tickers).
    ///
    /// Computes simple returns for each ticker, builds the drawdown
    /// series, and designates one ticker as the benchmark. The `dates`
    /// vector should have one entry per price row; internally the date and
    /// return series are trimmed by one element to align with the return
    /// computation (returns have length `n_prices - 1`).
    ///
    /// # Arguments
    ///
    /// * `dates` - Chronologically sorted date vector, one entry per price
    ///   observation.
    /// * `prices` - Price matrix: `prices[i]` is the full price series for
    ///   ticker `i`.
    /// * `ticker_names` - Names corresponding to each column of `prices`.
    /// * `benchmark_ticker` - Name of the benchmark ticker. Uses column 0 if
    ///   `None`; returns an error if a non-`None` ticker name is not found.
    /// * `freq` - Observation frequency, used to derive the annualization factor.
    /// # Returns
    ///
    /// A fully initialized [`Performance`] instance, or an error if
    /// validation fails.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::InputError::Invalid`] when:
    ///
    /// * `prices` or `dates` is empty.
    /// * `ticker_names.len() != prices.len()`.
    /// * any price column length differs from `dates.len()`.
    /// * `benchmark_ticker` is supplied but not found in `ticker_names`.
    /// * derived returns are non-finite or below `-1.0`.
    ///
    /// # Tracing
    ///
    /// Emits a `debug`-level `tracing` span named `Performance::new` with
    /// `n_tickers`, `n_dates`, and `freq` fields.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_analytics::Performance;
    /// use finstack_core::dates::{Date, Month, PeriodKind};
    ///
    /// let dates: Vec<Date> = (1..=10)
    ///     .map(|d| Date::from_calendar_date(2025, Month::January, d).unwrap())
    ///     .collect();
    /// let prices = vec![(0..10).map(|i| 100.0 + i as f64).collect::<Vec<_>>()];
    /// let perf = Performance::new(
    ///     dates,
    ///     prices,
    ///     vec!["SPY".into()],
    ///     None,
    ///     PeriodKind::Daily,
    /// ).unwrap();
    /// assert_eq!(perf.ticker_names(), &["SPY"]);
    /// ```
    #[tracing::instrument(level = "debug", skip(dates, prices, ticker_names, benchmark_ticker), fields(n_tickers = prices.len(), n_dates = dates.len(), freq = ?freq))]
    pub fn new(
        dates: Vec<Date>,
        prices: Vec<Vec<f64>>,
        ticker_names: Vec<String>,
        benchmark_ticker: Option<&str>,
        freq: PeriodKind,
    ) -> crate::Result<Self> {
        if prices.is_empty() || dates.is_empty() {
            return Err(invalid_return_series("<panel>", 0, "prices or dates is empty").into());
        }
        validate_strictly_ascending_dates(&dates)?;
        if let Some((col_idx, bad)) = prices
            .iter()
            .enumerate()
            .find(|(_, price_col)| price_col.len() != dates.len())
        {
            let ticker = ticker_names
                .get(col_idx)
                .cloned()
                .unwrap_or_else(|| format!("col[{col_idx}]"));
            return Err(invalid_return_series(
                ticker,
                0,
                format!(
                    "price column length {} does not match dates length {}",
                    bad.len(),
                    dates.len()
                ),
            )
            .into());
        }

        let returns_matrix: Vec<Vec<f64>> = prices
            .iter()
            .map(|price_col| simple_returns(price_col)[1..].to_vec())
            .collect();
        let return_dates = if dates.len() > 1 {
            dates[1..].to_vec()
        } else {
            dates.clone()
        };
        Self::assemble(
            dates,
            return_dates,
            returns_matrix,
            ticker_names,
            benchmark_ticker,
            freq,
        )
    }

    /// Construct from a pre-computed return matrix (columns = tickers).
    ///
    /// Use this when you already have a return panel and want to skip the
    /// price → return conversion handled by [`Self::new`]. The supplied
    /// `dates` are the return-aligned observation dates (one entry per
    /// return row).
    ///
    /// A synthetic prior date is prepended to the internal price-date grid
    /// so that CAGR and other date-aware metrics see a holding period of
    /// `dates.len()` periods. The prior date is derived from the first
    /// observed gap (`dates[1] - dates[0]`) when at least two dates are
    /// supplied, and otherwise falls back to `dates[0]`.
    ///
    /// # Arguments
    ///
    /// * `dates` - Chronologically sorted return-aligned dates.
    /// * `returns` - Return matrix: `returns[i]` is the simple-return series
    ///   for ticker `i`, with one entry per `dates` row.
    /// * `ticker_names` - Names corresponding to each column of `returns`.
    /// * `benchmark_ticker` - Name of the benchmark ticker. Uses column 0 if
    ///   `None`; returns an error if a non-`None` ticker name is not found.
    /// * `freq` - Observation frequency, used to derive the annualization factor.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::InputError::Invalid`] when inputs are empty,
    /// the column count does not match `ticker_names`, any return column has
    /// the wrong length, the benchmark name is unknown, or any return value
    /// is non-finite or `< -1.0`.
    pub fn from_returns(
        dates: Vec<Date>,
        returns: Vec<Vec<f64>>,
        ticker_names: Vec<String>,
        benchmark_ticker: Option<&str>,
        freq: PeriodKind,
    ) -> crate::Result<Self> {
        if returns.is_empty() || dates.is_empty() {
            return Err(invalid_return_series("<panel>", 0, "returns or dates is empty").into());
        }
        validate_strictly_ascending_dates(&dates)?;

        let prior_date = if dates.len() >= 2 {
            let gap = (dates[1] - dates[0]).whole_days();
            dates[0]
                .checked_sub(Duration::days(gap))
                .unwrap_or(dates[0])
        } else {
            dates[0]
        };
        let mut price_dates = Vec::with_capacity(dates.len() + 1);
        price_dates.push(prior_date);
        price_dates.extend_from_slice(&dates);

        Self::assemble(
            price_dates,
            dates,
            returns,
            ticker_names,
            benchmark_ticker,
            freq,
        )
    }

    /// Validate return columns, build per-ticker drawdown caches, and finalize state.
    ///
    /// Shared by [`Self::new`] (which pre-computes simple returns from prices)
    /// and [`Self::from_returns`] (which receives a return matrix directly).
    fn assemble(
        price_dates: Vec<Date>,
        return_dates: Vec<Date>,
        returns: Vec<Vec<f64>>,
        ticker_names: Vec<String>,
        benchmark_ticker: Option<&str>,
        freq: PeriodKind,
    ) -> crate::Result<Self> {
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

        let benchmark_idx = match benchmark_ticker {
            Some(name) => ticker_names.iter().position(|t| t == name).ok_or_else(|| {
                invalid_return_series(
                    name,
                    0,
                    "benchmark ticker not found among supplied ticker_names",
                )
            })?,
            None => 0,
        };

        let expected_len = return_dates.len();
        let mut all_returns: Vec<Vec<f64>> = Vec::with_capacity(returns.len());
        let mut all_drawdowns: Vec<Vec<f64>> = Vec::with_capacity(returns.len());
        for (ticker, col) in ticker_names.iter().zip(returns.into_iter()) {
            let clean = clean_return_column(col, ticker, expected_len)?;
            let dd = to_drawdown_series(&clean);
            all_drawdowns.push(dd);
            all_returns.push(clean);
        }

        let end_idx = all_returns.first().map_or(0, Vec::len);

        Ok(Self {
            price_dates,
            dates: return_dates,
            returns: all_returns,
            ticker_names,
            benchmark_idx,
            drawdowns: all_drawdowns,
            active_window_drawdowns: None,
            freq,
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
    /// the peak watermark is reset to the first observation of the active
    /// range, so any drawdown that began before `start` is *not* carried
    /// over. As a consequence:
    ///
    /// - [`Self::max_drawdown`], [`Self::mean_drawdown`],
    ///   [`Self::drawdown_series`], [`Self::drawdown_details`],
    ///   [`Self::ulcer_index`], [`Self::pain_index`], [`Self::cdar`],
    ///   [`Self::recovery_factor`], [`Self::sterling_ratio`],
    ///   [`Self::burke_ratio`], [`Self::martin_ratio`],
    ///   [`Self::pain_ratio`], [`Self::calmar`], and
    ///   [`Self::max_drawdown_duration`] all reflect drawdowns measured
    ///   *only* over `[start, end]`.
    /// - To preserve a watermark from before `start`, call these methods on
    ///   the un-windowed `Performance` first or fork the instance.
    ///
    /// # Arguments
    ///
    /// * `start` - First date to include (inclusive).
    /// * `end`   - Last date to include (inclusive).
    pub fn reset_date_range(&mut self, start: Date, end: Date) {
        self.start_idx = self.dates.partition_point(|&d| d < start);
        self.end_idx = self.dates.partition_point(|&d| d <= end);
        self.refresh_active_drawdown_cache();
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
    /// # Returns
    ///
    /// `Ok(())` on success, or an [`crate::error::InputError::Invalid`] if `ticker` is
    /// not found among the loaded tickers.
    pub fn reset_bench_ticker(&mut self, ticker: &str) -> crate::Result<()> {
        let idx = self
            .ticker_names
            .iter()
            .position(|t| t == ticker)
            .ok_or_else(|| {
                invalid_return_series(ticker, 0, "ticker not found among loaded ticker_names")
            })?;
        self.benchmark_idx = idx;
        Ok(())
    }

    fn active_range(&self) -> core::ops::Range<usize> {
        self.start_idx..self.end_idx
    }

    fn full_range_len(&self) -> usize {
        self.returns.first().map_or(0, Vec::len)
    }

    fn using_full_range(&self) -> bool {
        self.start_idx == 0 && self.end_idx >= self.full_range_len()
    }

    fn refresh_active_drawdown_cache(&mut self) {
        if self.using_full_range() {
            self.active_window_drawdowns = None;
            return;
        }

        self.active_window_drawdowns = Some(
            self.returns
                .iter()
                .map(|series| {
                    let end = self.end_idx.min(series.len());
                    let start = self.start_idx.min(end);
                    to_drawdown_series(&series[start..end])
                })
                .collect(),
        );
    }

    fn active_holding_period(&self) -> Option<(Date, Date)> {
        let range = self.active_range();
        if range.start >= range.end || self.price_dates.len() < 2 {
            return None;
        }
        let last_price_idx = self.price_dates.len() - 1;
        let start_idx = range.start.min(last_price_idx);
        let end_idx = range.end.min(last_price_idx);
        if start_idx >= end_idx {
            return None;
        }
        Some((self.price_dates[start_idx], self.price_dates[end_idx]))
    }

    /// Reject `ticker_idx` outside the loaded ticker columns.
    ///
    /// Per-ticker public methods route through this guard so an invalid index
    /// surfaces as an explicit error instead of silently producing an empty
    /// slice that downstream metrics turn into a plausible-looking `0.0`.
    fn ensure_ticker_idx(&self, ticker_idx: usize) -> crate::Result<()> {
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
        let range = self.active_range();
        self.returns
            .get(ticker_idx)
            .map(|r| {
                let end = range.end.min(r.len());
                &r[range.start.min(end)..end]
            })
            .unwrap_or(&[])
    }

    fn active_bench(&self) -> &[f64] {
        self.active_returns(self.benchmark_idx)
    }

    /// Date slice corresponding to the currently active analysis window.
    pub fn active_dates(&self) -> &[Date] {
        let range = self.active_range();
        let end = range.end.min(self.dates.len());
        &self.dates[range.start.min(end)..end]
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

    fn active_bench_drawdown_values(&self) -> &[f64] {
        self.active_drawdown_values(self.benchmark_idx)
    }

    fn ann(&self) -> f64 {
        self.freq.annualization_factor()
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

    // ── Final accessors ──

    /// Full return-aligned date vector (independent of any active window).
    ///
    /// Returns the date grid that pairs with each row of internal returns,
    /// covering the full constructed range. To get just the dates inside
    /// the currently selected analysis window, use [`Self::active_dates`].
    pub fn dates(&self) -> &[Date] {
        &self.dates
    }
    /// Ticker names in column order.
    ///
    /// # Returns
    ///
    /// The names supplied at construction time.
    pub fn ticker_names(&self) -> &[String] {
        &self.ticker_names
    }
    /// Index of the benchmark ticker.
    ///
    /// # Returns
    ///
    /// The zero-based index of the active benchmark ticker.
    pub fn benchmark_idx(&self) -> usize {
        self.benchmark_idx
    }
    /// Observation frequency.
    ///
    /// # Returns
    ///
    /// The frequency used to annualize facade-level metrics.
    pub fn freq(&self) -> PeriodKind {
        self.freq
    }
}

/// Lookback returns for each period horizon.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LookbackReturns {
    /// Month-to-date compounded return per ticker.
    pub mtd: Vec<f64>,
    /// Quarter-to-date compounded return per ticker.
    pub qtd: Vec<f64>,
    /// Year-to-date compounded return per ticker.
    pub ytd: Vec<f64>,
    /// Fiscal-year-to-date compounded return per ticker (None if no fiscal config).
    pub fytd: Option<Vec<f64>>,
}
