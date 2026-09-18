//! Regression tests for stochastic structured-credit tranche PV.

use finstack_quant_cashflows::builder::{
    DefaultModelSpec, FloatingRateSpec, PrepaymentModelSpec, RecoveryModelSpec,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_models::credit::pool::{
    CorrelationStructure, StochasticDefaultSpec, StochasticPrepaySpec,
};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, DealType, PoolAsset, PricingMode, StochasticPricingResult, StructuredCredit,
    Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
};
use finstack_quant_valuations::instruments::{
    InstrumentEnvelope, InstrumentJson, InstrumentPricingOverrides,
};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::{parse_boxed_instrument_from_json, price_instrument};
use finstack_quant_valuations::results::{ValuationDetails, ValuationResult};
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

fn discount_curve(base_date: Date) -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (5.0, 0.90)])
        .build()
        .expect("discount curve")
}

fn fixed_market() -> MarketContext {
    MarketContext::new().insert(discount_curve(as_of()))
}

fn pool(balance: f64) -> AssetPool {
    let mut pool = AssetPool::new("POOL", DealType::Abs, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "A1",
        Money::new(balance, Currency::USD).expect("valid money fixture"),
        0.06,
        legal_maturity(),
        DayCount::Thirty360,
    ));
    pool
}

fn two_tranches(floating_senior: bool) -> TrancheStructure {
    let senior_coupon = if floating_senior {
        TrancheCoupon::Floating(FloatingRateSpec {
            index_id: CurveId::new("SOFR-3M"),
            spread_bp: rust_decimal_macros::dec!(150),
            gearing: rust_decimal_macros::dec!(1),
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_frequency: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 2,
            fixing_calendar_id: None,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
        })
    } else {
        TrancheCoupon::Fixed { rate: 0.05 }
    };

    TrancheStructure::new(vec![
        Tranche::new(
            "SR",
            0.0,
            80.0,
            TrancheSeniority::Senior,
            Money::new(800_000.0, Currency::USD).expect("valid money fixture"),
            senior_coupon,
            legal_maturity(),
        )
        .unwrap(),
        Tranche::new(
            "EQ",
            80.0,
            100.0,
            TrancheSeniority::Equity,
            Money::new(200_000.0, Currency::USD).expect("valid money fixture"),
            TrancheCoupon::Fixed { rate: 0.0 },
            legal_maturity(),
        )
        .unwrap(),
    ])
    .unwrap()
}

