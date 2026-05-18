//! Tests for CMS floorlet static-replication integral sign (Andersen-Piterbarg §16.2).
//!
//! ## The Defect
//!
//! The floorlet branch of `CmsReplicationPricer` uses `g(K)·P_sw(K) + ∫g'·P_sw dk`,
//! but the correct Andersen-Piterbarg identity requires a **minus** sign:
//!
//! ```text
//! V_floor = g(K)·P_sw(K) − ∫_{K_min}^K g'(k)·P_sw(k) dk
//! ```
//!
//! ## Mathematical Derivation of the Sign
//!
//! The AP formula for the CMS floorlet is derived via integration by parts (IBP).
//! The floorlet price is defined as:
//!
//! ```text
//! V_floor = A₀ · ∫_0^K g(k) · Q^A(S < k) dk
//! ```
//!
//! where `P_sw'(k) = ∂/∂k [A₀ · E^A[(k−S)^+]] = A₀ · Q^A(S < k)`.
//!
//! Applying IBP with `u = P_sw(k)`, `dv = g'(k) dk`:
//!
//! ```text
//! ∫_0^K g'(k)·P_sw(k) dk = [g(k)·P_sw(k)]_0^K − ∫_0^K g(k)·P_sw'(k) dk
//!                         = g(K)·P_sw(K) − ∫_0^K g(k)·A₀·Q^A(S < k) dk
//!                         = g(K)·P_sw(K) − V_floor
//! ```
//!
//! Rearranging: **`V_floor = g(K)·P_sw(K) − ∫_0^K g'(k)·P_sw(k) dk`** (MINUS).
//!
//! For the caplet, the same IBP gives:
//! ```text
//! ∫_K^∞ g'(k)·C_sw(k) dk = −g(K)·C_sw(K) + V_cap  ⟹  V_cap = g(K)·C_sw + ∫g'·C_sw
//! ```
//! confirming the PLUS sign is correct for the caplet branch.
//!
//! ## Key Consequence: V_floor < boundary
//!
//! Since `g(k)` is strictly **increasing** in `k` (because `A_par(k)` is strictly decreasing),
//! for all `k < K`:  `g(k) < g(K)`, so the replication integral is strictly positive and
//! `V_floor = boundary − δ_floor < boundary`.
//!
//! With the WRONG (`+`) sign, the formula gives `boundary + δ_floor > boundary`.
//! With the CORRECT (`−`) sign, the formula gives `boundary − δ_floor < boundary`.
//!
//! ## Annuity convention (audit item 12)
//!
//! The boundary term is `g(K)·C_sw(K)`. The Radon-Nikodym weight `g(k) =
//! DF_pay/A_par(k)` is defined with the closed-form par annuity `A_par(k)`; for
//! annuity-consistency the replicating swaption price `C_sw(k)` is expressed on
//! the **same** par annuity, `C_sw(k) = A_par(k)·Black76(F,k,σ,T)`, so the
//! product collapses to
//!
//! ```text
//! boundary = g(K)·C_sw(K) = DF_pay · Black76(F, K, σ(K), T)
//! ```
//!
//! with the annuity cancelling. (The earlier convention used the market
//! annuity `A₀` for `C_sw`, leaving a spurious `A₀/A_par(K)` residual.) The
//! tests below recompute `boundary` from this corrected, annuity-cancelled
//! form using independently-evaluated `DF_pay` and Black-76 prices — so the
//! integral-SIGN assertions remain self-contained.

use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::surfaces::VolSurface;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::market_data::term_structures::ForwardCurve;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_valuations::instruments::rates::cms_option::replication_pricer::CmsReplicationPricer;
use finstack_valuations::instruments::rates::cms_option::CmsOption;
use finstack_valuations::instruments::{OptionType, PricingOverrides};
use finstack_valuations::pricer::Pricer;
use rust_decimal::Decimal;
use time::Month;

