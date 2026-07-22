//! Config-sensitivity tests: public stochastic knobs must move the price.
//!
//! Guards against the "silently inert config" class from the 2026-07
//! structured-credit audit: a parameter that passes validation but never
//! reaches the engine produces bit-identical results under a fixed seed, so
//! each test here varies exactly one input and asserts the output changes.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, CorrelationStructure, DealType, PoolAsset, PricingMode, StochasticDefaultSpec,
    StochasticPrepaySpec, StochasticPricingResult, StructuredCredit, Tranche, TrancheCoupon,
    TrancheSeniority, TrancheStructure,
};
use time::Month;

fn closing_date() -> Date {
    Date::from_calendar_date(2024, Month::January, 1).unwrap()
}

fn as_of() -> Date {
    Date::from_calendar_date(2024, Month::January, 1).unwrap()
}

fn legal_maturity() -> Date {
    Date::from_calendar_date(2026, Month::January, 1).unwrap()
}

fn fixed_market() -> MarketContext {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of())
        .knots(vec![(0.0, 1.0), (5.0, 0.90)])
        .build()
        .expect("discount curve");
    MarketContext::new().insert(curve)
}

/// Ten equal names so the systematic/idiosyncratic split has something to
/// correlate: with a single asset, per-name default dispersion is invariant
/// to asset correlation and the sensitivity assertions would be vacuous.
fn pool() -> AssetPool {
    let mut pool = AssetPool::new("POOL", DealType::ABS, Currency::USD);
    for i in 0..10 {
        pool.assets.push(PoolAsset::fixed_rate_bond(
            format!("A{i}"),
            Money::new(100_000.0, Currency::USD),
            0.06,
            legal_maturity(),
            DayCount::Thirty360,
        ));
    }
    pool
}

fn tranches() -> TrancheStructure {
    TrancheStructure::new(vec![
        Tranche::new(
            "SR",
            0.0,
            80.0,
            TrancheSeniority::Senior,
            Money::new(800_000.0, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.05 },
            legal_maturity(),
        )
        .unwrap(),
        Tranche::new(
            "EQ",
            80.0,
            100.0,
            TrancheSeniority::Equity,
            Money::new(200_000.0, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.0 },
            legal_maturity(),
        )
        .unwrap(),
    ])
    .unwrap()
}

/// Both variants of every test share this deal id, so `derive_seed` gives
/// them identical Monte Carlo draws — any output difference is the config.
fn structured_credit() -> StructuredCredit {
    let mut sc = StructuredCredit::new_abs(
        "ABS-CFG-SENS",
        pool(),
        tranches(),
        closing_date(),
        legal_maturity(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse");
    sc.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    sc.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    sc.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
    sc.credit_model.stochastic_prepay_spec = Some(StochasticPrepaySpec::deterministic(
        sc.credit_model.prepayment_spec.clone(),
    ));
    sc
}

fn price(sc: &StructuredCredit, market: &MarketContext) -> StochasticPricingResult {
    sc.price_stochastic_with_mode(
        market,
        as_of(),
        PricingMode::MonteCarlo {
            num_paths: 512,
            antithetic: false,
        },
    )
    .expect("stochastic pricing")
}

/// The deal-level `CorrelationStructure` must reach the copula: higher asset
/// correlation concentrates defaults on bad factor paths, widening the loss
/// distribution (Vasicek 2002). Exercises the `asset_correlation_override`
/// conduit end-to-end from the public deal type.
#[test]
fn correlation_structure_widens_the_loss_distribution() {
    let market = fixed_market();

    let mut low = structured_credit();
    low.credit_model.stochastic_default_spec =
        Some(StochasticDefaultSpec::gaussian_copula(0.25, 0.10));
    low.credit_model.correlation_structure = Some(CorrelationStructure::flat(0.05, 0.0));

    let mut high = low.clone();
    high.credit_model.correlation_structure = Some(CorrelationStructure::flat(0.60, 0.0));

    let low_result = price(&low, &market);
    let high_result = price(&high, &market);

    assert!(
        high_result.unexpected_loss.amount() > low_result.unexpected_loss.amount() + 1.0,
        "asset correlation must widen the loss distribution: rho=0.05 UL {}, rho=0.60 UL {}",
        low_result.unexpected_loss.amount(),
        high_result.unexpected_loss.amount()
    );
}

/// The intensity-process mean reversion κ must reach the systematic factor:
/// κ sets the factor autocorrelation `φ = e^{−κ/12}`, so κ=0 (persistent
/// factor) and κ=5 (fast mean reversion) must produce different loss
/// distributions. A silently inert κ yields bit-identical results under the
/// shared seed.
#[test]
fn intensity_mean_reversion_changes_the_loss_distribution() {
    let market = fixed_market();

    let mut persistent = structured_credit();
    persistent.credit_model.stochastic_default_spec = Some(
        StochasticDefaultSpec::intensity_process(0.10, 1.0, 0.0, 0.5),
    );

    let mut mean_reverting = persistent.clone();
    mean_reverting.credit_model.stochastic_default_spec = Some(
        StochasticDefaultSpec::intensity_process(0.10, 1.0, 5.0, 0.5),
    );

    let persistent_result = price(&persistent, &market);
    let reverting_result = price(&mean_reverting, &market);

    assert!(
        (persistent_result.unexpected_loss.amount() - reverting_result.unexpected_loss.amount())
            .abs()
            > 1e-6,
        "mean reversion must change the loss distribution: kappa=0 UL {}, kappa=5 UL {}",
        persistent_result.unexpected_loss.amount(),
        reverting_result.unexpected_loss.amount()
    );
}
