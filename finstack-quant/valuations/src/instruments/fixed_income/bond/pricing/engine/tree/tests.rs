//! Pricing-engine components for fixed-income bonds.
//!
#![allow(clippy::expect_used, clippy::panic)]

use super::bond_valuator::BondValuator;
use super::tree_pricer::TreePricer;
use crate::instruments::fixed_income::bond::types::{Bond, CallPut, CallPutSchedule};
use crate::instruments::InstrumentPricingOverrides;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use time::Month;
fn create_test_bond() -> Bond {
    use crate::instruments::fixed_income::bond::CashflowSpec;

    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid test date");

    Bond::builder()
        .id("TEST_BOND".into())
        .notional(Money::from((
            1000_i64,
            finstack_quant_core::currency::Currency::USD,
        )))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(
            CashflowSpec::fixed(
                0.05,
                finstack_quant_core::dates::Tenor::semi_annual(),
                finstack_quant_core::dates::DayCount::Act365F,
            )
            .expect("finite test coupon"),
        )
        .discount_curve_id("USD-OIS".into())
        .credit_curve_id_opt(None)
        .instrument_pricing_overrides(
            InstrumentPricingOverrides::default().with_quoted_clean_price_pct(98.5),
        )
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
        start: call_date,
        end: call_date,
        price_pct_of_par: 102.0,
        make_whole: None,
    });
    bond.instrument_pricing_overrides.model_config.hw1f_sigma = Some(0.01);
    bond.instrument_pricing_overrides
        .model_config
        .hw1f_mean_reversion = Some(0.03);
    bond.call_put = Some(call_put);
    bond
}
fn create_make_whole_callable_bond() -> Bond {
    let mut bond = create_test_bond();
    let call_date = Date::from_calendar_date(2027, Month::January, 1).expect("Valid test date");
    let mut call_put = CallPutSchedule::default();
    call_put.calls.push(CallPut {
        start: call_date,
        end: call_date,
        price_pct_of_par: 102.0,
        make_whole: Some(crate::instruments::fixed_income::bond::MakeWholeSpec {
            reference_curve_id: CurveId::from("USD-TSY"),
            spread_bp: 25.0,
        }),
    });
    bond.instrument_pricing_overrides.model_config.hw1f_sigma = Some(0.01);
    bond.instrument_pricing_overrides
        .model_config
        .hw1f_mean_reversion = Some(0.03);
    bond.call_put = Some(call_put);
    bond
}
fn create_test_market_context() -> MarketContext {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let discount_curve =
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.85), (10.0, 0.70)])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("DiscountCurve builder should succeed with valid test data");
    let treasury_curve =
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-TSY")
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
/// Price `bond` on its own tree configuration at `oas_bp`, quote the result
/// as a clean price, and solve it back through the production OAS metric.
fn oas_metric_round_trip_bp(bond: &Bond, oas_bp: f64) -> f64 {
    use crate::instruments::common_impl::traits::Instrument;
    use crate::metrics::MetricId;

    let market = create_test_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let quote = crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext::new(
        bond, &market, as_of,
    )
    .expect("quote context");
    let pricer =
        TreePricer::with_config(super::bond_tree_config(bond, &market).expect("tree config"));
    let dirty = pricer
        .price_at_oas(bond, &market, quote.quote_date, oas_bp)
        .expect("price at OAS");
    let clean_pct = (dirty - quote.accrued_at_quote_date) / bond.notional.amount() * 100.0;
    let mut quoted = bond.clone();
    quoted
        .instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price_pct = Some(clean_pct);
    let result = quoted
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Oas],
            crate::instruments::PricingOptions::default().with_model(crate::pricer::ModelKey::Tree),
        )
        .expect("OAS metric");
    result.measures["oas"] * 10_000.0
}
#[test]
fn oas_metric_round_trips_a_tree_price_for_plain_and_callable_bonds() {
    let mut plain = create_test_bond();
    plain.instrument_pricing_overrides.model_config.hw1f_sigma = Some(0.01);
    plain
        .instrument_pricing_overrides
        .model_config
        .hw1f_mean_reversion = Some(0.03);
    for bond in [plain, create_callable_bond()] {
        let implied = oas_metric_round_trip_bp(&bond, 150.0);
        assert!(
            (implied - 150.0).abs() < 1.0e-4,
            "{}: OAS metric must recover 150bp, got {implied}",
            bond.id.as_str()
        );
    }
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
fn test_bond_valuator_maps_call_window_to_exercise_candidates() {
    // A call window is exercisable at its ends, the coupon dates inside it and
    // month-ends. Month-ends are closer together than this 0.1-year uniform
    // grid, so each of the 11 steps spanning 2027-01-01..2028-01-01 carries a
    // call.
    let bond = create_test_bond();
    let mut json = serde_json::to_value(&bond).expect("Bond serialization should succeed");
    json.as_object_mut()
        .expect("serialized bond should be an object")
        .insert(
            "call_put".to_string(),
            serde_json::json!({
                "calls": [{
                    "start": "2027-01-01",
                    "end": "2028-01-01",
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
        call_steps, 11,
        "every step inside the window should carry a call"
    );
}

#[test]
fn test_windowed_call_lowers_pv_vs_endpoint_only_exercise() {
    // More exercise opportunities can only increase the issuer option value,
    // so the windowed callable PV must be <= the single-endpoint callable PV.
    let market_context = create_test_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let pricer = TreePricer::new();

    let mut single = create_test_bond();
    let mut call_put = CallPutSchedule::default();
    call_put.calls.push(CallPut {
        start: Date::from_calendar_date(2027, Month::January, 1).expect("date"),
        end: Date::from_calendar_date(2027, Month::January, 1).expect("date"),
        price_pct_of_par: 100.0,
        make_whole: None,
    });
    single.call_put = Some(call_put);

    let mut windowed = create_test_bond();
    let mut call_put = CallPutSchedule::default();
    call_put.calls.push(CallPut {
        start: Date::from_calendar_date(2027, Month::January, 1).expect("date"),
        end: Date::from_calendar_date(2029, Month::January, 1).expect("date"),
        price_pct_of_par: 100.0,
        make_whole: None,
    });
    windowed.call_put = Some(call_put);

    let pv_single = pricer
        .price_at_oas(&single, &market_context, as_of, 0.0)
        .expect("single-date callable PV");
    let pv_windowed = pricer
        .price_at_oas(&windowed, &market_context, as_of, 0.0)
        .expect("windowed callable PV");

    assert!(
        pv_windowed <= pv_single + 1e-9,
        "windowed callable must not be worth more than endpoint-only: windowed={pv_windowed}, single={pv_single}"
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
        start: call_date,
        end: call_date,
        price_pct_of_par: 100.0,
        make_whole: None,
    });
    bond.instrument_pricing_overrides.model_config.hw1f_sigma = Some(0.01);
    bond.instrument_pricing_overrides
        .model_config
        .hw1f_mean_reversion = Some(0.03);
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
    use finstack_quant_core::market_data::term_structures::HazardCurve;
    use finstack_quant_core::HashMap;
    use finstack_quant_models::trees::two_factor_rates_credit::{
        RatesCreditConfig, RatesCreditTree,
    };

    let bond = create_test_bond();
    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    let discount_curve =
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
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
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (5.0, 0.85)])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("Curve builder should succeed with valid test data");
    let high_hazard2 =
        finstack_quant_core::market_data::term_structures::HazardCurve::builder("HAZ-HIGH")
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
            finstack_quant_core::dates::DayCountContext::default(),
        )
        .unwrap_or(0.0);
    let steps = 40usize;

    let maturity = bond.maturity;
    let valuator_low = BondValuator::new(bond.clone(), &ctx_low, as_of, time_to_maturity, steps)
        .expect("valuator");
    let valuator_high =
        BondValuator::new(bond, &ctx_high, as_of, time_to_maturity, steps).expect("valuator");

    use finstack_quant_models::TreeModel;
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
    let low_targets =
        crate::instruments::common_impl::pricing::rates_credit::build_rates_credit_targets(
            disc_low.as_ref(),
            low_hc_ref.as_ref(),
            as_of,
            maturity,
            time_to_maturity,
            steps,
        )
        .expect("low targets");
    tree_low.calibrate(&low_targets).expect("calibration low");

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
    let high_targets =
        crate::instruments::common_impl::pricing::rates_credit::build_rates_credit_targets(
            disc_high.as_ref(),
            high_hc_ref.as_ref(),
            as_of,
            maturity,
            time_to_maturity,
            steps,
        )
        .expect("high targets");
    tree_high
        .calibrate(&high_targets)
        .expect("calibration high");

    let vars = HashMap::<&'static str, f64>::default();

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
#[test]
fn bdt_tree_accepts_more_than_one_thousand_steps() {
    // The BDT step-alignment search used `clamp(tree_steps, 1000)`, which
    // panics (min > max) once tree_steps exceeds 1000.
    let mut bond = create_test_bond();
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start: Date::from_calendar_date(2027, Month::January, 1).expect("date"),
            end: Date::from_calendar_date(2027, Month::January, 1).expect("date"),
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: Vec::new(),
    });
    bond.instrument_pricing_overrides = InstrumentPricingOverrides::default();
    bond.instrument_pricing_overrides.model_config.bdt_sigma = Some(0.20);
    bond.instrument_pricing_overrides.model_config.tree_model =
        Some(crate::instruments::ShortRateTreeModel::BlackDermanToy);
    let mut config = super::bond_tree_config(&bond, &create_test_market_context()).expect("config");
    config.tree_steps = 1200;
    let market = create_test_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    let price = TreePricer::with_config(config)
        .price_at_oas(&bond, &market, as_of, 0.0)
        .expect("1,200-step BDT price");
    assert!(price.is_finite() && price > 0.0);
}

