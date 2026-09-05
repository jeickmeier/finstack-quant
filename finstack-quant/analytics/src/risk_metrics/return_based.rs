//! Return-based risk metrics: mean, volatility, Sharpe, Sortino, CAGR, and more.
//!
//! Crate-internal building blocks for [`crate::Performance`]. Functions take
//! `&[f64]` return slices. Annualization uses the caller-supplied factor
//! (typically from `PeriodKind::annualization_factor()`).

use crate::dates::{Date, DayCount, DayCountContext, HolidayCalendar};
use crate::math::stats::{mean, mean_var, variance};
use crate::math::summation::kahan_sum;

use super::tail_risk::cornish_fisher_var;

/// True when annualization is requested but `ann_factor` is not a positive finite
/// periods-per-year count (e.g. zero, negative, NaN, or infinity).
#[inline]
pub(crate) fn invalid_annualization_factor(annualize: bool, ann_factor: f64) -> bool {
    annualize && (!ann_factor.is_finite() || ann_factor <= 0.0)
}

/// Day-count convention for CAGR annualization over explicit calendar dates.
///
/// [`CagrDayCount::Act365_25`] is the default used by [`crate::Performance::cagr`].
/// [`CagrDayCount::DayCount`] wraps any core
/// [`finstack_quant_core::dates::DayCount`] (Act/365F, Act/Act, Bus/252, …).
/// `Bus252` requires a holiday calendar on the facade; missing calendar is
/// an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CagrDayCount {
    /// Actual calendar days divided by 365.25 (default).
    #[default]
    Act365_25,
    /// Any core day-count convention.
    DayCount(DayCount),
}

impl std::str::FromStr for CagrDayCount {
    type Err = crate::error::Error;

    /// Parse a CAGR day-count label.
    ///
    /// Accepts `"act365_25"`, `"act_365_25"` or `"act/365.25"` for
    /// [`CagrDayCount::Act365_25`]; every other token is parsed as a core
    /// [`DayCount`] name (for example `"act_365f"`, `"bus_252"`).
    fn from_str(label: &str) -> Result<Self, Self::Err> {
        match label {
            "act365_25" | "act_365_25" | "act/365.25" => Ok(Self::Act365_25),
            other => other
                .parse::<DayCount>()
                .map(Self::DayCount)
                .map_err(crate::error::Error::Validation),
        }
    }
}

impl std::fmt::Display for CagrDayCount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Act365_25 => f.write_str("act365_25"),
            Self::DayCount(day_count) => write!(f, "{day_count}"),
        }
    }
}

/// Compound annual growth rate from a return series over explicit dates.
///
/// Computes:
///
/// ```text
/// CAGR = (Π(1 + r_i))^(1/years) - 1
/// ```
///
/// where `years` is the year fraction between `start` and `end` under
/// `day_count`.
///
/// # Arguments
///
/// * `returns`   - Slice of simple period returns.
/// * `start`     - Start date of the holding period.
/// * `end`       - End date of the holding period.
/// * `day_count` - Convention used to convert the holding period to years.
/// * `calendar`  - Holiday calendar required when `day_count` is
///   [`CagrDayCount::DayCount`] with [`DayCount::Bus252`]. Ignored for
///   Act/365.25 and day-counts that do not need a calendar.
///
/// # Returns
///
/// Annualized growth rate as a decimal.
///
/// # Errors
///
/// Returns [`crate::error::InputError::Invalid`] when `returns` is empty or the
/// date range has a non-positive span. Propagates
/// [`crate::error::InputError::MissingCalendarForBus252`] when Bus/252 is
/// requested without a calendar.
pub(crate) fn cagr(
    returns: &[f64],
    start: Date,
    end: Date,
    day_count: CagrDayCount,
    calendar: Option<&dyn HolidayCalendar>,
) -> crate::Result<f64> {
    if returns.is_empty() {
        tracing::debug!(reason = "empty_returns", "invalid CAGR input");
        return Err(crate::error::InputError::Invalid.into());
    }

    let total = 1.0 + crate::returns::comp_total(returns);
    let years = annualized_years(start, end, day_count, calendar)?;
    if years <= 0.0 {
        tracing::debug!(
            ?start,
            ?end,
            ?day_count,
            reason = "non_positive_date_span",
            "invalid CAGR input"
        );
        return Err(crate::error::InputError::Invalid.into());
    }
    Ok(total.powf(1.0 / years) - 1.0)
}

fn annualized_years(
    start: Date,
    end: Date,
    day_count: CagrDayCount,
    calendar: Option<&dyn HolidayCalendar>,
) -> crate::Result<f64> {
    match day_count {
        CagrDayCount::Act365_25 => {
            let days = (end - start).whole_days() as f64;
            Ok(if days <= 0.0 { 0.0 } else { days / 365.25 })
        }
        CagrDayCount::DayCount(convention) => {
            let ctx = DayCountContext {
                calendar,
                ..DayCountContext::default()
            };
            convention.year_fraction(start, end, ctx)
        }
    }
}

