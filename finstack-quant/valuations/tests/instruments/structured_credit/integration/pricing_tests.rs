//! Integration tests for full pricing and metrics computation.
//!
//! Tests end-to-end pricing workflow with market data and metric requests.

use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, DealType, PoolAsset, StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority,
    TrancheStructure,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::Month;

fn test_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn maturity_date() -> Date {
    Date::from_calendar_date(2030, Month::December, 31).unwrap()
}

fn create_simple_pool() -> AssetPool {
    let mut pool = AssetPool::new("POOL", DealType::ABS, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "A1",
        Money::new(5_000_000.0, Currency::USD),
        0.06,
        Date::from_calendar_date(2029, Month::January, 1).unwrap(),
        finstack_quant_core::dates::DayCount::Thirty360,
    ));
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "A2",
        Money::new(3_000_000.0, Currency::USD),
        0.05,
        Date::from_calendar_date(2028, Month::January, 1).unwrap(),
        finstack_quant_core::dates::DayCount::Thirty360,
    ));
    pool
}

fn create_simple_clo_pool() -> AssetPool {
    let mut pool = AssetPool::new("CLO_POOL", DealType::CLO, Currency::USD);
    pool.assets.push(
        PoolAsset::fixed_rate_bond(
            "L1",
            Money::new(5_000_000.0, Currency::USD),
            0.06,
            Date::from_calendar_date(2029, Month::January, 1).unwrap(),
            finstack_quant_core::dates::DayCount::Thirty360,
        )
        .with_rating(finstack_quant_core::types::CreditRating::BB),
    );
    pool.assets.push(
        PoolAsset::fixed_rate_bond(
            "L2",
            Money::new(3_000_000.0, Currency::USD),
            0.08,
            Date::from_calendar_date(2028, Month::January, 1).unwrap(),
            finstack_quant_core::dates::DayCount::Thirty360,
        )
        .with_rating(finstack_quant_core::types::CreditRating::B),
    );
    pool
}

fn create_simple_cmbs_pool() -> AssetPool {
    let mut pool = AssetPool::new("CMBS_POOL", DealType::CMBS, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "CMBS-LOAN-1",
        Money::new(10_000_000.0, Currency::USD),
        0.05,
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
        finstack_quant_core::dates::DayCount::Thirty360,
    ));
    pool
}

fn create_simple_tranches() -> TrancheStructure {
    let senior = Tranche::new(
        "SENIOR",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(8_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.035 },
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
    )
    .unwrap();
    TrancheStructure::new(vec![senior]).unwrap()
}

fn flat_discount_curve(rate: f64, base: Date) -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots(vec![
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
        ])
        .build()
        .unwrap()
}

// ============================================================================
// Basic Pricing Tests
// ============================================================================

