//! Series, correlation, lookback, and period-aggregation methods on
//! [`Performance`].

use super::{LookbackReturns, Performance, invalid_return_series};
use crate::aggregation::{
    LookbackSample, PeriodStats, PeriodicReturn, group_period_buckets, period_stats_inner,
};
use crate::drawdown::{DrawdownEpisode, drawdown_details, to_drawdown_series};
use crate::returns::{comp_sum, comp_total, excess_returns, finite_result, validate_cash};
use finstack_quant_core::dates::{Date, DateExt, FiscalConfig, PeriodKind};
use finstack_quant_core::math::summation::NeumaierAccumulator;
use time::Month;

impl Performance {
    /// Per-period simple returns for each ticker over the active window.
    ///
    /// Reads back the raw return panel the [`Performance`] was built from (or
    /// derived from prices). Prefer it over [`Performance::excess_returns`]
    /// with an all-zero risk-free vector, which returns the same values but
    /// needs a panel-length `rf` argument, or over un-compounding
    /// [`Performance::cumulative_returns`], which matches only up to
    /// floating-point noise.
    ///
    /// Series are span-aware and therefore ragged across tickers on
    /// edge-ragged panels: row `i` has the length of
    /// [`Performance::get_active_dates_for_ticker`] for ticker `i`, which may be
    /// shorter than [`Performance::get_active_dates`]. Use
    /// [`Performance::get_returns_for_ticker`] when only one column is needed.
    ///
    /// # Returns
    ///
    /// One vector per ticker, in [`Performance::get_ticker_names`] order, holding
    /// simple returns as decimal fractions (`0.01` for `+1%`) in date order.
    pub fn get_returns(&self) -> Vec<Vec<f64>> {
        self.map_tickers(|i| self.active_returns(i).to_vec())
    }

    /// Cumulative compounded returns for each ticker.
    pub fn cumulative_returns(&self) -> finstack_quant_core::Result<Vec<Vec<f64>>> {
        (0..self.ticker_names.len())
            .map(|i| comp_sum(self.active_returns(i)))
            .collect()
    }

    /// Drawdown series for each ticker.
    ///
    /// Values are non-positive fractions such as `-0.25` for a 25% drawdown.
    pub fn get_drawdown_series(&self) -> Vec<Vec<f64>> {
        self.map_tickers(|i| self.active_drawdown_values(i).to_vec())
    }

    /// Top-N drawdown episodes for a specific ticker.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::InputError::InvalidReturnSeries`] when
    /// `ticker_idx` is outside the loaded ticker columns.
    ///
    /// # Arguments
    ///
    /// * `ticker_idx` - Zero-based column index of the ticker in the loaded performance panel.
    /// * `n` - Maximum number of episodes to return, most severe first.
    pub fn drawdown_details(
        &self,
        ticker_idx: usize,
        n: usize,
    ) -> finstack_quant_core::Result<Vec<DrawdownEpisode>> {
        self.ensure_ticker_idx(ticker_idx)?;
        let (dd, dates) = self.active_drawdown_path(ticker_idx);
        drawdown_details(&dd, dates, n)
    }

    /// Common-sample Pearson correlation matrix with shared observation provenance.
    ///
    /// Every entry uses the exact intersection of all loaded ticker spans and
    /// the active date range. No pairwise fallback or numerical repair is applied.
    /// Correlation is dimensionless and needs no observation frequency.
    ///
    /// # Returns
    ///
    /// A complete matrix in loaded ticker order, its common observation count,
    /// and the first/last included return-ending dates.
    ///
    /// # Errors
    ///
    /// Returns a validation error for fewer than two common observations or
    /// a constant common-sample column, and a calculation error if stable
    /// evaluation cannot produce a valid finite PSD Pearson matrix.
    pub fn correlation_matrix(&self) -> finstack_quant_core::Result<crate::CorrelationResult> {
        let common = self.common_active_span();
        let columns: Vec<_> = (0..self.ticker_names.len())
            .map(|i| self.returns_for_span(i, common))
            .collect();
        crate::correlation::estimate(&self.ticker_names, &columns, self.dates_for_span(common))
    }