/// Mean return, optionally annualized.
///
/// Computes the **arithmetic** mean of `returns`. When `annualize` is `true`,
/// that mean is scaled by `ann_factor` (e.g., 252 for daily data):
///
/// ```text
/// μ_ann = μ_period × ann_factor
/// ```
///
/// This is **simple** annualization of the average **per-period** return, not a
/// compounded (geometric) annual return. For growth over time that compounds
/// period returns, use [`cagr`]. Volatility in this
/// module uses the usual root-time rule (`σ_ann = σ_period × √ann_factor`); mean
/// return uses **linear** scaling instead.
///
/// # Arguments
///
/// * `returns`    - Slice of period simple returns.
/// * `annualize`  - Whether to multiply the mean by `ann_factor`.
/// * `ann_factor` - Number of periods per year (e.g., 252 daily, 12 monthly).
///
/// # Returns
///
/// Arithmetic mean return, annualized if requested. Returns `0.0` for an
/// empty slice. When `annualize` is `true`, returns [`f64::NAN`] if `ann_factor`
/// is not finite or is `<= 0`.
#[must_use]
pub(crate) fn mean_return(returns: &[f64], annualize: bool, ann_factor: f64) -> f64 {
    if invalid_annualization_factor(annualize, ann_factor) {
        return f64::NAN;
    }
    let m = mean(returns);
    if annualize {
        m * ann_factor
    } else {
        m
    }
}

/// Annualized mean and volatility from one Welford pass.
///
/// Equivalent to `(mean_return(returns, true, ann_factor),
/// volatility(returns, true, ann_factor))` but walks the slice once instead
/// of twice. Used by callers (e.g. Sharpe / M²) that need both.
///
/// # Arguments
///
/// * `returns`    - Slice of period simple returns.
/// * `ann_factor` - Number of periods per year (e.g., 252 daily, 12 monthly).
///
/// # Returns
///
/// `(annualized_mean, annualized_volatility)`. Returns `(NaN, NaN)` for the
/// same invalid annualization-factor cases as the individual functions.
#[must_use]
pub(crate) fn mean_vol_annualized(returns: &[f64], ann_factor: f64) -> (f64, f64) {
    if invalid_annualization_factor(true, ann_factor) {
        return (f64::NAN, f64::NAN);
    }
    let (m, var) = mean_var(returns);
    (m * ann_factor, var.sqrt() * ann_factor.sqrt())
}

/// Volatility (standard deviation of returns), optionally annualized.
///
/// Uses **sample** standard deviation (n-1 denominator), consistent with
/// Bloomberg, QuantLib, and the `OnlineStats::variance()` convention.
/// Annualizes by multiplying by `sqrt(ann_factor)` following the
/// square-root-of-time rule.
///
/// # Arguments
///
/// * `returns`    - Slice of period simple returns.
/// * `annualize`  - Whether to scale by `sqrt(ann_factor)`.
/// * `ann_factor` - Number of periods per year (e.g., 252 daily, 12 monthly).
///
/// # Returns
///
/// Sample standard deviation of `returns` (n-1 denominator), annualized if requested.
/// Returns `0.0` for an empty slice. When `annualize` is `true`, returns
/// [`f64::NAN`] if `ann_factor` is not finite or is `<= 0`.
#[must_use]
pub(crate) fn volatility(returns: &[f64], annualize: bool, ann_factor: f64) -> f64 {
    if invalid_annualization_factor(annualize, ann_factor) {
        return f64::NAN;
    }
    let v = variance(returns).sqrt();
    if annualize {
        v * ann_factor.sqrt()
    } else {
        v
    }
}
/// Sharpe ratio = annualized excess return / annualized volatility.
///
/// The annualized risk-free rate is geometrically decompounded to the
/// observation frequency before subtraction from the period arithmetic
/// mean, then the excess is scaled by `ann_factor`:
///
/// ```text
/// rf_period = (1 + rf_annual)^{1/N} − 1
/// excess_ann = (μ − rf_period) × N
/// Sharpe = excess_ann / σ_ann
/// ```
///
/// # Arguments
///
/// * `ann_return`     - Linearly annualized arithmetic mean (`μ × N`).
/// * `ann_vol`        - Annualized portfolio volatility.
/// * `risk_free_rate` - Annualized risk-free rate (e.g., `0.02` for 2%).
/// * `ann_factor`     - Periods per year `N` used to decompound
///   `risk_free_rate`. Pass `1.0` when `ann_return` and `risk_free_rate`
///   are already in the same (annual) units.
///
/// # Returns
///
/// The Sharpe ratio. When `ann_vol` is zero: returns `f64::INFINITY` if
/// excess return is positive, `f64::NEG_INFINITY` if negative, and `0.0`
/// if both are zero (matching [`sortino`] convention).
///
/// # References
///
/// - Sharpe (1966): see docs/REFERENCES.md#sharpe1966
#[must_use]
pub(crate) fn sharpe(ann_return: f64, ann_vol: f64, risk_free_rate: f64, ann_factor: f64) -> f64 {
    let excess = crate::returns::annualized_excess_return(ann_return, risk_free_rate, ann_factor);
    if !excess.is_finite() || !ann_vol.is_finite() {
        return f64::NAN;
    }
    if ann_vol == 0.0 {
        return if excess > 0.0 {
            f64::INFINITY
        } else if excess < 0.0 {
            f64::NEG_INFINITY
        } else {
            0.0
        };
    }
    excess / ann_vol
}

