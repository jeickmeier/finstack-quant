//! Series, correlation, lookback, and period-aggregation methods on
//! [`Performance`].
//!
//! Pure layout split from `performance.rs`; no behavior changes.

use super::{LookbackReturns, Performance};
use crate::aggregation::{
    group_by_period, group_by_period_dated, period_stats_from_grouped, PeriodStats,
};
use crate::dates::{Date, FiscalConfig, HolidayCalendar, PeriodKind};
use crate::drawdown::{drawdown_details, to_drawdown_series, DrawdownEpisode};
use crate::lookback;
use crate::math::stats::correlation;
use crate::returns::{comp_sum, comp_total, excess_returns};

impl Performance {
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
    pub fn drawdown_details(
        &self,
        ticker_idx: usize,
        n: usize,
    ) -> crate::Result<Vec<DrawdownEpisode>> {
        self.ensure_ticker_idx(ticker_idx)?;
        let dd = self.active_drawdown_values(ticker_idx);
        let dates = self.active_dates_for_ticker_unchecked(ticker_idx);
        Ok(drawdown_details(dd, dates, n))
    }

    /// Pearson correlation matrix of all tickers.
    ///
    /// Computes pairwise correlations over the active date window.
    /// The diagonal is always `1.0`.
    pub fn correlation_matrix(&self) -> Vec<Vec<f64>> {
        let n = self.ticker_names().len();
        let mut matrix = vec![vec![0.0; n]; n];
        if n == 0 {
            return matrix;
        }

        for i in 0..n {
            let (head, tail) = matrix.split_at_mut(i + 1);
            let row_i = &mut head[i];
            row_i[i] = 1.0;
            for (offset, row_j) in tail.iter_mut().enumerate() {
                let j = i + 1 + offset;
                let (lhs, rhs) = self.active_two_ticker_returns(i, j);
                let corr = if lhs.len() < 2 {
                    0.0
                } else {
                    correlation(lhs, rhs)
                };
                row_i[j] = corr;
                row_j[i] = corr;
            }
        }
        matrix
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
    pub fn excess_returns(&self, rf: &[f64], nperiods: Option<f64>) -> Vec<Vec<f64>> {
        self.map_tickers(|i| excess_returns(self.active_returns(i), rf, nperiods))
    }

    /// Compounded returns for each lookback period (MTD, QTD, YTD, FYTD) at `ref_date`.
    ///
    /// # Errors
    /// Returns an error if the fiscal start cannot be adjusted on the supplied
    /// calendar.
    pub fn lookback_returns(
        &self,
        ref_date: Date,
        fiscal_config: FiscalConfig,
        calendar: &dyn HolidayCalendar,
    ) -> crate::Result<LookbackReturns> {
        let compute = |selector: fn(&[Date], Date) -> core::ops::Range<usize>| -> Vec<f64> {
            self.map_tickers(|i| {
                let range = selector(self.active_dates_for_ticker_unchecked(i), ref_date);
                let r = self.active_returns(i);
                let start = range.start.min(r.len());
                let end = range.end.min(r.len()).max(start);
                comp_total(&r[start..end])
            })
        };

        let fytd = self
            .map_tickers(|i| {
                let dates = self.active_dates_for_ticker_unchecked(i);
                let range = lookback::fytd_select(dates, ref_date, fiscal_config, calendar)?;
                let r = self.active_returns(i);
                let start = range.start.min(r.len());
                let end = range.end.min(r.len()).max(start);
                Ok(comp_total(&r[start..end]))
            })
            .into_iter()
            .collect::<crate::Result<Vec<_>>>()?;

        Ok(LookbackReturns {
            mtd: compute(lookback::mtd_select),
            qtd: compute(lookback::qtd_select),
            ytd: compute(lookback::ytd_select),
            fytd: Some(fytd),
        })
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
        agg_freq: PeriodKind,
        fiscal_config: Option<FiscalConfig>,
    ) -> crate::Result<PeriodStats> {
        self.ensure_ticker_idx(ticker_idx)?;
        let grouped = group_by_period(
            self.active_dates_for_ticker_unchecked(ticker_idx),
            self.active_returns(ticker_idx),
            agg_freq,
            fiscal_config,
        );
        Ok(period_stats_from_grouped(&grouped))
    }

    /// Calendar-bucketed compounded returns per ticker.
    ///
    /// Returns one `Vec<(Date, f64)>` per ticker — each entry is
    /// `(period_end_date, compounded_return)` for one calendar bucket of `freq`.
    /// Buckets compound via the shared kernel, so they reconcile exactly with
    /// [`Performance::cumulative_returns`]. Calendar bucketing only.
    pub fn periodic_returns(&self, freq: PeriodKind) -> Vec<Vec<(Date, f64)>> {
        self.map_tickers(|i| {
            group_by_period_dated(
                self.active_dates_for_ticker_unchecked(i),
                self.active_returns(i),
                freq,
            )
        })
    }
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
    }
}