    /// Cumulative relative wealth versus the active benchmark.
    ///
    /// Returns `portfolio_wealth / benchmark_wealth - 1` over each ticker's
    /// overlapping dates. Log-growth differences are accumulated directly,
    /// so identical paths remain zero even when absolute wealth would
    /// overflow or round to zero.
    pub fn cumulative_returns_outperformance(&self) -> finstack_quant_core::Result<Vec<Vec<f64>>> {
        self.map_pair_returns(|port, bench| {
            let mut relative_log_growth = NeumaierAccumulator::new();
            port.iter()
                .zip(bench)
                .map(|(&p, &b)| {
                    if b <= -1.0 {
                        return Ok(f64::NAN);
                    }
                    if p <= -1.0 {
                        return Ok(-1.0);
                    }
                    // Preserve adjacent large growth factors before taking logarithms.
                    let relative_move = (p - b) / (1.0 + b);
                    let log_ratio = if relative_move.is_finite() && relative_move > -1.0 {
                        relative_move.ln_1p()
                    } else {
                        p.ln_1p() - b.ln_1p()
                    };
                    relative_log_growth.add(log_ratio);
                    finite_result(
                        relative_log_growth.total().exp_m1(),
                        "cumulative relative wealth",
                    )
                })
                .collect()
        })
    }

    /// Paired portfolio drawdown minus benchmark drawdown, rebased on the shared actual opening.
    ///
    /// # Errors
    ///
    /// Returns a validation error when no benchmark is selected, or a calculation
    /// error when a requested drawdown cannot be represented.
    pub fn drawdown_difference(&self) -> finstack_quant_core::Result<Vec<Vec<f64>>> {
        self.map_pair_returns(|port, bench| {
            let port_dd = to_drawdown_series(port)?;
            let bench_dd = to_drawdown_series(bench)?;
            Ok(port_dd
                .iter()
                .zip(bench_dd.iter())
                .map(|(p, b)| p - b)
                .collect())
        })
    }

    /// Arithmetic differential returns against explicitly aligned annual-effective cash.
    ///
    /// # Arguments
    ///
    /// * `rf` - Finite annual-effective decimal cash assumptions, each greater than
    ///   `-1`, assigned positionally to every active global return endpoint. For
    ///   example, `0.02` means 2% annual effective cash for that interval.
    ///
    /// # Returns
    ///
    /// Ticker-major arithmetic excess observations on each ticker's own active
    /// endpoints. Differential returns may be below `-1` and are not investment wealth.
    ///
    /// # Errors
    ///
    /// Returns a validation error for missing observation frequency, invalid cash
    /// anywhere in the supplied grid or an inexact grid length; calculation failures
    /// propagate without partial output.
    pub fn excess_returns(&self, rf: &[f64]) -> finstack_quant_core::Result<Vec<Vec<f64>>> {
        let ann = self.ann()?;
        let panel_len = self.get_active_dates().len();
        if rf.len() != panel_len {
            return Err(invalid_return_series(
                "<rf>",
                rf.len(),
                format!(
                    "rf.len() = {} does not match active date grid length {}",
                    rf.len(),
                    panel_len
                ),
            )
            .into());
        }
        for &cash in rf {
            validate_cash(cash)?;
        }
        (0..self.ticker_names.len())
            .map(|i| {
                let span = self.active_span_for_ticker(i);
                let returns = self.active_returns(i);
                let offset = span.start.saturating_sub(self.start_idx);
                let rf_aligned =
                    &rf[offset.min(rf.len())..offset.saturating_add(returns.len()).min(rf.len())];
                excess_returns(returns, rf_aligned, ann)
            })
            .collect()
    }

