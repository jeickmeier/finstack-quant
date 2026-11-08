//! Benchmark-relative statistics on exactly paired active observations.

use super::Performance;
use crate::benchmark::{
    BetaResult, GreeksResult, RollingGreeks, batting_average, beta, capture_ratio, down_capture,
    greeks, information_ratio, m_squared_from_returns, r_squared, rolling_greeks, tracking_error,
    treynor_from_returns, up_capture,
};
use crate::multi_factor::{MultiFactorResult, ReturnKind, multi_factor_greeks};
use crate::returns::validate_cash;
use finstack_quant_core::Result;

impl Performance {
    /// Annual arithmetic excess return per unit of intercept-OLS benchmark beta.
    ///
    /// Uses exactly paired observations; two pairs and varying benchmark are
    /// required. Zero beta gives signed infinite reward/risk for nonzero reward
    /// and undefined for zero reward, without a beta-magnitude cutoff.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Explicit finite annual-effective decimal cash rate
    ///   greater than `-1`, geometrically converted at the observation frequency.
    ///
    /// # Errors
    ///
    /// Returns a validation error for missing benchmark/frequency or invalid
    /// cash, and a calculation error for unresolved numerical failure.
    pub fn treynor(&self, risk_free_rate: f64) -> Result<Vec<f64>> {
        let ann = self.ann()?;
        validate_cash(risk_free_rate)?;
        self.map_pair_returns(|r, bench| treynor_from_returns(r, bench, risk_free_rate, ann))
    }

    /// Signed annualized geometric capture on strictly positive benchmark periods.
    ///
    /// Uses the declared full observation frequency and selected observation
    /// count. Empty regimes are undefined; ratios are not multiplied by 100.
    /// Returns an error for missing frequency/benchmark or numerical failure.
    pub fn up_capture(&self) -> Result<Vec<f64>> {
        let ann = self.ann()?;
        self.map_pair_returns(|r, bench| up_capture(r, bench, ann))
    }

    /// Signed annualized geometric capture on strictly negative benchmark periods.
    ///
    /// Empty regimes are undefined. Negative values retain portfolio gains in
    /// declining benchmark periods. Errors identify missing required context or
    /// unresolved numerical evaluation.
    pub fn down_capture(&self) -> Result<Vec<f64>> {
        let ann = self.ann()?;
        self.map_pair_returns(|r, bench| down_capture(r, bench, ann))
    }

    /// Up capture divided by down capture on their respective strict regimes.
    ///
    /// Both regimes must be available and down capture nonzero; otherwise the
    /// result is undefined. Errors identify missing context or numerical failure.
    pub fn capture_ratio(&self) -> Result<Vec<f64>> {
        let ann = self.ann()?;
        self.map_pair_returns(|r, bench| capture_ratio(r, bench, ann))
    }

    /// Annualized centered sample SD of paired arithmetic active differences.
    ///
    /// Fewer than two paired observations is unavailable; observed constant
    /// differences have zero tracking error. Errors identify missing required
    /// frequency/benchmark or an unresolved calculation failure.
    pub fn tracking_error(&self) -> Result<Vec<f64>> {
        let ann = self.ann()?;
        self.map_pair_returns(|r, bench| tracking_error(r, bench, ann))
    }

    /// Arithmetic active reward divided by centered sample tracking risk.
    ///
    /// Requires two pairs. Genuine zero risk gives signed infinity for nonzero
    /// reward and undefined for zero reward. Errors identify missing required
    /// frequency/benchmark or an unresolved calculation failure.
    pub fn information_ratio(&self) -> Result<Vec<f64>> {
        let ann = self.ann()?;
        self.map_pair_returns(|r, bench| information_ratio(r, bench, ann))
    }

    /// Centered intercept-OLS R-squared on each ticker's benchmark overlap.
    ///
    /// Fewer than two pairs or either constant series gives undefined results.
    /// No cash or observation frequency is required. Missing benchmark and
    /// numerical evaluation failures return errors.
    pub fn r_squared(&self) -> Result<Vec<f64>> {
        self.map_pair_returns(r_squared)
    }

    /// Intercept-OLS beta and fixed 95% classical Student-t uncertainty.
    ///
    /// Two pairs and a varying benchmark suffice for beta; uncertainty requires
    /// three pairs. Unavailable fields preserve NaN independently and retain the
    /// actual sample count. No cash/frequency is required. Errors identify missing
    /// benchmark and unresolved numerical evaluation.
    pub fn beta(&self) -> Result<Vec<BetaResult>> {
        self.map_pair_returns(beta)
    }

