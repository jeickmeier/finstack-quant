//! Cross-implementation parity guardrails.
//!
//! The audit's dominant root cause was *parallel pricing pathways drifting out
//! of sync*: a bug in one pricer went unnoticed because a sibling pricer for
//! the same instrument did the calculation correctly. The strategic fix is a
//! suite of cross-implementation parity tests that pin one code path against a
//! genuinely-independent sibling, so any future divergence fails a test.
//!
//! Each test here is a **genuine** cross-check — two different code paths that
//! must agree — not a tautology and not the same path priced twice. The
//! tolerances for Monte-Carlo comparisons are the combined MC standard error,
//! not arbitrarily-loose bands.
//!
//! Tests added here (the parity checks not already covered elsewhere):
//!
//! 1. [`gbm_barrier`] — GBM barrier MC vs Heston barrier MC in the degenerate
//!    limit where the Heston variance process collapses to constant variance
//!    (`v0 = θ = σ²`, `σ_v → 0`, `ρ = 0`), i.e. Heston ≡ GBM.
//! 2. [`sabr_beta`] — SABR implied-vol cross-consistency across the β branches
//!    (β=0 normal, interior β, β=1 lognormal), guarding the Hagan
//!    `(1−β)`-exponent fix.
//! 3. [`asian_geometric`] — geometric-average Asian option: registered MC GBM
//!    pricer vs the registered Kemna-Vorst closed-form pricer.
//!
//! Parity tests added by *earlier* remediation tasks are intentionally **not**
//! duplicated here; they are verified to still pass and referenced for the
//! record:
//!
//! * `tests/instruments/exotic_harness/tarn_tree_parity.rs` — HW1F MC vs
//!   trinomial-tree parity for a TARN floating note (Task 15).
//! * `tests/instruments/structured_credit/unit/per_name_copula_tests.rs`
//!   (`large_granular_pool_per_name_matches_lhp`, …) — per-name Gaussian-copula
//!   default-simulation MC vs the large-homogeneous-pool (LHP) limit (Task 17).
//! * `finstack/valuations/src/instruments/rates/snowball/pricer.rs`
//!   (`deterministic_mc_snowball_matches_discounted_coupon_formula`) — HW1F MC
//!   in the σ→0 limit vs the discounted in-advance coupon formula (Task 21).

#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

/// GBM barrier MC vs Heston barrier MC, degenerate (Heston collapses to GBM).
///
/// # The two code paths
///
/// * `BarrierOptionMcPricer` (`ModelKey::MonteCarloGBM`) — simulates a
///   [`finstack_monte_carlo::process::gbm::GbmProcess`] with the
///   exact-lognormal discretization and a [`BarrierOptionPayoff`].
/// * `BarrierOptionHestonMcPricer` (`ModelKey::MonteCarloHeston`) — simulates a
///   two-factor [`finstack_monte_carlo::process::heston::HestonProcess`] with
///   the QE discretization, and the *same* `BarrierOptionPayoff`.
///
/// These are genuinely different engines: a one-factor exact-GBM stepper vs a
/// two-factor QE stochastic-volatility stepper.
///
/// # Why they must agree here
///
/// The Heston SDE is
/// ```text
/// dS = (r-q)S dt + √v S dW₁,   dv = κ(θ-v) dt + σᵥ √v dW₂,   dW₁·dW₂ = ρ dt.
/// ```
/// Set `v₀ = θ = σ²`, `σᵥ → 0`, `ρ = 0`. The variance process then has no
/// diffusion and starts already at its mean, so `v_t ≡ σ²` for all `t`. The
/// spot SDE collapses to `dS = (r-q)S dt + σ S dW₁` — exactly GBM with constant
/// log-volatility `σ`. At the QE-discretization level the degenerate spot
/// update `S·exp((r-q)dt − ½σ²dt + σ√dt·z)` is algebraically identical to the
/// exact-GBM step, so the two pricers must price the *same* barrier option to
/// within Monte-Carlo noise.
///
/// # What regression this guards
///
/// A drift in either barrier MC pricer — a wrong drift sign, a missing
/// dividend, a mis-mapped barrier type, an off-by-one in the maturity step, a
/// botched discount factor — would break this equality even though each pricer
/// might still look internally plausible. The Heston pricer is the *only* SV
/// barrier pricer, so without this test its degenerate limit is unchecked.
///
/// # Tolerance
///
/// Both pricers price the same option with the same path count (100 000) and
/// step density (252/yr), so their per-path payoff variances are essentially
/// equal: `seᴳ ≈ seᴴ`. The two estimates use independent RNG streams (the
/// path-dependent pricer's stream vs Philox) and independent seeds, so the
/// difference has standard error `√(seᴳ² + seᴴ²) ≈ seᴴ·√2`. We take a 4·√2·seᴴ
/// band: a 4-sigma bound (false-failure probability ≈ 6·10⁻⁵) that is still far
/// tighter than any structural mis-pricing, which would be of order the price
/// itself (~several units, vs a tolerance of ~0.1).
mod gbm_barrier {
    use finstack_core::currency::Currency;
    use finstack_core::dates::{Date, DayCount};
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::scalars::MarketScalar;
    use finstack_core::market_data::surfaces::VolSurface;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::money::Money;
    use finstack_core::types::InstrumentId;
    use finstack_valuations::instruments::exotics::barrier_option::{BarrierOption, BarrierType};
    use finstack_valuations::instruments::{Attributes, OptionType, PricingOverrides};
    use finstack_valuations::metrics::MetricId;
    use finstack_valuations::pricer::{standard_registry, InstrumentType, ModelKey, PricerKey};
    use time::Month;