/// Downside deviation: semi-standard deviation below a minimum acceptable return.
///
/// Computes the root-mean-square of returns falling below `mar`, using
/// the full series length as the denominator (population convention),
/// consistent with Sortino & van der Meer (1991):
///
/// ```text
/// DD = sqrt( (1/n) × Σ min(r_i − MAR, 0)² )
/// ```
///
/// # Arguments
///
/// * `returns`    - Slice of period simple returns.
/// * `mar`        - Minimum acceptable return (threshold). Use `0.0` for
///   the standard Sortino definition.
/// * `annualize`  - Whether to scale by `sqrt(ann_factor)`.
/// * `ann_factor` - Number of periods per year.
///
/// # Returns
///
/// The downside deviation (non-negative). Returns `0.0` for an empty
/// slice or when no returns fall below `mar`. When `annualize` is `true`,
/// returns [`f64::NAN`] if `ann_factor` is not finite or is `<= 0`.
///
/// # References
///
/// - Sortino & van der Meer (1991): see docs/REFERENCES.md#sortinoVanDerMeer1991
#[must_use]
pub(crate) fn downside_deviation(
    returns: &[f64],
    mar: f64,
    annualize: bool,
    ann_factor: f64,
) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }
    if invalid_annualization_factor(annualize, ann_factor) {
        return f64::NAN;
    }
    if !mar.is_finite() || returns.iter().any(|r| !r.is_finite()) {
        return f64::NAN;
    }
    let downside_sq = kahan_sum(returns.iter().filter(|&&r| r < mar).map(|&r| {
        let d = r - mar;
        d * d
    }));
    let dd = (downside_sq / returns.len() as f64).sqrt();
    if annualize {
        dd * ann_factor.sqrt()
    } else {
        dd
    }
}
/// Sortino ratio: penalises only downside volatility.
///
/// Unlike the Sharpe ratio, the Sortino ratio uses the **downside deviation**
/// (semi-standard deviation of negative returns) as the risk denominator,
/// leaving upside volatility unrewarded:
///
/// ```text
/// Sortino = (annualized mean return) / (annualized downside deviation)
/// ```
///
/// Downside deviation is computed over the full return series (denominator
/// is `n`, not the number of negative observations), consistent with the
/// Sortino & van der Meer (1991) definition.
///
/// # Arguments
///
/// * `returns` - Slice of period simple returns.
/// * `annualize` - Whether to annualize both numerator and denominator.
/// * `ann_factor` - Number of periods per year.
/// * `mar` - Minimum acceptable return per period in decimal form.
///
/// # Returns
///
/// The Sortino ratio. Returns `±∞` when the mean is nonzero but there
/// are no negative returns (zero downside risk), and `0.0` when the
/// mean is zero or the downside deviation is zero. When `annualize` is
/// `true`, returns [`f64::NAN`] if `ann_factor` is not finite or is `<= 0`.
///
/// # References
///
/// - Sortino & van der Meer (1991): see docs/REFERENCES.md#sortinoVanDerMeer1991
#[must_use]
pub(crate) fn sortino(returns: &[f64], annualize: bool, ann_factor: f64, mar: f64) -> f64 {
    if returns.len() < 2 || invalid_annualization_factor(annualize, ann_factor) {
        return f64::NAN;
    }
    let excess_mean = mean(returns) - mar;
    let dd = downside_deviation(returns, mar, false, ann_factor);
    if !excess_mean.is_finite() || !dd.is_finite() {
        return f64::NAN;
    }
    if dd == 0.0 {
        return if excess_mean > 0.0 {
            f64::INFINITY
        } else if excess_mean < 0.0 {
            f64::NEG_INFINITY
        } else {
            0.0
        };
    }
    if annualize {
        (excess_mean * ann_factor) / (dd * ann_factor.sqrt())
    } else {
        excess_mean / dd
    }
}
/// Geometric mean return per period.
///
/// The compound-average return: the constant per-period return that
/// would produce the same terminal wealth as the actual series.
///
/// ```text
/// geo_mean = (Π(1 + r_i))^(1/n) − 1
/// ```
///
/// Computed in log-space with Kahan summation for numerical stability.
/// Returns [`f64::NEG_INFINITY`] if any return is `<= -1.0`, which
/// represents a full wipeout (or worse) and avoids the upward bias that
/// a positive clamp would introduce near total loss.
///
/// # Arguments
///
/// * `returns` - Slice of period simple returns.
///
/// # Returns
///
/// The geometric mean return. Returns [`f64::NAN`] for an empty slice.
#[must_use]
pub(crate) fn geometric_mean(returns: &[f64]) -> f64 {
    if returns.is_empty() {
        return f64::NAN;
    }
    let mut saw_total_wipeout = false;
    for &r in returns {
        if r < -1.0 {
            return f64::NEG_INFINITY;
        }
        if (r + 1.0).abs() < f64::EPSILON {
            saw_total_wipeout = true;
        }
    }
    if saw_total_wipeout {
        return -1.0;
    }
    let n = returns.len() as f64;
    let log_sum = kahan_sum(returns.iter().map(|&r| (1.0 + r).ln()));
    (log_sum / n).exp() - 1.0
}