/// Callable bond with every pricing override cleared, so the tests below
/// control exactly which model inputs the tree sees.
fn bare_callable_bond() -> Bond {
    let mut bond = create_callable_bond();
    bond.instrument_pricing_overrides = InstrumentPricingOverrides::default();
    bond
}

/// Reference price from a Hull-White tree configured directly, bypassing the
/// bond's override resolution. The step count and compounding match what the
/// bond path uses (100 steps, default compounding), so the two must agree to
/// floating-point noise: the same calibrated lattice values the same bond.
fn direct_tree_price(bond: &Bond, tree_model: super::TreeModelChoice) -> f64 {
    let market = create_test_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    super::TreePricer::with_config(super::TreePricerConfig {
        tree_steps: 100,
        tree_model,
        ..super::TreePricerConfig::default()
    })
    .price_at_oas(bond, &market, as_of, 0.0)
    .expect("direct tree price")
}

fn bond_tree_price(bond: &Bond) -> finstack_quant_core::Result<f64> {
    let market = create_test_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    bond.price_for_model_raw(crate::pricer::ModelKey::Tree, &market, as_of)
}

#[test]
fn bond_tree_ignores_implied_volatility() {
    let mut hw_only = bare_callable_bond();
    hw_only
        .instrument_pricing_overrides
        .model_config
        .hw1f_mean_reversion = Some(0.05);
    hw_only.instrument_pricing_overrides.model_config.hw1f_sigma = Some(0.01);
    let mut with_option_vol = hw_only.clone();
    with_option_vol
        .instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.20);

    // Independent reference: the same lattice built from (κ, σ) directly.
    let reference = direct_tree_price(
        &hw_only,
        super::TreeModelChoice::HullWhite {
            kappa: 0.05,
            sigma: 0.01,
        },
    );
    for (label, bond) in [("hw1f only", hw_only), ("with option vol", with_option_vol)] {
        let pv = bond_tree_price(&bond).expect("Hull-White tree price");
        // Same kernel and inputs: agreement to 1e-9 of notional is floating
        // point noise, while a wrong κ (the old 0.03 default) or σ (the 0.20
        // option vol) moves this callable by whole price points.
        assert!(
            (pv - reference).abs() < 1e-9 * bond.notional.amount(),
            "{label}: tree PV {pv} must equal the direct (κ=0.05, σ=0.01) tree {reference}"
        );
    }
}