    /// Compounded returns for each lookback period (MTD, QTD, YTD, FYTD) at `ref_date`.
    ///
    /// MTD, QTD and YTD start on the first calendar day of `ref_date`'s month,
    /// calendar quarter (January, April, July, October) and year.
    ///
    /// FYTD is the first observation on or after the fiscal calendar start
    /// through `ref_date`. Holidays are not skipped. The first included
    /// simple return still spans the prior close.
    ///
    /// # Arguments
    ///
    /// * `ref_date` - Inclusive end of each lookback window; `None` uses the last
    ///   date of [`Self::get_active_dates`].
    /// * `fiscal_config` - Fiscal year start month and day for FYTD; `None`
    ///   uses the calendar year (January 1).
    ///
    /// # Returns
    ///
    /// The date the windows ended on and per-ticker compounded simple returns for
    /// MTD, QTD, YTD, and FYTD. All four vectors contain one entry per ticker.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when `ref_date` is `None`
    /// and the panel's active date range is empty.
    pub fn lookback_returns(
        &self,
        ref_date: Option<Date>,
        fiscal_config: Option<FiscalConfig>,
    ) -> finstack_quant_core::Result<LookbackReturns> {
        let ref_date = match ref_date {
            Some(date) => date,
            None => *self.get_active_dates().last().ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "lookback_returns requires a ref_date when no dates are active".to_string(),
                )
            })?,
        };
        let fiscal_config = fiscal_config.unwrap_or_else(FiscalConfig::calendar_year);
        let month_start = |month: Month| {
            Date::from_calendar_date(ref_date.year(), month, 1)
                .map_err(|err| finstack_quant_core::Error::Validation(err.to_string()))
        };
        // Construct every requested boundary before inspecting sample availability.
        let mtd_start = month_start(ref_date.month())?;
        let qtd_start = month_start(Month::January.nth_next((ref_date.quarter() - 1) * 3))?;
        let ytd_start = month_start(Month::January)?;
        let fytd_start = fiscal_config.year_start(ref_date.fiscal_year(fiscal_config))?;
        let compute =
            |period_start: Date| -> finstack_quant_core::Result<(Vec<f64>, Vec<LookbackSample>)> {
                let mut values = Vec::with_capacity(self.ticker_names.len());
                let mut samples = Vec::with_capacity(self.ticker_names.len());
                for i in 0..self.ticker_names.len() {
                    let dates = self.active_dates_for_ticker_unchecked(i);
                    let returns = self.active_returns(i);
                    let start = dates.partition_point(|&d| d < period_start);
                    let end = dates.partition_point(|&d| d <= ref_date).max(start);
                    if start == end {
                        values.push(f64::NAN);
                        samples.push(LookbackSample {
                            n_obs: 0,
                            holding_start: None,
                            holding_end: None,
                        });
                    } else {
                        let span = self.active_span_for_ticker(i);
                        values.push(comp_total(&returns[start..end])?);
                        let sample = LookbackSample {
                            n_obs: end - start,
                            holding_start: Some(self.price_dates[span.start + start]),
                            holding_end: Some(self.price_dates[span.start + end]),
                        };
                        sample.validate()?;
                        samples.push(sample);
                    }
                }
                Ok((values, samples))
            };
        let (mtd, mtd_samples) = compute(mtd_start)?;
        let (qtd, qtd_samples) = compute(qtd_start)?;
        let (ytd, ytd_samples) = compute(ytd_start)?;
        let (fytd, fytd_samples) = compute(fytd_start)?;
        Ok(LookbackReturns {
            ref_date,
            ticker_names: self.get_ticker_names().to_vec(),
            fiscal_config,
            mtd,
            qtd,
            ytd,
            fytd,
            mtd_samples,
            qtd_samples,
            ytd_samples,
            fytd_samples,
        })
    }

    /// Period-aggregated statistics for a specific ticker.
    ///
    /// # Arguments
    ///
    /// * `ticker_idx` - Zero-based column index of the ticker.
    /// * `aggregation_frequency` - Calendar or fiscal bucket the per-period returns
    ///   are compounded into before the statistics are taken.
    /// * `fiscal_config` - Fiscal year start used to bucket fiscal periods;
    ///   `None` uses calendar periods; `Some` applies fiscal boundaries at every frequency.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::InputError::InvalidReturnSeries`] when
    /// `ticker_idx` is outside the loaded ticker columns, or a validation error
    /// when a containing period boundary is outside the supported date range.
    pub fn period_stats(
        &self,
        ticker_idx: usize,
        aggregation_frequency: PeriodKind,
        fiscal_config: Option<FiscalConfig>,
    ) -> finstack_quant_core::Result<PeriodStats> {
        self.ensure_ticker_idx(ticker_idx)?;
        let grouped = group_period_buckets(
            self.active_dates_for_ticker_unchecked(ticker_idx),
            self.active_returns(ticker_idx),
            aggregation_frequency,
            fiscal_config,
        )?;
        period_stats_inner(grouped.into_iter().map(|point| point.value))
    }

    /// Calendar-bucketed compounded returns per ticker.
    ///
    /// Returns one `Vec<PeriodicReturn>` per ticker — each entry is the
    /// last actual observed endpoint and compounded return of one calendar bucket of `frequency`.
    /// Buckets compound original observations via the shared kernel, so their unrounded wealth reconciles with
    /// [`Performance::cumulative_returns`]. Returns are simple decimal fractions
    /// (`0.01` means 1%), ticker series follow [`Performance::get_ticker_names`] order,
    /// and points within each series are chronological. Calendar bucketing only.
    ///
    /// # Arguments
    ///
    /// * `frequency` - Calendar bucket size. Supported [`PeriodKind`] values are
    ///   daily, weekly, monthly, quarterly, semi_annual, and annual.
    ///
    /// # Returns
    ///
    /// A ticker-major panel of chronological [`PeriodicReturn`] points over
    /// the active analysis window.
    ///
    /// # Errors
    ///
    /// Returns a validation error when a containing calendar period boundary
    /// is outside the supported date range.
    pub fn periodic_returns(
        &self,
        frequency: PeriodKind,
    ) -> finstack_quant_core::Result<Vec<Vec<PeriodicReturn>>> {
        (0..self.get_ticker_names().len())
            .map(|i| {
                group_period_buckets(
                    self.active_dates_for_ticker_unchecked(i),
                    self.active_returns(i),
                    frequency,
                    None,
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Performance;
    use finstack_quant_core::dates::PeriodKind;
    use time::Month;

    fn jan(day: u8) -> Date {
        Date::from_calendar_date(2024, Month::January, day).expect("valid date")
    }

    /// Build a single-ticker `Performance` with daily returns spanning
    /// January and February 2021 (2021-01-04 through 2021-02-26).
    fn sample_two_month_daily_performance() -> Performance {
        let jan_dates: Vec<Date> = (4u8..=29)
            .filter_map(|d| Date::from_calendar_date(2021, Month::January, d).ok())
            .collect();
        let feb_dates: Vec<Date> = (1u8..=26)
            .filter_map(|d| Date::from_calendar_date(2021, Month::February, d).ok())
            .collect();

        let mut dates = jan_dates;
        dates.extend(feb_dates);

        let n = dates.len();
        let returns = vec![vec![0.001_f64; n]];

        Performance::new(&crate::performance::returns_panel(
            {
                let mut grid = vec![Date::from_calendar_date(2021, Month::January, 1).unwrap()];
                grid.extend(dates);
                grid
            },
            returns,
            vec!["TEST".to_string()],
            None,
            PeriodKind::Daily,
        ))
        .unwrap()
    }

    fn rectangular_perf() -> Performance {
        let dates = vec![jan(2), jan(3), jan(4), jan(5)];
        Performance::new(&crate::performance::returns_panel(
            {
                let mut grid = vec![jan(1)];
                grid.extend(dates);
                grid
            },
            vec![vec![0.01, 0.02, 0.03, 0.04], vec![0.05, 0.06, 0.07, 0.08]],
            vec!["A".into(), "B".into()],
            None,
            PeriodKind::Daily,
        ))
        .expect("rectangular panel")
    }

    fn ragged_perf() -> Performance {
        let dates = vec![jan(2), jan(3), jan(4), jan(5)];
        Performance::new(&crate::performance::returns_panel(
            {
                let mut grid = vec![jan(1)];
                grid.extend(dates);
                grid
            },
            vec![
                vec![0.01, 0.02, 0.03, 0.04],
                vec![f64::NAN, f64::NAN, 0.10, 0.20],
            ],
            vec!["A".into(), "B".into()],
            None,
            PeriodKind::Daily,
        ))
        .expect("ragged panel")
    }

    #[test]
    fn lookback_windows_start_on_calendar_and_fiscal_boundaries() {
        let on = |year: i32, month: Month, day: u8| {
            Date::from_calendar_date(year, month, day).expect("valid date")
        };
        let dates = vec![
            on(2024, Month::December, 31),
            on(2025, Month::January, 1),
            on(2025, Month::February, 27),
            on(2025, Month::February, 28),
            on(2025, Month::March, 31),
            on(2025, Month::April, 1),
            on(2025, Month::April, 30),
            on(2025, Month::May, 1),
            on(2025, Month::May, 15),
            on(2025, Month::May, 16),
        ];
        let r = [
            0.01, 0.02, -0.03, 0.04, 0.05, -0.06, 0.07, 0.08, -0.09, 0.10,
        ];
        let perf = Performance::new(&crate::performance::returns_panel(
            {
                let mut grid = vec![on(2024, Month::December, 30)];
                grid.extend(dates);
                grid
            },
            vec![r.to_vec()],
            vec!["A".to_string()],
            None,
            PeriodKind::Daily,
        ))
        .expect("panel");
        let ref_date = on(2025, Month::May, 15);

        let us_federal = perf
            .lookback_returns(Some(ref_date), Some(FiscalConfig::us_federal()))
            .expect("lookback");
        assert_eq!(us_federal.mtd, vec![comp_total(&r[7..9]).unwrap()]);
        assert_eq!(us_federal.qtd, vec![comp_total(&r[5..9]).unwrap()]);
        assert_eq!(us_federal.ytd, vec![comp_total(&r[1..9]).unwrap()]);
        assert_eq!(us_federal.fytd, vec![comp_total(&r[0..9]).unwrap()]);

        let clamped = perf
            .lookback_returns(
                Some(ref_date),
                Some(FiscalConfig::new(2, 30).expect("day 30 clamps")),
            )
            .expect("lookback");
        assert_eq!(clamped.fytd, vec![comp_total(&r[3..9]).unwrap()]);
    }

    #[test]
    fn lookback_returns_default_fiscal_year_is_calendar_year() {
        let perf = sample_two_month_daily_performance();
        let ref_date = *perf.get_dates().last().unwrap();
        let omitted = perf
            .lookback_returns(Some(ref_date), None)
            .expect("lookback");
        let explicit = perf
            .lookback_returns(Some(ref_date), Some(FiscalConfig::calendar_year()))
            .expect("lookback");
        assert_eq!(omitted.fytd, explicit.fytd);
        assert_eq!(omitted.ytd, explicit.fytd);
    }

    #[test]
    fn lookback_returns_ends_on_the_last_active_date_when_no_date_is_given() {
        let mut perf = sample_two_month_daily_performance();
        let dates = perf.get_dates().to_vec();
        let window_end = dates[dates.len() / 2];
        perf.reset_date_range(dates[0], window_end).unwrap();

        let omitted = perf.lookback_returns(None, None).expect("lookback");
        assert_eq!(Some(&omitted.ref_date), perf.get_active_dates().last());
        let explicit = perf
            .lookback_returns(Some(omitted.ref_date), None)
            .expect("lookback");
        assert_eq!(omitted.mtd, explicit.mtd);
        assert_eq!(omitted.ref_date, explicit.ref_date);

        let requested = perf
            .lookback_returns(Some(dates[0]), None)
            .expect("lookback");
        assert_eq!(requested.ref_date, dates[0]);
    }

    #[test]
    fn lookback_returns_rejects_no_date_over_an_empty_active_range() {
        let mut perf = sample_two_month_daily_performance();
        let dates = perf.get_dates().to_vec();
        let after = dates[dates.len() - 1].next_day().unwrap();
        perf.reset_date_range(after, after).unwrap();
        assert!(perf.get_active_dates().is_empty());
        let error = perf
            .lookback_returns(None, None)
            .expect_err("no active dates");
        assert!(error.to_string().contains("ref_date"));
    }

    #[test]
    fn periodic_returns_monthly_has_one_bucket_per_month() {
        let perf = sample_two_month_daily_performance();
        let periodic = perf
            .periodic_returns(PeriodKind::Monthly)
            .expect("calendar periods");
        assert_eq!(periodic.len(), perf.get_ticker_names().len());
        assert_eq!(periodic[0].len(), 2);
        assert_eq!(periodic[0][0].date.month(), Month::January);
        assert_eq!(periodic[0][1].date.month(), Month::February);
        let cum = perf.cumulative_returns().unwrap();
        let total = *cum[0].last().unwrap();
        let chained = (1.0 + periodic[0][0].value) * (1.0 + periodic[0][1].value) - 1.0;
        assert!((chained - total).abs() < 1e-12);
    }

    #[test]
    fn lookback_cutoff_retains_actual_partial_holding_coverage() {
        let perf = Performance::new(&crate::performance::returns_panel(
            vec![jan(1), jan(2), jan(5)],
            vec![vec![0.01, 0.02]],
            vec!["A".into()],
            None,
            PeriodKind::Daily,
        ))
        .unwrap();
        let result = perf.lookback_returns(Some(jan(4)), None).unwrap();
        assert_eq!(result.ref_date, jan(4));
        assert!((result.mtd[0] - 0.01).abs() < 1e-16);
        assert_eq!(
            result.mtd_samples[0],
            LookbackSample {
                n_obs: 1,
                holding_start: Some(jan(1)),
                holding_end: Some(jan(2)),
            }
        );
        assert_eq!(result.fiscal_config, FiscalConfig::calendar_year());
        let mut wire = serde_json::to_value(&result).unwrap();
        wire["mtd_samples"][0]["holding_end"] = serde_json::json!("2024-01-05");
        assert!(serde_json::from_value::<LookbackReturns>(wire).is_err());
    }

    #[test]
    fn empty_lookback_preserves_undefined_values_and_validated_wire_coverage() {
        let perf = rectangular_perf();
        let before = jan(1);
        let result = perf.lookback_returns(Some(before), None).unwrap();
        assert!(result.mtd.iter().all(|value| value.is_nan()));
        assert!(result.mtd_samples.iter().all(|sample| sample.n_obs == 0
            && sample.holding_start.is_none()
            && sample.holding_end.is_none()));
        let wire = serde_json::to_value(&result).unwrap();
        assert_eq!(wire["mtd"][0], "nan");
        let restored: LookbackReturns = serde_json::from_value(wire.clone()).unwrap();
        assert!(restored.mtd[0].is_nan());
        let mut invalid = wire;
        invalid["mtd"][0] = serde_json::json!(0.0);
        assert!(serde_json::from_value::<LookbackReturns>(invalid).is_err());
    }

    #[test]
    fn excess_returns_rectangular_identity_with_zero_rf() {
        let perf = rectangular_perf();
        let rf = vec![0.0; perf.get_active_dates().len()];
        let excess = perf.excess_returns(&rf).expect("aligned rf");
        assert_eq!(excess, perf.get_returns());
    }

    #[test]
    fn excess_returns_ragged_uses_panel_index_not_series_head() {
        let perf = ragged_perf();
        let rf = vec![0.001, 0.002, 0.003, 0.004];
        let excess = perf.excess_returns(&rf).expect("aligned periodic rf");
        assert_eq!(excess[0].len(), 4);
        let q = |cash: f64| (cash.ln_1p() / 252.0).exp_m1();
        assert!((excess[0][0] - (0.01 - q(0.001))).abs() < 1e-15);
        assert_eq!(excess[1].len(), 2);
        assert!(
            (excess[1][0] - (0.10 - q(0.003))).abs() < 1e-15,
            "ticker B must subtract rf[2], not rf[0]; got {}",
            excess[1][0]
        );
        assert!((excess[1][1] - (0.20 - q(0.004))).abs() < 1e-15);
        assert!((excess[1][0] - (0.10 - q(0.001))).abs() > 1e-6);
    }

    #[test]
    fn excess_returns_length_mismatch_is_error() {
        let perf = rectangular_perf();
        let err = perf
            .excess_returns(&[0.0, 0.0])
            .expect_err("length mismatch");
        let msg = err.to_string();
        assert!(
            msg.contains("rf.len()") || msg.contains("active date"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn correlation_matrix_two_asset_rectangular_identity() {
        let dates = vec![jan(1), jan(2), jan(3), jan(4), jan(5)];
        let r = vec![0.01, 0.02, -0.01, 0.03];
        let perf = Performance::new(&crate::performance::returns_panel(
            dates,
            vec![r.clone(), r],
            vec!["A".into(), "B".into()],
            None,
            PeriodKind::Daily,
        ))
        .expect("panel");
        let result = perf.correlation_matrix().expect("psd correlation");
        let m = result.matrix;
        assert_eq!(m[0][0], 1.0);
        assert_eq!(m[1][1], 1.0);
        assert!((m[0][1] - 1.0).abs() < 1e-12 && m[0][1] <= 1.0);
        assert!((m[1][0] - 1.0).abs() < 1e-12 && m[1][0] <= 1.0);
    }

    #[test]
    fn correlation_matrix_zero_variance_pair_is_error() {
        let dates = vec![jan(1), jan(2), jan(3), jan(4), jan(5)];
        let perf = Performance::new(&crate::performance::returns_panel(
            dates,
            vec![vec![0.01, 0.02, -0.01, 0.03], vec![0.05, 0.05, 0.05, 0.05]],
            vec!["A".into(), "B".into()],
            None,
            PeriodKind::Daily,
        ))
        .expect("panel");
        let err = perf.correlation_matrix().expect_err("zero-variance pair");
        let msg = err.to_string();
        assert!(
            msg.contains("zero-variance") || msg.contains("correlation"),
            "unexpected error: {msg}"
        );
    }
}