/// Omega ratio: probability-weighted gain-to-loss ratio above a threshold.
///
/// ```text
/// Ω(L) = Σ max(r_i − L, 0) / Σ max(L − r_i, 0)
/// ```
///
/// Unlike the Sharpe ratio (which uses only mean and variance), the Omega
/// ratio incorporates the full return distribution.
///
/// # Arguments
///
/// * `returns`   - Slice of period simple returns.
/// * `threshold` - Return threshold (typically `0.0`).
///
/// # Returns
///
/// The Omega ratio. Returns `f64::INFINITY` if gains exist but no losses,
/// `1.0` if all returns equal the threshold (neutral outcome per
/// Keating-Shadwick), and [`f64::NAN`] for an empty slice.
///
/// # References
///
/// - Keating & Shadwick (2002): see docs/REFERENCES.md#keatingShadwick2002
#[must_use]
pub(crate) fn omega_ratio(returns: &[f64], threshold: f64) -> f64 {
    if returns.is_empty() {
        return f64::NAN;
    }
    let mut gains = 0.0_f64;
    let mut losses = 0.0_f64;
    for &r in returns {
        if r > threshold {
            gains += r - threshold;
        } else {
            losses += threshold - r;
        }
    }
    if losses == 0.0 {
        return if gains > 0.0 { f64::INFINITY } else { 1.0 };
    }
    gains / losses
}

/// Gain-to-pain ratio: total return divided by total losses.
///
/// ```text
/// GtP = Σ r_i / Σ |r_i| for r_i < 0
/// ```
///
/// Popular among CTA and systematic macro managers as a simple
/// measure of return efficiency relative to the pain of drawdowns.
///
/// # Arguments
///
/// * `returns` - Slice of period simple returns.
///
/// # Returns
///
/// The gain-to-pain ratio. Returns `f64::INFINITY` when total return is
/// positive but there are no losses, and [`f64::NAN`] for an empty slice.
///
/// # References
///
/// - Schwager (2012): see docs/REFERENCES.md#schwager2012
#[must_use]
pub(crate) fn gain_to_pain(returns: &[f64]) -> f64 {
    if returns.is_empty() {
        return f64::NAN;
    }
    let total: f64 = kahan_sum(returns.iter().copied());
    let abs_losses: f64 = kahan_sum(returns.iter().filter(|&&r| r < 0.0).map(|&r| r.abs()));
    if abs_losses == 0.0 {
        return if total > 0.0 { f64::INFINITY } else { 0.0 };
    }
    total / abs_losses
}