#[test]
fn bond_tree_requires_short_rate_sigma() {
    let option_vol_only = {
        let mut bond = bare_callable_bond();
        bond.instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.01);
        bond
    };
    let sigma_only = {
        let mut bond = bare_callable_bond();
        bond.instrument_pricing_overrides.model_config.hw1f_sigma = Some(0.01);
        bond
    };
    for (label, bond, expected) in [
        (
            "no volatility",
            bare_callable_bond(),
            "instrument_pricing_overrides.model_config.hw1f_sigma",
        ),
        (
            "option vol only",
            option_vol_only,
            "instrument_pricing_overrides.model_config.hw1f_sigma",
        ),
        (
            "sigma without mean reversion",
            sigma_only,
            "instrument_pricing_overrides.model_config.hw1f_mean_reversion",
        ),
    ] {
        let error = bond_tree_price(&bond).expect_err(label);
        assert!(error.to_string().contains(expected), "{label}: {error}");
    }
}

#[test]
fn bond_tree_reads_hw1f_parameters_from_market_scalars() {
    use finstack_quant_core::market_data::scalars::MarketScalar;

    let bond = bare_callable_bond();
    let (kappa_key, sigma_key) =
        finstack_quant_models::rates::hull_white::hw1f_scalar_keys("USD-OIS");
    let market = create_test_market_context()
        .insert_price(kappa_key.as_str(), MarketScalar::Unitless(0.05))
        .insert_price(sigma_key.as_str(), MarketScalar::Unitless(0.01));
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let pv = bond
        .price_for_model_raw(crate::pricer::ModelKey::Tree, &market, as_of)
        .expect("tree price from pre-fitted market scalars");
    let reference = direct_tree_price(
        &bond,
        super::TreeModelChoice::HullWhite {
            kappa: 0.05,
            sigma: 0.01,
        },
    );
    assert!((pv - reference).abs() < 1e-9 * bond.notional.amount());
}