    const SPOT: f64 = 100.0;
    const STRIKE: f64 = 100.0;
    const RATE: f64 = 0.04;
    const DIV_YIELD: f64 = 0.0;
    /// Black-Scholes log-volatility. The degenerate Heston long-run and initial
    /// variances are both `VOL²` so the SV model has constant variance `VOL²`.
    const VOL: f64 = 0.20;

    fn date(y: i32, m: Month, d: u8) -> Date {
        Date::from_calendar_date(y, m, d).expect("valid date")
    }

    /// Market with a flat vol surface at `VOL` and Heston scalars set so the
    /// variance process is (near-)deterministic and equal to the GBM variance:
    /// `HESTON_V0 = HESTON_THETA = VOL²`, `HESTON_SIGMA_V ≈ 0`, `HESTON_RHO = 0`.
    ///
    /// `HESTON_SIGMA_V` cannot be exactly zero — `HestonParams::new` rejects a
    /// non-positive vol-of-vol — so we use `1e-8`. The variance then wanders by
    /// at most `O(1e-8)` per step, utterly negligible against MC noise.
    fn market(as_of: Date) -> MarketContext {
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (5.0, (-RATE * 5.0).exp())])
            .build()
            .expect("discount curve");
        let surface = VolSurface::builder("SPX-VOL")
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .strikes(&[70.0, 85.0, 100.0, 115.0, 130.0])
            .row(&[VOL; 5])
            .row(&[VOL; 5])
            .row(&[VOL; 5])
            .row(&[VOL; 5])
            .build()
            .expect("vol surface");

        // Degenerate Heston parameters. v0 = theta = VOL^2 pins the variance at
        // the GBM variance; sigma_v ~ 0 removes vol-of-vol; rho = 0 removes the
        // spot/variance correlation (and hence the QE variance-correction term).
        let variance = VOL * VOL;
        MarketContext::new()
            .insert(discount)
            .insert_surface(surface)
            .insert_price("SPX", MarketScalar::Price(Money::new(SPOT, Currency::USD)))
            .insert_price("SPX-DIV", MarketScalar::Unitless(DIV_YIELD))
            .insert_price("HESTON_V0", MarketScalar::Unitless(variance))
            .insert_price("HESTON_THETA", MarketScalar::Unitless(variance))
            .insert_price("HESTON_SIGMA_V", MarketScalar::Unitless(1.0e-8))
            .insert_price("HESTON_RHO", MarketScalar::Unitless(0.0))
            .insert_price("HESTON_KAPPA", MarketScalar::Unitless(2.0))
    }

    /// A genuinely-active down-and-out call: barrier `85` sits below the `100`
    /// spot, so a non-trivial fraction of paths knock out. The barrier is
    /// monitored by the *same* `BarrierOptionPayoff` (same 252/yr grid, same
    /// Brownian-bridge correction) under both pricers, so the barrier treatment
    /// cancels and the test isolates the spot *process*.
    fn down_and_out_call(expiry: Date) -> BarrierOption {
        BarrierOption {
            id: InstrumentId::new("BARRIER-GBM-HESTON-PARITY"),
            underlying_ticker: "SPX".to_string(),
            strike: STRIKE,
            barrier: Money::new(85.0, Currency::USD),
            rebate: None,
            rebate_timing: Default::default(),
            option_type: OptionType::Call,
            barrier_type: BarrierType::DownAndOut,
            expiry,
            observed_barrier_breached: None,
            notional: Money::new(1.0, Currency::USD),
            day_count: DayCount::Act365F,
            // false: no extra Gobet-Miri barrier shift. Both pricers still apply
            // the same Brownian-bridge hit correction inside the shared payoff;
            // keeping the flag identical is what makes the barrier cancel.
            use_gobet_miri: false,
            discount_curve_id: "USD-OIS".into(),
            spot_id: "SPX".into(),
            vol_surface_id: "SPX-VOL".into(),
            div_yield_id: Some("SPX-DIV".into()),
            pricing_overrides: PricingOverrides::default(),
            monitoring_frequency: None,
            attributes: Attributes::new(),
        }
    }

    /// GBM barrier MC vs degenerate-Heston barrier MC must agree within the
    /// combined Monte-Carlo standard error.
    #[test]
    fn gbm_barrier_mc_matches_degenerate_heston_barrier_mc() {
        let as_of = date(2025, Month::January, 1);
        let expiry = date(2026, Month::January, 1); // 1 year
        let market = market(as_of);
        let option = down_and_out_call(expiry);

        let registry = standard_registry();

        // --- GBM barrier MC (ModelKey::MonteCarloGBM) ---------------------------
        let gbm_pricer = registry
            .get_pricer(PricerKey::new(
                InstrumentType::BarrierOption,
                ModelKey::MonteCarloGBM,
            ))
            .expect("GBM barrier pricer is registered");
        let gbm_pv = gbm_pricer
            .price_dyn(&option, &market, as_of)
            .expect("GBM barrier MC price")
            .value
            .amount();

        // --- Heston barrier MC, degenerate (ModelKey::MonteCarloHeston) ---------
        let heston_pricer = registry
            .get_pricer(PricerKey::new(
                InstrumentType::BarrierOption,
                ModelKey::MonteCarloHeston,
            ))
            .expect("Heston barrier pricer is registered");
        let heston_result = heston_pricer
            .price_dyn(&option, &market, as_of)
            .expect("Heston barrier MC price");
        let heston_pv = heston_result.value.amount();
        // The Heston barrier pricer publishes its MC standard error as a custom
        // measure; this is the genuine, pricer-reported error used to size the
        // tolerance (no hand-tuned constant).
        let heston_se = *heston_result
            .measures
            .get(&MetricId::custom("mc_stderr"))
            .expect("Heston barrier MC publishes an mc_stderr measure");

        // Both pricers are positive and finite (ITM call, active barrier).
        assert!(
            gbm_pv.is_finite() && gbm_pv > 0.0,
            "GBM barrier PV must be finite and positive, got {gbm_pv}"
        );
        assert!(
            heston_pv.is_finite() && heston_pv > 0.0,
            "Heston barrier PV must be finite and positive, got {heston_pv}"
        );

        // Combined standard error of the difference of two independent MC
        // estimators with (near-)equal per-path variance: √(seᴳ²+seᴴ²) ≈ √2·seᴴ.
        let combined_se = heston_se * std::f64::consts::SQRT_2;
        let tol = 4.0 * combined_se;
        let diff = (gbm_pv - heston_pv).abs();

        println!(
            "barrier GBM↔Heston(degenerate): gbm={gbm_pv:.6}  heston={heston_pv:.6} \
             (seᴴ={heston_se:.6})  |Δ|={diff:.6}  tol={tol:.6}"
        );

        assert!(
            diff < tol,
            "GBM barrier MC and degenerate-Heston barrier MC disagree: \
             gbm={gbm_pv:.6}, heston={heston_pv:.6}, |Δ|={diff:.6} > tol={tol:.6} \
             (4·√2·seᴴ). With v0=θ=σ² and σᵥ→0 the Heston variance is constant \
             and equal to the GBM variance, so the two pricers must agree to \
             within MC noise — a gap this large indicates a genuine drift \
             between the GBM and Heston barrier pricing pathways."
        );
    }
}