    /// Annual Jensen alpha, beta and centered OLS fit statistics on paired spans.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Explicit finite annual-effective decimal cash rate
    ///   greater than `-1`, converted geometrically to each observation period.
    ///
    /// # Errors
    ///
    /// Returns errors for missing benchmark/frequency, invalid cash and
    /// unresolved numerical evaluation. Unavailable fields remain NaN.
    pub fn greeks(&self, risk_free_rate: f64) -> Result<Vec<GreeksResult>> {
        let ann = self.ann()?;
        validate_cash(risk_free_rate)?;
        self.map_pair_returns(|r, bench| greeks(r, bench, ann, risk_free_rate))
    }

    /// Compact trailing-window Jensen alpha and beta on exact paired dates.
    ///
    /// Complete windows are labelled by their final observation date; oversized
    /// windows return empty arrays, and one-observation windows retain undefined
    /// point estimates without dropping their dates.
    ///
    /// # Arguments
    ///
    /// * `ticker_idx` - Zero-based ticker column in the loaded panel.
    /// * `window` - Explicit positive count of paired observations per window.
    /// * `risk_free_rate` - Explicit finite annual-effective decimal cash rate
    ///   greater than `-1`, converted geometrically at the declared frequency.
    ///
    /// # Errors
    ///
    /// Returns errors for invalid ticker/window/cash, missing benchmark/frequency,
    /// malformed alignment, or unresolved numerical evaluation.
    pub fn rolling_greeks(
        &self,
        ticker_idx: usize,
        window: usize,
        risk_free_rate: f64,
    ) -> Result<RollingGreeks> {
        self.ensure_ticker_idx(ticker_idx)?;
        Self::ensure_window(window)?;
        let ann = self.ann()?;
        validate_cash(risk_free_rate)?;
        let benchmark_idx = self.benchmark()?;
        let span = self.active_two_ticker_span(ticker_idx, benchmark_idx);
        rolling_greeks(
            self.returns_for_span(ticker_idx, span),
            self.returns_for_span(benchmark_idx, span),
            self.dates_for_span(span),
            window,
            ann,
            risk_free_rate,
        )
    }

    /// Fraction of paired observations with strict ticker outperformance.
    ///
    /// Ties remain in the denominator; empty overlap is undefined. No frequency
    /// or cash is required. Missing benchmark or numeric failure returns an error.
    pub fn batting_average(&self) -> Result<Vec<f64>> {
        self.map_pair_returns(batting_average)
    }

    /// Absolute arithmetic annual risk-matched return, including annual cash.
    ///
    /// Uses the same paired span for portfolio and benchmark sample SD. Zero
    /// portfolio SD is undefined; positive portfolio SD and zero benchmark SD
    /// returns arithmetic annual cash. No volatility floor is applied.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Explicit finite annual-effective decimal cash rate
    ///   greater than `-1`; annual cash is `A * expm1(log1p(rate) / A)`.
    ///
    /// # Errors
    ///
    /// Returns errors for missing benchmark/frequency, invalid cash or unresolved
    /// calculation failure. Insufficient paired history remains undefined.
    pub fn m_squared(&self, risk_free_rate: f64) -> Result<Vec<f64>> {
        let ann = self.ann()?;
        validate_cash(risk_free_rate)?;
        self.map_pair_returns(|r, bench| m_squared_from_returns(r, bench, risk_free_rate, ann))
    }

    /// Multifactor intercept OLS for a ticker's total investment returns.
    ///
    /// The panel response is total returns; only the response has geometric
    /// period cash subtracted. Factor arrays already carry their caller-selected
    /// interpretation. Use the raw canonical multifactor function for an
    /// explicitly excess-return response.
    ///
    /// # Arguments
    ///
    /// * `ticker_idx` - Zero-based panel ticker column.
    /// * `factor_returns` - Finite factor arrays exactly matching the ticker's
    ///   active return dates and lengths, prealigned by the caller.
    /// * `risk_free_rate` - Explicit finite annual-effective decimal cash rate
    ///   greater than `-1`, converted geometrically at the declared frequency.
    ///
    /// # Errors
    ///
    /// Returns errors for invalid ticker/cash, missing frequency, malformed factor
    /// inputs, deficient design rank or unresolved numerical evaluation.
    pub fn multi_factor_greeks(
        &self,
        ticker_idx: usize,
        factor_returns: &[&[f64]],
        risk_free_rate: f64,
    ) -> Result<MultiFactorResult> {
        self.ensure_ticker_idx(ticker_idx)?;
        multi_factor_greeks(
            self.active_returns(ticker_idx),
            factor_returns,
            self.ann()?,
            ReturnKind::Total { risk_free_rate },
        )
    }
}
