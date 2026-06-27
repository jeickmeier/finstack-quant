//! Since-issue distribution walker and return-floor lowering.
//!
//! Provides [`realized_distributions`], which reconstructs per-coupon-date cash
//! distributions from a bond's full cashflow schedule. The output feeds
//! [`lower_return_floor`], which compiles a [`ReturnFloorSpec`] into a concrete
//! [`CallPutSchedule`] (Task 5), and the MOIC/XIRR metrics (Task 10).
//!
//! Floating-rate caveat (v1): cumulative coupons are forward-projected off the
//! forward curve, which ignores the correlation between the realized rate path
//! and when the floor binds. This is an approximation; the path-exact treatment
//! is the deferred v2 Longstaff-Schwartz Monte Carlo payoff.

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

impl Bond {
    /// Return a pricing-ready clone of this bond with any `return_floor` lowered
    /// into `call_put` (merged with contractual calls) and `return_floor` cleared.
    ///
    /// When no floor is present the clone is unchanged. Gating callers on
    /// `self.return_floor.is_some()` avoids the clone otherwise.
    pub(crate) fn effective_for_pricing(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Bond> {
        let mut clone = self.clone();
        if let Some(spec) = self.return_floor.as_ref() {
            let merged = lower_return_floor(self, spec, curves, as_of)?;
            clone.call_put = Some(merged);
            clone.return_floor = None;
        }
        Ok(clone)
    }
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

    #[test]
    fn effective_for_pricing_lowers_floor_into_call_put() {
        let curves = MarketContext::new();
        let as_of = date!(2024 - 01 - 15);

        // Plain bond: effective == itself, no call schedule added, no floor.
        let plain = fixed_10pct_bullet();
        let eff_plain = plain.effective_for_pricing(&curves, as_of).unwrap();
        assert!(eff_plain.call_put.is_none());
        assert!(eff_plain.return_floor.is_none());

        // Floored bond: floor lowered into call_put, return_floor cleared.
        let floored = fixed_10pct_bullet().min_moic(1.25);
        let eff_floored = floored.effective_for_pricing(&curves, as_of).unwrap();
        assert!(eff_floored.return_floor.is_none());
        assert!(eff_floored
            .call_put
            .as_ref()
            .is_some_and(|c| !c.calls.is_empty()));
    }

    #[test]
    fn base_value_hook_prices_floored_bond_via_tree() {
        use crate::instruments::common_impl::traits::Instrument;
        use finstack_quant_core::market_data::context::MarketContext;
        use finstack_quant_core::market_data::term_structures::DiscountCurve;
        use finstack_quant_core::math::interp::InterpStyle;

        let as_of = date!(2024 - 01 - 15);

        // Build a flat 5% discount curve so the tree engine has something to work with.
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (3.0, 0.857)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("flat discount curve for smoke test");
        let market = MarketContext::new().insert(disc);

        // A floored bond must route through the base_value hook and return Ok with a
        // positive price. The plain bond (no floor) must also still price correctly.
        let plain = fixed_10pct_bullet();
        let price_plain = plain.base_value(&market, as_of).unwrap();
        assert!(
            price_plain.amount() > 0.0,
            "plain bond price should be positive"
        );

        let floored = fixed_10pct_bullet().min_moic(1.25);
        let price_floored = floored.base_value(&market, as_of).unwrap();
        assert!(
            price_floored.amount() > 0.0,
            "floored bond price should be positive"
        );
        assert_eq!(
            price_floored.currency(),
            price_plain.currency(),
            "currency should match notional"
        );
    }

    #[test]
    fn floating_floor_uses_projected_coupons() {
        use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
        use finstack_quant_core::math::interp::InterpStyle;

        let bond = Bond::example_floating().unwrap().min_moic(1.20);
        let as_of = date!(2024 - 01 - 15);

        // Build a flat 5% discount curve named "USD-OIS" (the bond's discount_curve_id).
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (6.0, 0.741)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("flat discount curve for FRN smoke test");

        // Build a flat 4.5% forward curve named "USD-SOFR-3M" (the bond's index_id).
        // Tenor 0.25 = quarterly (3M SOFR), matching the FloatingRateSpec in example_floating.
        let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(as_of)
            .knots([(0.0, 0.045), (6.0, 0.045)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("flat forward curve for FRN smoke test");

        // MarketContext must carry BOTH the discount curve and the forward curve so that
        // Bond::full_cashflow_schedule can project the floating coupons.
        let curves = MarketContext::new().insert(disc).insert(fwd);

        let spec = bond.return_floor.as_ref().unwrap();
        let sched = lower_return_floor(&bond, spec, &curves, as_of).unwrap();

        // The FRN has quarterly coupons over 5 years inside the protection window,
        // so the schedule must be non-empty.
        assert!(
            !sched.calls.is_empty(),
            "Expected non-empty call schedule for floored FRN; got 0 entries"
        );

        // Every floored redemption price must be at or above par.
        assert!(
            sched
                .calls
                .iter()
                .all(|c| c.price_pct_of_par >= 100.0 - 1e-9),
            "All floored call prices must be >= 100% of par; got: {:?}",
            sched
                .calls
                .iter()
                .map(|c| c.price_pct_of_par)
                .collect::<Vec<_>>()
        );
    }

    // ── Task 11: guarantee tests ──────────────────────────────────────────────
    //
    // These tests prove that the return floor is CALL-PROTECTION ONLY: every
    // early-call path in the lowered schedule delivers MOIC/XIRR >= target across
    // rate scenarios.  They do NOT assert MoicToWorst >= target (that would be
    // wrong — the unfloored maturity path can legitimately be below target).

    /// Build a flat discount curve for the test bond's discount_curve_id "USD-OIS".
    ///
    /// `rate` is the continuously-compounding equivalent approximated via
    /// `df(T) = exp(-r*T)`.  For simplicity we pass in discount factors at
    /// representative tenors; the curve is flat so we only need two knots.
    fn flat_discount_market(rate: f64, as_of: finstack_quant_core::dates::Date) -> MarketContext {
        use finstack_quant_core::market_data::term_structures::DiscountCurve;
        use finstack_quant_core::math::interp::InterpStyle;

        // df at t=0 is always 1.0; df at t=6y approximates a flat r curve.
        let df6 = (-rate * 6.0_f64).exp();
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0_f64, 1.0_f64), (6.0_f64, df6)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("flat discount curve");
        MarketContext::new().insert(disc)
    }

    /// Build a 5-year 10% annual coupon (semi-annual by Bond::fixed convention),
    /// par-100, bullet bond.
    fn fixed_5y_10pct() -> Bond {
        Bond::fixed(
            "T11",
            Money::new(100.0, Currency::USD),
            Rate::from_percent(10.0),
            date!(2024 - 01 - 15),
            date!(2029 - 01 - 15),
            "USD-OIS",
        )
        .unwrap()
    }

    /// Test 1 — MOIC floor holds on every early-call path across rate scenarios.
    ///
    /// For a 5y 10% bullet at par-100 with min_moic(1.25), every call date in
    /// the lowered schedule must deliver MOIC >= 1.25 regardless of the discount
    /// rate environment (the floor is a contractual redemption price guarantee,
    /// not rate-dependent).
    #[test]
    fn moic_floor_holds_on_every_early_call_path_across_rate_scenarios() {
        let moic_target = 1.25_f64;
        let v0 = 100.0_f64;
        let bond = fixed_5y_10pct().min_moic(moic_target);
        let as_of = date!(2024 - 01 - 15);
        let spec = bond.return_floor.as_ref().unwrap();

        let flat_rates = [0.0_f64, 0.02, 0.05, 0.10, 0.20];

        // For a FIXED-coupon bond the discount rate does NOT affect
        // `realized_distributions` — the cashflow schedule is curve-independent,
        // so this loop is effectively redundant for a fixed bond. It is kept for
        // parity with the floating test (Test 3), where varying the forward level
        // genuinely changes the projected coupons and thus the floored redemption.
        for &r in &flat_rates {
            let market = flat_discount_market(r, as_of);
            let sched = lower_return_floor(&bond, spec, &market, as_of)
                .unwrap_or_else(|e| panic!("lower_return_floor failed at rate={r}: {e}"));

            assert!(
                !sched.calls.is_empty(),
                "Expected non-empty call schedule at rate={r}"
            );

            let dist = realized_distributions(&bond, &market, bond.issue_date)
                .unwrap_or_else(|e| panic!("realized_distributions failed at rate={r}: {e}"));

            for call in &sched.calls {
                // Find the DistPoint whose date matches the call's start_date.
                let point = dist
                    .iter()
                    .find(|p| p.date == call.start_date)
                    .unwrap_or_else(|| {
                        panic!(
                            "No DistPoint found for call date {} at rate={r}",
                            call.start_date
                        )
                    });

                let cash_incl = point.cum_before + point.coupon;
                // Redemption = price_pct_of_par% of par notional (V0 = 100).
                let redemption = call.price_pct_of_par / 100.0 * v0;
                let moic = (cash_incl + redemption) / v0;

                assert!(
                    moic >= moic_target - 1e-9,
                    "MOIC floor violated: rate={r}, date={}, moic={moic:.8}, target={moic_target}",
                    call.start_date
                );
            }
        }
    }

    /// Test 1b ("teeth") — the MOIC check is not vacuous.
    ///
    /// Test 1 verifies `(cash_incl + lowered_redemption)/V0 >= target`, but on the
    /// dates where the floor binds above par the lowered redemption is derived from
    /// the same arithmetic, so that check cannot fail there. This mutation test
    /// proves the lowered redemption is the GENUINE minimum: cutting it by 5 points
    /// breaks the target. That confirms the lowering sets redemption to exactly
    /// what's needed (not arbitrarily high), so the property tests have bite.
    #[test]
    fn moic_check_has_teeth_redemption_below_floor_breaks_target() {
        let moic_target = 1.25_f64;
        let v0 = 100.0_f64;
        let bond = fixed_5y_10pct().min_moic(moic_target);
        let as_of = date!(2024 - 01 - 15);
        let spec = bond.return_floor.as_ref().unwrap();

        let market = flat_discount_market(0.05, as_of);
        let sched = lower_return_floor(&bond, spec, &market, as_of).unwrap();
        let dist = realized_distributions(&bond, &market, bond.issue_date).unwrap();

        // Find a call date where the floor binds strictly ABOVE par — an early
        // date where coupons received so far do not yet clear the target alone.
        let binding_call = sched
            .calls
            .iter()
            .find(|c| c.price_pct_of_par > 100.0 + 1e-9)
            .expect("expected at least one early call where the floor binds above par");

        let point = dist
            .iter()
            .find(|p| p.date == binding_call.start_date)
            .expect("DistPoint for the binding call date");
        let cash_incl = point.cum_before + point.coupon;

        // Sanity: the lowered redemption hits the target exactly (within tol).
        let at_floor = binding_call.price_pct_of_par / 100.0 * v0;
        let moic_at_floor = (cash_incl + at_floor) / v0;
        assert!(
            (moic_at_floor - moic_target).abs() < 1e-6,
            "lowered redemption should hit target exactly: moic={moic_at_floor:.8}"
        );

        // Mutation: a redemption 5 points BELOW the lowered price breaches target.
        let short = (binding_call.price_pct_of_par - 5.0) / 100.0 * v0;
        assert!(
            (cash_incl + short) / v0 < moic_target,
            "lowered redemption is not the binding minimum (date={}, price_pct={})",
            binding_call.start_date,
            binding_call.price_pct_of_par
        );
    }

    /// Test 2 — XIRR floor holds on every early-call path (fixed-rate bond).
    ///
    /// For a 5y 10% bullet at par-100 with min_xirr(0.12), we reconstruct the
    /// full investor cashflow stream for each call path and verify the solved
    /// XIRR is >= 12%.  By construction the floor binds on every early call for
    /// a 10% bond with a 12% target, so each path should return ~0.12 exactly.
    #[test]
    fn xirr_floor_holds_on_every_early_call_path() {
        let xirr_target = 0.12_f64;
        let v0 = 100.0_f64;
        let bond = fixed_5y_10pct().min_xirr(Rate::from_percent(12.0));
        let as_of = date!(2024 - 01 - 15);
        let spec = bond.return_floor.as_ref().unwrap();

        let market = flat_discount_market(0.05, as_of);
        let sched = lower_return_floor(&bond, spec, &market, as_of).unwrap();

        assert!(
            !sched.calls.is_empty(),
            "Expected non-empty call schedule for XIRR-floored bond"
        );

        let dist = realized_distributions(&bond, &market, bond.issue_date).unwrap();

        for call in &sched.calls {
            let point = dist
                .iter()
                .find(|p| p.date == call.start_date)
                .unwrap_or_else(|| panic!("No DistPoint for call date {}", call.start_date));

            // Reconstruct the investor cashflow stream for this call path:
            //   (issue, -V0), then each coupon point.date <= call.start_date,
            //   then the redemption at call.start_date.
            let mut flows: Vec<(finstack_quant_core::dates::Date, f64)> =
                vec![(bond.issue_date, -v0)];

            for p in &dist {
                if p.date > call.start_date {
                    break;
                }
                if p.date < call.start_date {
                    flows.push((p.date, p.coupon));
                }
            }

            // At the call date: coupon paid at that date + redemption.
            let redemption = call.price_pct_of_par / 100.0 * v0;
            flows.push((call.start_date, point.coupon + redemption));

            let realized = finstack_quant_core::cashflow::xirr(&flows, None).unwrap_or_else(|e| {
                panic!("XIRR solver failed at call date {}: {e}", call.start_date)
            });

            assert!(
                realized >= xirr_target - 1e-6,
                "XIRR floor violated: date={}, realized={realized:.8}, target={xirr_target}",
                call.start_date
            );
        }
    }

    /// Test 3 — Floating: MOIC floor holds on every early-call path.
    ///
    /// `Bond::example_floating().min_moic(1.20)` is a quarterly SOFR-linked FRN
    /// with $1M notional over 5 years.  We vary the forward rate across two
    /// levels (4.5% and 8%) to confirm the floor holds regardless of where
    /// projected coupons land.
    #[test]
    fn floating_moic_floor_holds_on_every_early_call_path() {
        use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
        use finstack_quant_core::math::interp::InterpStyle;

        let moic_target = 1.20_f64;
        let bond = Bond::example_floating().unwrap().min_moic(moic_target);
        let as_of = date!(2024 - 01 - 15);
        let v0 = bond.notional.amount(); // $1,000,000
        let spec = bond.return_floor.as_ref().unwrap();

        let forward_levels = [0.045_f64, 0.08_f64];

        for &fwd_rate in &forward_levels {
            let disc = DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0_f64, 1.0_f64), (6.0_f64, 0.741_f64)])
                .interp(InterpStyle::Linear)
                .build()
                .expect("flat discount curve for FRN test");

            let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
                .base_date(as_of)
                .knots([(0.0_f64, fwd_rate), (6.0_f64, fwd_rate)])
                .interp(InterpStyle::Linear)
                .build()
                .expect("flat forward curve for FRN test");

            let market = MarketContext::new().insert(disc).insert(fwd);

            let sched = lower_return_floor(&bond, spec, &market, as_of)
                .unwrap_or_else(|e| panic!("lower_return_floor failed at fwd={fwd_rate}: {e}"));

            assert!(
                !sched.calls.is_empty(),
                "Expected non-empty call schedule at fwd_rate={fwd_rate}"
            );

            let dist =
                realized_distributions(&bond, &market, bond.issue_date).unwrap_or_else(|e| {
                    panic!("realized_distributions failed at fwd_rate={fwd_rate}: {e}")
                });

            for call in &sched.calls {
                let point = dist
                    .iter()
                    .find(|p| p.date == call.start_date)
                    .unwrap_or_else(|| {
                        panic!(
                            "No DistPoint for call date {} at fwd_rate={fwd_rate}",
                            call.start_date
                        )
                    });

                let cash_incl = point.cum_before + point.coupon;
                let redemption = call.price_pct_of_par / 100.0 * v0;
                let moic = (cash_incl + redemption) / v0;

                assert!(
                    moic >= moic_target - 1e-9,
                    "Floating MOIC floor violated: fwd={fwd_rate}, date={}, moic={moic:.8}, target={moic_target}",
                    call.start_date
                );
            }
        }
    }

    /// Test 4 — Honesty: `XirrToWorst` is NOT bounded by the floor target.
    ///
    /// For a 5y 10% bond with min_xirr(0.12), the held-to-maturity path returns
    /// ~10% (below the 12% target).  Since `XirrToWorst` takes the minimum over
    /// ALL exits including maturity, it must be < 0.12.  This confirms the metric
    /// is honest: call protection does NOT guarantee the maturity return.
    #[test]
    fn xirr_to_worst_is_not_bounded_by_floor_target_maturity_path_dominates() {
        use crate::instruments::fixed_income::bond::metrics::return_metrics::xirr::XirrToWorstCalculator;
        use crate::metrics::{MetricCalculator, MetricContext};
        use std::sync::Arc;

        let xirr_target = 0.12_f64;
        let bond = fixed_5y_10pct().min_xirr(Rate::from_percent(12.0));
        let as_of = date!(2024 - 01 - 15);

        // Use a 5% flat discount environment — the XIRR to worst should be ~10%
        // (the bond's coupon rate) since the maturity path is the worst exit.
        let market = Arc::new(flat_discount_market(0.05, as_of));
        let base_value = Money::new(100.0, Currency::USD);

        let mut ctx = MetricContext::new(
            Arc::new(bond),
            market,
            as_of,
            base_value,
            MetricContext::default_config(),
        );

        let xirr_worst = XirrToWorstCalculator.calculate(&mut ctx).unwrap();

        // The maturity path (10% coupon, par redemption) dominates — so XirrToWorst
        // is near 10%, well below the 12% floor target.
        assert!(
            xirr_worst < xirr_target,
            "XirrToWorst should be < floor target {xirr_target} (maturity path dominates), got {xirr_worst:.6}"
        );
    }
}