// ─── standalone math helpers ─────────────────────────────────────────────────

/// Standard-normal CDF (Hart approximation; max absolute error ≤ 1.5e-7).
fn n_cdf(x: f64) -> f64 {
    if x > 8.0 {
        return 1.0;
    }
    if x < -8.0 {
        return 0.0;
    }
    let a = [
        0.319381530_f64,
        -0.356563782,
        1.781477937,
        -1.821255978,
        1.330274429,
    ];
    let k = 1.0 / (1.0 + 0.2316419 * x.abs());
    let poly = k * (a[0] + k * (a[1] + k * (a[2] + k * (a[3] + k * a[4]))));
    let pdf = (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let r = 1.0 - pdf * poly;
    if x >= 0.0 {
        r
    } else {
        1.0 - r
    }
}

/// Black-76 undiscounted put price.
fn b76_put(f: f64, k: f64, vol: f64, t: f64) -> f64 {
    if t <= 0.0 || vol <= 0.0 || f <= 0.0 || k <= 0.0 {
        return (k - f).max(0.0);
    }
    let sig_t = vol * t.sqrt();
    let d1 = (f / k).ln() / sig_t + sig_t / 2.0;
    let d2 = d1 - sig_t;
    k * n_cdf(-d2) - f * n_cdf(-d1)
}

/// Closed-form par annuity (same formula used by `CmsReplicationPricer`).
///
/// `A_par(k) = (1 − (1 + k/m)^{−n·m}) / k`
///
/// This function is **strictly decreasing** in `k` for `k > 0`, so
/// `g(k) = df_pay / A_par(k)` is strictly increasing and `g'(k) > 0`.
fn par_annuity(rate: f64, tenor_years: f64, m: f64) -> f64 {
    if rate.abs() < 1e-9 {
        return tenor_years; // L'Hôpital limit
    }
    let nm = tenor_years * m;
    (1.0 - (1.0 + rate / m).powf(-nm)) / rate
}

// ─── market builder ───────────────────────────────────────────────────────────

/// Build a single-curve market: OIS flat at `r`, vol flat at `v`.
///
/// Using a single flat curve for both discounting and forwarding ensures the
/// forward swap rate equals exactly `r` (eliminating day-count drift between
/// Act360 float and Act365F OIS).  `discount_curve_id` and `forward_curve_id`
/// are both set to "USD-FLAT" in the instruments created below.
fn single_curve_market(as_of: Date, r: f64, v: f64) -> MarketContext {
    let mut mkt = MarketContext::new();

    let ois_knots: Vec<(f64, f64)> = [
        0.0, 0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 12.0, 15.0, 20.0, 30.0,
    ]
    .iter()
    .map(|&t| (t, (-r * t).exp()))
    .collect();

    mkt = mkt.insert(
        DiscountCurve::builder(CurveId::new("USD-FLAT"))
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots(ois_knots)
            .build()
            .unwrap(),
    );

    // Forward curve with same rate so F_swap = r exactly.
    let fwd_knots = vec![(0.0, r), (30.0, r)];
    mkt = mkt.insert(
        ForwardCurve::builder(CurveId::new("USD-FLAT-FWD"), 0.25)
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots(fwd_knots)
            .build()
            .unwrap(),
    );

    // Flat vol surface
    let strikes = vec![
        0.005, 0.01, 0.015, 0.02, 0.025, 0.03, 0.035, 0.04, 0.05, 0.06, 0.07, 0.08, 0.10,
    ];
    let expiries = vec![0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 15.0, 20.0];
    let flat_row = vec![v; strikes.len()];
    let mut builder = VolSurface::builder(CurveId::new("USD-FLAT-VOL"))
        .expiries(&expiries)
        .strikes(&strikes);
    for _ in 0..expiries.len() {
        builder = builder.row(&flat_row);
    }
    mkt = mkt.insert_surface(builder.build().unwrap());
    mkt
}

/// Create a single-period CMS option using the single-curve market.
fn single_curve_cms(
    fixing: Date,
    payment: Date,
    strike_rate: f64,
    cms_tenor: f64,
    option_type: OptionType,
) -> CmsOption {
    CmsOption {
        id: InstrumentId::new("CMS-TEST"),
        strike: Decimal::try_from(strike_rate).expect("valid strike"),
        cms_tenor,
        fixing_dates: vec![fixing],
        payment_dates: vec![payment],
        accrual_fractions: vec![1.0],
        option_type,
        notional: Money::new(1.0, Currency::USD),
        day_count: DayCount::Act365F,
        swap_convention: None,
        swap_fixed_freq: Some(Tenor::semi_annual()),
        swap_float_freq: Some(Tenor::quarterly()),
        // Same day count for both legs so forward rate equals OIS rate exactly.
        swap_day_count: Some(DayCount::Act365F),
        swap_float_day_count: Some(DayCount::Act365F),
        // Both legs use the same "USD-FLAT" curve so the single-curve path is taken.
        discount_curve_id: CurveId::new("USD-FLAT"),
        forward_curve_id: CurveId::new("USD-FLAT"),
        vol_surface_id: CurveId::new("USD-FLAT-VOL"),
        pricing_overrides: PricingOverrides::default(),
        attributes: Default::default(),
    }
}

/// Price a `CmsOption` using `CmsReplicationPricer` directly.
fn replication_price(inst: &CmsOption, mkt: &MarketContext, as_of: Date) -> f64 {
    CmsReplicationPricer::new()
        .price_dyn(inst, mkt, as_of)
        .expect("replication pricing should succeed")
        .value
        .amount()
}

// ─── tests ───────────────────────────────────────────────────────────────────

/// A CMS floorlet must have strictly positive PV.
#[test]
fn test_cms_replication_floorlet_positive() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let fixing = Date::from_calendar_date(2026, Month::July, 1).unwrap();
    let payment = Date::from_calendar_date(2026, Month::October, 1).unwrap();
    let mkt = single_curve_market(as_of, 0.03, 0.20);

    let floor = single_curve_cms(fixing, payment, 0.03, 10.0, OptionType::Put);
    let pv = replication_price(&floor, &mkt, as_of);

    assert!(
        pv > 0.0 && pv.is_finite(),
        "CMS floorlet PV must be strictly positive and finite; got {pv:.8}"
    );
}

