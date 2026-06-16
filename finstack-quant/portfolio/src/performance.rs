//! Portfolio performance measurement — TWRR, MWRR, and GIPS-style linking.
//!
//! The raw PV delta on `ReplaySummary.total_pnl` ignores external
//! cashflows (contributions, withdrawals, fees, dividends) and so
//! conflates manager alpha with client capital moves, which is not a
//! legitimate return under any industry standard.
//!
//! This module adds:
//!
//! * [`TwrrPeriod`] + [`twrr_modified_dietz`] — per-period return using
//!   the Modified-Dietz approximation
//!   `r = (EMV − BMV − Σ CF_i) / (BMV + Σ w_i · CF_i)`
//!   where `w_i = (T − t_i) / T` is the day-weighted cashflow fraction.
//!   This is the CFA / GIPS-standard sub-period return (GIPS 2020
//!   Standard 2.A.6) when a true time-weighted return is not available
//!   due to mid-period flows.
//!
//! * [`twrr_linked`] — geometric linking across sub-periods
//!   `R_total = Π (1 + r_i) − 1`.
//!   Produces the annualised return over the full horizon.
//!
//! * [`mwr_xirr`] — money-weighted return via the existing
//!   [`finstack_quant_core::cashflow::xirr`] solver. Returns the annualised
//!   dollar-weighted IRR of the cashflow stream, Newton-Raphson with
//!   fallback to bracketed bisection.
//!
//! # Conventions
//!
//! All monetary quantities are `f64` in this module to match the rest
//! of the `portfolio` crate's signatures (see workspace INVARIANTS.md
//! — Decimal migration is tracked as a separate work item). Times are
//! year-fractions produced by the caller's day-count convention; this
//! module is day-count agnostic.
//!
//! # References
//!
//! - CFA Institute, *GIPS Standards* (2020 edition), §2.A — Calculation
//!   Methodology, Modified-Dietz, Time-Weighted Returns.
//! - Feibel, B. (2003). *Investment Performance Measurement.* Wiley.
//!   Ch. 2–4 (Dietz formulas), Ch. 9 (money-weighted return).
//! - Shaw, W. T. (1990). "On the solution of f(x) = 0 by Newton-Raphson
//!   when f has no simple root." (Context for Newton+Brent fallback in
//!   XIRR.)

use finstack_quant_core::dates::Date;
use serde::{Deserialize, Serialize};

/// A single sub-period of a portfolio, with the information needed to
/// compute a Modified-Dietz return.
///
/// * `beginning_market_value` — portfolio PV at the period's start.
/// * `ending_market_value` — portfolio PV at the period's end.
/// * `cashflows` — external flows during the period. Each flow's
///   `fraction_of_period_remaining` is the time from the flow to the
///   period end divided by the total period length, i.e., the Dietz
///   weight `w_i = (T − t_i) / T ∈ [0, 1]`. A flow *at the start* of
///   the period has `w_i = 1`; a flow *at the end* has `w_i = 0`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwrrPeriod {
    /// PV at period start.
    pub beginning_market_value: f64,
    /// PV at period end.
    pub ending_market_value: f64,
    /// External cashflows during the period.
    ///
    /// Sign convention: **positive** = contribution into the portfolio
    /// (capital added by the client); **negative** = withdrawal. This
    /// matches `ReplaySummary.total_pnl` conventions already in use in
    /// `portfolio::replay`.
    pub cashflows: Vec<DietzFlow>,
}

/// A single external cashflow within a TWRR sub-period.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DietzFlow {
    /// Signed flow amount (positive = contribution; negative = withdrawal).
    pub amount: f64,
    /// Day-weighted fraction of the period from the flow to period end,
    /// `w = (T − t_flow) / T ∈ [0, 1]`. Flow at period start has `w = 1`;
    /// flow at period end has `w = 0`.
    pub fraction_of_period_remaining: f64,
}

