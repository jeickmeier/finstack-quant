#![allow(clippy::expect_used, clippy::panic)]

use super::bond_valuator::BondValuator;
use super::tree_pricer::TreePricer;
use crate::instruments::fixed_income::bond::types::{Bond, CallPut, CallPutSchedule};
use crate::instruments::PricingOverrides;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use time::Month;
fn create_test_bond() -> Bond {
    use crate::instruments::fixed_income::bond::CashflowSpec;

    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid test date");

    Bond::builder()
        .id("TEST_BOND".into())
        .notional(Money::new(1000.0, finstack_core::currency::Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(
            CashflowSpec::fixed(
                0.05,
                finstack_core::dates::Tenor::semi_annual(),
                finstack_core::dates::DayCount::Act365F,
            )
            .expect("finite test coupon"),
        )
        .discount_curve_id("USD-OIS".into())
        .credit_curve_id_opt(None)
        .pricing_overrides(PricingOverrides::default().with_quoted_clean_price(98.5))
        .call_put_opt(None)
        .custom_cashflows_opt(None)
        .attributes(Default::default())
        .settlement_convention_opt(Some(
            crate::instruments::fixed_income::bond::BondSettlementConvention {
                settlement_days: 2,
                ..Default::default()
            },
        ))
        .build()
        .expect("Bond builder should succeed with valid test data")
}
fn create_callable_bond() -> Bond {
    let mut bond = create_test_bond();
    let call_date = Date::from_calendar_date(2027, Month::January, 1).expect("Valid test date");
    let mut call_put = CallPutSchedule::default();
    call_put.calls.push(CallPut {
        start_date: call_date,
        end_date: call_date,
        price_pct_of_par: 102.0,
        make_whole: None,
    });
    bond.call_put = Some(call_put);
    bond
}
fn create_make_whole_callable_bond() -> Bond {
    let mut bond = create_test_bond();
    let call_date = Date::from_calendar_date(2027, Month::January, 1).expect("Valid test date");
    let mut call_put = CallPutSchedule::default();
    call_put.calls.push(CallPut {
        start_date: call_date,
        end_date: call_date,
        price_pct_of_par: 102.0,
        make_whole: Some(crate::instruments::fixed_income::bond::MakeWholeSpec {
            reference_curve_id: CurveId::from("USD-TSY"),
            spread_bps: 25.0,
        }),
    });
    bond.call_put = Some(call_put);
    bond
}
fn create_test_market_context() -> MarketContext {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let discount_curve =
        finstack_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.85), (10.0, 0.70)])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("DiscountCurve builder should succeed with valid test data");
    let treasury_curve =
        finstack_core::market_data::term_structures::DiscountCurve::builder("USD-TSY")
            .base_date(base_date)
            .knots([(0.0, 1.0), (1.0, 0.985), (5.0, 0.93), (10.0, 0.86)])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("Treasury curve should build");
    MarketContext::new()
        .insert(discount_curve)
        .insert(treasury_curve)
}
#[test]
fn test_bond_valuator_creation() {
    let bond = create_test_bond();
    let market_context = create_test_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let valuator = BondValuator::new(bond, &market_context, as_of, 5.0, 50);
    assert!(valuator.is_ok());
    let valuator = valuator.expect("BondValuator creation should succeed in test");
    assert!(valuator.cashflow_vec.iter().any(|&c| c > 0.0));
    assert!(market_context.get_discount("USD-OIS").is_ok());
}
#[test]
fn test_oas_calculator_plain_bond() {
    let bond = create_test_bond();
    let market_context = create_test_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let calculator = TreePricer::new();
    let oas = calculator.calculate_oas(&bond, &market_context, as_of, 98.5);
    assert!(oas.is_ok());
    let oas_bp = oas.expect("OAS calculation should succeed in test");
    assert!(oas_bp > 0.0);
    assert!(oas_bp < 5000.0);
}
#[test]
fn test_oas_calculator_callable_bond() {
    let bond = create_callable_bond();
    let market_context = create_test_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let calculator = TreePricer::new();
    let oas = calculator.calculate_oas(&bond, &market_context, as_of, 98.5);
    assert!(oas.is_ok());
    let oas_bp = oas.expect("OAS calculation should succeed in test");
    assert!(oas_bp > 0.0);
}
#[test]
fn test_bond_valuator_with_calls() {
    let bond = create_callable_bond();
    let market_context = create_test_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let valuator = BondValuator::new(bond, &market_context, as_of, 5.0, 50)
        .expect("BondValuator creation should succeed in test");
    assert!(valuator.call_vec.iter().any(|c| c.is_some()));
    assert!(valuator.put_vec.iter().all(|p| p.is_none()));
}

