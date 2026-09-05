//! Series, correlation, lookback, and period-aggregation methods on
//! [`Performance`].

use super::{LookbackReturns, Performance};
use crate::aggregation::{
    group_by_period, group_by_period_dated, period_stats_from_grouped, PeriodStats,
};
use crate::correlation::{
    nearest_correlation_matrix, validate_correlation_matrix, NearestCorrelationOpts,
};
use crate::dates::{Date, FiscalConfig, PeriodKind};
use crate::drawdown::{drawdown_details, to_drawdown_series, DrawdownEpisode};
use crate::lookback;
use crate::math::linalg::unflatten_square;
use crate::math::stats::{correlation, mean_var};
use crate::returns::{comp_sum, comp_total, excess_returns};

impl Performance {
    /// Per-period simple returns for each ticker.
    ///
    /// This is the canonical accessor for the raw return panel over the active
    /// window. Prefer it over calling [`Performance::excess_returns`] with an
    /// all-zero risk-free vector or hand-un-compounding
    /// [`Performance::cumulative_returns`].
    ///
    /// Series are span-aware and therefore ragged across tickers on
    /// edge-ragged panels: row `i` has the length of
    /// [`Performance::active_dates_for_ticker`] for ticker `i`, which may be
    /// shorter than [`Performance::active_dates`]. Use
    /// [`Performance::returns_for_ticker`] when only one column is needed.
    ///
    /// # Returns
    ///
    /// One vector per ticker, in [`Performance::ticker_names`] order, holding
    /// simple returns as decimal fractions (`0.01` for `+1%`) in date order.
    pub fn returns(&self) -> Vec<Vec<f64>> {
        self.map_tickers(|i| self.active_returns(i).to_vec())
    }

    /// Cumulative compounded returns for each ticker.
    pub fn cumulative_returns(&self) -> Vec<Vec<f64>> {
        self.map_tickers(|i| comp_sum(self.active_returns(i)))
    }

    /// Drawdown series for each ticker.
    ///
    /// Values are non-positive fractions such as `-0.25` for a 25% drawdown.
    pub fn drawdown_series(&self) -> Vec<Vec<f64>> {
        self.map_tickers(|i| self.active_drawdown_values(i).to_vec())
    }

    /// Top-N drawdown episodes for a specific ticker.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::InputError::InvalidReturnSeries`] when
    /// `ticker_idx` is outside the loaded ticker columns.
    ///
    /// # Arguments
    ///
    /// * `ticker_idx` - Zero-based column index of the ticker in the loaded performance panel
    /// * `n` - Count of elements, paths, or periods requested by the caller
    pub fn drawdown_details(
        &self,
        ticker_idx: usize,
        n: usize,
    ) -> crate::Result<Vec<DrawdownEpisode>> {
        self.ensure_ticker_idx(ticker_idx)?;
        let (dd, dates) = self.active_drawdown_path(ticker_idx);
        Ok(drawdown_details(&dd, dates, n))
    }

    /// Pearson correlation matrix of all tickers, repaired to a valid
    /// correlation matrix when needed.
    ///
    /// Uses the complete-case common window when every ticker has at least
    /// two observations on the intersection of all active spans. Otherwise
    /// uses pairwise intersecting spans. Any non-finite off-diagonal or
    /// zero-variance pair is an error (no silent `NaN` display matrix).
    /// The flattened matrix is then checked with
    /// [`crate::correlation::validate_correlation_matrix`]; on failure it
    /// is repaired with Higham's nearest-correlation projection.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::InputError::InvalidReturnSeries`] when a pair
    /// is degenerate, an off-diagonal is non-finite, or Higham repair fails.
    ///
    /// # Returns
    ///
    /// An `n × n` matrix in [`Self::ticker_names`] order. The diagonal is
    /// `1.0`. The result passes
    /// [`crate::correlation::validate_correlation_matrix`].
    pub fn correlation_matrix(&self) -> crate::Result<Vec<Vec<f64>>> {
        self.correlation_matrix_with_repair_flag()
            .map(|(matrix, _)| matrix)
    }