/// Modified-Dietz per-period return.
///
/// ```text
/// r = (EMV − BMV − Σ CF_i) / (BMV + Σ w_i · CF_i)
/// ```
///
/// Returns `None` when the denominator (BMV adjusted for weighted
/// cashflows) is non-positive — a typical sign of an error-case
/// portfolio (zero NAV after redemptions, or mis-specified flows)
/// where the return is not meaningfully defined.
#[must_use]
pub fn twrr_modified_dietz(period: &TwrrPeriod) -> Option<f64> {
    if period.cashflows.iter().any(|flow| {
        !flow.fraction_of_period_remaining.is_finite()
            || !(0.0..=1.0).contains(&flow.fraction_of_period_remaining)
    }) {
        return None;
    }

    let numerator = period.ending_market_value
        - period.beginning_market_value
        - period.cashflows.iter().map(|f| f.amount).sum::<f64>();

    let denominator = period.beginning_market_value
        + period
            .cashflows
            .iter()
            .map(|f| f.fraction_of_period_remaining * f.amount)
            .sum::<f64>();

    if !denominator.is_finite() || denominator <= 0.0 {
        return None;
    }
    let r = numerator / denominator;
    r.is_finite().then_some(r)
}

/// Result of geometrically linking sub-period returns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinkedReturn {
    /// Cumulative return over the full horizon: `Π(1 + r_i) − 1`.
    pub cumulative: f64,
    /// Annualised return assuming the supplied `horizon_years` covers
    /// the full sequence of periods.
    pub annualised: f64,
    /// Number of sub-periods linked.
    pub num_periods: usize,
}

/// A dated cashflow amount for money-weighted return calculations.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DatedCashflow {
    /// Cashflow date.
    pub date: Date,
    /// Signed amount. Contributions from the investor are negative; terminal
    /// value or distributions back to the investor are positive.
    pub amount: f64,
}

/// Geometrically link sub-period returns. GIPS 2020 §2.A.6.b.i.
///
/// Returns `None` if any sub-period return is non-finite.
///
/// # `horizon_years` convention
///
/// `horizon_years` is the elapsed time covered by the full sequence of
/// sub-periods, expressed as a **fraction of a 365-day calendar year**
/// (i.e. an annualization base of 365 calendar days). The function does
/// not assume a trading-day calendar and does *not* know about 252 / 360 /
/// 365.25 conventions — callers that report on a trading-day basis must
/// convert (e.g. `trading_days / 252` for ACT/252-quoted strategies)
/// before passing the value here.
///
/// Recommended computation at the call site:
/// `(end_date - start_date).whole_days() as f64 / 365.0`. This matches
/// GIPS guidance for periods longer than one year and keeps results
/// comparable across calendar regimes. Pass `0.0` (or any non-positive
/// value) to skip annualization and return only the cumulative figure. Per GIPS
/// 2020, horizons shorter than one year are also reported as cumulative rather
/// than annualized. In those cases the `annualised` field mirrors `cumulative`.
#[must_use]
pub fn twrr_linked(periods: &[f64], horizon_years: f64) -> Option<LinkedReturn> {
    if periods.iter().any(|r| !r.is_finite()) {
        return None;
    }
    let growth: f64 = periods.iter().map(|r| 1.0 + r).product();
    if !growth.is_finite() || growth <= 0.0 {
        return None;
    }
    let cumulative = growth - 1.0;
    let annualised = if horizon_years >= 1.0 {
        growth.powf(1.0 / horizon_years) - 1.0
    } else {
        cumulative
    };
    Some(LinkedReturn {
        cumulative,
        annualised,
        num_periods: periods.len(),
    })
}

/// Money-weighted return via XIRR. Thin convenience wrapper over
/// [`finstack_quant_core::cashflow::xirr`] for symmetry with [`twrr_linked`].
///
/// The sign convention expected by `xirr` is that contributions
/// (capital in) are **negative** and withdrawals / terminal value
/// are **positive** — opposite to [`DietzFlow::amount`] above — because
/// XIRR solves the PV-zero condition from the investor's perspective.
/// Callers coming from Dietz-flow input should flip signs at the
/// boundary.
pub fn mwr_xirr(cashflows: &[(Date, f64)]) -> finstack_quant_core::Result<f64> {
    finstack_quant_core::cashflow::xirr(cashflows, None)
}