/// SABR implied-volatility cross-consistency across the β branches.
///
/// # The code paths being cross-checked
///
/// [`SABRModel::implied_volatility`] has three structurally distinct branches
/// selected by the CEV exponent β:
///
/// * **β = 0** — the *normal* (Bachelier) branch: `f_mid^(1-β)` is forced to
///   `1`, the log-moneyness `z` uses an arithmetic difference, and the
///   time-correction term drops the `(1-β)²α²` and `ρβνα` pieces.
/// * **0 < β < 1** — the *general CEV* branch with the Obloj (2008)
///   geometric-mean `z`-correction.
/// * **β = 1** — the *lognormal* branch: `f_mid^(1-β) = 1`, `z` uses
///   `ln(F/K)`, and the `(1-β)²` time-correction pieces vanish.
///
/// These are genuinely different formulas, not a parametrized single path. A
/// regression in one branch does not show up in the others — exactly the
/// drift-between-pathways pattern the audit flagged.
///
/// # What regression this guards
///
/// The audit's M1 fix corrected the Hagan `(1-β)` exponent in the volatility
/// denominator. Before the fix the β=1 branch was *wrong* (it used `F^β = F`
/// instead of `F^(1-β) = 1`, so ATM vol came out as `α/F` rather than `α`)
/// while the β=0.5 branch was *accidentally correct*. A single-branch unit test
/// on β=0.5 would have passed throughout. A **cross-β** consistency check is
/// the structural guardrail against that whole class of regression: it asserts
/// the branches agree where the SABR model says they must.
///
/// The genuine `SABRModel` (the production pricer type) is exercised — not a
/// re-implementation.
mod sabr_beta {
    use finstack_valuations::models::{SABRModel, SABRParameters};