#[test]
fn test_bond_valuator_maps_call_period_to_listed_endpoint_steps() {
    let bond = create_test_bond();
    let mut json = serde_json::to_value(&bond).expect("Bond serialization should succeed");
    json.as_object_mut()
        .expect("serialized bond should be an object")
        .insert(
            "call_put".to_string(),
            serde_json::json!({
                "calls": [{
                    "start_date": "2027-01-01",
                    "end_date": "2028-01-01",
                    "price_pct_of_par": 101.0
                }],
                "puts": []
            }),
        );
    let bond: Bond = serde_json::from_value(json).expect("bond should accept call periods");

    let market_context = create_test_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let valuator = BondValuator::new(bond, &market_context, as_of, 5.0, 50)
        .expect("BondValuator creation should succeed in test");

    let call_steps = valuator.call_vec.iter().filter(|c| c.is_some()).count();
    assert_eq!(
        call_steps, 2,
        "call period should map only listed start/end exercise dates, not interior coupon dates"
    );
}

#[test]
fn test_bond_valuator_make_whole_call_exceeds_floor_when_reference_curve_is_low() {
    let bond = create_make_whole_callable_bond();
    let market_context = create_test_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let valuator = BondValuator::new(bond, &market_context, as_of, 5.0, 50)
        .expect("BondValuator creation should succeed in test");

    let (call_step, call_price) = valuator
        .call_vec
        .iter()
        .enumerate()
        .find_map(|(idx, price)| price.map(|value| (idx, value)))
        .expect("call price should be present");
    let floor_price = valuator.outstanding_principal_vec[call_step] * 1.02;

    assert!(
        call_price >= floor_price,
        "make-whole call price should never fall below floor: call_price={call_price}, floor={floor_price}"
    );
    assert!(
        call_price > floor_price,
        "make-whole call price should exceed floor with lower treasury curve: call_price={call_price}, floor={floor_price}"
    );
}

#[test]
fn test_bond_valuator_street_call_redemption_includes_accrued_interest() {
    let mut bond = create_test_bond();
    let call_date = Date::from_calendar_date(2027, Month::April, 1).expect("Valid test date");
    let mut call_put = CallPutSchedule::default();
    call_put.calls.push(CallPut {
        start_date: call_date,
        end_date: call_date,
        price_pct_of_par: 100.0,
        make_whole: None,
    });
    bond.call_put = Some(call_put);

    let market_context = create_test_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let valuator = BondValuator::new(bond, &market_context, as_of, 5.0, 50)
        .expect("BondValuator creation should succeed in test");

    let (call_step, call_price) = valuator
        .call_vec
        .iter()
        .enumerate()
        .find_map(|(idx, price)| price.map(|value| (idx, value)))
        .expect("call price should be present");
    let floor_price = valuator.outstanding_principal_vec[call_step];

    assert!(
        call_price > floor_price,
        "off-cycle clean street call should settle with accrued interest: call_price={call_price}, floor={floor_price}"
    );
}