    /// Correlation matrix plus a flag saying whether Higham repair was applied.
    ///
    /// Same estimator and error behaviour as [`Self::correlation_matrix`];
    /// the boolean is `true` when the raw pairwise matrix failed
    /// [`crate::correlation::validate_correlation_matrix`] and was projected
    /// to the nearest valid correlation matrix, so a reader can tell a
    /// clean estimate from a repaired one.
    ///
    /// # Errors
    ///
    /// As [`Self::correlation_matrix`].
    pub fn correlation_matrix_with_repair_flag(&self) -> crate::Result<(Vec<Vec<f64>>, bool)> {
        let n = self.ticker_names().len();
        let mut matrix = vec![vec![0.0; n]; n];
        if n == 0 {
            return Ok((matrix, false));
        }

        let common = self.common_active_span();
        let complete_case = common.len() >= 2;

        for (i, row) in matrix.iter_mut().enumerate() {
            row[i] = 1.0;
        }
        let pairs: Vec<(usize, usize)> = (0..n)
            .flat_map(|i| (i + 1..n).map(move |j| (i, j)))
            .collect();
        for (i, j) in pairs {
            let (lhs, rhs) = if complete_case {
                (
                    self.returns_for_span(i, common),
                    self.returns_for_span(j, common),
                )
            } else {
                self.active_two_ticker_returns(i, j)
            };
            let corr = pair_correlation(lhs, rhs, i, j)?;
            matrix[i][j] = corr;
            matrix[j][i] = corr;
        }
        finalize_correlation_matrix(matrix)
    }

    /// Cumulative outperformance versus the active benchmark.
    pub fn cumulative_returns_outperformance(&self) -> Vec<Vec<f64>> {
        self.map_tickers(|i| {
            let (port, bench) = self.active_pair_returns(i);
            let port_cum = comp_sum(port);
            let bench_cum = comp_sum(bench);
            port_cum
                .iter()
                .zip(bench_cum.iter())
                .map(|(p, b)| ((1.0 + p) / (1.0 + b)) - 1.0)
                .collect()
        })
    }

    /// Drawdown difference versus the active benchmark.
    pub fn drawdown_difference(&self) -> Vec<Vec<f64>> {
        self.map_tickers(|i| {
            let (port, bench) = self.active_pair_returns(i);
            let port_dd = to_drawdown_series(port);
            let bench_dd = to_drawdown_series(bench);
            port_dd
                .iter()
                .zip(bench_dd.iter())
                .map(|(p, b)| p - b)
                .collect()
        })
    }

    /// Excess returns (portfolio minus risk-free) for each ticker.
    ///
    /// `rf` is aligned to [`Self::active_dates`] (the panel grid), not to
    /// each ticker's possibly shorter active span. A ticker that starts
    /// later subtracts `rf[panel_index]` for the dates it actually
    /// observes.
    ///
    /// When `nperiods` is `None`, the risk-free series is treated as
    /// annualized and geometrically decompounded with the panel
    /// annualization factor. Pass `Some(1.0)` to treat `rf` as already
    /// periodic.
    ///
    /// # Arguments
    ///
    /// * `rf` - Risk-free rate per panel date. Length must equal
    ///   [`Self::active_dates`]. Values are annualized decimal rates when
    ///   `nperiods` is `None` or greater than `1`; already-periodic simple
    ///   rates when `nperiods` is `Some(1.0)`.
    /// * `nperiods` - Optional compounding periods per year used to
    ///   decompound `rf`. `None` uses the panel frequency (e.g. `252` for
    ///   daily). `Some(1.0)` leaves `rf` unadjusted.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::InputError::InvalidReturnSeries`] when
    /// `rf.len()` does not equal the active panel date count.
    ///
    /// # Returns
    ///
    /// One excess-return vector per ticker, each aligned to that ticker's
    /// active span.
    pub fn excess_returns(
        &self,
        rf: &[f64],
        nperiods: Option<f64>,
    ) -> crate::Result<Vec<Vec<f64>>> {
        let panel_len = self.active_dates().len();
        if rf.len() != panel_len {
            return Err(crate::error::InputError::InvalidReturnSeries {
                ticker: "<rf>".into(),
                index: rf.len(),
                reason: format!(
                    "rf.len() = {} does not match active date grid length {}",
                    rf.len(),
                    panel_len
                ),
            }
            .into());
        }
        let nperiods = nperiods.unwrap_or(self.ann());
        Ok(self.map_tickers(|i| {
            let span = self.active_span_for_ticker(i);
            let returns = self.active_returns(i);
            let offset = span.start.saturating_sub(self.start_idx);
            let rf_aligned = rf
                .get(offset..offset.saturating_add(returns.len()))
                .unwrap_or(&[]);
            excess_returns(returns, rf_aligned, Some(nperiods))
        }))
    }

