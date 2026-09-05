//! Freestanding scalar metrics over a single simple-return series.
//!
//! These are the four numbers a desk reaches for first — Sharpe, Sortino,
//! annualized volatility and maximum drawdown — exposed without building a
//! [`crate::Performance`] panel. Each takes a slice of per-period simple
//! decimal returns (`0.01` for +1%) and the number of periods per year used
//! for annualization (`252.0` daily, `12.0` monthly, ...). They share the
//! kernels the panel methods use, so `sharpe(&r, rf, 252.0)` equals
//! `Performance::sharpe(rf)[i]` for ticker `i` built at daily frequency.

use crate::drawdown::to_drawdown_series;
use crate::risk_metrics;

/// Sharpe ratio of one return series.
///
/// Annualized excess arithmetic mean over annualized sample volatility. The
/// annual `risk_free_rate` is geometrically decompounded to the observation
/// frequency before subtraction, exactly as [`crate::Performance::sharpe`].
///
/// # Arguments
///
/// * `returns` - Per-period simple decimal returns in date order.
/// * `risk_free_rate` - Annualized risk-free rate as a decimal (`0.02` for 2%).
/// * `periods_per_year` - Observations per year used to annualize (`252.0`
///   daily, `52.0` weekly, `12.0` monthly).
///
/// # Returns
///
/// The Sharpe ratio; `±∞` when volatility is zero with a non-zero excess
/// return, `0.0` when both are zero, and `NaN` when `periods_per_year` is
/// not a positive finite number, an input is non-finite, or fewer than two
/// observations are available.
#[must_use]
pub fn sharpe(returns: &[f64], risk_free_rate: f64, periods_per_year: f64) -> f64 {
    if returns.len() < 2 {
        return f64::NAN;
    }
    let (ann_return, ann_vol) = risk_metrics::mean_vol_annualized(returns, periods_per_year);
    if !ann_return.is_finite() || !ann_vol.is_finite() {
        return f64::NAN;
    }
    risk_metrics::sharpe(ann_return, ann_vol, risk_free_rate, periods_per_year)
}

/// Annualized Sortino ratio of one return series.
///
/// # Arguments
///
/// * `returns` - Per-period simple decimal returns in date order.
/// * `mar` - Minimum acceptable return **per period** as a decimal (not
///   annualized), matching [`crate::Performance::sortino`].
/// * `periods_per_year` - Observations per year used to annualize.
///
/// # Returns
///
/// The Sortino ratio; `±∞` when there is no downside deviation but a
/// non-zero excess mean, and `NaN` for non-finite inputs, invalid
/// `periods_per_year`, or fewer than two observations.
#[must_use]
pub fn sortino(returns: &[f64], mar: f64, periods_per_year: f64) -> f64 {
    risk_metrics::sortino(returns, true, periods_per_year, mar)
}

/// Annualized sample volatility (n−1 denominator) of one return series.
///
/// # Arguments
///
/// * `returns` - Per-period simple decimal returns in date order.
/// * `periods_per_year` - Observations per year; the per-period standard
///   deviation is scaled by its square root.
///
/// # Returns
///
/// Annualized volatility as a decimal; `0.0` for an empty slice and `NaN`
/// when `periods_per_year` is not a positive finite number.
#[must_use]
pub fn volatility(returns: &[f64], periods_per_year: f64) -> f64 {
    risk_metrics::volatility(returns, true, periods_per_year)
}

/// Maximum peak-to-trough drawdown of one return series.
///
/// Compounds the returns into a wealth path and reports the deepest
/// fractional decline from a running peak.
///
/// # Arguments
///
/// * `returns` - Per-period simple decimal returns in date order.
///
/// # Returns
///
/// A non-positive fraction (`-0.25` for a 25% loss); `0.0` when the series
/// never falls below its running peak or is empty. Returns `NaN` if the
/// return or reconstructed drawdown path contains a non-finite value.
#[must_use]
pub fn max_drawdown(returns: &[f64]) -> f64 {
    crate::drawdown::max_drawdown(&to_drawdown_series(returns))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::{Date, Month, PeriodKind};
    use crate::Performance;

    const RETURNS: [f64; 6] = [0.01, -0.02, 0.015, 0.003, -0.01, 0.02];

    fn panel() -> Performance {
        let dates: Vec<Date> = (1..=6)
            .map(|d| Date::from_calendar_date(2025, Month::January, d).unwrap())
            .collect();
        Performance::from_returns(
            dates,
            vec![RETURNS.to_vec()],
            vec!["A".into()],
            None,
            PeriodKind::Daily,
        )
        .unwrap()
    }

    #[test]
    fn free_functions_agree_with_daily_panel() {
        let perf = panel();
        assert!((sharpe(&RETURNS, 0.02, 252.0) - perf.sharpe(0.02)[0]).abs() < 1e-12);
        assert!((sortino(&RETURNS, 0.0, 252.0) - perf.sortino(0.0)[0]).abs() < 1e-12);
        assert!((volatility(&RETURNS, 252.0) - perf.volatility(true)[0]).abs() < 1e-12);
        assert!((max_drawdown(&RETURNS) - perf.max_drawdown()[0]).abs() < 1e-12);
    }

    #[test]
    fn invalid_periods_per_year_is_nan() {
        assert!(sharpe(&RETURNS, 0.0, 0.0).is_nan());
        assert!(volatility(&RETURNS, f64::NAN).is_nan());
        assert_eq!(max_drawdown(&[]), 0.0);
    }
}