/// **Core sign test**: V_floor_replication < boundary.
///
/// The replication floorlet is `V_floor = boundary − ∫_{K_min}^K g'(k)·C_sw(k) dk`.
/// Because `g'(k) > 0` (g is strictly increasing) the integral is strictly
/// positive, so the correct (−) sign gives `V_floor < boundary`.
///
/// With the **correct** (−) sign: `boundary − δ < boundary`. ✓
/// With the **wrong** (+) sign:   `boundary + δ > boundary`. ✗
///
/// The boundary is the annuity-consistent `g(K)·C_sw(K) = DF_pay·Black76_put(K)`
/// (audit item 12), computed independently here from `DF_pay` and Black-76 so
/// the assertion is self-contained.
///
/// ## Parameters chosen to maximise δ/boundary ratio
///
/// - Strike K = 3% = F_swap (ATM), vol = 40%, T = 5Y, tenor = 20Y.
///   High vol and long tenor maximise the integral `∫g'·P_sw dk`.
/// - For a 20Y CMS at 3% flat, `par_annuity(0.03, 20, 2) ≈ 14.87`,
///   `g'(k) ≈ 0.20/par_ann(k)²` is significant over [0, K].
/// - At vol=40%, T=5: σ·F·√T ≈ 0.40·0.03·2.24 = 0.027, so the 6σ range is
///   wide enough for a large δ_floor.
#[test]
fn test_cms_replication_floorlet_below_boundary() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    // 5-year option on 20Y CMS, single-curve 3%, vol 40%
    let fixing = Date::from_calendar_date(2030, Month::January, 2).unwrap();
    let payment = Date::from_calendar_date(2030, Month::April, 3).unwrap();
    let mkt = single_curve_market(as_of, 0.03, 0.40);

    let strike = 0.03_f64;
    let cms_tenor = 20.0_f64;
    let m = 2.0_f64; // semi-annual fixed payments

    let floor = single_curve_cms(fixing, payment, strike, cms_tenor, OptionType::Put);
    let v_floor = replication_price(&floor, &mkt, as_of);

    // Compute the boundary g(K)·C_sw(K) independently.
    //
    // Annuity-consistency (audit item 12): the static-replication boundary is
    // `g(k)·C_sw(k)` with `g(k) = DF_pay/A_par(k)` and the swaption price
    // expressed on the SAME closed-form par annuity, `C_sw(k) =
    // A_par(k)·Black76(F,k,σ,T)`. The annuity cancels cleanly:
    //
    //   boundary = g(K)·C_sw(K) = (DF_pay/A_par(K))·(A_par(K)·Black76) = DF_pay·Black76
    //
    // The pre-item-12 pricer used the *market* annuity `A₀` for `C_sw` while
    // dividing by `A_par(K)` in `g`, leaving a spurious `A₀/A_par(K)` residual.
    // `par_annuity` / `a0` are therefore no longer part of the boundary; they
    // are retained below only for the diagnostic print.
    //
    // Time to fixing (Act365F from 2025-01-01 to 2030-01-02):
    let t_fix = 5.004_f64; // ≈ 5 years + 1 day in Act365F
                           // DF to payment date (Act365F continuous at 3%):
    let t_pay = 5.252_f64; // ≈ 5.25 years to 2030-04-03
    let df_pay = (-0.03_f64 * t_pay).exp();
    let a_par_k = par_annuity(strike, cms_tenor, m); // diagnostic only
    let a0: f64 = (1..=40)
        .map(|i| 0.5 * (-0.03 * (t_fix + 0.5 * i as f64)).exp())
        .sum(); // diagnostic only
    // Black-76 ATM put: vol=40%, T=t_fix ≈ 5Y, F=K=3%.
    let p_sw_k = b76_put(strike, strike, 0.40, t_fix);
    // Annuity-consistent boundary: g(K)·C_sw(K) = DF_pay·Black76_put(K).
    let boundary = df_pay * p_sw_k;

    println!(
        "20Y CMS ATM floor test (vol=40%, T=5Y):\n  \
         v_floor={v_floor:.8}  boundary=DF_pay·Black76_put(K)={boundary:.8}\n  \
         df_pay={df_pay:.6}  A₀(diag)={a0:.4}  A_par(K)(diag)={a_par_k:.4}\n  \
         Black76_put(K)={p_sw_k:.8}"
    );

    // The CMS replication floorlet must be strictly below the boundary term.
    // With the wrong (+) sign in the integrand the formula gives boundary + δ > boundary,
    // so this assertion fails. With the correct (−) sign it gives boundary − δ < boundary.
    assert!(
        v_floor < boundary,
        "CMS replication floorlet must satisfy V_floor < g(K)·P_sw(K) (boundary); \
         got v_floor={v_floor:.8} >= boundary={boundary:.8}. \
         This indicates the wrong (+) integral sign in the floorlet branch of \
         CmsReplicationPricer. The correct formula is g(K)·P_sw(K) − ∫g'·P_sw dk."
    );
}

