//! Inflation01 (inflation rate sensitivity) metric for `InflationSwap`.
//!
//! # Finite-Difference Method
//!
//! Inflation01 is computed by **central finite differences** on the inflation
//! curve — the same approach the YoY path (`YoYInflation01Calculator`) uses,
//! so the zero-coupon and YoY metrics are mutually consistent and both agree
//! with a bumped-curve DV01:
//!
//! ```text
//! Inflation01 = (PV(+1bp) − PV(−1bp)) / 2
//! ```
//!
//! Dividing by 2 (not by 2 × bump_size_decimal) produces the PV change for a
//! **1bp move**, consistent with the DV01/CS01 convention throughout the
//! workspace (see `sensitivity_central_diff` where the divisor is 2·1 = 2 for
//! a ±1bp bump expressed in bp-units).
//!
//! # Why not the closed form
//!
//! The previous analytical approximation
//!
//! ```text
//! Inflation01 ≈ N · I(T)/I(0) · DF(T) · T · 1bp
//! ```
//!
//! had a **lag mismatch**: the maturity time `T` in the `dPV/dπ` factor was
//! computed to the *lagged* maturity (`maturity − indexation lag`, ACT/365F)
//! while the discount factor `DF(T)` used the *unlagged* maturity on the
//! discount curve's own day count. The two `T`s referred to different dates,
//! so the analytic sensitivity disagreed with the bumped-curve DV01. It also
//! silently assumed continuous compounding `exp(π·T)` whereas inflation curves
//! may compound discretely. Finite differences re-use the instrument's actual
//! `value()` (lag, day counts, compounding all consistent) and so carry no
//! such mismatch.
//!
//! # Sign Convention
//!
//! - **PayFixed**: positive Inflation01 (benefits from higher inflation).
//! - **ReceiveFixed**: negative Inflation01 (loses from higher inflation).
//!
//! The bumped-curve `value()` already carries the leg signs, so the finite
//! difference reproduces the correct sign without an explicit branch.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::inflation_swap::InflationSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_core::Result;

/// Standard inflation curve bump: 1bp (0.0001 in decimal).
pub(crate) const INFLATION_BUMP_BP: f64 = 0.0001;

/// Calculates Inflation01 (1bp inflation rate sensitivity) for zero-coupon
/// inflation swaps via central finite differences on the inflation curve.
pub(crate) struct Inflation01Calculator;

