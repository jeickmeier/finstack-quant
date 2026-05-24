//! Integration tests for per-name copula default simulation.
//!
//! These exercise the structured-credit Monte Carlo engine through the
//! public `price_stochastic_with_mode` entry point, verifying that finite
//! pools are now simulated name-by-name (latent variable
//! `Aᵢ = √ρ·Z + √(1−ρ)·εᵢ`) rather than collapsed to the
//! large-homogeneous-pool (LHP) limit.
//!
//! # Parent-failure anchor
//!
//! `concentration_changes_loss_dispersion` is the test that genuinely fails
//! on the parent commit. On the parent the engine applies one pool-wide MDR
//! uniformly to every asset, so the realized loss depends only on the pool's
//! *total* balance — splitting that balance into 40 vs 600 names produces
//! bit-identical cashflows and therefore identical loss dispersion. With
//! per-name simulation the concentrated pool carries strictly more
//! idiosyncratic dispersion, so the strict inequality holds only on the
//! patched engine.

use finstack_cashflows::builder::{DefaultModelSpec, RecoveryModelSpec};
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::money::Money;
use finstack_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, CorrelationStructure, DealType, PoolAsset, PoolGranularity, PricingMode,
    StochasticDefaultSpec, StochasticPrepaySpec, StochasticPricingResult, StructuredCredit,
    Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
};
use time::Month;

fn close() -> Date {
    Date::from_calendar_date(2024, Month::January, 1).expect("valid date")
}

fn maturity() -> Date {
    Date::from_calendar_date(2027, Month::January, 1).expect("valid date")
}

fn market() -> MarketContext {
    MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(close())
            .knots(vec![(0.0, 1.0), (1.0, 0.97), (3.0, 0.91), (5.0, 0.85)])
            .build()
            .expect("discount curve"),
    )
}

