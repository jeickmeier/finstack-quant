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
        let bench = self.active_bench();
        self.map_tickers(|i| {
            let r = self.active_returns(i);
            let ann_ret = risk_metrics::mean_return(r, true, ann);
            let beta = beta_only(r, bench);
            treynor(ann_ret, risk_free_rate, beta)
        })
    }

    /// Up-market capture ratio for each ticker versus the active benchmark.
    pub fn up_capture(&self) -> Vec<f64> {
        let bench = self.active_bench();
        self.map_tickers(|i| up_capture(self.active_returns(i), bench))
    }

    /// Down-market capture ratio for each ticker versus the active benchmark.
    pub fn down_capture(&self) -> Vec<f64> {
        let bench = self.active_bench();
        self.map_tickers(|i| down_capture(self.active_returns(i), bench))
    }

    /// Capture ratio (up-capture divided by down-capture) for each ticker.
    pub fn capture_ratio(&self) -> Vec<f64> {
        let bench = self.active_bench();
        self.map_tickers(|i| capture_ratio(self.active_returns(i), bench))
    }

    /// Annualized tracking error for each ticker versus the active benchmark.
    pub fn tracking_error(&self) -> Vec<f64> {
        let bench = self.active_bench();
        self.map_tickers(|i| tracking_error(self.active_returns(i), bench, true, self.ann()))
    }

    /// Annualized information ratio for each ticker versus the active benchmark.
    pub fn information_ratio(&self) -> Vec<f64> {
        let bench = self.active_bench();
        self.map_tickers(|i| information_ratio(self.active_returns(i), bench, true, self.ann()))
    }

    /// R-squared for each ticker versus the active benchmark.
    pub fn r_squared(&self) -> Vec<f64> {
        let bench = self.active_bench();
        self.map_tickers(|i| r_squared(self.active_returns(i), bench))
    }

    /// OLS beta estimates for each ticker versus the active benchmark.
    pub fn beta(&self) -> Vec<BetaResult> {
        let bench = self.active_bench();
        self.map_tickers(|i| beta(self.active_returns(i), bench))
    }

    /// Single-factor greeks for each ticker versus the active benchmark.
    ///
    /// Alpha is annualized using the configured observation frequency.
    pub fn greeks(&self) -> Vec<GreeksResult> {
        let bench = self.active_bench();
        self.map_tickers(|i| greeks(self.active_returns(i), bench, self.ann()))
    }

    /// Rolling greeks (alpha, beta) for a specific ticker vs the benchmark.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::InputError::InvalidReturnSeries`] when
    /// `ticker_idx` is outside the loaded ticker columns.
    pub fn rolling_greeks(&self, ticker_idx: usize, window: usize) -> crate::Result<RollingGreeks> {
        self.ensure_ticker_idx(ticker_idx)?;
        Ok(rolling_greeks(
            self.active_returns(ticker_idx),
            self.active_bench(),
            self.active_dates(),
            window,
            self.ann(),
        ))
    }

    /// Batting average for each ticker versus the active benchmark.
    ///
    /// Fraction of periods where the ticker's return exceeds the benchmark's
    /// return over the active window.
    pub fn batting_average(&self) -> Vec<f64> {
        let bench = self.active_bench();
        self.map_tickers(|i| batting_average(self.active_returns(i), bench))
    }

    /// M-squared (Modigliani-Modigliani) for each ticker.
    pub fn m_squared(&self, risk_free_rate: f64) -> Vec<f64> {
        let ann = self.ann();
        let bench = self.active_bench();
        let (_, bench_vol) = risk_metrics::mean_vol_annualized(bench, ann);
        self.map_tickers(|i| {
            let (ann_ret, ann_vol) = risk_metrics::mean_vol_annualized(self.active_returns(i), ann);
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
