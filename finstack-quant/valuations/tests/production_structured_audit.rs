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
    let base = StructuredCredit::example().expect("example");
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
            .with_calendar_id("nyse")
            .value(&market(closing), closing)
            .is_ok());
    }
}

#[test]
fn production_structured_representative_line_matches_asset_cashflows() {
    let mut deal = StructuredCredit::example().expect("example");
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
fn production_structured_line_preserves_rates_recovery_and_seasoning() {
    let mut deal = StructuredCredit::example().expect("example");
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
fn production_structured_zero_vol_stochastic_matches_deterministic_with_constant_rates() {
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCreditPricingMode;
    let mut deal = StructuredCredit::example().expect("example");
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.1);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.05);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.7, 9);
    let as_of = deal.closing_date;
    let market = market(as_of);
    let deterministic = deal.value(&market, as_of).expect("deterministic");
    let stochastic = deal
        .price_stochastic_with_mode(
            &market,
            as_of,
            StructuredCreditPricingMode::MonteCarlo {
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
    let mut deal = StructuredCredit::example().expect("example");
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

/// Identity pin for the retired deal-level override channel: before its
/// removal, `StructuredCredit::example().expect("example")` overridden to a constant 23% CPR,
/// 12% CDR and 71% recovery with a nine-month lag valued at these exact bits.
/// The same assumptions stated on `credit_model` must reproduce them bit for
/// bit.
#[test]
fn production_structured_credit_model_reproduces_retired_override_value() {
    const OVERRIDE_VALUE_BITS: u64 = 0x4197_c7fb_a284_c023; // 99_745_512.6296392
    let mut deal = StructuredCredit::example().expect("example");
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.23);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.12);
    deal.credit_model.recovery_spec.rate = 0.71;
    deal.credit_model.recovery_spec.recovery_lag = 9;
    assert!(deal.credit_model.stochastic_prepay_spec.is_none());
    assert!(deal.credit_model.stochastic_default_spec.is_none());
    let as_of = deal.closing_date;
    let value = deal.value(&market(as_of), as_of).expect("value");
    assert_eq!(
        value.amount().to_bits(),
        OVERRIDE_VALUE_BITS,
        "{}",
        value.amount()
    );
}
