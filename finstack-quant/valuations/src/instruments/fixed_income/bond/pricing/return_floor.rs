//! Since-issue distribution walker and return-floor lowering.
//!
//! Provides [`realized_distributions`], which reconstructs per-coupon-date cash
//! distributions from a bond's full cashflow schedule. The output feeds
//! [`lower_return_floor`], which compiles a [`ReturnFloorSpec`] into a concrete
//! [`CallPutSchedule`] (Task 5), and the MOIC/XIRR metrics (Task 10).

use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;

use crate::cashflow::primitives::CFKind;
use crate::instruments::fixed_income::bond::{
    Bond, CallPut, CallPutSchedule, IssuePrice, ProtectionWindow, ReturnFloorKind, ReturnFloorSpec,
};

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

// ── Return-floor lowering ─────────────────────────────────────────────────────

/// Resolve invested capital `V0` from the issue price spec.
fn invested_capital(bond: &Bond, price: &IssuePrice) -> finstack_quant_core::Result<f64> {
    Ok(match price {
        IssuePrice::Par => bond.notional.amount(),
        IssuePrice::PctOfPar(pct) => bond.notional.amount() * pct / 100.0,
        IssuePrice::Amount(m) => {
            if m.currency() != bond.notional.currency() {
                return Err(finstack_quant_core::Error::Validation(
                    "return floor IssuePrice::Amount currency must match notional".to_string(),
                ));
            }
            m.amount()
        }
    })
}

/// Return `true` when `d` falls inside the protection window.
fn in_window(window: &ProtectionWindow, d: Date, issue: Date, maturity: Date) -> bool {
    match window {
        ProtectionWindow::Full => d > issue && d < maturity,
        ProtectionWindow::From(start) => d >= *start && d < maturity,
        ProtectionWindow::Between { start, end } => d >= *start && d <= *end,
    }
}