#[test]
fn test_structured_credit_value_computation() {
    // Arrange
    let sc = StructuredCredit::new_abs(
        "TEST_ABS",
        create_simple_pool(),
        create_simple_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse");

    let market = MarketContext::new().insert(flat_discount_curve(0.04, test_date()));

    // Act
    let result = sc.value(&market, test_date());

    // Assert
    assert!(result.is_ok());
    let value = result.unwrap();
    assert!(value.amount() > 0.0);
}

#[test]
fn test_structured_credit_dirty_price() {
    // Arrange
    let sc = StructuredCredit::new_abs(
        "TEST_ABS",
        create_simple_pool(),
        create_simple_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse");

    let market = MarketContext::new().insert(flat_discount_curve(0.04, test_date()));

    // Act
    let result = sc.price_with_metrics(
        &market,
        test_date(),
        &[MetricId::DirtyPrice],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    let result = result.expect("Structured credit clean/dirty pricing should succeed");
    assert!(result.measures.contains_key("dirty_price"));

    let price = result.measures["dirty_price"];
    assert!(
        price > 0.0 && price < 200.0,
        "Price should be reasonable: {}",
        price
    );
}

#[test]
fn test_structured_credit_clean_price() {
    // Arrange
    let sc = StructuredCredit::new_abs(
        "TEST_ABS",
        create_simple_pool(),
        create_simple_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse");

    let market = MarketContext::new().insert(flat_discount_curve(0.04, test_date()));

    // Act
    let result = sc.price_with_metrics(
        &market,
        test_date(),
        &[
            MetricId::DirtyPrice,
            MetricId::CleanPrice,
            MetricId::Accrued,
        ],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    let result = result.expect("Structured credit clean/dirty pricing should succeed");

    let dirty = result.measures["dirty_price"];
    let clean = result.measures["clean_price"];
    let accrued = result.measures["accrued"];

    // Clean should be <= Dirty
    assert!(clean <= dirty + 0.01); // Small tolerance for rounding
    assert!(accrued >= 0.0);
}

// ============================================================================
// Tranche Cashflow Tests
// ============================================================================

#[test]
fn test_structured_credit_tranche_cashflows_generated() {
    let sc = StructuredCredit::new_abs(
        "TEST_ABS",
        create_simple_pool(),
        create_simple_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse");

    let market = MarketContext::new().insert(flat_discount_curve(0.04, test_date()));

    let cashflows = sc
        .get_tranche_cashflows("SENIOR", &market, test_date())
        .expect("tranche cashflows should be generated");

    assert!(!cashflows.cashflows.is_empty());
}

#[test]
fn test_structured_credit_tranche_value_computation() {
    let sc = StructuredCredit::new_abs(
        "TEST_ABS",
        create_simple_pool(),
        create_simple_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse");

    let market = MarketContext::new().insert(flat_discount_curve(0.04, test_date()));

    let pv = sc
        .value_tranche("SENIOR", &market, test_date())
        .expect("tranche PV should be computed");

    assert!(pv.amount() > 0.0);
}

// ============================================================================
// Metrics Suite Tests
// ============================================================================

#[test]
fn test_structured_credit_full_metric_suite() {
    // Arrange
    let sc = StructuredCredit::new_clo(
        "TEST_CLO",
        create_simple_pool(),
        create_simple_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse");

    let market = MarketContext::new().insert(flat_discount_curve(0.04, test_date()));

    // Act: Request comprehensive metrics
    let result = sc.price_with_metrics(
        &market,
        test_date(),
        &[
            MetricId::Accrued,
            MetricId::DirtyPrice,
            MetricId::CleanPrice,
            MetricId::WAL,
            MetricId::DurationMac,
            MetricId::DurationMod,
            MetricId::ZSpread,
            MetricId::Cs01,
            MetricId::SpreadDuration,
            MetricId::Ytm,
            MetricId::WAM,
            MetricId::CPR,
            MetricId::CDR,
        ],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(
        result.is_ok(),
        "Full metric suite should compute: {:?}",
        result.err()
    );

    let result = result.unwrap();
    assert_eq!(result.measures.len(), 13, "Should compute all 13 metrics");

    // Verify all metrics are finite
    for (key, value) in &result.measures {
        assert!(value.is_finite(), "Metric {} should be finite", key);
    }
}

#[test]
fn test_prepayment01_default01_nonzero_for_curve_specs() {
    // Regression: PSA/SDA curve specs ignore the flat `cpr`/`cdr` fields, so
    // bumping those fields produced identically-zero Prepayment01/Default01.
    // The calculators now bump `speed_multiplier` for curve-shaped specs.
    use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec};

    let mut sc = StructuredCredit::new_abs(
        "TEST_ABS_CURVE_BUMPS",
        create_simple_pool(),
        create_simple_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse");
    sc.credit_model.prepayment_spec = PrepaymentModelSpec::psa(1.5);
    sc.credit_model.default_spec = DefaultModelSpec::sda(1.0);

    let market = MarketContext::new().insert(flat_discount_curve(0.04, test_date()));

    let result = sc
        .price_with_metrics(
            &market,
            test_date(),
            &[MetricId::Prepayment01, MetricId::Default01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("curve-spec sensitivity metrics should compute");

    let prepayment01 = *result.measures.get("prepayment01").unwrap();
    let default01 = *result.measures.get("default01").unwrap();
    assert!(
        prepayment01.is_finite() && prepayment01.abs() > 1e-6,
        "Prepayment01 must be non-zero for PSA spec, got {prepayment01}"
    );
    assert!(
        default01.is_finite() && default01.abs() > 1e-6,
        "Default01 must be non-zero for SDA spec, got {default01}"
    );
}

#[test]
fn test_scenario_price_shock_scales_structured_credit_pv_and_dollar_risk_once() {
    use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec};
    use finstack_quant_valuations::instruments::ScenarioPricingOverrides;

    let mut baseline = StructuredCredit::new_abs(
        "TEST_ABS_SCENARIO_RISK",
        create_simple_pool(),
        create_simple_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse");
    baseline.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.06);
    baseline.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.02);
    baseline.credit_model.recovery_spec.rate = 0.40;

    let market = MarketContext::new().insert(flat_discount_curve(0.04, test_date()));
    let metrics = [
        MetricId::Prepayment01,
        MetricId::Default01,
        MetricId::Recovery01,
        MetricId::Severity01,
    ];
    let baseline_result = baseline
        .price_with_metrics(
            &market,
            test_date(),
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("baseline structured-credit metrics");

    let mut shocked = baseline;
    shocked.scenario_pricing_overrides =
        ScenarioPricingOverrides::default().with_price_shock_pct(-0.10);
    let shocked_result = shocked
        .price_with_metrics(
            &market,
            test_date(),
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("shocked structured-credit metrics");

    assert!((shocked_result.value.amount() - baseline_result.value.amount() * 0.90).abs() < 1e-8);
    for metric in metrics {
        let baseline_measure = baseline_result.measures[metric.as_str()];
        let shocked_measure = shocked_result.measures[metric.as_str()];
        assert!(
            baseline_measure.abs() > 1e-8,
            "{} baseline should be materially non-zero",
            metric.as_str()
        );
        assert!(
            (shocked_measure - baseline_measure * 0.90).abs()
                <= 1e-10 * baseline_measure.abs().max(1.0),
            "{} should scale with the scenario-adjusted PV lifecycle: baseline={baseline_measure}, shocked={shocked_measure}",
            metric.as_str()
        );
    }
}

#[test]
fn test_structured_credit_registry_exposes_clo_warf() {
    let sc = StructuredCredit::new_clo(
        "TEST_CLO_WARF",
        create_simple_clo_pool(),
        create_simple_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse");

    let market = MarketContext::new().insert(flat_discount_curve(0.04, test_date()));

    let result = sc
        .price_with_metrics(
            &market,
            test_date(),
            &[MetricId::CloWarf],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("CLO metric request should succeed");

    assert!(
        result.measures.contains_key("clo_warf"),
        "CLO WARF should be computed through the metric registry"
    );
}

#[test]
fn test_structured_credit_registry_exposes_cmbs_dscr() {
    let mut sc = StructuredCredit::new_cmbs(
        "TEST_CMBS_DSCR",
        create_simple_cmbs_pool(),
        create_simple_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse");
    sc.credit_factors.annual_noi = Some(Money::new(1_250_000.0, Currency::USD));
    sc.credit_factors.annual_debt_service = Some(Money::new(1_000_000.0, Currency::USD));

    let market = MarketContext::new().insert(flat_discount_curve(0.04, test_date()));

    let result = sc
        .price_with_metrics(
            &market,
            test_date(),
            &[MetricId::CmbsDscr],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("CMBS metric request should succeed");

    assert!(
        result.measures.contains_key("cmbs_dscr"),
        "CMBS DSCR should be computed through the metric registry"
    );
}

#[test]
fn test_structured_credit_registry_wal_matches_cashflow_wal() {
    let sc = StructuredCredit::new_abs(
        "TEST_ABS_WAL",
        create_simple_pool(),
        create_simple_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse");

    let market = MarketContext::new().insert(flat_discount_curve(0.04, test_date()));
    let valuation = sc
        .price_with_metrics(
            &market,
            test_date(),
            &[MetricId::WAL],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("WAL metric request should succeed");
    // Library-self-calculated WAL regression benchmark.
    //
    // Re-blessed (cash-conservation remediation): the loss-allocation fix
    // (audit item 2) replaced the "cumulative loss vs original face" cap with
    // an incremental "loss vs current balance" cap. For this ABS deal — which
    // carries the `abs_auto_standard` default assumptions — that shifts the
    // tranche write-down schedule, hence the principal-payment timing the WAL
    // is weighted on.
    //   old: 2.2255397693750574   new: 2.226010097887204   (+0.00047 y, ~0.02%)
    // The benchmark is library-self-calculated (no external oracle), so the
    // golden-test policy permits a documented re-bless.
    //
    // :
    // (1) default/prepay/scheduled ordering now follows the stated
    //     Intex/Moody's & SIFMA convention — MDR applied to the
    //     BEGINNING-of-period balance, scheduled principal on the survivor,
    //     SMM last (previously scheduled → default → prepay); and
    // (2) the level-pay annuity uses the NOMINAL periodic rate
    //     (rate × months/12, US mortgage convention, matching
    //     mbs_passthrough) instead of effective compounding.
    // Both changes slightly front-load defaults/scheduled principal, so
    // principal returns marginally earlier and the WAL shortens.
    //   old: 2.226010097887204   new: 2.2250660687393284   (−0.00094 y, ~0.04%)
    // The canonical provider now seeds classified schedule rows. WAL therefore
    // weights only principal-like rows instead of the former final fallback,
    // which treated every positive cash settlement (including interest) as
    // principal. This is the deterministic principal-only benchmark.
    //   old: 2.2250660687393284   new: 2.342301092973921
    let expected = 2.342_301_092_973_921_f64;
    let actual = valuation.measures["wal"];

    assert!(
        (actual - expected).abs() < 1e-10,
        "Registry WAL {} should match the deterministic reporting benchmark {}",
        actual,
        expected
    );
}

#[test]
fn test_structured_credit_empty_metrics_request() {
    // Arrange
    let sc = StructuredCredit::new_clo(
        "TEST_CLO",
        create_simple_pool(),
        create_simple_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse");

    let market = MarketContext::new().insert(flat_discount_curve(0.04, test_date()));

    // Act: Request NO metrics
    let result = sc.price_with_metrics(
        &market,
        test_date(),
        &[],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(result.is_ok());
    let result = result.unwrap();
    assert!(result.measures.is_empty());
}

#[test]
fn test_structured_credit_metric_dependency_resolution() {
    // Arrange: CleanPrice depends on DirtyPrice and Accrued
    let sc = StructuredCredit::new_abs(
        "TEST_ABS",
        create_simple_pool(),
        create_simple_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse");

    let market = MarketContext::new().insert(flat_discount_curve(0.04, test_date()));

    // Act: Request only CleanPrice (dependencies should auto-compute)
    let result = sc.price_with_metrics(
        &market,
        test_date(),
        &[MetricId::CleanPrice],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(result.is_ok());
    let result = result.unwrap();
    assert!(result.measures.contains_key("clean_price"));
}

// ============================================================================
// Performance and Edge Cases
// ============================================================================

#[test]
fn test_structured_credit_pool_balance_cleanup() {
    // Arrange: AssetPool with very small remaining balance
    let mut pool = AssetPool::new("SMALL_POOL", DealType::ABS, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "A1",
        Money::new(50.0, Currency::USD), // Below cleanup threshold
        0.06,
        maturity_date(),
        finstack_quant_core::dates::DayCount::Thirty360,
    ));

    let tranches = create_simple_tranches();
    let sc = StructuredCredit::new_abs(
        "SMALL_ABS",
        pool,
        tranches,
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse");

    let market = MarketContext::new().insert(flat_discount_curve(0.04, test_date()));

    // Act
    let result = sc.dated_cashflows(&market, test_date());

    // Assert: Should handle small balances gracefully
    assert!(result.is_ok());
}

#[test]
fn test_sensitivity01_units_reconcile_with_reprice() {
    // Quant review Note: pin the producer unit conventions —
    // `Prepayment01` is $ per 1bp of CPR, `Recovery01` is $ per 1% of
    // recovery — by reconciling `metric × shift` against a true reprice with
    // the shifted parameter. This is the multiplication the public
    // attribution helpers (`measure_prepayment_shift` in bp,
    // `measure_recovery_shift` in pct-pt) document, so a producer/consumer
    // unit drift (the old per-unit figures were 10⁴× / 10²× larger) fails
    // loudly here.
    use finstack_quant_cashflows::builder::PrepaymentModelSpec;

    let mut sc = StructuredCredit::new_abs(
        "TEST_ABS_UNIT_RECON",
        create_simple_pool(),
        create_simple_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse");
    sc.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.06);
    sc.credit_model.recovery_spec.rate = 0.40;

    let market = MarketContext::new().insert(flat_discount_curve(0.04, test_date()));

    let result = sc
        .price_with_metrics(
            &market,
            test_date(),
            &[MetricId::Prepayment01, MetricId::Recovery01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("sensitivity metrics should compute");
    let base_pv = result.value.amount();
    let prepayment01 = *result
        .measures
        .get(MetricId::Prepayment01.as_str())
        .unwrap();
    let recovery01 = *result.measures.get(MetricId::Recovery01.as_str()).unwrap();

    // Reprice with CPR shifted by +20bp; first-order prediction must hold.
    let shift_bp = 20.0;
    let mut sc_up = sc.clone();
    sc_up.credit_model.prepayment_spec =
        PrepaymentModelSpec::constant_cpr(0.06 + shift_bp / 10_000.0);
    let pv_up = sc_up
        .value(&market, test_date())
        .expect("shifted reprice")
        .amount();
    let predicted = prepayment01 * shift_bp;
    let actual = pv_up - base_pv;
    assert!(
        (predicted - actual).abs() <= 0.10 * actual.abs().max(1.0),
        "Prepayment01 × Δbp must first-order match a reprice: predicted {predicted}, actual {actual}"
    );

    // Reprice with recovery shifted by +5 percentage points. (`sc` is no
    // longer needed: move it.)
    let shift_pct = 5.0;
    let mut sc_rec = sc;
    sc_rec.credit_model.recovery_spec.rate = 0.40 + shift_pct / 100.0;
    let pv_rec = sc_rec
        .value(&market, test_date())
        .expect("recovery reprice")
        .amount();
    let predicted = recovery01 * shift_pct;
    let actual = pv_rec - base_pv;
    assert!(
        (predicted - actual).abs() <= 0.10 * actual.abs().max(1.0),
        "Recovery01 × Δpct must first-order match a reprice: predicted {predicted}, actual {actual}"
    );
}