/// Modified Sharpe ratio: excess return divided by corresponding-horizon
/// Cornish-Fisher VaR.
///
/// Replaces the standard deviation in the Sharpe denominator with the
/// Cornish-Fisher adjusted VaR, accounting for skewness and kurtosis. Both
/// numerator and denominator are measured at the annual horizon implied by
/// `ann_factor`. Excess return uses the same geometric rf decompounding as
/// [`sharpe`]:
///
/// ```text
/// Modified Sharpe = ((μ − rf_period) × N) / |CF-VaR_N|
/// ```
///
/// Passing `Some(ann_factor)` to [`cornish_fisher_var`] is intentional:
/// it scales the mean and volatility and applies the i.i.d. horizon decay to
/// skewness and excess kurtosis. A one-period VaR denominator would be
/// inconsistent with the annualized numerator.
///
/// # Arguments
///
/// * `returns`        - Slice of period simple returns.
/// * `risk_free_rate` - Annualized risk-free rate in decimal form.
/// * `confidence`     - VaR confidence level (e.g., `0.95`).
/// * `ann_factor`     - Number of periods per year used to decompound
///   `risk_free_rate` and to put both the excess-return numerator and
///   Cornish-Fisher VaR denominator on the annual horizon.
///
/// # Returns
///
/// The Modified Sharpe ratio. Returns `0.0` for empty slices and
/// [`f64::NAN`] when the Cornish-Fisher VaR is unexpectedly non-negative.
///
/// # References
///
/// - Gregoriou & Gueyie (2003): see docs/REFERENCES.md#gregoriou2003
#[must_use]
pub(crate) fn modified_sharpe(
    returns: &[f64],
    risk_free_rate: f64,
    confidence: f64,
    ann_factor: f64,
) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }
    let excess_return = crate::returns::annualized_excess_return(
        mean_return(returns, true, ann_factor),
        risk_free_rate,
        ann_factor,
    );
    let cf_var = cornish_fisher_var(returns, confidence, Some(ann_factor));
    if cf_var >= 0.0 {
        return f64::NAN;
    }
    excess_return / cf_var.abs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::Month;
    use crate::math::stats::{mean, variance};

    fn jan1(year: i32) -> crate::dates::Date {
        crate::dates::Date::from_calendar_date(year, Month::January, 1).expect("valid date")
    }

    #[test]
    fn cagr_basic() {
        let r = [0.10];
        let c =
            cagr(&r, jan1(2024), jan1(2025), CagrDayCount::default(), None).expect("valid CAGR");
        assert!((c - 0.10).abs() < 0.01);
    }

    #[test]
    fn cagr_with_act_365_fixed() {
        let r = [0.10];
        let c = cagr(
            &r,
            jan1(2024),
            jan1(2025),
            CagrDayCount::DayCount(DayCount::Act365F),
            None,
        )
        .expect("valid CAGR");
        assert!((c - 0.09971358593414137).abs() < 1.0e-12);
    }

    #[test]
    fn cagr_default_convention_is_act_365_25() {
        let r = [0.10];
        let c_default =
            cagr(&r, jan1(2024), jan1(2025), CagrDayCount::default(), None).expect("valid CAGR");
        let c_fixed = cagr(
            &r,
            jan1(2024),
            jan1(2025),
            CagrDayCount::DayCount(DayCount::Act365F),
            None,
        )
        .expect("valid CAGR");
        assert!(c_default > c_fixed);
        assert!((c_default - 0.09978518245839707).abs() < 1.0e-12);
    }

    #[test]
    fn cagr_bus252_without_calendar_is_err() {
        let r = [0.10];
        let err = cagr(
            &r,
            jan1(2024),
            jan1(2025),
            CagrDayCount::DayCount(DayCount::Bus252),
            None,
        )
        .expect_err("Bus252 requires a calendar");
        let msg = err.to_string();
        assert!(
            msg.to_lowercase().contains("calendar") || msg.to_lowercase().contains("bus"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn mean_return_volatility_nan_when_annualized_with_invalid_factor() {
        let r = [0.01_f64, 0.02];
        assert!(mean_return(&r, true, 0.0).is_nan());
        assert!(mean_return(&r, true, -1.0).is_nan());
        assert!(mean_return(&r, true, f64::NAN).is_nan());
        assert!(volatility(&r, true, 0.0).is_nan());
        assert!(volatility(&r, true, f64::INFINITY).is_nan());
    }

    #[test]
    fn downside_deviation_and_sortino_nan_when_annualized_with_invalid_factor() {
        let r = [0.01_f64, -0.02, 0.03];
        assert!(downside_deviation(&r, 0.0, true, 0.0).is_nan());
        assert!(sortino(&r, true, 0.0, 0.0).is_nan());
    }

    #[test]
    fn factor_cagr_rejects_bad_ann_factor() {
        assert!(cagr_from_factor(&[0.01, 0.02], 0.0).is_err());
        assert!(cagr_from_factor(&[0.01, 0.02], -1.0).is_err());
        assert!(cagr_from_factor(&[0.01, 0.02], f64::NAN).is_err());
    }

    #[test]
    fn factor_cagr_accepts_single_period() {
        assert!((cagr_from_factor(&[0.10], 1.0).expect("valid CAGR") - 0.10).abs() < 1.0e-12);
    }

    #[test]
    fn date_cagr_rejects_non_positive_spans() {
        let returns = [0.10];

        assert!(cagr(
            &returns,
            jan1(2024),
            jan1(2024),
            CagrDayCount::default(),
            None,
        )
        .is_err());
        assert!(cagr(
            &returns,
            jan1(2025),
            jan1(2024),
            CagrDayCount::default(),
            None,
        )
        .is_err());
    }

    #[test]
    fn mean_return_annualized_scales_linearly_not_compounded() {
        let r = [0.01, 0.02, 0.03];
        let m_ann = mean_return(&r, true, 252.0);
        let mean_p = mean(&r);
        assert!((m_ann - mean_p * 252.0).abs() < 1e-10);
        let cagr_ann = cagr_from_factor(&r, 252.0).expect("valid CAGR");
        assert!(
            cagr_ann.is_finite() && (m_ann - cagr_ann).abs() > 1e-6,
            "arithmetic annualized mean should differ from compounded cagr"
        );
    }

    #[test]
    fn sharpe_basic() {
        assert!((sharpe(0.10, 0.15, 0.0, 1.0) - 0.6666).abs() < 0.01);
        assert_eq!(sharpe(0.10, 0.0, 0.0, 1.0), f64::INFINITY);
        assert_eq!(sharpe(-0.05, 0.0, 0.0, 1.0), f64::NEG_INFINITY);
        assert_eq!(sharpe(0.02, 0.0, 0.02, 1.0), 0.0);
    }

    #[test]
    fn sharpe_with_risk_free_rate() {
        assert!((sharpe(0.10, 0.15, 0.02, 1.0) - 0.5333).abs() < 0.01);
    }

    #[test]
    fn sharpe_daily_excess_uses_geometric_rf() {
        let mu = 0.0004_f64;
        let ann_factor = 252.0;
        let rf_annual = 0.02;
        let ann_return = mu * ann_factor;
        let ann_vol = 0.15;
        let rf_period = 1.02_f64.powf(1.0 / ann_factor) - 1.0;
        let expected_excess = (mu - rf_period) * ann_factor;
        let linear_excess = ann_return - rf_annual;
        assert!(
            (expected_excess - linear_excess).abs() > 1e-6,
            "geometric and linear rf subtraction must differ at daily frequency"
        );
        let s = sharpe(ann_return, ann_vol, rf_annual, ann_factor);
        assert!((s * ann_vol - expected_excess).abs() < 1e-12);
        assert!((s * ann_vol - linear_excess).abs() > 1e-6);
    }

    #[test]
    fn sortino_positive_returns() {
        let r = [0.01, 0.02, 0.03, -0.005, 0.01];
        let s = sortino(&r, false, 252.0, 0.0);
        assert!(s > 0.0);
    }

    #[test]
    fn downside_deviation_hand_calc() {
        let r = [0.01, -0.02, 0.03, -0.01, 0.005];
        let dd = downside_deviation(&r, 0.0, false, 252.0);
        assert!((dd - 0.01).abs() < 1e-14);
    }

    #[test]
    fn downside_deviation_annualized() {
        let r = [0.01, -0.02, 0.03, -0.01, 0.005];
        let dd_raw = downside_deviation(&r, 0.0, false, 252.0);
        let dd_ann = downside_deviation(&r, 0.0, true, 252.0);
        assert!((dd_ann - dd_raw * 252.0_f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn downside_deviation_all_positive() {
        let dd = downside_deviation(&[0.01, 0.02, 0.03], 0.0, false, 252.0);
        assert_eq!(dd, 0.0);
    }

    #[test]
    fn downside_deviation_empty() {
        assert_eq!(downside_deviation(&[], 0.0, false, 252.0), 0.0);
    }

    #[test]
    fn downside_deviation_with_mar() {
        let r = [0.01, 0.02, 0.03, 0.005];
        let dd = downside_deviation(&r, 0.02, false, 252.0);
        let expected = (0.000325_f64 / 4.0).sqrt();
        assert!((dd - expected).abs() < 1e-14);
    }

    #[test]
    fn sortino_consistent_with_downside_deviation() {
        let r = [0.01, 0.02, 0.03, -0.005, 0.01];
        let m = mean(&r);
        let dd = downside_deviation(&r, 0.0, false, 252.0);
        let s = sortino(&r, false, 252.0, 0.0);
        assert!((s - m / dd).abs() < 1e-12);
    }

    #[test]
    fn sortino_respects_mar_in_numerator_and_denominator() {
        let r = [0.01, 0.02, 0.03, 0.04];
        let mar = 0.02;
        let expected = (mean(&r) - mar) / downside_deviation(&r, mar, false, 252.0);
        let actual = sortino(&r, false, 252.0, mar);
        assert!((actual - expected).abs() < 1e-12);
    }

    #[test]
    fn geometric_mean_constant() {
        let gm = geometric_mean(&[0.05, 0.05, 0.05]);
        assert!((gm - 0.05).abs() < 1e-12);
    }

    #[test]
    fn geometric_mean_volatility_drag_exact() {
        let gm = geometric_mean(&[0.10, -0.10]);
        let expected = 0.99_f64.sqrt() - 1.0;
        assert!((gm - expected).abs() < 1e-12);
    }

    #[test]
    fn geometric_mean_empty() {
        assert!(geometric_mean(&[]).is_nan());
    }

    #[test]
    fn geometric_mean_total_wipeout_returns_minus_one() {
        assert_eq!(geometric_mean(&[0.10, -1.0]), -1.0);
        assert_eq!(geometric_mean(&[-1.5]), f64::NEG_INFINITY);
    }

    #[test]
    fn geometric_mean_less_than_arithmetic() {
        let r = [0.05, 0.10, -0.03, 0.08];
        let gm = geometric_mean(&r);
        let am = mean(&r);
        assert!(gm < am);
    }

    #[test]
    fn omega_ratio_hand_calc() {
        let r = [0.05, -0.02, 0.03, -0.01, 0.04];
        let omega = omega_ratio(&r, 0.0);
        assert!((omega - 4.0).abs() < 1e-12);
    }

    #[test]
    fn omega_ratio_no_losses() {
        assert_eq!(omega_ratio(&[0.01, 0.02, 0.03], 0.0), f64::INFINITY);
    }

    #[test]
    fn omega_ratio_empty() {
        assert!(omega_ratio(&[], 0.0).is_nan());
    }

    #[test]
    fn gain_to_pain_hand_calc() {
        let r = [0.05, -0.02, 0.03, -0.01, 0.04];
        let gtp = gain_to_pain(&r);
        assert!((gtp - 3.0).abs() < 1e-12);
    }

    #[test]
    fn gain_to_pain_no_losses() {
        assert_eq!(gain_to_pain(&[0.01, 0.02]), f64::INFINITY);
    }

    #[test]
    fn gain_to_pain_empty() {
        assert!(gain_to_pain(&[]).is_nan());
    }

    #[test]
    fn modified_sharpe_is_finite_when_cf_var_is_a_loss() {
        let r = [-0.06, -0.03, -0.02, 0.01, 0.02, 0.025, 0.03, 0.04];
        let ms = modified_sharpe(&r, 0.02, 0.95, 252.0);
        assert!(ms.is_finite());
    }

    #[test]
    fn modified_sharpe_uses_annual_excess_over_annual_cf_var() {
        let returns = [-0.06, -0.03, -0.02, 0.01, 0.02, 0.025, 0.03, 0.04];
        let risk_free_rate = 0.02;
        let confidence = 0.95;
        let ann_factor = 12.0;

        let period_mean = mean(&returns);
        let period_vol = variance(&returns).sqrt();
        let rf_period = (1.0_f64 + risk_free_rate).powf(1.0 / ann_factor) - 1.0;
        let annual_excess = (period_mean - rf_period) * ann_factor;

        let annual_skew = crate::risk_metrics::skewness(&returns) / ann_factor.sqrt();
        let annual_kurt = crate::risk_metrics::kurtosis(&returns) / ann_factor;
        let z = crate::math::special_functions::standard_normal_inv_cdf(1.0 - confidence);
        let z2 = z * z;
        let z3 = z2 * z;
        let annual_z_cf = z + (z2 - 1.0) * annual_skew / 6.0 + (z3 - 3.0 * z) * annual_kurt / 24.0
            - (2.0 * z3 - 5.0 * z) * annual_skew * annual_skew / 36.0;
        let annual_cf_var = period_mean * ann_factor + annual_z_cf * period_vol * ann_factor.sqrt();
        assert!(annual_cf_var < 0.0);

        let expected = annual_excess / annual_cf_var.abs();
        let actual = modified_sharpe(&returns, risk_free_rate, confidence, ann_factor);
        assert!(
            (actual - expected).abs() < 1.0e-12,
            "{actual} vs {expected}"
        );
    }

    #[test]
    fn modified_sharpe_differs_from_one_period_var_denominator() {
        let returns = [-0.06, -0.03, -0.02, 0.01, 0.02, 0.025, 0.03, 0.04];
        let risk_free_rate = 0.02;
        let confidence = 0.95;
        let ann_factor = 12.0;

        let period_mean = mean(&returns);
        let rf_period = (1.0_f64 + risk_free_rate).powf(1.0 / ann_factor) - 1.0;
        let annual_excess = (period_mean - rf_period) * ann_factor;
        let one_period_cf_var = cornish_fisher_var(&returns, confidence, None);
        assert!(one_period_cf_var < 0.0);

        let inconsistent = annual_excess / one_period_cf_var.abs();
        let actual = modified_sharpe(&returns, risk_free_rate, confidence, ann_factor);
        assert!(
            (actual - inconsistent).abs() > 1.0e-6,
            "annual-horizon and one-period denominators must not be interchangeable"
        );
    }

    #[test]
    fn modified_sharpe_empty() {
        assert_eq!(modified_sharpe(&[], 0.02, 0.95, 252.0), 0.0);
    }

    #[test]
    fn modified_sharpe_positive_cf_var_returns_nan() {
        let r = [0.03; 12];
        let ms = modified_sharpe(&r, 0.0, 0.95, 12.0);
        assert!(ms.is_nan());
    }

    #[test]
    fn cagr_empty_is_err() {
        assert!(cagr(&[], jan1(2024), jan1(2025), CagrDayCount::default(), None,).is_err());
        assert!(cagr_from_factor(&[], 252.0).is_err());
    }

    #[test]
    fn parametric_var_scales_mean_and_vol_by_horizon() {
        let returns = [0.01, -0.02, 0.03, -0.01, 0.02, -0.005];
        let ann_factor = 12.0;
        let m = mean(&returns);
        let vol = variance(&returns).sqrt();
        let z = crate::math::special_functions::standard_normal_inv_cdf(0.05);
        let expected = m * ann_factor + z * vol * ann_factor.sqrt();
        let actual = crate::risk_metrics::parametric_var(&returns, 0.95, Some(ann_factor));
        assert!((actual - expected).abs() < 1e-14, "{actual} vs {expected}");
    }

    /// Test-only CAGR from a fixed annualisation factor (no calendar dates).
    fn cagr_from_factor(returns: &[f64], ann_factor: f64) -> crate::Result<f64> {
        if returns.is_empty() {
            tracing::debug!(reason = "empty_returns", "invalid CAGR input");
            return Err(crate::error::InputError::Invalid.into());
        }
        if !ann_factor.is_finite() || ann_factor <= 0.0 {
            tracing::debug!(
                ann_factor,
                reason = "invalid_annualization_factor",
                "invalid CAGR input"
            );
            return Err(crate::error::InputError::Invalid.into());
        }
        let total = 1.0 + crate::returns::comp_total(returns);
        let years = returns.len() as f64 / ann_factor;
        if years > 0.0 {
            Ok(total.powf(1.0 / years) - 1.0)
        } else {
            Err(crate::error::InputError::Invalid.into())
        }
    }

    /// Numeric fixture pinning freestanding building-block outputs.
    ///
    /// Lives in `src/` (rather than `tests/`) so it can reach `pub(crate)`
    /// building-block functions directly. Public-API consumers should go
    /// through [`crate::performance::Performance`].
    mod fixture_tests {
        use super::{cagr_from_factor, sharpe, sortino};
        use crate::benchmark::{multi_factor_greeks, rolling_greeks, ReturnKind};
        use crate::dates::Date;
        use crate::risk_metrics::{expected_shortfall, value_at_risk};
        use serde::Deserialize;

        const API_INVARIANTS_FIXTURE: &str =
            include_str!("../../tests/data/api_invariants_data.json");

        #[derive(Deserialize)]
        struct Fixture {
            returns: Vec<f64>,
            benchmark: Vec<f64>,
            factors: Vec<Vec<f64>>,
            dates: Vec<Date>,
            expected: Expected,
        }

        #[derive(Deserialize)]
        struct Expected {
            cagr_factor: f64,
            sharpe: f64,
            sortino: f64,
            value_at_risk: f64,
            expected_shortfall: f64,
            rolling_greeks: ExpectedRollingGreeks,
            multi_factor_greeks: ExpectedMultiFactorGreeks,
        }

        #[derive(Deserialize)]
        struct ExpectedRollingGreeks {
            alphas: Vec<f64>,
            betas: Vec<f64>,
        }

        #[derive(Deserialize)]
        struct ExpectedMultiFactorGreeks {
            alpha: f64,
            betas: Vec<f64>,
            r_squared: f64,
            adjusted_r_squared: f64,
            residual_vol: f64,
        }

        fn fixture() -> Fixture {
            serde_json::from_str(API_INVARIANTS_FIXTURE)
                .expect("API invariants fixture should parse")
        }

        fn assert_close(actual: f64, expected: f64) {
            assert!(
                (actual - expected).abs() < 1.0e-12,
                "actual={actual}, expected={expected}"
            );
        }

        fn assert_vec_close(actual: &[f64], expected: &[f64]) {
            assert_eq!(actual.len(), expected.len());
            for (&actual, &expected) in actual.iter().zip(expected.iter()) {
                assert_close(actual, expected);
            }
        }

        #[test]
        fn rust_core_matches_api_invariants_fixture() {
            let fixture = fixture();
            let expected = &fixture.expected;

            assert_close(
                cagr_from_factor(&fixture.returns, 252.0).expect("valid fixture CAGR"),
                expected.cagr_factor,
            );
            assert_close(sharpe(0.12, 0.18, 0.02, 1.0), expected.sharpe);
            assert_close(
                sortino(&fixture.returns, true, 252.0, 0.0),
                expected.sortino,
            );
            assert_close(
                value_at_risk(&fixture.returns, 0.95),
                expected.value_at_risk,
            );
            assert_close(
                expected_shortfall(&fixture.returns, 0.95),
                expected.expected_shortfall,
            );

            let rolling = rolling_greeks(
                &fixture.returns,
                &fixture.benchmark,
                &fixture.dates,
                5,
                252.0,
                0.0,
            );
            assert_vec_close(&rolling.alphas, &expected.rolling_greeks.alphas);
            assert_vec_close(&rolling.betas, &expected.rolling_greeks.betas);

            let factor_refs: Vec<&[f64]> = fixture.factors.iter().map(Vec::as_slice).collect();
            let multi =
                multi_factor_greeks(&fixture.returns, &factor_refs, 252.0, ReturnKind::Excess)
                    .expect("valid fixture multi-factor regression");
            assert_close(multi.alpha, expected.multi_factor_greeks.alpha);
            assert_vec_close(&multi.betas, &expected.multi_factor_greeks.betas);
            assert_close(multi.r_squared, expected.multi_factor_greeks.r_squared);
            assert_close(
                multi.adjusted_r_squared,
                expected.multi_factor_greeks.adjusted_r_squared,
            );
            assert_close(
                multi.residual_vol,
                expected.multi_factor_greeks.residual_vol,
            );
        }
    }
}