/// CMS put-call spread consistency check.
///
/// At ATM (K = F_swap) the AP formula gives:
///
/// ```text
/// V_cap − V_floor_correct  = δ_cap + δ_floor   (correct sign)
/// V_cap − V_floor_wrong    = δ_cap − δ_floor   (wrong sign)
/// ```
///
/// where `δ_cap = ∫g'·C_sw dk` and `δ_floor = ∫g'·P_sw dk` are both positive.
///
/// Equivalently: `(V_cap − V_floor) - (V_cap - boundary) = δ_floor_term`.
///   - correct sign: `+δ_floor` → spread > V_cap - boundary
///   - wrong sign:   `−δ_floor` → spread < V_cap - boundary
///
/// We use `boundary = g(K)·P_sw(K)` (the independently computed boundary term).
/// At ATM, `g(K)·C_sw(K) = g(K)·P_sw(K) = boundary`, so:
///   `V_cap - boundary = δ_cap`
///
/// Therefore the assertion `spread > V_cap - boundary = δ_cap` becomes:
///   - correct sign: `δ_cap + δ_floor > δ_cap` → TRUE  ✓
///   - wrong sign:   `δ_cap − δ_floor > δ_cap` → FALSE ✗ (since δ_floor > 0)
///
/// Numerically for 20Y CMS, vol=40%, T=5Y: δ_cap ≈ 0.00345, δ_floor ≈ 0.00057.
#[test]
fn test_cms_replication_spread_exceeds_cap_integral() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let fixing = Date::from_calendar_date(2030, Month::January, 2).unwrap();
    let payment = Date::from_calendar_date(2030, Month::April, 3).unwrap();
    // High vol (40%) and long tenor (20Y) to make δ_floor significant.
    let mkt = single_curve_market(as_of, 0.03, 0.40);

    let strike = 0.03_f64;
    let cms_tenor = 20.0_f64;
    let t_fix = 5.004_f64;
    let t_pay = 5.252_f64;
    let m = 2.0_f64;
    let vol = 0.40_f64;

    let cap = single_curve_cms(fixing, payment, strike, cms_tenor, OptionType::Call);
    let floor = single_curve_cms(fixing, payment, strike, cms_tenor, OptionType::Put);

    let v_cap = replication_price(&cap, &mkt, as_of);
    let v_floor = replication_price(&floor, &mkt, as_of);
    let spread = v_cap - v_floor;

    // Recompute the boundary using the same annuity-consistent convention as
    // the pricer (audit item 12): g(K)·C_sw(K) = DF_pay·Black76(K), with the
    // par annuity cancelling. `par_annuity`/`a0` are unused for the boundary.
    let df_pay = (-0.03_f64 * t_pay).exp();
    let _ = (par_annuity(strike, cms_tenor, m), cms_tenor, m); // not used in the boundary
    // At ATM (K = F_swap = 3%), Black76_put(K) = Black76_call(K), so the cap
    // and floor boundaries coincide.
    let p_sw_k = b76_put(strike, strike, vol, t_fix);
    let boundary = df_pay * p_sw_k; // = g(K)·C_sw(K) at ATM

    // δ_cap = V_cap − boundary_cap = V_cap − boundary (at ATM)
    let delta_cap = v_cap - boundary;

    println!(
        "Spread vs δ_cap test (20Y CMS, vol=40%, T=5Y):\n  \
         V_cap={v_cap:.8}  V_floor={v_floor:.8}  spread={spread:.8}\n  \
         boundary={boundary:.8}  δ_cap={delta_cap:.8}"
    );

    assert!(v_cap > 0.0, "caplet must be positive, got {v_cap:.8}");
    assert!(v_floor > 0.0, "floorlet must be positive, got {v_floor:.8}");
    assert!(
        delta_cap > 0.0,
        "δ_cap = V_cap − g(K)·P_sw(K) must be positive; got {delta_cap:.8}"
    );

    // With correct (−) sign: spread = δ_cap + δ_floor > δ_cap.
    // With wrong (+) sign:   spread = δ_cap − δ_floor < δ_cap.
    assert!(
        spread > delta_cap,
        "AP cap-floor spread must exceed δ_cap = V_cap − g(K)·P_sw(K) = {delta_cap:.8}; \
         got spread={spread:.8}  [cap={v_cap:.8}  floor={v_floor:.8}  boundary={boundary:.8}]. \
         This indicates the wrong (+) integral sign in the CmsReplicationPricer floorlet branch: \
         with the correct (−) sign the spread is δ_cap + δ_floor > δ_cap."
    );
}