    /// **β=1 ATM, ν=0 ⇒ vol = α, for any forward.**
    ///
    /// With β=1, ν=0, ρ=0 the SABR dynamics reduce to `dF = αF dW` — pure GBM
    /// with constant log-vol α — so the ATM implied vol is exactly α and is
    /// *independent of the forward level*. The pre-M1-fix code returned `α/F`,
    /// which equals α only at `F = 1`; sweeping the forward over four orders of
    /// magnitude makes that bug fail loudly. This pins the β=1 branch.
    #[test]
    fn beta_one_atm_vol_equals_alpha_for_any_forward() {
        let alpha = 0.22_f64;
        let params =
            SABRParameters::new(alpha, 1.0, 0.0, 0.0).expect("β=1, ν=0 SABR params are valid");
        let model = SABRModel::new(params);

        for &forward in &[0.02_f64, 1.0, 100.0, 4_000.0] {
            let vol = model
                .implied_volatility(forward, forward, 1.0)
                .expect("β=1 ATM vol should compute");
            assert!(
                (vol - alpha).abs() < 1e-10,
                "β=1, ν=0 ATM SABR vol must equal α={alpha} independent of the \
                 forward; got {vol} at F={forward}. A forward-dependent result \
                 is the signature of the pre-M1 Hagan-exponent bug (α/F)."
            );
        }
    }

    /// **The off-ATM branch limits to the ATM branch across all three β
    /// branches.**
    ///
    /// `SABRModel::implied_volatility` contains *two* internal code paths: an
    /// ATM short-circuit (taken when forward and strike are within a relative
    /// `1e-8`) and the full off-ATM `z/χ(z)` expansion. Pricing exactly ATM
    /// (`K = F`) takes the short-circuit; pricing a hair away (`K = F·(1±ε)`
    /// with `ε ≫ 1e-8`) takes the off-ATM path. As `ε → 0` the off-ATM result
    /// must converge to the ATM result. We assert this for β ∈ {0, 0.5, 1} and
    /// for strikes bracketing the forward from both sides. A failure means the
    /// off-ATM and ATM code paths for that β branch disagree in the limit — a
    /// structural inconsistency within the branch.
    #[test]
    fn off_atm_vol_converges_to_atm_branch_across_beta_branches() {
        let forward = 100.0_f64;
        let expiry = 1.5_f64;
        // alpha is scaled per beta so each branch produces a sensible vol:
        // for beta<1 the vol scales like alpha / F^(1-beta).
        for &(beta, alpha) in &[
            (0.0_f64, 0.20_f64 * forward), // normal branch: alpha is a price vol
            (0.5_f64, 0.20_f64 * forward.sqrt()),
            (1.0_f64, 0.20_f64),
        ] {
            let params =
                SABRParameters::new(alpha, beta, 0.35, -0.25).expect("SABR params should be valid");
            let model = SABRModel::new(params);

            // Exactly ATM: routed through the ATM short-circuit path.
            let atm = model
                .implied_volatility(forward, forward, expiry)
                .expect("ATM vol should compute");
            // Strikes a hair away from the forward, from both sides: each is
            // routed through the off-ATM z/χ(z) branch, but close enough that
            // it must collapse to the ATM value.
            for &eps in &[1e-6_f64, -1e-6_f64] {
                let near_atm = model
                    .implied_volatility(forward, forward * (1.0 + eps), expiry)
                    .expect("near-ATM vol should compute");
                let rel = (near_atm - atm).abs() / atm;
                assert!(
                    rel < 1e-4,
                    "β={beta}: near-ATM (K=F·(1+{eps:e})) implied vol \
                     ({near_atm}) must converge to the ATM-branch vol ({atm}); \
                     relative gap {rel:.2e}. A mismatch means the off-ATM and \
                     ATM code paths for this β branch disagree in the limit."
                );
            }
        }
    }