impl MetricCalculator for Inflation01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap: &InflationSwap = context.instrument_as()?;
        let as_of = context.as_of;

        // Bump the inflation curve up by 1bp and reprice.
        let bump_up = BumpSpec::inflation_shift_pct(INFLATION_BUMP_BP * 100.0);
        let curves_up = context.curves.as_ref().bump([MarketBump::Curve {
            id: swap.inflation_index_id.clone(),
            spec: bump_up,
        }])?;
        let pv_up = swap.value(&curves_up, as_of)?.amount();

        // Bump the inflation curve down by 1bp and reprice.
        let bump_down = BumpSpec::inflation_shift_pct(-INFLATION_BUMP_BP * 100.0);
        let curves_down = context.curves.as_ref().bump([MarketBump::Curve {
            id: swap.inflation_index_id.clone(),
            spec: bump_down,
        }])?;
        let pv_down = swap.value(&curves_down, as_of)?.amount();

        // Central difference: Inflation01 = (PV_up - PV_down) / 2.
        //
        // The curve was bumped by exactly ±1bp (INFLATION_BUMP_BP = 0.0001).
        // A symmetric ±1bp central difference already produces the PV change
        // for a 1bp move; no further normalization by the decimal bump value is
        // needed. Dividing by 2 (not by 2 × INFLATION_BUMP_BP) is the correct
        // per-1bp convention — consistent with DV01 and CS01 in the workspace.
        Ok((pv_up - pv_down) / 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::INFLATION_BUMP_BP;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::rates::inflation_swap::InflationSwap;
    use crate::instruments::PricingOptions;
    use crate::metrics::MetricId;
    use finstack_core::market_data::bumps::{BumpSpec, MarketBump};
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::{DiscountCurve, InflationCurve};
    use finstack_core::types::CurveId;
    use time::macros::date;

    fn market(as_of: finstack_core::dates::Date) -> MarketContext {
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (10.0, 0.7)])
            .build()
            .expect("discount curve");
        // Use an epoch-anchored inflation curve so time fractions are computed
        // from the curve's own base_date, not the valuation date. This avoids
        // negative-time lookups when the lagged CPI date precedes as_of.
        let epoch = finstack_core::dates::Date::from_calendar_date(1970, time::Month::January, 1)
            .expect("epoch");
        let inflation = InflationCurve::builder("US-CPI")
            .base_date(epoch)
            .base_cpi(100.0)
            .knots([(0.0, 100.0), (55.0, 155.0), (60.0, 180.0)])
            .build()
            .expect("inflation curve");
        MarketContext::new().insert(discount).insert(inflation)
    }

    fn sample_swap() -> InflationSwap {
        use finstack_core::currency::Currency;
        use finstack_core::market_data::scalars::InflationLag;
        use rust_decimal::Decimal;
        use time::Month;
        // Build a swap whose start_date is 2024-01-15, maturity 2029-01-15.
        // as_of will be 2023-07-01 — well before start, so all CPI lookups lie
        // in the forward region of the inflation curve.
        InflationSwap::builder()
            .id(finstack_core::types::InstrumentId::new("TEST-SWAP"))
            .notional(finstack_core::money::Money::new(1_000_000.0, Currency::USD))
            .start_date(
                finstack_core::dates::Date::from_calendar_date(2024, Month::January, 15)
                    .expect("start"),
            )
            .maturity(
                finstack_core::dates::Date::from_calendar_date(2029, Month::January, 15)
                    .expect("maturity"),
            )
            .fixed_rate(Decimal::try_from(0.02).expect("rate"))
            .inflation_index_id(CurveId::new("US-CPI"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(finstack_core::dates::DayCount::Act365F)
            .side(crate::instruments::common_impl::parameters::legs::PayReceive::PayFixed)
            .lag_override(InflationLag::None)
            .attributes(crate::instruments::common_impl::traits::Attributes::new())
            .build()
            .expect("swap")
    }

    /// Inflation01 must equal `(pv_up - pv_down) / 2`, not
    /// `(pv_up - pv_down) / (2 * bump_decimal)`.
    ///
    /// The "…01" convention throughout the workspace (DV01, CS01, Inflation01)
    /// is PV-change-per-1bp. A ±1bp central bump produces that directly as
    /// `(pv_up - pv_down) / 2`. Dividing by the decimal bump value (0.0002)
    /// instead yields the per-*unit* derivative — 10 000× too large — which is
    /// the bug this test guards against.
    #[test]
    fn inflation01_is_per_1bp_not_per_unit() {
        let as_of = date!(2023 - 07 - 01);
        let swap = sample_swap();
        let market = market(as_of);

        // Compute the two bumped PVs the same way the calculator does.
        let bump = INFLATION_BUMP_BP;
        let curves_up = market
            .bump([MarketBump::Curve {
                id: swap.inflation_index_id.clone(),
                spec: BumpSpec::inflation_shift_pct(bump * 100.0),
            }])
            .expect("bump up");
        let curves_down = market
            .bump([MarketBump::Curve {
                id: swap.inflation_index_id.clone(),
                spec: BumpSpec::inflation_shift_pct(-bump * 100.0),
            }])
            .expect("bump down");
        let pv_up = swap.value(&curves_up, as_of).expect("pv up").amount();
        let pv_down = swap.value(&curves_down, as_of).expect("pv down").amount();

        // Expected: per-1bp sensitivity — the half-difference.
        let expected_per_1bp = (pv_up - pv_down) / 2.0;
        // The old (buggy) value would be 10 000× larger.
        let buggy_per_unit = (pv_up - pv_down) / (2.0 * bump);

        let result = swap
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Inflation01],
                PricingOptions::default(),
            )
            .expect("price with metrics");
        let reported = *result
            .measures
            .get("inflation01")
            .expect("inflation01 measure");

        // Must match the per-1bp value.
        assert!(
            (reported - expected_per_1bp).abs() < 1e-8 * expected_per_1bp.abs().max(1.0),
            "Inflation01 {reported} should equal per-1bp value {expected_per_1bp}"
        );
        // Must NOT match the per-unit (buggy) value.
        assert!(
            (reported - buggy_per_unit).abs() > expected_per_1bp.abs() * 100.0,
            "Inflation01 {reported} must NOT equal the per-unit (10 000×-inflated) value {buggy_per_unit}"
        );
    }

    /// Inflation01 sign: a pay-fixed inflation swap benefits from rising
    /// inflation, so Inflation01 must be positive.
    #[test]
    fn inflation01_sign_positive_for_pay_fixed() {
        let as_of = date!(2023 - 07 - 01);
        let swap = sample_swap();
        let market = market(as_of);

        let result = swap
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Inflation01],
                PricingOptions::default(),
            )
            .expect("price with metrics");
        let reported = *result
            .measures
            .get("inflation01")
            .expect("inflation01 measure");

        assert!(
            reported > 0.0,
            "Pay-fixed Inflation01 should be positive (higher inflation → higher PV), got {reported}"
        );
    }
}
