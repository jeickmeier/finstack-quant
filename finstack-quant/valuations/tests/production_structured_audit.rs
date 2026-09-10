//! Independent input-contract regressions for the structured-credit audit.

use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec, RepLine, StructuredCredit,
};
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

fn market(as_of: Date) -> MarketContext {
    MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (20.0, (-0.03_f64 * 20.0).exp())])
            .build()
            .expect("discount curve"),
    )
}

#[test]
fn production_structured_constructor_uses_closing_date() {
    let base = StructuredCredit::example();
    let closing = date!(2026 - 09 - 09);
    for (deal, expected) in [
        (
            StructuredCredit::new_abs(
                "ABS",
                base.pool.clone(),
                base.tranches.clone(),
                closing,
                base.maturity,
                "USD-OIS",
            ),
            date!(2026 - 10 - 09),
        ),
        (
            StructuredCredit::new_clo(
                "CLO",
                base.pool.clone(),
                base.tranches.clone(),
                closing,
                base.maturity,
                "USD-OIS",
            ),
            date!(2026 - 12 - 09),
        ),
        (
            StructuredCredit::new_cmbs(
                "CMBS",
                base.pool.clone(),
                base.tranches.clone(),
                closing,
                base.maturity,
                "USD-OIS",
            ),
            date!(2026 - 10 - 09),
        ),
        (
            StructuredCredit::new_rmbs(
                "RMBS",
                base.pool.clone(),
                base.tranches.clone(),
                closing,
                base.maturity,
                "USD-OIS",
            ),
            date!(2026 - 10 - 09),
        ),
    ] {
        assert_eq!(deal.first_payment_date, expected);
        assert!(deal
            .with_payment_calendar("nyse")
            .value(&market(closing), closing)
            .is_ok());
    }
}

#[test]
fn production_structured_representative_line_matches_asset_cashflows() {
    let mut deal = StructuredCredit::example();
    deal.first_payment_date = date!(2024 - 04 - 01);
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    let as_of = deal.closing_date;
    let market = market(as_of);
    let expected = deal.value(&market, as_of).expect("asset deal");
    let asset = deal.pool.assets.remove(0);
    deal.pool.rep_lines = Some(vec![RepLine::new(
        "REP",
        asset.balance,
        asset.rate,
        asset.maturity,
        DayCount::Act360,
        asset.asset_type.clone(),
    )]);
    assert_eq!(
        deal.pool.total_balance().expect("representative balance"),
        asset.balance
    );
    let actual = deal.value(&market, as_of).expect("representative deal");
    assert!((actual.amount() - expected.amount()).abs() < 1e-6);
    deal.pool.assets.push(asset);
    assert!(
        deal.value(&market, as_of).is_err(),
        "duplicate collateral representations must fail"
    );
}

#[test]
fn production_structured_recovery_override_matches_model_configuration() {
    let mut deal = StructuredCredit::example();
    deal.first_payment_date = date!(2024 - 04 - 01);
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.20);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.70, 9);
    let as_of = deal.closing_date;
    let market = market(as_of);
    let expected = deal.value(&market, as_of).expect("configured recovery");
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.10, 0);
    deal.behavior_overrides.recovery_rate = Some(0.70);
    deal.behavior_overrides.recovery_lag_months = Some(9);
    let actual = deal.value(&market, as_of).expect("overridden recovery");
    assert!(
        (actual.amount() - expected.amount()).abs() < 1e-6,
        "override {} != canonical {}",
        actual.amount(),
        expected.amount()
    );
}