#[test]
fn bond_bdt_tree_reads_bdt_sigma() {
    let mut bond = bare_callable_bond();
    bond.instrument_pricing_overrides.model_config = serde_json::from_value(serde_json::json!({
        "tree_model": "black_derman_toy",
        "bdt_sigma": 0.20
    }))
    .expect("bdt_sigma is a model_config field");
    let pv = bond_tree_price(&bond).expect("BDT tree price");
    let market = create_test_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let reference = super::TreePricer::with_config(super::TreePricerConfig {
        tree_steps: 100,
        tree_model: super::TreeModelChoice::BlackDermanToy { sigma: 0.20 },
        tree_compounding: finstack_quant_models::trees::TreeCompounding::Simple,
        ..super::TreePricerConfig::default()
    })
    .price_at_oas(&bond, &market, as_of, 0.0)
    .expect("direct BDT price");
    assert!((pv - reference).abs() < 1e-9 * bond.notional.amount());

    // The option-vol quote is not a BDT input.
    let mut option_vol_only = bare_callable_bond();
    option_vol_only
        .instrument_pricing_overrides
        .model_config
        .tree_model = Some(crate::instruments::ShortRateTreeModel::BlackDermanToy);
    option_vol_only
        .instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.20);
    let error = bond_tree_price(&option_vol_only).expect_err("BDT needs bdt_sigma");
    assert!(error
        .to_string()
        .contains("instrument_pricing_overrides.model_config.bdt_sigma"));
}

/// Risk-free (no `credit_curve_id`) callable floating-rate note on the
/// rates-only tree, with a par call on a coupon date.
fn risk_free_callable_frn(maturity: Date) -> Bond {
    let issue = Date::from_calendar_date(2025, Month::January, 15).expect("Valid test date");
    let call_date = Date::from_calendar_date(2027, Month::January, 15).expect("Valid test date");
    let mut bond = Bond::floating(
        "RISK_FREE_CALLABLE_FRN",
        Money::from((1000_i64, finstack_quant_core::currency::Currency::USD)),
        "USD-SOFR-3M",
        200,
        issue,
        maturity,
        finstack_quant_core::dates::Tenor::quarterly(),
        finstack_quant_core::dates::DayCount::Act360,
        "USD-OIS",
    )
    .expect("floating bond builds");
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start: call_date,
            end: call_date,
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: Vec::new(),
    });
    bond
}

fn frn_market() -> MarketContext {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let forward = finstack_quant_core::market_data::term_structures::ForwardCurve::builder(
        "USD-SOFR-3M",
        0.25,
    )
    .base_date(base_date)
    .knots([(0.0, 0.04), (10.0, 0.04)])
    .interp(InterpStyle::Linear)
    .build()
    .expect("forward curve");
    create_test_market_context().insert(forward)
}