    /// Compounded returns for each lookback period (MTD, QTD, YTD, FYTD) at `ref_date`.
    ///
    /// FYTD is the first observation on or after the fiscal calendar start
    /// through `ref_date`. Holidays are not skipped. The first included
    /// simple return still spans the prior close.
    ///
    /// # Arguments
    ///
    /// * `ref_date` - Inclusive end of each lookback window.
    /// * `fiscal_config` - Fiscal year start month and day for FYTD.
    ///
    /// # Returns
    ///
    /// Per-ticker compounded simple returns for MTD, QTD, YTD, and FYTD.
    /// All four vectors contain one entry per ticker.
    pub fn lookback_returns(&self, ref_date: Date, fiscal_config: FiscalConfig) -> LookbackReturns {
        let compute = |selector: fn(&[Date], Date) -> core::ops::Range<usize>| -> Vec<f64> {
            self.map_tickers(|i| {
                let range = selector(self.active_dates_for_ticker_unchecked(i), ref_date);
                let r = self.active_returns(i);
                let start = range.start.min(r.len());
                let end = range.end.min(r.len()).max(start);
                comp_total(&r[start..end])
            })
        };

        let fytd = self.map_tickers(|i| {
            let dates = self.active_dates_for_ticker_unchecked(i);
            let range = lookback::fytd_select(dates, ref_date, fiscal_config);
            let r = self.active_returns(i);
            let start = range.start.min(r.len());
            let end = range.end.min(r.len()).max(start);
            comp_total(&r[start..end])
        });

        LookbackReturns {
            ticker_names: self.ticker_names().to_vec(),
            mtd: compute(lookback::mtd_select),
            qtd: compute(lookback::qtd_select),
            ytd: compute(lookback::ytd_select),
            fytd,
        }
    }

    /// Period-aggregated statistics for a specific ticker.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::InputError::InvalidReturnSeries`] when
    /// `ticker_idx` is outside the loaded ticker columns.
    pub fn period_stats(
        &self,
        ticker_idx: usize,
        aggregation_frequency: PeriodKind,
        fiscal_config: Option<FiscalConfig>,
    ) -> crate::Result<PeriodStats> {
        self.ensure_ticker_idx(ticker_idx)?;
        let grouped = group_by_period(
            self.active_dates_for_ticker_unchecked(ticker_idx),
            self.active_returns(ticker_idx),
            aggregation_frequency,
            fiscal_config,
        );
        Ok(period_stats_from_grouped(&grouped))
    }

    /// Calendar-bucketed compounded returns per ticker.
    ///
    /// Returns one `Vec<(Date, f64)>` per ticker — each entry is
    /// `(period_end_date, compounded_return)` for one calendar bucket of `frequency`.
    /// Buckets compound via the shared kernel, so they reconcile exactly with
    /// [`Performance::cumulative_returns`]. Returns are simple decimal fractions
    /// (`0.01` means 1%), ticker series follow [`Performance::ticker_names`] order,
    /// and points within each series are chronological. Calendar bucketing only.
    ///
    /// # Arguments
    ///
    /// * `frequency` - Calendar bucket size. Supported [`PeriodKind`] values are
    ///   daily, weekly, monthly, quarterly, semi_annual, and annual.
    ///
    /// # Returns
    ///
    /// A ticker-major panel of chronological `(period_end_date,
    /// compounded_return)` points over the active analysis window.
    pub fn periodic_returns(&self, frequency: PeriodKind) -> Vec<Vec<(Date, f64)>> {
        self.map_tickers(|i| {
            group_by_period_dated(
                self.active_dates_for_ticker_unchecked(i),
                self.active_returns(i),
                frequency,
            )
        })
    }
}

fn pair_correlation(lhs: &[f64], rhs: &[f64], i: usize, j: usize) -> crate::Result<f64> {
    if lhs.len() < 2 || rhs.len() < 2 {
        return Err(crate::error::InputError::InvalidReturnSeries {
            ticker: format!("<{i},{j}>"),
            index: lhs.len().min(rhs.len()),
            reason: format!(
                "correlation pair ({i}, {j}) has fewer than 2 overlapping observations"
            ),
        }
        .into());
    }
    let (_, var_l) = mean_var(lhs);
    let (_, var_r) = mean_var(rhs);
    if var_l == 0.0 || var_r == 0.0 {
        return Err(crate::error::InputError::InvalidReturnSeries {
            ticker: format!("<{i},{j}>"),
            index: 0,
            reason: format!("correlation pair ({i}, {j}) has a zero-variance series"),
        }
        .into());
    }
    let corr = correlation(lhs, rhs);
    if !corr.is_finite() {
        return Err(crate::error::InputError::InvalidReturnSeries {
            ticker: format!("<{i},{j}>"),
            index: 0,
            reason: format!("correlation pair ({i}, {j}) produced a non-finite coefficient"),
        }
        .into());
    }
    Ok(corr)
}