#[test]
fn production_structured_scenario_grid_overrides_resolved_assumptions() {
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
        scenario_table, ScenarioGrid,
    };
    let mut deal = StructuredCredit::example();
    deal.behavior_overrides.cdr_annual = Some(0.2);
    deal.behavior_overrides.recovery_rate = Some(0.7);
    deal.behavior_overrides.recovery_lag_months = Some(9);
    let as_of = deal.closing_date;
    let grid = ScenarioGrid {
        cprs: vec![0.0],
        cdrs: vec![0.0, 0.3],
        severities: vec![0.5],
        recovery_lag: None,
    };
    let overridden = scenario_table(
        &deal,
        deal.tranches.tranches[0].id.as_str(),
        &market(as_of),
        as_of,
        &grid,
    )
    .expect("scenario table");
    deal.behavior_overrides.cdr_annual = None;
    deal.behavior_overrides.recovery_rate = None;
    deal.behavior_overrides.recovery_lag_months = None;
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.7, 9);
    let canonical = scenario_table(
        &deal,
        deal.tranches.tranches[0].id.as_str(),
        &market(as_of),
        as_of,
        &grid,
    )
    .expect("canonical table");
    for (actual, expected) in overridden.cells.iter().zip(&canonical.cells) {
        assert!(
            (actual.price - expected.price).abs() < 1e-9,
            "actual {}, expected {}",
            actual.price,
            expected.price
        );
    }
}

#[test]
fn production_structured_line_preserves_rates_recovery_and_seasoning() {
    let mut deal = StructuredCredit::example();
    let asset = &mut deal.pool.assets[0];
    asset.smm_override = Some(0.01);
    asset.mdr_override = Some(0.02);
    asset.recovery_rate = Some(0.8);
    asset.acquisition_date = Some(date!(2023 - 01 - 01));
    let as_of = deal.closing_date;
    let expected = deal.value(&market(as_of), as_of).expect("canonical asset");
    let asset = deal.pool.assets.remove(0);
    let mut line = RepLine::new(
        "REP",
        asset.balance,
        asset.rate,
        asset.maturity,
        asset.day_count,
        asset.asset_type,
    );
    line.seasoning_months = 12;
    line.cpr = Some(1.0 - 0.99_f64.powi(12));
    line.cdr = Some(1.0 - 0.98_f64.powi(12));
    line.recovery_rate = Some(0.8);
    deal.pool.rep_lines = Some(vec![line]);
    let actual = deal
        .value(&market(as_of), as_of)
        .expect("representative collateral");
    assert!((actual.amount() - expected.amount()).abs() < 1e-6);
}

#[test]
fn production_structured_zero_vol_stochastic_matches_deterministic_with_overrides() {
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::PricingMode;
    let mut deal = StructuredCredit::example();
    deal.behavior_overrides.cpr_annual = Some(0.1);
    deal.behavior_overrides.cdr_annual = Some(0.05);
    deal.behavior_overrides.recovery_rate = Some(0.7);
    deal.behavior_overrides.recovery_lag_months = Some(9);
    let as_of = deal.closing_date;
    let market = market(as_of);
    let deterministic = deal.value(&market, as_of).expect("deterministic");
    let stochastic = deal
        .price_stochastic_with_mode(
            &market,
            as_of,
            PricingMode::MonteCarlo {
                num_paths: 1,
                antithetic: false,
            },
        )
        .expect("zero-vol stochastic");
    assert!(
        (stochastic.npv.amount() - deterministic.amount()).abs() < 1e-6,
        "stochastic {}, deterministic {}",
        stochastic.npv.amount(),
        deterministic.amount()
    );
}

#[test]
fn production_waterfall_credit_enhancement_uses_current_collateral_and_cash() {
    use finstack_quant_core::{currency::Currency, money::Money};
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::AbsCreditEnhancementCalculator;
    use finstack_quant_valuations::metrics::{MetricCalculator, MetricContext};
    use std::sync::Arc;
    let mut deal = StructuredCredit::example();
    deal.tranches.tranches[0].current_balance =
        Money::new(80_000_000.0, Currency::USD).expect("notes");
    deal.pool.reserve_account = Money::new(20_000_000.0, Currency::USD).expect("reserve");
    let as_of = deal.closing_date;
    deal.value(&market(as_of), as_of)
        .expect("seasoned current balances remain valid through full loss allocation");
    let mut context = MetricContext::new(
        Arc::new(deal),
        Arc::new(market(as_of)),
        as_of,
        Money::new(0.0, Currency::USD).expect("PV"),
        MetricContext::default_config(),
    );
    let actual = AbsCreditEnhancementCalculator
        .calculate(&mut context)
        .expect("enhancement");
    // 100 collateral + 20 reserve supports 80 senior notes: 40 / 120 enhancement.
    assert!((actual - 100.0 / 3.0).abs() < 1e-12, "{actual}");
}