    /// **β monotone-continuity: the smile deforms smoothly as β varies.**
    ///
    /// Holding the ATM vol fixed (by re-scaling α so `α/F^(1-β)` is constant),
    /// the implied vol at a fixed off-ATM strike must vary *continuously* with
    /// β — no jump as β crosses from the general branch into the β=1 special
    /// case. We sample β on a fine grid through β=1 and assert successive vols
    /// differ by less than a small bound. A discontinuity at β≈1 would betray
    /// an inconsistency between the general-CEV branch and the β=1 branch — the
    /// precise failure mode the M1 fix addressed.
    #[test]
    fn implied_vol_is_continuous_in_beta_through_the_lognormal_branch() {
        let forward = 100.0_f64;
        let strike = 115.0_f64;
        let expiry = 1.0_f64;
        let nu = 0.30_f64;
        let rho = -0.20_f64;
        // Target ATM vol level; alpha(beta) keeps alpha / F^(1-beta) constant
        // so the only thing moving is the *shape* contributed by beta.
        let atm_target = 0.20_f64;

        let vol_at = |beta: f64| -> f64 {
            let alpha = atm_target * forward.powf(1.0 - beta);
            let params =
                SABRParameters::new(alpha, beta, nu, rho).expect("SABR params should be valid");
            SABRModel::new(params)
                .implied_volatility(forward, strike, expiry)
                .expect("vol should compute")
        };

        // Fine β grid straddling the β=1 lognormal special-case boundary. The
        // model clamps |1-β| < 1e-4 to exactly β=1, so values just below 1
        // exercise the general branch and β=1 exercises the special case.
        let betas = [0.90, 0.95, 0.98, 0.999, 1.0];
        let vols: Vec<f64> = betas.iter().map(|&b| vol_at(b)).collect();

        for w in vols.windows(2) {
            let step = (w[1] - w[0]).abs();
            assert!(
                step < 5e-3,
                "SABR implied vol must be continuous in β through the β=1 \
                 lognormal branch: a step of {step:.4} between adjacent β \
                 values ({vols:?}) signals a discontinuity between the \
                 general-CEV and β=1 code paths."
            );
        }

        // Sanity: an OTM strike with this skew genuinely produces a smile, so
        // the continuity check above is non-vacuous (the vols are not all
        // identical).
        let spread = vols.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            - vols.iter().cloned().fold(f64::INFINITY, f64::min);
        assert!(
            spread > 1e-4,
            "expected a non-trivial β-dependence so the continuity test is \
             meaningful; got near-constant vols {vols:?}"
        );
    }

    /// **β=0 normal branch behaves correctly: ATM, ν=0 ⇒ vol = α.**
    ///
    /// With β=0 and ν=0 the SABR model is the pure Bachelier (normal) model
    /// with constant normal vol α, so the ATM implied (normal) vol is exactly
    /// α. This pins the normal branch — the branch that was *accidentally*
    /// fine before M1 — so a future regression there is caught too.
    #[test]
    fn beta_zero_normal_branch_atm_vol_equals_alpha() {
        // alpha here is a normal (absolute) vol, e.g. 2.0 price units.
        let alpha = 2.0_f64;
        let params =
            SABRParameters::new(alpha, 0.0, 0.0, 0.0).expect("β=0, ν=0 SABR params are valid");
        let model = SABRModel::new(params);

        for &forward in &[50.0_f64, 100.0, 250.0] {
            let vol = model
                .implied_volatility(forward, forward, 2.0)
                .expect("β=0 ATM vol should compute");
            assert!(
                (vol - alpha).abs() < 1e-10,
                "β=0, ν=0 ATM normal SABR vol must equal α={alpha}; got {vol} \
                 at F={forward}."
            );
        }
    }
}