fn flatten_matrix(matrix: &[Vec<f64>]) -> Vec<f64> {
    matrix.iter().flatten().copied().collect()
}

fn finalize_correlation_matrix(matrix: Vec<Vec<f64>>) -> crate::Result<(Vec<Vec<f64>>, bool)> {
    let n = matrix.len();
    for (i, row) in matrix.iter().enumerate() {
        for (j, &value) in row.iter().enumerate() {
            if i != j && !value.is_finite() {
                return Err(crate::error::InputError::InvalidReturnSeries {
                    ticker: format!("<{i},{j}>"),
                    index: 0,
                    reason: format!("correlation ρ[{i},{j}] is non-finite"),
                }
                .into());
            }
        }
    }
    let flat = flatten_matrix(&matrix);
    if validate_correlation_matrix(&flat, n).is_ok() {
        return Ok((matrix, false));
    }
    let repaired = nearest_correlation_matrix(&flat, n, NearestCorrelationOpts::default())
        .map_err(|err| crate::error::InputError::InvalidReturnSeries {
            ticker: "<correlation>".into(),
            index: 0,
            reason: err.to_string(),
        })?;
    if validate_correlation_matrix(&repaired, n).is_err() {
        return Err(crate::error::InputError::InvalidReturnSeries {
            ticker: "<correlation>".into(),
            index: 0,
            reason: "Higham repair did not produce a valid correlation matrix".into(),
        }
        .into());
    }
    Ok((unflatten_square(&repaired, n), true))
}

#[cfg(test)]
mod periodic_returns_tests {
    use super::*;
    use crate::dates::{Month, PeriodKind};
    use crate::Performance;

    /// Build a single-ticker `Performance` with daily returns spanning
    /// January and February 2021 (2021-01-04 through 2021-02-26).
    fn sample_two_month_daily_performance() -> Performance {
        // Build dates: weekdays in January (4..=29) and February (1..=26) 2021.
        // We use calendar days for simplicity — just enough to guarantee
        // observations in two distinct calendar months.
        let jan_dates: Vec<Date> = (4u8..=29)
            .filter_map(|d| Date::from_calendar_date(2021, Month::January, d).ok())
            .collect();
        let feb_dates: Vec<Date> = (1u8..=26)
            .filter_map(|d| Date::from_calendar_date(2021, Month::February, d).ok())
            .collect();

        let mut dates = jan_dates;
        dates.extend(feb_dates);

        let n = dates.len();
        // Simple positive daily returns — no NaN spans.
        let returns = vec![vec![0.001_f64; n]];

        Performance::from_returns(
            dates,
            returns,
            vec!["TEST".to_string()],
            None,
            PeriodKind::Daily,
        )
        .unwrap()
    }

    #[test]
    fn periodic_returns_monthly_has_one_bucket_per_month() {
        let perf = sample_two_month_daily_performance();
        let periodic = perf.periodic_returns(PeriodKind::Monthly);
        assert_eq!(periodic.len(), perf.ticker_names().len());
        // Single-ticker fixture spanning Jan+Feb -> 2 buckets.
        assert_eq!(periodic[0].len(), 2);

        // Period-end dates fall in the expected calendar months.
        assert_eq!(periodic[0][0].0.month(), Month::January);
        assert_eq!(periodic[0][1].0.month(), Month::February);

        // Buckets chain to the full-period cumulative return (exact reconciliation).
        let cum = perf.cumulative_returns();
        let total = *cum[0].last().unwrap();
        let chained = (1.0 + periodic[0][0].1) * (1.0 + periodic[0][1].1) - 1.0;
        assert!((chained - total).abs() < 1e-12);
    }
}

#[cfg(test)]
mod excess_returns_tests {
    use super::*;
    use crate::dates::{Month, PeriodKind};
    use crate::Performance;

    fn jan(day: u8) -> Date {
        Date::from_calendar_date(2024, Month::January, day).expect("valid date")
    }

    fn rectangular_perf() -> Performance {
        let dates = vec![jan(2), jan(3), jan(4), jan(5)];
        Performance::from_returns(
            dates,
            vec![vec![0.01, 0.02, 0.03, 0.04], vec![0.05, 0.06, 0.07, 0.08]],
            vec!["A".into(), "B".into()],
            None,
            PeriodKind::Daily,
        )
        .expect("rectangular panel")
    }

