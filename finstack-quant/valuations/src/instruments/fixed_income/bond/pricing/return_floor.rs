//! Since-issue distribution walker for return-floor analysis.
//!
//! Provides [`realized_distributions`], which reconstructs per-coupon-date cash
//! distributions from a bond's full cashflow schedule. The output feeds the
//! return-floor lowering (Task 5) and MOIC/XIRR metrics (Task 10).

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;

use crate::cashflow::primitives::CFKind;
use crate::instruments::fixed_income::bond::Bond;

/// One candidate redemption date with the cash paid at it and the running state.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DistPoint {
    /// Coupon / candidate date (strictly after the issue `anchor`).
    pub date: Date,
    /// Cash distribution paid AT this date (cash coupons + any amortization).
    /// `PIK`, final `Notional` redemptions, fees, and margin flows are excluded.
    pub coupon: f64,
    /// Cumulative cash distributions paid strictly BEFORE this date, since issue.
    pub cum_before: f64,
    /// Outstanding principal at this date (initial notional minus amortization since issue).
    pub outstanding: f64,
}

/// Walk the bond's full cashflow schedule and reconstruct, per coupon date after
/// the issue `anchor`, the cash paid at the date, the cumulative cash before it
/// (since issue), and the outstanding principal.
///
/// # Cash classification
///
/// | CFKind                    | Treatment              |
/// |---------------------------|------------------------|
/// | `Fixed`, `FloatReset`, `Stub` | Cash coupon (counted) |
/// | `Amortization`            | Cash + reduces outstanding |
/// | `Notional`                | Excluded (final redemption / initial exchange) |
/// | `PIK`                     | Excluded (non-cash)    |
/// | Everything else           | Excluded               |
///
/// This classification is a deliberate, conservative v1 simplification. Only
/// standard bond cash kinds are counted (`Fixed`/`FloatReset`/`Stub` coupons and
/// `Amortization`). `Notional` (final redemption — the quantity the floor itself
/// adjusts), `PIK` (non-cash), and any fee/inflation/prepayment kinds that a
/// non-standard `custom_cashflows` schedule might carry are NOT counted.
/// Dropping a real cash distribution is conservative for a MOIC/XIRR floor: it
/// raises the required redemption, so the investor still clears the target. Do
/// not "fix" this without accounting for that bias direction.
///
/// The floor is an issuer-side term, so `anchor` should be the bond's issue
/// date. Floating coupons are forward-projected via `full_cashflow_schedule`.
///
/// # Errors
///
/// Propagates any error from [`Bond::full_cashflow_schedule`].
pub(crate) fn realized_distributions(
    bond: &Bond,
    curves: &MarketContext,
    anchor: Date,
) -> finstack_quant_core::Result<Vec<DistPoint>> {
    let schedule = bond.full_cashflow_schedule(curves)?;
    let initial = bond.notional.amount();

    // Sort defensively; schedule is usually already ordered. `schedule` is an
    // owned local not used after this point, so move `flows` instead of cloning.
    let mut flows = schedule.flows;
    flows.sort_by_key(|cf| cf.date);

    let mut points: Vec<DistPoint> = Vec::new();
    // Cash paid strictly before the current date group, since `anchor`.
    let mut cum_before = 0.0_f64;
    // Sum of amortization since issue (drives outstanding principal).
    let mut amortized = 0.0_f64;

    let mut i = 0;
    while i < flows.len() {
        let d = flows[i].date;

        // Aggregate all cashflows on this date into coupon and amortization buckets.
        let mut coupon_at_d = 0.0_f64;
        let mut amort_at_d = 0.0_f64;
        let mut j = i;
        while j < flows.len() && flows[j].date == d {
            let cf = &flows[j];
            match cf.kind {
                CFKind::Fixed | CFKind::FloatReset | CFKind::Stub => {
                    coupon_at_d += cf.amount.amount().abs();
                }
                CFKind::Amortization => {
                    amort_at_d += cf.amount.amount().abs();
                }
                // Notional redemptions (initial or final), PIK (non-cash),
                // fees, margin flows, etc. — excluded from distributions.
                _ => {}
            }
            j += 1;
        }

        // Only record dates strictly after the anchor (issue date).
        let cash_at_d = coupon_at_d + amort_at_d;
        if d > anchor && (coupon_at_d > 0.0 || amort_at_d > 0.0) {
            points.push(DistPoint {
                date: d,
                coupon: cash_at_d,
                cum_before,
                outstanding: initial - amortized,
            });
        }

        // Advance running state (applies even if d <= anchor so that
        // amortization before the observation window is tracked correctly).
        if d > anchor {
            cum_before += cash_at_d;
        }
        amortized += amort_at_d;
        i = j;
    }

    Ok(points)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::bond::Bond;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::Rate;
    use time::macros::date;

    fn fixed_10pct_bullet() -> Bond {
        // 2-year, 10% annual (semi-annual by Bond::fixed convention), bullet, $100 notional.
        Bond::fixed(
            "L",
            Money::new(100.0, Currency::USD),
            Rate::from_percent(10.0),
            date!(2024 - 01 - 15),
            date!(2026 - 01 - 15),
            "USD-OIS",
        )
        .unwrap()
    }

    #[test]
    fn distributions_accumulate_coupons_and_outstanding_is_constant_for_bullet() {
        let bond = fixed_10pct_bullet();
        let curves = MarketContext::new();
        let dist = realized_distributions(&bond, &curves, date!(2024 - 01 - 15)).unwrap();

        // Bullet: outstanding stays at notional until maturity — no amortization.
        assert!(
            dist.iter().all(|d| (d.outstanding - 100.0).abs() < 1e-6),
            "Expected outstanding == 100.0 for bullet; got: {:?}",
            dist.iter().map(|d| d.outstanding).collect::<Vec<_>>()
        );

        // Cumulative-before-date is monotonically non-decreasing.
        for w in dist.windows(2) {
            assert!(
                w[1].cum_before + 1e-9 >= w[0].cum_before,
                "cum_before not non-decreasing: {} < {}",
                w[1].cum_before,
                w[0].cum_before
            );
        }

        // Semi-annual 10% on $100 notional → ~$5 per coupon (30/360 basis).
        // There should be 4 coupon dates for a 2-year bullet.
        assert_eq!(
            dist.len(),
            4,
            "Expected 4 coupon dates for a 2-year semi-annual bullet"
        );
        for d in &dist {
            assert!(
                (d.coupon - 5.0).abs() < 0.1,
                "Expected ~$5 coupon, got {}",
                d.coupon
            );
        }
    }

    #[test]
    fn cum_before_starts_at_zero_for_first_coupon() {
        let bond = fixed_10pct_bullet();
        let curves = MarketContext::new();
        let dist = realized_distributions(&bond, &curves, date!(2024 - 01 - 15)).unwrap();

        assert!(!dist.is_empty(), "Expected at least one distribution point");
        assert!(
            dist[0].cum_before.abs() < 1e-12,
            "cum_before for first coupon should be 0, got {}",
            dist[0].cum_before
        );
    }

    #[test]
    fn empty_when_anchor_is_at_maturity() {
        let bond = fixed_10pct_bullet();
        let curves = MarketContext::new();
        // Anchor at maturity → no dates strictly after anchor.
        let dist = realized_distributions(&bond, &curves, date!(2026 - 01 - 15)).unwrap();
        assert!(
            dist.is_empty(),
            "Expected no distribution points when anchor == maturity"
        );
    }
}