/// Lower a [`ReturnFloorSpec`] into a [`CallPutSchedule`].
///
/// For each coupon date that falls inside the protection window and on or after
/// `as_of`, the minimum redemption price is computed so the investor meets the
/// target MOIC or XIRR (anchored at the bond's issue date and issue price).
/// Any contractual call already on the bond is merged at shared dates (max
/// price). The resulting price is floored at par (100).
///
/// The floor binds only on early issuer-called redemptions; it does not apply
/// at maturity. A held-to-maturity investor receives the bond's natural return,
/// which may be below the target — this is call protection, not a maturity
/// guarantee.
///
/// The XIRR floor is defined on an Act/365F basis (matching
/// `core::cashflow::xirr`) unless `spec.day_count` overrides it; the XIRR
/// verification metric MUST discount on the same basis for the guarantee to
/// hold exactly.
///
/// # MOIC floor
///
/// ```text
/// R(t) = m * V0 − cash_incl(t)
/// price_pct(t) = 100 * max(R(t), contractual(t), 100) / outstanding(t)
/// ```
///
/// # XIRR floor
///
/// ```text
/// R(t) = (1 + r)^yf(issue, t) * [ V0 − Σ q.coupon / (1+r)^yf(issue, q.date) ]
/// price_pct(t) = 100 * max(R(t), contractual(t), 100) / outstanding(t)
/// ```
///
/// # Limitations
///
/// Composition with contractual **make-whole** calls is not supported in v1: a
/// make-whole call's effective price can exceed the floor, so silently keeping
/// only its `price_pct_of_par` would under-price it. If a make-whole call is
/// active at any candidate date, this function returns an error rather than
/// mis-price.
///
/// # Errors
///
/// Returns `Err` if the spec fails validation, if cashflow generation fails, or
/// if a contractual make-whole call is active within the protection window.
pub(crate) fn lower_return_floor(
    bond: &Bond,
    spec: &ReturnFloorSpec,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<CallPutSchedule> {
    spec.validate()?;
    let v0 = invested_capital(bond, &spec.issue_price)?;
    let issue = bond.issue_date;
    let dist = realized_distributions(bond, curves, issue)?;
    // The XIRR floor must discount on the same basis as the verification metric
    // (`core::cashflow::xirr`, Act/365F) for the guarantee to hold exactly.
    let dc = spec.day_count.unwrap_or(DayCount::Act365F);

    let mut calls: Vec<CallPut> = Vec::new();

    for p in &dist {
        if !in_window(&spec.window, p.date, issue, bond.maturity) || p.date < as_of {
            continue;
        }
        // Normally unreachable (a positive-notional bond has positive outstanding),
        // but guards against custom schedules that fully amortize the principal.
        if p.outstanding <= 0.0 {
            continue;
        }

        // Cash paid from issue up to and including this date.
        let cash_incl = p.cum_before + p.coupon;

        let r = match spec.kind {
            ReturnFloorKind::Moic(m) => m * v0 - cash_incl,
            ReturnFloorKind::Xirr(rate) => {
                let rate_dec = rate.as_decimal();
                let yf_t = dc.year_fraction(issue, p.date, DayCountContext::default())?;
                // Discount each coupon from issue to its date at the target rate.
                let mut pv_coupons = 0.0_f64;
                for q in &dist {
                    if q.date > p.date {
                        break;
                    }
                    let yf = dc.year_fraction(issue, q.date, DayCountContext::default())?;
                    pv_coupons += q.coupon / (1.0_f64 + rate_dec).powf(yf);
                }
                (v0 - pv_coupons) * (1.0_f64 + rate_dec).powf(yf_t)
            }
        };

        // Merge with any contractual call active at this date. Make-whole calls
        // are not supported in v1 (their effective price can exceed the floor).
        let contractual = match bond.call_put.as_ref().and_then(|cp| {
            cp.calls
                .iter()
                .find(|c| p.date >= c.start_date && p.date <= c.end_date)
        }) {
            Some(c) if c.make_whole.is_some() => {
                return Err(finstack_quant_core::Error::Validation(
                    "return floor composition with make-whole contractual calls is not \
                     supported in v1; remove the make-whole or the return floor"
                        .to_string(),
                ));
            }
            Some(c) => Some(c.price_pct_of_par),
            None => None,
        };

        // Floor redemption as % of outstanding; never below contractual or par.
        let floor_pct = 100.0 * r / p.outstanding;
        let price_pct = floor_pct.max(contractual.unwrap_or(100.0)).max(100.0);

        calls.push(CallPut {
            start_date: p.date,
            end_date: p.date,
            price_pct_of_par: price_pct,
            make_whole: None,
        });
    }

    // Preserve any existing puts (contractual calls already folded into `calls`).
    let puts = bond
        .call_put
        .as_ref()
        .map(|cp| cp.puts.clone())
        .unwrap_or_default();

    Ok(CallPutSchedule { calls, puts })
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

    // ── lower_return_floor tests ──────────────────────────────────────────────

    use crate::instruments::fixed_income::bond::ReturnFloorSpec;

    #[test]
    fn moic_floor_steps_down_to_par() {
        // 2y, 10% (semi-annual), par-100 bullet; MOIC 1.25.
        // By 1y, total coupons = 10, so floor redemption = 125 - 10 = 115 => 115% of par.
        let bond = fixed_10pct_bullet();
        let curves = MarketContext::new();
        let spec = ReturnFloorSpec::moic(1.25);
        let sched = lower_return_floor(&bond, &spec, &curves, date!(2024 - 01 - 15)).unwrap();
        let yr1 = sched
            .calls
            .iter()
            .find(|c| c.start_date == date!(2025 - 01 - 15))
            .unwrap();
        assert!(
            (yr1.price_pct_of_par - 115.0).abs() < 0.5,
            "got {}",
            yr1.price_pct_of_par
        );
    }

    #[test]
    fn xirr_floor_meets_target_at_each_call() {
        let bond = fixed_10pct_bullet();
        let curves = MarketContext::new();
        let spec = ReturnFloorSpec::xirr(finstack_quant_core::types::Rate::from_percent(12.0));
        let sched = lower_return_floor(&bond, &spec, &curves, date!(2024 - 01 - 15)).unwrap();
        let yr1 = sched
            .calls
            .iter()
            .find(|c| c.start_date == date!(2025 - 01 - 15))
            .unwrap();
        // Realized cashflows if called at year 1: -100 invested, two $5 coupons, redemption.
        let redemption = yr1.price_pct_of_par; // % of par == cash on $100 notional
        let flows = [
            (date!(2024 - 01 - 15), -100.0),
            (date!(2024 - 07 - 15), 5.0),
            (date!(2025 - 01 - 15), 5.0 + redemption),
        ];
        let realized = finstack_quant_core::cashflow::xirr(&flows, None).unwrap();
        assert!(
            (realized - 0.12).abs() < 1e-4,
            "realized XIRR {realized} != 12% (redemption {redemption})"
        );
    }

    #[test]
    fn floor_never_below_par_or_contractual() {
        let bond = fixed_10pct_bullet();
        let curves = MarketContext::new();
        let sched = lower_return_floor(
            &bond,
            &ReturnFloorSpec::moic(1.25),
            &curves,
            date!(2024 - 01 - 15),
        )
        .unwrap();
        assert!(sched
            .calls
            .iter()
            .all(|c| c.price_pct_of_par >= 100.0 - 1e-9));
    }
}
