//! Benchmark-relative metrics on [`Performance`] (alpha/beta/greeks,
//! tracking error, information ratio, R², capture, multi-factor).
//!
//! Pure layout split from `performance.rs`; no behavior changes.

use super::Performance;
use crate::benchmark::{
    batting_average, beta, beta_only, capture_ratio, down_capture, greeks, information_ratio,
    m_squared, multi_factor_greeks, r_squared, rolling_greeks, tracking_error, treynor, up_capture,
    BetaResult, GreeksResult, MultiFactorResult, RollingGreeks,
};
use crate::risk_metrics;

impl Performance {
    /// Treynor ratio for each ticker.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Annualized risk-free rate in decimal form.
    ///
    /// # Returns
    ///
    /// One Treynor ratio per ticker in column order, using the active
    /// benchmark to estimate beta.
    pub fn treynor(&self, risk_free_rate: f64) -> Vec<f64> {
        let ann = self.ann();
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            let ann_ret = risk_metrics::mean_return(r, true, ann);
            let beta = beta_only(r, bench);
            treynor(ann_ret, risk_free_rate, beta)
        })
    }

    /// Empyrical-style annualized geometric up-capture for each ticker versus the active benchmark.
    pub fn up_capture(&self) -> Vec<f64> {
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            up_capture(r, bench, self.ann())
        })
    }

    /// Empyrical-style annualized geometric down-capture for each ticker versus the active benchmark.
    pub fn down_capture(&self) -> Vec<f64> {
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            down_capture(r, bench, self.ann())
        })
    }

    /// Empyrical-style annualized geometric capture ratio for each ticker.
    pub fn capture_ratio(&self) -> Vec<f64> {
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            capture_ratio(r, bench, self.ann())
        })
    }

    /// Annualized tracking error for each ticker versus the active benchmark.
    pub fn tracking_error(&self) -> Vec<f64> {
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            tracking_error(r, bench, true, self.ann())
        })
    }

    /// Annualized information ratio for each ticker versus the active benchmark.
    pub fn information_ratio(&self) -> Vec<f64> {
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            information_ratio(r, bench, true, self.ann())
        })
    }

    /// R-squared for each ticker versus the active benchmark.
    pub fn r_squared(&self) -> Vec<f64> {
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            r_squared(r, bench)
        })
    }

    /// OLS beta estimates for each ticker versus the active benchmark.
    pub fn beta(&self) -> Vec<BetaResult> {
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            beta(r, bench)
        })
    }

    /// Single-factor greeks for each ticker versus the active benchmark.
    ///
    /// Alpha is annualized Jensen alpha using the configured observation
    /// frequency and the supplied annualized risk-free rate.
    pub fn greeks(&self, risk_free_rate: f64) -> Vec<GreeksResult> {
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            greeks(r, bench, self.ann(), risk_free_rate)
        })
    }

    /// Rolling greeks (alpha, beta) for a specific ticker vs the benchmark.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::InputError::InvalidReturnSeries`] when
    /// `ticker_idx` is outside the loaded ticker columns.
    pub fn rolling_greeks(
        &self,
        ticker_idx: usize,
        window: usize,
        risk_free_rate: f64,
    ) -> crate::Result<RollingGreeks> {
        self.ensure_ticker_idx(ticker_idx)?;
        let (r, bench) = self.active_pair_returns(ticker_idx);
        Ok(rolling_greeks(
            r,
            bench,
            self.active_pair_dates(ticker_idx),
            window,
            self.ann(),
            risk_free_rate,
        ))
    }

    /// Batting average for each ticker versus the active benchmark.
    ///
    /// Fraction of periods where the ticker's return exceeds the benchmark's
    /// return over the active window.
    pub fn batting_average(&self) -> Vec<f64> {
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            batting_average(r, bench)
        })
    }

    /// M-squared (Modigliani-Modigliani) for each ticker.
    pub fn m_squared(&self, risk_free_rate: f64) -> Vec<f64> {
        let ann = self.ann();
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            let (ann_ret, ann_vol) = risk_metrics::mean_vol_annualized(r, ann);
            let (_, bench_vol) = risk_metrics::mean_vol_annualized(bench, ann);
            m_squared(ann_ret, ann_vol, bench_vol, risk_free_rate)
        })
    }

    /// Multi-factor regression for a specific ticker.
    ///
    /// # Errors
    ///
    /// Propagates errors from the underlying multi-factor greeks calculation
    /// when factor inputs are mismatched, non-finite, insufficient, or numerically singular.
    pub fn multi_factor_greeks(
        &self,
        ticker_idx: usize,
        factor_returns: &[&[f64]],
    ) -> crate::Result<MultiFactorResult> {
        self.ensure_ticker_idx(ticker_idx)?;
        multi_factor_greeks(self.active_returns(ticker_idx), factor_returns, self.ann())
    }
}