/// Geometric-average Asian option: registered MC GBM pricer vs the registered
/// Kemna-Vorst closed-form pricer.
///
/// # The two registered pricers
///
/// The pricer registry holds two pricers for `InstrumentType::AsianOption` that
/// both apply to a *geometric*-averaging Asian:
///
/// * `AsianOptionMcPricer` (`ModelKey::MonteCarloGBM`) — simulates GBM paths
///   and evaluates a discrete geometric-average payoff.
/// * `AsianOptionAnalyticalGeometricPricer` (`ModelKey::AsianGeometricBS`) —
///   the Kemna-Vorst (1990) closed form: because the geometric average of
///   jointly-lognormal prices is itself lognormal, the option is priced as a
///   vanilla Black-Scholes option with an *adjusted* volatility and drift.
///
/// These are genuinely different code paths — a path-simulation engine vs an
/// analytic Black-Scholes formula with moment-matched parameters.
///
/// # Why they must agree
///
/// Unlike the *arithmetic* Asian (whose Turnbull-Wakeman analytic is only an
/// approximation), the *geometric* Asian closed form is **exact** for GBM: the
/// discrete geometric average is exactly lognormal. So the MC estimate must
/// converge to the closed-form price with no model bias — only Monte-Carlo
/// standard error.
///
/// To keep the comparison exact, the fixing dates are placed at `tᵢ = i·T/n`
/// for `i = 1..n` — precisely the equally-spaced schedule the Kemna-Vorst
/// discrete-fixing variance/drift adjustment assumes — and the MC step density
/// (252/yr) is an exact multiple of `n` (= 12) so every interior fixing lands
/// cleanly on its own simulation step. (The final fixing coincides with expiry
/// and is read one step early by the MC pricer; see *Residual* below.)
///
/// # What regression this guards
///
/// The geometric Asian closed form and the Asian MC engine are independent
/// implementations of the same payoff. A drift in either — a wrong
/// discrete-fixing variance adjustment in the closed form, a mis-indexed fixing
/// step or a botched log-sum in the MC payoff — would break this equality. The
/// MC geometric path is *not* used as a control variate for itself here (the
/// control-variate machinery only fires for *arithmetic* averaging), so this is
/// a genuine cross-check rather than a tautology.
///
/// # Tolerance
///
/// The geometric-Asian call payoff `DF·max(G−K, 0)` with `G` lognormal is a
/// bounded random variable; we estimate its per-path standard deviation from
/// the closed-form-implied lognormal distribution of `G` and form the MC
/// standard error `σ_payoff/√N`. The parity band is `4·se` — a 4-sigma bound.
///
/// # Residual: the maturity-fixing time grid
///
/// The Asian MC pricer maps each fixing date to an integer simulation step and
/// clamps the result to `num_steps − 1`, so the final fixing (which coincides
/// with expiry) is read at `T·(num_steps−1)/num_steps` rather than exactly `T`.
/// With `num_steps = 252` (one year) this shifts one of the twelve fixings by
/// ~0.4% of the averaging window. The resulting change in `Var[ln G]` is
/// `O(1/(n·num_steps))` — empirically far below one MC standard error — so it
/// is absorbed many times over by the 4-sigma band and does not weaken the
/// guardrail: a genuine pricer drift moves the price by percent-level amounts,
/// i.e. several `se`.
mod asian_geometric {
    use finstack_core::currency::Currency;
    use finstack_core::dates::{Date, DayCount, DayCountContext};
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::scalars::MarketScalar;
    use finstack_core::market_data::surfaces::VolSurface;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::money::Money;
    use finstack_core::types::InstrumentId;
    use finstack_valuations::instruments::exotics::asian_option::{AsianOption, AveragingMethod};
    use finstack_valuations::instruments::{Attributes, OptionType, PricingOverrides};
    use finstack_valuations::pricer::{standard_registry, InstrumentType, ModelKey, PricerKey};
    use time::Month;

    const SPOT: f64 = 100.0;
    const STRIKE: f64 = 100.0;
    const RATE: f64 = 0.05;
    const DIV_YIELD: f64 = 0.0;
    const VOL: f64 = 0.25;
    /// Number of equally-spaced fixings. Twelve monthly fixings over one year.
    const N_FIXINGS: usize = 12;

    fn date(y: i32, m: Month, d: u8) -> Date {
        Date::from_calendar_date(y, m, d).expect("valid date")
    }