fn structured_credit(floating_senior: bool) -> StructuredCredit {
    let mut sc = StructuredCredit::new_abs(
        "ABS-STOCH-PV",
        pool(1_000_000.0),
        two_tranches(floating_senior),
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
    sc.credit_model.stochastic_default_spec = Some(StochasticDefaultSpec::deterministic(
        sc.credit_model.default_spec.clone(),
    ));
    sc.instrument_pricing_overrides = InstrumentPricingOverrides::default();
    sc.instrument_pricing_overrides.model_config.mc_paths = Some(1);
    sc
}

fn stochastic_single_path(sc: &StructuredCredit, market: &MarketContext) -> ValuationResult {
    let json = serde_json::to_string(&InstrumentEnvelope::new(InstrumentJson::StructuredCredit(
        Box::new(sc.clone()),
    )))
    .expect("instrument envelope JSON");
    let instrument = parse_boxed_instrument_from_json(&json, None).expect("parse envelope");
    price_instrument(
        &instrument,
        market,
        "2024-01-01",
        "structured_credit_stochastic",
        &[],
        None,
        finstack_quant_valuations::instruments::PricingOptions::default(),
    )
    .expect("stochastic json pricing")
}

fn stochastic_details(result: &ValuationResult) -> StochasticPricingResult {
    let Some(ValuationDetails::StructuredCreditStochastic(details)) = &result.details else {
        panic!("expected structured-credit stochastic details");
    };
    details.clone()
}

#[test]
fn single_path_stochastic_pv_matches_deterministic_tranche_pv() {
    let sc = structured_credit(false);
    let market = fixed_market();

    let result = stochastic_single_path(&sc, &market);
    let details = stochastic_details(&result);

    for tranche in &details.tranche_results {
        let deterministic = sc
            .value_tranche(&tranche.tranche_id, &market, as_of())
            .expect("deterministic tranche pv");
        assert!(
            (tranche.npv.amount() - deterministic.amount()).abs() < 1.0,
            "{} stochastic PV {} should match deterministic PV {}",
            tranche.tranche_id,
            tranche.npv.amount(),
            deterministic.amount()
        );
    }
}

#[test]
fn stochastic_json_result_contains_full_tranche_details() {
    let sc = structured_credit(false);
    let market = fixed_market();

    let result = stochastic_single_path(&sc, &market);
    let details = stochastic_details(&result);

    assert_eq!(details.tranche_results.len(), sc.tranches.tranches.len());
    assert_eq!(result.value, details.npv);
    assert!(result
        .measures
        .contains_key(&MetricId::custom("expected_loss")));
    assert!(result
        .measures
        .contains_key(&MetricId::custom("tranche_npv::SR")));
    assert!(result
        .measures
        .contains_key(&MetricId::custom("tranche_expected_loss::EQ")));
}

#[test]
fn invalid_attachment_detachment_errors_locally() {
    let mut sc = structured_credit(false);
    sc.tranches.tranches[1].attachment_point = Some(75.0);
    let market = fixed_market();

    let err = sc
        .price_stochastic_with_mode(
            &market,
            as_of(),
            PricingMode::MonteCarlo {
                num_paths: 1,
                antithetic: false,
            },
        )
        .expect_err("overlapping tranches should fail");

    assert!(
        err.to_string().contains("tranche"),
        "error should identify tranche validation, got {err}"
    );
}

#[test]
fn floating_coupon_market_data_error_propagates() {
    let sc = structured_credit(true);
    let market_without_forward = fixed_market();

    let err = sc
        .price_stochastic_with_mode(
            &market_without_forward,
            as_of(),
            PricingMode::MonteCarlo {
                num_paths: 1,
                antithetic: false,
            },
        )
        .expect_err("missing forward curve should fail");

    assert!(
        err.to_string().contains("SOFR-3M"),
        "missing curve should be propagated, got {err}"
    );
}

#[test]
fn junior_expected_loss_comes_from_writedowns() {
    let mut sc = structured_credit(false);
    sc.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.95);
    sc.credit_model.stochastic_default_spec = Some(StochasticDefaultSpec::deterministic(
        sc.credit_model.default_spec.clone(),
    ));
    let market = fixed_market();

    let result = sc
        .price_stochastic_with_mode(
            &market,
            as_of(),
            PricingMode::MonteCarlo {
                num_paths: 1,
                antithetic: false,
            },
        )
        .expect("stochastic pricing");

    let senior = result
        .tranche_results
        .iter()
        .find(|tranche| tranche.tranche_id == "SR")
        .expect("senior tranche");
    let equity = result
        .tranche_results
        .iter()
        .find(|tranche| tranche.tranche_id == "EQ")
        .expect("equity tranche");

    assert!(equity.expected_loss.amount() > 0.0);
    assert!(equity.npv.amount() < senior.npv.amount());
}

#[test]
fn oversized_explicit_tree_errors_before_pricing() {
    let mut sc = structured_credit(false);
    sc.instrument_pricing_overrides.model_config.tree_steps = Some(24);
    let market = fixed_market();

    let err = sc
        .price_stochastic_with_mode(&market, as_of(), PricingMode::Tree)
        .expect_err("oversized path-preserving tree should fail");

    assert!(
        err.to_string().contains("max_tree_paths"),
        "error should mention path cap, got {err}"
    );
}

#[test]
fn stochastic_default_volatility_changes_loss_dispersion() {
    let market = fixed_market();
    let pricing_mode = PricingMode::MonteCarlo {
        num_paths: 128,
        antithetic: false,
    };

    let mut low_vol = structured_credit(false);
    low_vol.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.25);
    low_vol.credit_model.stochastic_default_spec = Some(StochasticDefaultSpec::factor_correlated(
        low_vol.credit_model.default_spec.clone(),
        1.0,
        0.0,
    ));
    low_vol.credit_model.correlation_structure =
        Some(CorrelationStructure::flat(0.64, 0.0).expect("valid correlation"));

    let mut high_vol = low_vol.clone();
    high_vol.credit_model.stochastic_default_spec = Some(StochasticDefaultSpec::factor_correlated(
        high_vol.credit_model.default_spec.clone(),
        1.0,
        1.0,
    ));

    let low = low_vol
        .price_stochastic_with_mode(&market, as_of(), pricing_mode.clone())
        .expect("low-vol stochastic pricing");
    let high = high_vol
        .price_stochastic_with_mode(&market, as_of(), pricing_mode)
        .expect("high-vol stochastic pricing");

    assert!(
        high.unexpected_loss.amount() > low.unexpected_loss.amount() + 1.0,
        "default volatility should affect loss dispersion: low={}, high={}",
        low.unexpected_loss.amount(),
        high.unexpected_loss.amount()
    );
}

