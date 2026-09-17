//! Behavioral curve shapes through the deterministic engine: the ABS speed
//! convention, cumulative net-loss curves and rating-agency default timing
//! must reach the pool flows with the semantics the cashflows crate defines.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    calculate_tranche_wal, run_simulation, AssetPool, AssetType, DealType, PoolAsset,
    StructuredCredit, Tranche, TrancheCashflows, TrancheCoupon, TrancheSeniority, TrancheStructure,
};
use time::Month;

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).expect("month"), day).expect("date")
}

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("money")
}

fn close() -> Date {
    d(2024, 1, 1)
}

fn maturity() -> Date {
    d(2029, 1, 1)
}

fn market() -> MarketContext {
    let knots: Vec<(f64, f64)> = (0..=10)
        .map(|i| (f64::from(i), (-0.05 * f64::from(i)).exp()))
        .collect();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(close())
        .knots(knots)
        .build()
        .expect("curve");
    MarketContext::new().insert(curve)
}

/// Monthly auto ABS: ten 1M five-year loans at 8%, A 90% / E 10%.
///
/// `level_pay` chooses amortizing auto loans; otherwise the loans are bullets.
fn auto_abs(level_pay: bool) -> StructuredCredit {
    let mut pool = AssetPool::new("P", DealType::Abs, Currency::USD);
    for i in 0..10 {
        let mut asset = PoolAsset::fixed_rate_bond(
            format!("L{i}"),
            usd(1_000_000.0),
            0.08,
            maturity(),
            DayCount::Thirty360,
        );
        if level_pay {
            asset.asset_type = AssetType::NewAutoLoan { ltv: None };
        }
        pool.assets.push(asset);
    }
    let tranches = TrancheStructure::new(vec![
        Tranche::new(
            "A",
            0.0,
            90.0,
            TrancheSeniority::Senior,
            usd(9_000_000.0),
            TrancheCoupon::Fixed { rate: 0.05 },
            maturity(),
        )
        .expect("A"),
        Tranche::new(
            "E",
            90.0,
            100.0,
            TrancheSeniority::Equity,
            usd(1_000_000.0),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity(),
        )
        .expect("E"),
    ])
    .expect("structure");
    let mut deal =
        StructuredCredit::new_abs("ABS-CURVES", pool, tranches, close(), maturity(), "USD-OIS")
            .with_payment_calendar("nyse");
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.5, 0);
    deal
}

fn simulate(deal: &StructuredCredit) -> HashMap<String, TrancheCashflows> {
    run_simulation(deal, &market(), close()).expect("simulation")
}

/// The ABS convention prepays a constant share of the *original* balance, so
/// its SMM rises with seasoning and the senior note amortizes faster than
/// under a constant 1.5% SMM (the annualized month-1 rate).
#[test]
fn abs_speed_shortens_the_senior_wal_versus_a_constant_smm() {
    let mut abs = auto_abs(false);
    abs.credit_model.prepayment_spec = PrepaymentModelSpec::abs(0.015);
    let mut constant = auto_abs(false);
    constant.credit_model.prepayment_spec =
        PrepaymentModelSpec::constant_cpr(1.0 - 0.985_f64.powi(12));

    let abs_wal = calculate_tranche_wal(&simulate(&abs)["A"], close()).expect("wal");
    let constant_wal = calculate_tranche_wal(&simulate(&constant)["A"], close()).expect("wal");

    assert_ne!(abs_wal, constant_wal);
    assert!(
        abs_wal < constant_wal,
        "1.5% ABS WAL {abs_wal} must be shorter than constant 1.5% SMM WAL {constant_wal}"
    );
}

/// Lifetime loss the pool realizes: every dollar of the 10M pool comes back as
/// scheduled principal or recovery except the net loss, the senior note is
/// paid in full, and the residual principal reaching equity is 1M minus the
/// loss.
fn realized_loss(results: &HashMap<String, TrancheCashflows>) -> f64 {
    let senior = results["A"].total_principal.amount();
    assert!(
        (senior - 9_000_000.0).abs() < 1.0,
        "the senior note is covered by the pool: {senior}"
    );
    assert_eq!(results["A"].total_writedown.amount(), 0.0);
    1_000_000.0 - results["E"].total_principal.amount()
}

/// A cumulative net-loss curve is stated against the original balance, so on
/// a level-pay pool at zero CPR the lifetime loss must still equal the curve's
/// terminal value: 3% net loss on 10M = 300k.
#[test]
fn cumulative_loss_curve_is_reproduced_on_a_level_pay_pool() {
    let mut deal = auto_abs(true);
    // 3% lifetime net loss ramping over 36 months at 50% severity (6% defaults).
    let curve: Vec<f64> = (1..=36).map(|m| 3.0 * f64::from(m) / 36.0).collect();
    deal.credit_model.default_spec = DefaultModelSpec::cumulative_loss(curve, 0.5);

    let loss = realized_loss(&simulate(&deal));

    assert!(
        (loss - 300_000.0).abs() < 1.0,
        "lifetime loss should be the curve's 3% of 10M, got {loss}"
    );
}

/// Default timing spreads a lifetime default rate by year: 4% of the 10M pool
/// defaults over 15/30/30/15/10, and at 50% severity the pool loses exactly
/// 2% = 200k on a level-pay pool.
#[test]
fn timing_curve_defaults_sum_to_the_lifetime_rate_on_a_level_pay_pool() {
    let mut deal = auto_abs(true);
    deal.credit_model.default_spec =
        DefaultModelSpec::timing(0.04, vec![15.0, 30.0, 30.0, 15.0, 10.0]);

    let loss = realized_loss(&simulate(&deal));

    assert!(
        (loss - 200_000.0).abs() < 1.0,
        "lifetime loss should be 4% defaults × 50% severity on 10M, got {loss}"
    );
}

/// Curves stated against the original balance and severities by month of
/// default are deterministic-only: the stochastic entry point refuses them
/// instead of silently approximating.
#[test]
fn stochastic_pricing_rejects_lifetime_curves_and_severity_vectors() {
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::PricingMode;

    let mode = PricingMode::MonteCarlo {
        num_paths: 8,
        antithetic: false,
    };
    let mut lifetime = auto_abs(false);
    lifetime.credit_model.default_spec = DefaultModelSpec::cumulative_loss(vec![1.0, 2.0], 0.5);
    assert!(lifetime
        .price_stochastic_with_mode(&market(), close(), mode.clone())
        .is_err());

    let mut severity = auto_abs(false);
    severity.credit_model.recovery_spec =
        RecoveryModelSpec::with_lag(0.5, 0).with_severity_vector(vec![0.6]);
    assert!(severity
        .price_stochastic_with_mode(&market(), close(), mode)
        .is_err());
}
