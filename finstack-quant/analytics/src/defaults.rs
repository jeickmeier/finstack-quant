//! Values the host bindings use when a caller omits an analytics argument.
//!
//! Rust analytics APIs take every parameter explicitly. The Python and
//! WebAssembly bindings make many of them optional; each omitted argument
//! resolves to one of these constants, so both hosts share a single source for
//! every default.

use crate::dates::PeriodKind;

/// Observations per year assumed by the scalar [`crate::sharpe`],
/// [`crate::sortino`] and [`crate::volatility`] when the caller omits it.
///
/// The daily trading-day convention: equal to
/// `PeriodKind::Daily.annualization_factor()` (252).
pub const DEFAULT_PERIODS_PER_YEAR: f64 = PeriodKind::Daily.annualization_factor();

/// Annualized risk-free rate (decimal) used when a Sharpe-style, Treynor,
/// M-squared, Jensen-alpha or total-return regression call omits it.
///
/// Zero, so the metric reduces to its raw (non-excess) form.
pub const DEFAULT_RISK_FREE_RATE: f64 = 0.0;

/// Per-period minimum acceptable return (decimal) used when a Sortino,
/// downside-deviation or Omega-ratio call omits its threshold.
///
/// Zero: downside is measured against a flat return.
pub const DEFAULT_MAR: f64 = 0.0;

/// Observation frequency of a [`crate::Performance`] panel when the caller
/// omits it: daily.
pub const DEFAULT_FREQUENCY: PeriodKind = PeriodKind::Daily;

/// Calendar bucket used by [`crate::Performance::periodic_returns`] and
/// [`crate::Performance::period_stats`] when the caller omits it: monthly.
pub const DEFAULT_PERIODIC_FREQUENCY: PeriodKind = PeriodKind::Monthly;

/// Window length, in observations, of the rolling Sharpe, Sortino,
/// volatility and Greeks series when the caller omits it.
///
/// 63 observations is one quarter of daily data.
pub const DEFAULT_ROLLING_WINDOW: usize = 63;

/// Tail confidence (decimal probability) of VaR, expected shortfall, CDaR,
/// tail-ratio and modified-Sharpe calls when the caller omits it.
pub const DEFAULT_CONFIDENCE: f64 = 0.95;

/// Number of largest drawdowns used by the Sterling and Burke ratios and
/// reported by [`crate::Performance::drawdown_details`] when the caller omits
/// it.
pub const DEFAULT_DRAWDOWN_COUNT: usize = 5;

/// Whether [`crate::Performance::mean_return`] and
/// [`crate::Performance::volatility`] annualize when the caller omits the flag.
pub const DEFAULT_ANNUALIZE: bool = true;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periods_per_year_matches_daily_annualization() {
        assert_eq!(
            DEFAULT_PERIODS_PER_YEAR,
            PeriodKind::Daily.annualization_factor()
        );
        assert_eq!(DEFAULT_PERIODS_PER_YEAR, 252.0);
    }
}