/// CLO-style deal: `n_assets` identical fixed-rate loans summing to $100M,
/// tranched senior (0-80%) / mezzanine (80-92%) / equity (92-100%), with a
/// Gaussian-copula default model. Total notional, base CDR and correlation
/// are held fixed across `n_assets` so the only variable is granularity.
fn clo_deal(n_assets: usize, base_cdr: f64, correlation: f64) -> StructuredCredit {
    let total = 100_000_000.0;
    let per_asset = total / n_assets as f64;
    let mut pool = AssetPool::new("CLO-POOL", DealType::CLO, Currency::USD);
    for i in 0..n_assets {
        pool.assets.push(PoolAsset::fixed_rate_bond(
            format!("L{i}"),
            Money::new(per_asset, Currency::USD),
            0.07,
            maturity(),
            DayCount::Thirty360,
        ));
    }
    let tranches = TrancheStructure::new(vec![
        Tranche::new(
            "SR",
            0.0,
            80.0,
            TrancheSeniority::Senior,
            Money::new(total * 0.80, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.05 },
            maturity(),
        )
        .expect("senior"),
        Tranche::new(
            "MEZZ",
            80.0,
            92.0,
            TrancheSeniority::Mezzanine,
            Money::new(total * 0.12, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.08 },
            maturity(),
        )
        .expect("mezz"),
        Tranche::new(
            "EQ",
            92.0,
            100.0,
            TrancheSeniority::Equity,
            Money::new(total * 0.08, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity(),
        )
        .expect("equity"),
    ])
    .expect("structure");
    let mut sc = StructuredCredit::new_abs(
        "CLO-PER-NAME",
        pool,
        tranches,
        close(),
        maturity(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse");
    sc.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    sc.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
    let deterministic_prepay =
        StochasticPrepaySpec::deterministic(sc.credit_model.prepayment_spec.clone());
    sc.with_stochastic_default(StochasticDefaultSpec::gaussian_copula(
        base_cdr,
        correlation,
    ))
    .with_stochastic_prepay(deterministic_prepay)
    .with_correlation(CorrelationStructure::flat(correlation, 0.0));
    sc
}

fn tranche_pv(result: &StochasticPricingResult, id: &str) -> f64 {
    result
        .tranche_results
        .iter()
        .find(|t| t.tranche_id == id)
        .map(|t| t.npv.amount())
        .unwrap_or_else(|| panic!("tranche {id} missing"))
}

fn price(sc: &StructuredCredit, num_paths: usize) -> StochasticPricingResult {
    sc.price_stochastic_with_mode(
        &market(),
        close(),
        PricingMode::MonteCarlo {
            num_paths,
            antithetic: false,
        },
    )
    .expect("stochastic pricing")
}

fn price_with_granularity(
    sc: &StructuredCredit,
    num_paths: usize,
    granularity: PoolGranularity,
) -> StochasticPricingResult {
    let mut sc = sc.clone();
    sc.pricing_overrides
        .model_config
        .structured_credit_pool_granularity = Some(granularity);
    price(&sc, num_paths)
}

/// **Concentration sensitivity — the parent-failure anchor.**
///
/// A concentrated pool (40 names) and a granular pool (600 names) with the
/// *same* total notional, base CDR and correlation must produce different
/// loss dispersion: per-name simulation gives the concentrated pool strictly
/// more idiosyncratic lumpiness.
///
/// On the parent commit the engine applies one pool-wide MDR to every asset,
/// so the realized loss depends only on the total balance — both pools price
/// identically and `unexpected_loss` is equal. The strict inequality below
/// therefore fails on the parent and passes only with per-name simulation.
#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn concentration_changes_loss_dispersion() {
    let concentrated = price(&clo_deal(40, 0.05, 0.25), 6_000);
    let granular = price(&clo_deal(600, 0.05, 0.25), 6_000);

    // Per-name simulation: fewer names ⇒ each default is a larger share of
    // the pool ⇒ fatter loss tail ⇒ higher unexpected loss.
    assert!(
        concentrated.unexpected_loss.amount() > granular.unexpected_loss.amount() * 1.05,
        "concentrated pool (40 names) loss dispersion {:.0} must exceed the \
         granular pool (600 names) dispersion {:.0} — equal dispersion means \
         the pool-wide-MDR defect is still present",
        concentrated.unexpected_loss.amount(),
        granular.unexpected_loss.amount(),
    );
}

/// A concentrated pool priced per-name must produce a materially different
/// mezzanine/equity tranche value than the LHP fast-path on the *same* pool.
/// This isolates the per-name vs LHP difference via the explicit
/// `PoolGranularity` override.
#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn concentrated_pool_per_name_tranche_pv_differs_from_lhp() {
    let deal = clo_deal(40, 0.05, 0.25);
    let per_name = price_with_granularity(&deal, 8_000, PoolGranularity::PerName);
    let lhp = price_with_granularity(&deal, 8_000, PoolGranularity::LargeHomogeneous);

    let mezz_gap = (tranche_pv(&per_name, "MEZZ") - tranche_pv(&lhp, "MEZZ")).abs();
    let eq_gap = (tranche_pv(&per_name, "EQ") - tranche_pv(&lhp, "EQ")).abs();

    assert!(
        mezz_gap > 100_000.0 || eq_gap > 100_000.0,
        "per-name pricing of a 40-name pool must differ materially from the \
         LHP fast-path: mezz gap {mezz_gap:.0}, equity gap {eq_gap:.0}"
    );
}

/// **LHP-limit parity — the correctness anchor.**
///
/// A large, granular, homogeneous pool priced per-name must converge to the
/// LHP fast-path result for the *same* pool (per-name → LHP as `N → ∞`).
#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn large_granular_pool_per_name_matches_lhp() {
    let deal = clo_deal(600, 0.03, 0.20);
    let per_name = price_with_granularity(&deal, 2_000, PoolGranularity::PerName);
    let lhp = price_with_granularity(&deal, 2_000, PoolGranularity::LargeHomogeneous);

    for id in ["SR", "MEZZ", "EQ"] {
        let pn = tranche_pv(&per_name, id);
        let lh = tranche_pv(&lhp, id);
        let tol = (0.015 * lh.abs()).max(200_000.0);
        assert!(
            (pn - lh).abs() < tol,
            "{id}: per-name PV {pn:.0} should converge to LHP PV {lh:.0} \
             (|diff|={:.0}, tol={tol:.0})",
            (pn - lh).abs()
        );
    }
}

