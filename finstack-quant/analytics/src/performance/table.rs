//! Rectangular and tabular exits for [`Performance`].
//!
//! Ragged per-ticker series are aligned onto one date grid here, and the
//! standard per-ticker summary is assembled as a [`TableEnvelope`], so every
//! host (pandas, JS) reads the same rows.

use std::collections::BTreeSet;

use finstack_quant_core::table::{TableColumn, TableColumnData, TableColumnRole, TableEnvelope};

use super::Performance;
use crate::dates::{Date, PeriodKind};
use crate::risk_metrics::CagrDayCount;

impl Performance {
    /// Align ragged per-ticker series onto the active date grid.
    ///
    /// `panel[t]` holds one value per active date of ticker `t` (the shape
    /// returned by [`Self::returns`] and [`Self::cumulative_returns`]).
    /// Each series is padded with `NaN` on dates the ticker is not active,
    /// so every returned row has exactly `ticker_names().len()` columns.
    ///
    /// # Arguments
    ///
    /// * `panel` - Exactly one value series per ticker in [`Self::ticker_names`]
    ///   order, each containing exactly one entry per active date of that ticker.
    ///
    /// # Returns
    ///
    /// `(dates, columns)` where `dates` is [`Self::active_dates`] and
    /// `columns[t][i]` is ticker `t`'s value on `dates[i]` (or `NaN`).
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::InputError::InvalidReturnSeries`] if the ticker
    /// count differs or a series length does not match its ticker's active dates.
    pub fn aligned_panel(&self, panel: Vec<Vec<f64>>) -> crate::Result<(Vec<Date>, Vec<Vec<f64>>)> {
        if panel.len() != self.ticker_names().len() {
            return Err(super::invalid_return_series(
                "<panel>",
                panel.len(),
                format!(
                    "panel contains {} series but {} tickers are loaded",
                    panel.len(),
                    self.ticker_names().len()
                ),
            )
            .into());
        }
        let dates = self.active_dates();
        let mut columns = Vec::with_capacity(panel.len());
        for (ticker_idx, series) in panel.into_iter().enumerate() {
            let ticker_dates = self.active_dates_for_ticker(ticker_idx)?;
            if series.len() != ticker_dates.len() {
                return Err(super::invalid_return_series(
                    &self.ticker_names()[ticker_idx],
                    series.len().min(ticker_dates.len()),
                    format!(
                        "panel series contains {} values but {} active dates are required",
                        series.len(),
                        ticker_dates.len()
                    ),
                )
                .into());
            }
            let mut padded = vec![f64::NAN; dates.len()];
            let mut global_idx = 0usize;
            for (&date, &value) in ticker_dates.iter().zip(series.iter()) {
                while global_idx < dates.len() && dates[global_idx] < date {
                    global_idx += 1;
                }
                if global_idx < dates.len() && dates[global_idx] == date {
                    padded[global_idx] = value;
                }
            }
            columns.push(padded);
        }
        Ok((dates.to_vec(), columns))
    }

    /// Calendar-bucketed returns aligned onto the union of period-end dates.
    ///
    /// Same buckets as [`Self::periodic_returns`]; tickers with no bucket on a
    /// given period-end carry `NaN` there.
    ///
    /// # Arguments
    ///
    /// * `frequency` - Calendar bucket (`Daily` … `Annual`).
    ///
    /// # Returns
    ///
    /// `(period_end_dates, columns)` with `columns[t][i]` the compounded
    /// return of ticker `t` over the bucket ending on `period_end_dates[i]`.
    #[must_use]
    pub fn periodic_returns_aligned(&self, frequency: PeriodKind) -> (Vec<Date>, Vec<Vec<f64>>) {
        let panel = self.periodic_returns(frequency);
        let dates: Vec<Date> = panel
            .iter()
            .flat_map(|series| series.iter().map(|(d, _)| *d))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let columns = panel
            .into_iter()
            .map(|series| {
                let mut padded = vec![f64::NAN; dates.len()];
                for (d, v) in series {
                    if let Ok(pos) = dates.binary_search(&d) {
                        padded[pos] = v;
                    }
                }
                padded
            })
            .collect();
        (dates, columns)
    }

    /// The standard per-ticker summary: one row per ticker, 22 metric columns.
    ///
    /// Columns (in order): `ticker`, `cagr`, `mean_return`, `volatility`,
    /// `sharpe`, `sortino`, `calmar`, `max_drawdown`, `value_at_risk`,
    /// `expected_shortfall`, `tracking_error`, `information_ratio`,
    /// `skewness`, `kurtosis`, `geometric_mean`, `downside_deviation`,
    /// `omega_ratio`, `gain_to_pain`, `ulcer_index`, `pain_index`,
    /// `recovery_factor`, `tail_ratio`, `r_squared`.
    ///
    /// `mean_return` and `volatility` are annualized; `cagr` uses
    /// [`CagrDayCount::default`]. The MAR-based metrics (`sortino`,
    /// `downside_deviation`) and the `omega_ratio` threshold are fixed at
    /// `0.0`; call the individual methods for other thresholds.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Annualized risk-free rate in decimal (`0.02` =
    ///   2%); affects only `sharpe`.
    /// * `confidence` - Tail confidence in `(0, 1)` (`0.95` = 95%) applied to
    ///   `value_at_risk`, `expected_shortfall` and `tail_ratio`.
    ///
    /// # Errors
    ///
    /// Rejects a `confidence` outside `(0, 1)` and propagates [`Self::cagr`]
    /// and [`Self::calmar`] failures (for example a panel too short to
    /// annualize).
    pub fn summary(&self, risk_free_rate: f64, confidence: f64) -> crate::Result<TableEnvelope> {
        Self::ensure_confidence(confidence)?;
        let (var, es) = self.value_at_risk_and_es(confidence);
        let (skew, kurt) = self.skew_kurt();
        let metrics: [(&str, Vec<f64>); 22] = [
            ("cagr", self.cagr(CagrDayCount::default(), None)?),
            ("mean_return", self.mean_return(true)),
            ("volatility", self.volatility(true)),
            ("sharpe", self.sharpe(risk_free_rate)),
            ("sortino", self.sortino(0.0)),
            ("calmar", self.calmar()?),
            ("max_drawdown", self.max_drawdown()),
            ("value_at_risk", var),
            ("expected_shortfall", es),
            ("tracking_error", self.tracking_error()),
            ("information_ratio", self.information_ratio()),
            ("skewness", skew),
            ("kurtosis", kurt),
            ("geometric_mean", self.geometric_mean()),
            ("downside_deviation", self.downside_deviation(0.0)),
            ("omega_ratio", self.omega_ratio(0.0)),
            ("gain_to_pain", self.gain_to_pain()),
            ("ulcer_index", self.ulcer_index()),
            ("pain_index", self.pain_index()),
            ("recovery_factor", self.recovery_factor()),
            ("tail_ratio", self.tail_ratio(confidence)?),
            ("r_squared", self.r_squared()),
        ];
        let mut columns = Vec::with_capacity(metrics.len() + 1);
        columns.push(
            TableColumn::new(
                "ticker",
                TableColumnData::String(self.ticker_names().to_vec()),
            )
            .with_role(TableColumnRole::Dimension),
        );
        for (name, values) in metrics {
            columns.push(
                TableColumn::new(name, TableColumnData::Float64(values))
                    .with_role(TableColumnRole::Measure),
            );
        }
        TableEnvelope::new(columns)
    }
}