#[test]
fn risk_free_floating_callable_prices_on_deterministic_hull_white_tree() {
    let market = frn_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("Valid test date");
    let price = |bond: &Bond, model: crate::pricer::ModelKey| {
        bond.price_for_model_raw(model, &market, as_of)
    };

    // Every remedy the rates-only tree's errors name must be one it accepts.
    let bare = risk_free_callable_frn(maturity);
    let error = price(&bare, crate::pricer::ModelKey::Tree).expect_err("no HW1F inputs");
    assert!(
        error
            .to_string()
            .contains("instrument_pricing_overrides.model_config.hw1f_sigma"),
        "{error}"
    );
    let mut stochastic = bare.clone();
    stochastic
        .instrument_pricing_overrides
        .model_config
        .hw1f_mean_reversion = Some(0.03);
    stochastic
        .instrument_pricing_overrides
        .model_config
        .hw1f_sigma = Some(0.01);
    let error = price(&stochastic, crate::pricer::ModelKey::Tree)
        .expect_err("stochastic rates-only tree preprojects floating coupons");
    assert!(
        error
            .to_string()
            .contains("instrument_pricing_overrides.model_config.hw1f_sigma = 0"),
        "{error}"
    );

    let mut deterministic = stochastic;
    deterministic
        .instrument_pricing_overrides
        .model_config
        .hw1f_sigma = Some(0.0);
    let pv = price(&deterministic, crate::pricer::ModelKey::Tree)
        .expect("hw1f_sigma = 0 prices a floating callable on the rates-only tree");

    // Independent reference: with deterministic rates the issuer exercises a
    // single par call on a coupon date iff the bond is worth more than par
    // there, so the callable is the cheaper of the straight FRN and the FRN
    // redeemed at par on the call date, both discounted off the curves.
    let mut straight = risk_free_callable_frn(maturity);
    straight.call_put = None;
    let mut to_call = risk_free_callable_frn(
        Date::from_calendar_date(2027, Month::January, 15).expect("Valid test date"),
    );
    to_call.call_put = None;
    let straight_pv = price(&straight, crate::pricer::ModelKey::Discounting).expect("straight");
    let to_call_pv = price(&to_call, crate::pricer::ModelKey::Discounting).expect("to call");
    let reference = straight_pv.min(to_call_pv);
    // SOFR + 200 bp over a ~3-4% discount curve trades above par, so the
    // call binds: the callable must sit strictly below the straight bond.
    assert!(
        to_call_pv < straight_pv,
        "call must bind: {to_call_pv} vs {straight_pv}"
    );
    // Both sides project the same coupons off the same curves; 1e-9 of
    // notional is floating-point noise, while ignoring the call would miss
    // by the (several-point) call value.
    assert!(
        (pv - reference).abs() < 1e-9 * deterministic.notional.amount(),
        "deterministic HW tree {pv} must equal min(straight, to-call) {reference}"
    );

    // Zero volatility still requires the mean reversion.
    let mut sigma_zero_only = bare;
    sigma_zero_only
        .instrument_pricing_overrides
        .model_config
        .hw1f_sigma = Some(0.0);
    let error = price(&sigma_zero_only, crate::pricer::ModelKey::Tree)
        .expect_err("hw1f_sigma = 0 without mean reversion");
    assert!(
        error
            .to_string()
            .contains("instrument_pricing_overrides.model_config.hw1f_mean_reversion"),
        "{error}"
    );
}

#[test]
fn bond_tree_model_selector_rejects_retired_vol_model_key() {
    let hull_white: crate::instruments::ModelConfig =
        serde_json::from_value(serde_json::json!({ "tree_model": "hull_white" }))
            .expect("tree_model is a model_config field");
    assert_eq!(
        hull_white.tree_model,
        Some(crate::instruments::ShortRateTreeModel::HullWhite)
    );
    // schema-rejection-test: `vol_model` is now `tree_model`
    let retired = serde_json::json!({ "vol_model": "black", "bdt_sigma": 0.20 });
    assert!(serde_json::from_value::<crate::instruments::ModelConfig>(retired).is_err());
}