/// The LHP fast-path is name-count-independent by construction: it applies
/// one pool-wide conditional default rate, so a 40-name and a 600-name pool
/// with the same total notional / CDR / correlation produce *identical* loss
/// dispersion.
///
/// This is also the parent-failure proof for
/// `concentration_changes_loss_dispersion`: the parent commit's
/// pool-wide-MDR engine behaves exactly like this LHP path, so on the parent
/// the concentrated and granular pools price identically and the strict
/// `concentrated > granular` inequality there cannot hold.
#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn lhp_dispersion_is_independent_of_name_count() {
    let concentrated = price_with_granularity(
        &clo_deal(40, 0.05, 0.25),
        6_000,
        PoolGranularity::LargeHomogeneous,
    );
    let granular = price_with_granularity(
        &clo_deal(600, 0.05, 0.25),
        6_000,
        PoolGranularity::LargeHomogeneous,
    );
    assert!(
        (concentrated.unexpected_loss.amount() - granular.unexpected_loss.amount()).abs() < 1.0,
        "LHP-mode dispersion must be name-count-independent: 40-name={:.0}, 600-name={:.0}",
        concentrated.unexpected_loss.amount(),
        granular.unexpected_loss.amount(),
    );
}

/// Per-name pricing must be deterministic: repeated runs with the same seed
/// produce bit-identical NPVs (seeded `PhiloxRng` per-path substreams,
/// order-stable per-name draws).
#[test]
fn per_name_pricing_is_deterministic() {
    let deal = clo_deal(80, 0.04, 0.25);
    let first = price(&deal, 400);
    let second = price(&deal, 400);

    assert_eq!(
        first.npv.amount(),
        second.npv.amount(),
        "repeated per-name MC runs must produce bit-identical deal NPV"
    );
    for id in ["SR", "MEZZ", "EQ"] {
        assert_eq!(
            tranche_pv(&first, id),
            tranche_pv(&second, id),
            "{id}: repeated per-name runs must produce bit-identical tranche PV"
        );
    }
}

/// Per-name simulation must be bit-identical between serial and
/// rayon-parallel execution. The per-name idiosyncratic-draw substream is
/// keyed by path index (not by rayon scheduling order), and the path outputs
/// are collected in path order, so the result is independent of thread
/// count.
#[test]
fn per_name_pricing_is_serial_parallel_bit_identical() {
    let deal = clo_deal(70, 0.05, 0.25);

    // Single-threaded rayon pool ⇒ paths run strictly serially.
    let serial_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .expect("single-thread rayon pool");
    let serial = serial_pool.install(|| price(&deal, 600));

    // Wide pool ⇒ paths run concurrently across threads.
    let parallel_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(8)
        .build()
        .expect("multi-thread rayon pool");
    let parallel = parallel_pool.install(|| price(&deal, 600));

    assert_eq!(
        serial.npv.amount(),
        parallel.npv.amount(),
        "serial and parallel per-name MC must produce bit-identical deal NPV"
    );
    for id in ["SR", "MEZZ", "EQ"] {
        assert_eq!(
            tranche_pv(&serial, id),
            tranche_pv(&parallel, id),
            "{id}: serial and parallel per-name runs must produce bit-identical tranche PV"
        );
    }
    assert_eq!(
        serial.unexpected_loss.amount(),
        parallel.unexpected_loss.amount(),
        "serial and parallel per-name runs must produce bit-identical unexpected loss"
    );
}