#[test]
fn test_rates_credit_default_lowers_price() {
    use crate::instruments::common_impl::models::trees::two_factor_rates_credit::{
        RatesCreditConfig, RatesCreditTree,
    };
    use crate::instruments::common_impl::models::StateVariables;
    use finstack_core::market_data::term_structures::HazardCurve;

    let bond = create_test_bond();
    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    let discount_curve =
        finstack_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (5.0, 0.85)])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("Curve builder should succeed with valid test data");

    let low_hazard = HazardCurve::builder("HAZ-LOW")
        .base_date(base_date)
        .recovery_rate(0.4)
        .knots([(0.0, 0.01), (5.0, 0.01)])
        .build()
        .expect("Curve builder should succeed with valid test data");
    let _high_hazard = HazardCurve::builder("HAZ-HIGH")
        .base_date(base_date)
        .recovery_rate(0.4)
        .knots([(0.0, 0.05), (5.0, 0.05)])
        .build()
        .expect("Curve builder should succeed with valid test data");

    let ctx_low = MarketContext::new()
        .insert(discount_curve)
        .insert(low_hazard);
    let discount_curve2 =
        finstack_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (5.0, 0.85)])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("Curve builder should succeed with valid test data");
    let high_hazard2 =
        finstack_core::market_data::term_structures::HazardCurve::builder("HAZ-HIGH")
            .base_date(base_date)
            .recovery_rate(0.4)
            .knots([(0.0, 0.05), (5.0, 0.05)])
            .build()
            .expect("Curve builder should succeed with valid test data");
    let ctx_high = MarketContext::new()
        .insert(discount_curve2)
        .insert(high_hazard2);

    let as_of = base_date;
    let time_to_maturity = bond
        .cashflow_spec
        .day_count()
        .year_fraction(
            as_of,
            bond.maturity,
            finstack_core::dates::DayCountContext::default(),
        )
        .unwrap_or(0.0);
    let steps = 40usize;

    let valuator_low = BondValuator::new(bond.clone(), &ctx_low, as_of, time_to_maturity, steps)
        .expect("valuator");
    let valuator_high = BondValuator::new(bond.clone(), &ctx_high, as_of, time_to_maturity, steps)
        .expect("valuator");

    use crate::instruments::common_impl::models::TreeModel;
    let disc_low = ctx_low
        .get_discount("USD-OIS")
        .expect("Discount curve should exist");
    let low_hc_ref = ctx_low
        .get_hazard("HAZ-LOW")
        .expect("Hazard curve should exist in test context");
    let mut tree_low = RatesCreditTree::new(RatesCreditConfig {
        steps,
        ..Default::default()
    });
    tree_low
        .calibrate(disc_low.as_ref(), low_hc_ref.as_ref(), time_to_maturity)
        .expect("calibration low");

    let disc_high = ctx_high
        .get_discount("USD-OIS")
        .expect("Discount curve should exist");
    let high_hc_ref = ctx_high
        .get_hazard("HAZ-HIGH")
        .expect("Hazard curve should exist in test context");
    let mut tree_high = RatesCreditTree::new(RatesCreditConfig {
        steps,
        ..Default::default()
    });
    tree_high
        .calibrate(disc_high.as_ref(), high_hc_ref.as_ref(), time_to_maturity)
        .expect("calibration high");

    let vars = StateVariables::default();

    let pv_low = tree_low
        .price(vars.clone(), time_to_maturity, &ctx_low, &valuator_low)
        .expect("price low");

    let pv_high = tree_high
        .price(vars, time_to_maturity, &ctx_high, &valuator_high)
        .expect("price high");

    assert!(pv_high < pv_low, "pv_high={} pv_low={}", pv_high, pv_low);
}
#[test]
fn test_accrued_interest_via_quote_context() {
    use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;

    let bond = create_test_bond();
    let market_context = create_test_market_context();

    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let ctx_issue = QuoteDateContext::new(&bond, &market_context, issue)
        .expect("QuoteDateContext should succeed in test");
    assert!(
        ctx_issue.accrued_at_quote_date >= 0.0,
        "Accrued at issue quote_date should be non-negative"
    );

    let mid_period = Date::from_calendar_date(2025, Month::April, 1).expect("Valid test date");
    let ctx_mid = QuoteDateContext::new(&bond, &market_context, mid_period)
        .expect("QuoteDateContext should succeed in test");
    assert!(
        ctx_mid.accrued_at_quote_date > 0.0,
        "Accrued mid-period should be positive"
    );
}