/// Money-weighted return via XIRR from serde-friendly dated cashflow objects.
pub fn mwr_xirr_from_cashflows(cashflows: &[DatedCashflow]) -> finstack_quant_core::Result<f64> {
    let flows = cashflows
        .iter()
        .map(|cashflow| (cashflow.date, cashflow.amount))
        .collect::<Vec<_>>();
    mwr_xirr(&flows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    /// Canonical single-period Modified-Dietz from the CFA Institute
    /// GIPS Handbook worked example:
    ///
    /// * BMV = 10 000 000
    /// * EMV = 10 500 000
    /// * Single contribution of 1 000 000 at 40% of the way through
    ///   the period (so w = 0.60).
    /// * Modified-Dietz:
    ///   `(10_500_000 − 10_000_000 − 1_000_000) / (10_000_000 + 0.60 · 1_000_000)`
    ///   `= −500_000 / 10_600_000`
    ///   `≈ −0.0471698113`.
    #[test]
    fn modified_dietz_matches_canonical_gips_example() {
        let period = TwrrPeriod {
            beginning_market_value: 10_000_000.0,
            ending_market_value: 10_500_000.0,
            cashflows: vec![DietzFlow {
                amount: 1_000_000.0,
                fraction_of_period_remaining: 0.60,
            }],
        };
        let r = twrr_modified_dietz(&period).expect("finite return");
        let expected = -500_000.0 / 10_600_000.0;
        assert!(
            (r - expected).abs() < 1e-12,
            "Modified-Dietz deviates from GIPS reference: r = {r}, expected = {expected}"
        );
    }

    /// Zero-flow sanity: Modified-Dietz reduces to the simple return.
    #[test]
    fn modified_dietz_no_flows_equals_simple_return() {
        let period = TwrrPeriod {
            beginning_market_value: 100.0,
            ending_market_value: 110.0,
            cashflows: vec![],
        };
        let r = twrr_modified_dietz(&period).unwrap();
        assert!((r - 0.10).abs() < 1e-15);
    }

    #[test]
    fn minor6_modified_dietz_rejects_out_of_range_flow_weights() {
        let period = TwrrPeriod {
            beginning_market_value: 100.0,
            ending_market_value: 110.0,
            cashflows: vec![DietzFlow {
                amount: 10.0,
                fraction_of_period_remaining: 1.01,
            }],
        };

        assert!(
            twrr_modified_dietz(&period).is_none(),
            "minor 6: Dietz flow weights outside [0, 1] must not be clamped"
        );
    }

    /// Geometric linking of two sub-period returns.
    ///
    /// r_1 = +5%, r_2 = +3% over 1 year total:
    ///   cumulative = 1.05 · 1.03 − 1 = 0.0815
    ///   annualised = 0.0815 (already annual)
    #[test]
    fn linked_return_two_subperiods() {
        let linked = twrr_linked(&[0.05, 0.03], 1.0).expect("finite");
        assert!((linked.cumulative - 0.0815).abs() < 1e-12);
        assert!((linked.annualised - 0.0815).abs() < 1e-12);
        assert_eq!(linked.num_periods, 2);
    }

    /// Annualisation over a 2-year horizon with 10% cumulative return.
    #[test]
    fn linked_return_annualises_over_multi_year_horizon() {
        let linked = twrr_linked(&[0.10], 2.0).expect("finite");
        // annualised = (1.10)^(1/2) − 1 ≈ 4.88088%
        assert!((linked.annualised - 0.04880884817015154).abs() < 1e-12);
    }

    #[test]
    fn minor7_linked_return_does_not_annualise_sub_one_year_horizon() {
        let linked = twrr_linked(&[0.10], 0.5).expect("finite");

        assert_eq!(
            linked.annualised, linked.cumulative,
            "minor 7: sub-one-year horizons should report cumulative return"
        );
    }

    /// Non-finite period → None.
    #[test]
    fn linked_return_rejects_non_finite_period() {
        assert!(twrr_linked(&[0.05, f64::NAN], 1.0).is_none());
    }

    /// Zero denominator (full-redemption mid-period) → None.
    #[test]
    fn modified_dietz_returns_none_on_zero_denominator() {
        let period = TwrrPeriod {
            beginning_market_value: 0.0,
            ending_market_value: 0.0,
            cashflows: vec![],
        };
        assert!(twrr_modified_dietz(&period).is_none());
    }

    /// MWR convenience wrapper round-trips a known XIRR case: invest
    /// 100 at t=0 and redeem 110 at t=1y → IRR = 10%.
    #[test]
    fn mwr_xirr_round_trip_single_year() {
        let flows = [
            (date!(2025 - 01 - 01), -100.0),
            (date!(2026 - 01 - 01), 110.0),
        ];
        let irr = mwr_xirr(&flows).expect("solver converges");
        assert!((irr - 0.10).abs() < 1e-6);
    }
}