/// Regression test (audit item 12): the static-replication boundary term must
/// be annuity-consistent.
///
/// The boundary `g(K)·C_sw(K)` collapses to `DF_pay·Black76(F,K,σ,T)` — the
/// closed-form par annuity `A_par` cancels between `g(k) = DF_pay/A_par(k)` and
/// `C_sw(k) = A_par(k)·Black76(k)`. The pre-item-12 code paired `g` with the
/// *market*-annuity swaption `A₀·Black76`, leaving a spurious `A₀/A_par(K)`
/// residual that scaled the whole price by `A₀/A_par(F)` (far from 1 for a long
/// CMS tenor).
///
/// The test compares the static-replication price against the Hagan
/// first-order pricer (`CmsOptionPricer`, `ModelKey::Black76`). For a
/// near-immediate fixing (~3 days), the convexity adjustment and the
/// replication integral are both negligible, so both pricers reduce to the
/// SAME discounted Black-76 swaption-rate option:
///
/// ```text
/// V ≈ DF_pay · Black76(F, K, σ, T) · accrual · notional
/// ```
///
/// With the corrected (annuity-cancelled) boundary the two prices agree
/// closely. With the pre-fix `A₀` convention the static-replication price was
/// off by `A₀/A_par(F)` while the Hagan pricer was not — so they disagreed
/// sharply. Comparing the two pricers avoids hard-coding the exact forward
/// swap rate or discount factor.
#[test]
fn test_cms_replication_boundary_is_annuity_consistent() {
    use finstack_valuations::instruments::Instrument;

    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    // Near-immediate fixing: ~3 days out, so both the convexity adjustment and
    // the replication integral are negligible.
    let fixing = Date::from_calendar_date(2025, Month::January, 4).unwrap();
    let payment = Date::from_calendar_date(2025, Month::April, 4).unwrap();
    let mkt = single_curve_market(as_of, 0.03, 0.20);

    let strike = 0.03_f64; // ~ATM (forward ≈ 3% on the flat 3% curve)
    let cms_tenor = 10.0_f64;

    let cap = single_curve_cms(fixing, payment, strike, cms_tenor, OptionType::Call);

    // Static-replication price (the pricer under test for item 12).
    let v_replication = replication_price(&cap, &mkt, as_of);

    // Hagan first-order price via the default Black-76 CMS pricer. For a
    // near-immediate fixing the convexity adjustment ~ 0, so this equals
    // DF_pay·Black76(F,K)·accrual·notional — the same annuity-cancelled
    // boundary the corrected static replication must produce.
    let v_hagan = cap.value(&mkt, as_of).expect("hagan pricing").amount();

    println!(
        "Item 12 boundary test: v_replication={v_replication:.10}  v_hagan={v_hagan:.10}"
    );

    assert!(
        v_replication > 0.0 && v_hagan > 0.0,
        "both CMS caplet prices must be positive"
    );
    // The two pricers must agree closely for a near-immediate near-ATM caplet.
    // The pre-fix `A₀/A_par(F)` residual scaled the static-replication price by
    // a 10Y-CMS annuity ratio far from 1, breaking this agreement.
    let rel = (v_replication - v_hagan).abs() / v_hagan.max(1e-12);
    assert!(
        rel < 0.05,
        "static-replication and Hagan CMS caplet prices must agree for a \
         near-immediate near-ATM fixing (both reduce to DF_pay·Black76); got \
         v_replication={v_replication:.10}, v_hagan={v_hagan:.10} (rel diff {rel:.4}). \
         A large gap indicates the spurious A₀/A_par(K) residual in the \
         static-replication boundary."
    );
}