#[test]
fn non_pik_deferred_interest_cannot_consume_principal() {
    let market = MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(as_of())
            .knots([(0.0, 1.0), (5.0, 1.0)])
            .build()
            .unwrap(),
    );
    let mut sc = structured_credit(false);
    sc.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    sc.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    sc.credit_model.stochastic_prepay_spec = Some(StochasticPrepaySpec::deterministic(
        sc.credit_model.prepayment_spec.clone(),
    ));
    sc.credit_model.stochastic_default_spec = Some(StochasticDefaultSpec::deterministic(
        sc.credit_model.default_spec.clone(),
    ));

    sc.tranches.tranches[0].coupon = TrancheCoupon::Fixed { rate: 0.20 };
    sc.tranches.tranches[0].pik_enabled = false;
    sc.pool.assets[0].rate = 0.01;

    let result = sc
        .price_stochastic_with_mode(
            &market,
            as_of(),
            PricingMode::MonteCarlo {
                num_paths: 1,
                antithetic: false,
            },
        )
        .expect("stochastic pricing");

    let senior = result
        .tranche_results
        .iter()
        .find(|tranche| tranche.tranche_id == "SR")
        .expect("senior tranche");

    // With no losses or prepayments, senior receives 800,000 principal and
    // exactly the pool's 1% interest for two 30/360 years. The 20% unpaid
    // coupon claim cannot consume the equity holder's 200,000 principal.
    let expected = 800_000.0 + 1_000_000.0 * 0.01 * 2.0;
    assert!(
        (senior.npv.amount() - expected).abs() < 1e-6,
        "senior cash {}, required {expected}",
        senior.npv.amount()
    );
}

/// Market-correlated recoveries move with the same factor as defaults, so
/// the equity value under a dispersed recovery differs from the constant
/// recovery at the same mean.
#[test]
fn market_correlated_recovery_moves_equity_pv() {
    let mut constant = structured_credit(false);
    constant.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.20);
    constant.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
    constant
        .enable_stochastic_defaults()
        .expect("stochastic defaults");
    let mut dispersed = constant.clone();
    dispersed.credit_model.stochastic_recovery_spec = Some(
        finstack_quant_models::correlation::RecoverySpec::market_correlated(0.40, 0.30, 0.80)
            .expect("recovery spec"),
    );
    let market = fixed_market();
    let mode = PricingMode::MonteCarlo {
        num_paths: 64,
        antithetic: true,
    };
    let price = |deal: &StructuredCredit| {
        deal.price_stochastic_with_mode(&market, as_of(), mode.clone())
            .expect("stochastic pricing")
            .tranche_results
            .into_iter()
            .find(|t| t.tranche_id == "EQ")
            .expect("equity")
    };
    let constant_equity = price(&constant);
    let dispersed_equity = price(&dispersed);
    assert!(
        (constant_equity.npv.amount() - dispersed_equity.npv.amount()).abs() > 1.0,
        "dispersed recovery must change the equity PV: {} vs {}",
        constant_equity.npv.amount(),
        dispersed_equity.npv.amount()
    );
}

/// The average life averages over the paths that returned principal: a
/// class wiped out on every path reports no WAL and no such path, and a
/// class repaid on every path reports every path.
#[test]
fn average_life_counts_only_paths_that_return_principal() {
    let mut sc = structured_credit(false);
    sc.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.95);
    sc.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.0, 0);
    sc.credit_model.stochastic_default_spec = Some(StochasticDefaultSpec::deterministic(
        sc.credit_model.default_spec.clone(),
    ));
    let result = sc
        .price_stochastic_with_mode(
            &fixed_market(),
            as_of(),
            PricingMode::MonteCarlo {
                num_paths: 4,
                antithetic: false,
            },
        )
        .expect("stochastic pricing");
    let by_id = |id: &str| {
        result
            .tranche_results
            .iter()
            .find(|t| t.tranche_id == id)
            .expect("tranche")
    };
    let senior = by_id("SR");
    let equity = by_id("EQ");
    assert_eq!(senior.paths_with_principal, 4);
    assert!(senior.average_life > 0.0);
    assert_eq!(
        equity.paths_with_principal, 0,
        "the equity is wiped out on every path"
    );
    assert_eq!(equity.average_life, 0.0);
}