    fn ragged_perf() -> Performance {
        let dates = vec![jan(2), jan(3), jan(4), jan(5)];
        Performance::from_returns(
            dates,
            vec![
                vec![0.01, 0.02, 0.03, 0.04],
                vec![f64::NAN, f64::NAN, 0.10, 0.20],
            ],
            vec!["A".into(), "B".into()],
            None,
            PeriodKind::Daily,
        )
        .expect("ragged panel")
    }

    #[test]
    fn excess_returns_rectangular_identity_with_zero_rf() {
        let perf = rectangular_perf();
        let rf = vec![0.0; perf.active_dates().len()];
        let excess = perf.excess_returns(&rf, None).expect("aligned rf");
        assert_eq!(excess, perf.returns());
    }

    #[test]
    fn excess_returns_ragged_uses_panel_index_not_series_head() {
        let perf = ragged_perf();
        let rf = vec![0.001, 0.002, 0.003, 0.004];
        let excess = perf
            .excess_returns(&rf, Some(1.0))
            .expect("aligned periodic rf");
        assert_eq!(excess[0].len(), 4);
        assert!((excess[0][0] - (0.01 - 0.001)).abs() < 1e-15);
        assert_eq!(excess[1].len(), 2);
        assert!(
            (excess[1][0] - (0.10 - 0.003)).abs() < 1e-15,
            "ticker B must subtract rf[2], not rf[0]; got {}",
            excess[1][0]
        );
        assert!((excess[1][1] - (0.20 - 0.004)).abs() < 1e-15);
        assert!((excess[1][0] - (0.10 - 0.001)).abs() > 1e-6);
    }

    #[test]
    fn excess_returns_length_mismatch_is_error() {
        let perf = rectangular_perf();
        let err = perf
            .excess_returns(&[0.0, 0.0], None)
            .expect_err("length mismatch");
        let msg = err.to_string();
        assert!(
            msg.contains("rf.len()") || msg.contains("active date"),
            "unexpected error: {msg}"
        );
    }
}

#[cfg(test)]
mod correlation_matrix_tests {
    use super::*;
    use crate::correlation::validate_correlation_matrix;
    use crate::dates::{Month, PeriodKind};
    use crate::Performance;

    fn jan(day: u8) -> Date {
        Date::from_calendar_date(2024, Month::January, day).expect("valid date")
    }

    #[test]
    fn correlation_matrix_two_asset_rectangular_identity() {
        let dates = vec![jan(2), jan(3), jan(4), jan(5)];
        let r = vec![0.01, 0.02, -0.01, 0.03];
        let perf = Performance::from_returns(
            dates,
            vec![r.clone(), r],
            vec!["A".into(), "B".into()],
            None,
            PeriodKind::Daily,
        )
        .expect("panel");
        let m = perf.correlation_matrix().expect("psd correlation");
        assert!((m[0][0] - 1.0).abs() < 1e-12);
        assert!((m[1][1] - 1.0).abs() < 1e-12);
        assert!((m[0][1] - 1.0).abs() < 1e-12);
        assert!((m[1][0] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn correlation_matrix_repairs_indefinite_pairwise_fixture() {
        let indefinite = vec![
            vec![1.0, -0.55, -0.55],
            vec![-0.55, 1.0, -0.55],
            vec![-0.55, -0.55, 1.0],
        ];
        let flat: Vec<f64> = indefinite.iter().flatten().copied().collect();
        assert!(validate_correlation_matrix(&flat, 3).is_err());
        let (repaired, was_repaired) =
            finalize_correlation_matrix(indefinite).expect("Higham repair");
        assert!(was_repaired);
        let repaired_flat: Vec<f64> = repaired.iter().flatten().copied().collect();
        validate_correlation_matrix(&repaired_flat, 3).expect("repaired matrix is valid");
    }

    #[test]
    fn correlation_matrix_zero_variance_pair_is_error() {
        let dates = vec![jan(2), jan(3), jan(4), jan(5)];
        let perf = Performance::from_returns(
            dates,
            vec![vec![0.01, 0.02, -0.01, 0.03], vec![0.05, 0.05, 0.05, 0.05]],
            vec!["A".into(), "B".into()],
            None,
            PeriodKind::Daily,
        )
        .expect("panel");
        let err = perf.correlation_matrix().expect_err("zero-variance pair");
        let msg = err.to_string();
        assert!(
            msg.contains("zero-variance") || msg.contains("correlation"),
            "unexpected error: {msg}"
        );
    }
}