    fn market(as_of: Date) -> MarketContext {
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (3.0, (-RATE * 3.0).exp())])
            .build()
            .expect("discount curve");
        let surface = VolSurface::builder("SPX-VOL")
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .strikes(&[70.0, 85.0, 100.0, 115.0, 130.0])
            .row(&[VOL; 5])
            .row(&[VOL; 5])
            .row(&[VOL; 5])
            .row(&[VOL; 5])
            .build()
            .expect("vol surface");
        MarketContext::new()
            .insert(discount)
            .insert_surface(surface)
            .insert_price("SPX", MarketScalar::Price(Money::new(SPOT, Currency::USD)))
            .insert_price("SPX-DIV", MarketScalar::Unitless(DIV_YIELD))
    }

    /// A geometric-average Asian call with `N_FIXINGS` fixings at `i·T/n`,
    /// `i = 1..n` — the last fixing coincides with expiry. This is exactly the
    /// schedule the Kemna-Vorst discrete adjustment is derived for.
    fn geometric_asian_call(as_of: Date, expiry: Date) -> AsianOption {
        // Twelve monthly fixing dates anchored at `as_of + i months`, i = 1..12,
        // i.e. equally spaced at i*T/12 of the one-year averaging window.
        let fixing_dates: Vec<Date> = (1..=N_FIXINGS).map(|i| add_months(as_of, i)).collect();

        AsianOption {
            id: InstrumentId::new("ASIAN-GEO-MC-ANALYTIC-PARITY"),
            underlying_ticker: "SPX".to_string(),
            strike: STRIKE,
            option_type: OptionType::Call,
            averaging_method: AveragingMethod::Geometric,
            expiry,
            fixing_dates,
            notional: Money::new(1.0, Currency::USD),
            day_count: DayCount::Act365F,
            discount_curve_id: "USD-OIS".into(),
            spot_id: "SPX".into(),
            vol_surface_id: "SPX-VOL".into(),
            div_yield_id: Some("SPX-DIV".into()),
            pricing_overrides: PricingOverrides::default(),
            attributes: Attributes::new(),
            past_fixings: Vec::new(),
        }
    }

    /// Add `n` calendar months to a date (used to build `i·T/12` fixing dates).
    fn add_months(d: Date, n: usize) -> Date {
        let mut year = d.year();
        let mut month0 = d.month() as i32 - 1 + n as i32;
        year += month0 / 12;
        month0 %= 12;
        let month = Month::try_from((month0 + 1) as u8).expect("valid month");
        Date::from_calendar_date(year, month, d.day()).expect("valid date")
    }

    /// Closed-form standard deviation of the discounted geometric-Asian call
    /// payoff, used purely to size the MC tolerance (not to price).
    ///
    /// `G` is lognormal: `ln G ~ N(m, s²)` with `s² = σ²T(n+1)(2n+1)/(6n²)`
    /// and `E[G] = F_G`. For `Y = DF·max(G−K,0)` we have
    /// `Var[Y] = DF²·(E[(G−K)²·1_{G>K}] − E[(G−K)·1_{G>K}]²)`,
    /// and the truncated lognormal moments have closed forms via the normal CDF.
    fn geometric_payoff_std(t: f64, df: f64, n: usize) -> f64 {
        let nf = n as f64;
        let var_lng = VOL * VOL * t * (nf + 1.0) * (2.0 * nf + 1.0) / (6.0 * nf * nf);
        let s = var_lng.sqrt();
        // Forward of the geometric average E[G] (consistent with the
        // Kemna-Vorst parametrization the analytical pricer uses).
        let rate = -df.ln() / t;
        let drift = (rate - DIV_YIELD - 0.5 * VOL * VOL) * (nf + 1.0) / (2.0 * nf)
            + VOL * VOL * (nf + 1.0) * (2.0 * nf + 1.0) / (12.0 * nf * nf);
        let fwd_g = SPOT * drift.exp();
        // ln G ~ N(m, s²) with E[G] = exp(m + s²/2) = fwd_g  ⇒  m = ln(fwd_g) - s²/2.
        let m = fwd_g.ln() - 0.5 * s * s;

        let n_cdf = |x: f64| {
            // Standard normal CDF via erfc.
            0.5 * libm_erfc(-x / std::f64::consts::SQRT_2)
        };
        let d1 = (m + s * s - STRIKE.ln()) / s;
        let d2 = d1 - s;
        // E[(G-K)·1_{G>K}] = E[G]·N(d1) - K·N(d2)
        let e1 = fwd_g * n_cdf(d1) - STRIKE * n_cdf(d2);
        // E[(G-K)²·1_{G>K}] = E[G²]·N(d1+s) - 2K·E[G]·N(d1) + K²·N(d2)
        let e_g2 = (2.0 * m + 2.0 * s * s).exp();
        let e2 =
            e_g2 * n_cdf(d1 + s) - 2.0 * STRIKE * fwd_g * n_cdf(d1) + STRIKE * STRIKE * n_cdf(d2);
        let var_payoff = (e2 - e1 * e1).max(0.0);
        df * var_payoff.sqrt()
    }

    /// `erfc` via the Abramowitz-Stegun 7.1.26 rational approximation
    /// (|error| < 1.5e-7) — adequate for sizing a 4-sigma MC tolerance.
    fn libm_erfc(x: f64) -> f64 {
        let z = x.abs();
        let tt = 1.0 / (1.0 + 0.5 * z);
        let ans = tt
            * (-z * z - 1.265_512_23
                + tt * (1.000_023_68
                    + tt * (0.374_091_96
                        + tt * (0.096_784_18
                            + tt * (-0.186_288_06
                                + tt * (0.278_868_07
                                    + tt * (-1.135_203_98
                                        + tt * (1.488_515_87
                                            + tt * (-0.822_152_23 + tt * 0.170_872_77)))))))))
                .exp();
        if x >= 0.0 {
            ans
        } else {
            2.0 - ans
        }
    }

    /// Geometric-Asian MC GBM pricer must match the Kemna-Vorst closed-form
    /// pricer within the Monte-Carlo standard error.
    #[test]
    fn geometric_asian_mc_matches_kemna_vorst_closed_form() {
        let as_of = date(2025, Month::January, 1);
        let expiry = add_months(as_of, N_FIXINGS); // 12 months ⇒ 1 year
        let market = market(as_of);
        let option = geometric_asian_call(as_of, expiry);

        let t = DayCount::Act365F
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction");
        let df = (-RATE * t).exp();

        let registry = standard_registry();

        // --- MC GBM pricer (ModelKey::MonteCarloGBM) ----------------------------
        let mc_pricer = registry
            .get_pricer(PricerKey::new(
                InstrumentType::AsianOption,
                ModelKey::MonteCarloGBM,
            ))
            .expect("Asian MC pricer is registered");
        let mc_pv = mc_pricer
            .price_dyn(&option, &market, as_of)
            .expect("Asian geometric MC price")
            .value
            .amount();

        // --- Kemna-Vorst closed-form pricer (ModelKey::AsianGeometricBS) --------
        let analytic_pricer = registry
            .get_pricer(PricerKey::new(
                InstrumentType::AsianOption,
                ModelKey::AsianGeometricBS,
            ))
            .expect("Asian geometric analytical pricer is registered");
        let analytic_pv = analytic_pricer
            .price_dyn(&option, &market, as_of)
            .expect("Asian geometric closed-form price")
            .value
            .amount();

        assert!(
            mc_pv.is_finite() && mc_pv > 0.0,
            "geometric Asian MC PV must be finite and positive, got {mc_pv}"
        );
        assert!(
            analytic_pv.is_finite() && analytic_pv > 0.0,
            "geometric Asian closed-form PV must be finite and positive, got {analytic_pv}"
        );

        // MC standard error of the closed-form-implied payoff distribution.
        // The registered Asian MC pricer uses 100 000 paths by default.
        let num_paths = 100_000.0_f64;
        let se = geometric_payoff_std(t, df, N_FIXINGS) / num_paths.sqrt();
        let tol = 4.0 * se;
        let diff = (mc_pv - analytic_pv).abs();

        println!(
            "geometric Asian MC↔Kemna-Vorst: mc={mc_pv:.6}  analytic={analytic_pv:.6} \
             (se≈{se:.6})  |Δ|={diff:.6}  tol={tol:.6}"
        );

        assert!(
            diff < tol,
            "geometric Asian MC GBM and Kemna-Vorst closed-form prices \
             disagree: mc={mc_pv:.6}, analytic={analytic_pv:.6}, |Δ|={diff:.6} \
             > tol={tol:.6} (4·se). The discrete geometric average is exactly \
             lognormal under GBM, so the MC estimate must converge to the \
             closed form — a gap this large indicates a genuine drift between \
             the Asian MC payoff and the geometric closed-form pricer."
        );
    }
}
