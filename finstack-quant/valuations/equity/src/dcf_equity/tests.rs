use super::types::*;
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::{context::MarketContext, term_structures::DiscountCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::{MetricContext, MetricId};
use time::Month;

fn build_simple_dcf_gordon() -> DiscountedCashFlow {
    let valuation_date =
        Date::from_calendar_date(2025, Month::January, 1).expect("valid test valuation date");
    let cf_date =
        Date::from_calendar_date(2026, Month::January, 1).expect("valid test cashflow date");

    DiscountedCashFlow {
        id: InstrumentId::new("TEST-DCF-GORDON"),
        currency: Currency::USD,
        flows: vec![(cf_date, Money::from((100_i64, Currency::USD)))],
        wacc: 0.10,
        terminal_value: TerminalValueSpec::GordonGrowth {
            stable_growth_rate: 0.02,
        },
        valuation_date,
        forecast_timing: ForecastTiming::DatedReceipts,
        terminal_date: cf_date,
        normalized_annual_fcff: Some(Money::from((100_i64, Currency::USD))),
        equity_bridge: EquityBridge::default(),
        shares_outstanding: None,
        dilution_securities: Vec::new(),
        valuation_discounts: None,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::default(),
    }
}

#[test]
fn validate_accepts_simple_dcf() {
    assert!(build_simple_dcf_gordon().validate().is_ok());
}

#[test]
fn validate_rejects_non_finite_wacc() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.wacc = f64::NAN;
    assert!(dcf.validate().is_err());
}

#[test]
fn validate_rejects_wacc_at_minus_one() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.wacc = -1.0;
    let err = dcf.validate().expect_err("wacc=-1 must error");
    assert!(err.to_string().contains("(1 + wacc) <= 0"));
}

#[test]
fn validate_accepts_finite_positive_wacc_above_one() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.wacc = 1.1;
    assert!(dcf.validate().is_ok());
}

#[test]
fn validate_accepts_wacc_above_minus_one_when_growth_is_lower() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.wacc = -0.70;
    dcf.terminal_value = TerminalValueSpec::GordonGrowth {
        stable_growth_rate: -0.80,
    };
    assert!(dcf.validate().is_ok());
}

#[test]
fn validate_rejects_unsorted_flows() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.flows = vec![
        (
            Date::from_calendar_date(2027, Month::January, 1).unwrap(),
            Money::from((100_i64, Currency::USD)),
        ),
        (
            Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            Money::from((100_i64, Currency::USD)),
        ),
    ];
    let err = dcf.validate().expect_err("unsorted must error");
    assert!(err.to_string().contains("strictly ordered"));
}

#[test]
fn validate_rejects_flow_on_or_before_valuation_date() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.flows = vec![(dcf.valuation_date, Money::from((100_i64, Currency::USD)))];
    let err = dcf
        .validate()
        .expect_err("flow on valuation_date must error");
    assert!(err.to_string().contains("after the valuation date"));
}

#[test]
fn validate_rejects_non_positive_shares_outstanding() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.shares_outstanding = Some(0.0);
    assert!(dcf.validate().is_err());
    dcf.shares_outstanding = Some(-100.0);
    assert!(dcf.validate().is_err());
}

#[test]
fn validate_rejects_non_finite_terminal_value_params() {
    // NaN Gordon growth must fail validation (would bypass wacc > g).
    let mut dcf = build_simple_dcf_gordon();
    dcf.terminal_value = TerminalValueSpec::GordonGrowth {
        stable_growth_rate: f64::NAN,
    };
    assert!(dcf.validate().is_err());

    // NaN H-Model parameters must fail validation.
    let mut dcf = build_simple_dcf_gordon();
    dcf.terminal_value = TerminalValueSpec::HModel {
        high_growth_rate: f64::NAN,
        stable_growth_rate: 0.02,
        half_life_years: 5.0,
    };
    assert!(dcf.validate().is_err());

    // Non-finite exit-multiple inputs must fail validation.
    let mut dcf = build_simple_dcf_gordon();
    dcf.terminal_value = TerminalValueSpec::ExitMultiple {
        terminal_metric: Some(f64::INFINITY),
        multiple: 10.0,
    };
    assert!(dcf.validate().is_err());
}

#[test]
fn standalone_exit_multiple_requires_a_resolved_metric() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.normalized_annual_fcff = None;
    dcf.terminal_value = TerminalValueSpec::ExitMultiple {
        terminal_metric: None,
        multiple: 10.0,
    };
    let error = dcf.validate().expect_err("unresolved terminal metric");
    assert!(error.to_string().contains("requires an enterprise metric"));
}

#[test]
fn validate_rejects_negative_exit_multiple_product() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.normalized_annual_fcff = None;
    dcf.terminal_value = TerminalValueSpec::ExitMultiple {
        terminal_metric: Some(-100.0),
        multiple: 10.0,
    };
    let err = dcf.validate().expect_err("negative TV product must error");
    assert!(err.to_string().contains("nonnegative"));

    // A sign-consistent (non-negative) product passes.
    dcf.terminal_value = TerminalValueSpec::ExitMultiple {
        terminal_metric: Some(100.0),
        multiple: 10.0,
    };
    assert!(dcf.validate().is_ok());
}

#[test]
fn growth_requires_authored_normalized_annual_fcff_regardless_of_grid() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.normalized_annual_fcff = None;
    let error = dcf
        .validate()
        .expect_err("annual forecast does not establish terminal basis");
    assert!(error.to_string().contains("normalized annual FCFF"));
    dcf.normalized_annual_fcff = Some(Money::from((100_i64, Currency::USD)));
    assert!(dcf.validate().is_ok());
}

#[test]
fn validate_rejects_terminal_fcff_in_a_different_currency() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.normalized_annual_fcff = Some(Money::from((100_i64, Currency::GBP)));
    assert!(dcf.validate().is_err());
}

#[test]
fn terminal_value_guards_fail_closed_on_nan() {
    // NaN growth must error at terminal-value time, not produce NaN.
    let mut dcf = build_simple_dcf_gordon();
    dcf.terminal_value = TerminalValueSpec::GordonGrowth {
        stable_growth_rate: f64::NAN,
    };
    assert!(dcf.calculate_terminal_value().is_err());

    let mut dcf = build_simple_dcf_gordon();
    dcf.terminal_value = TerminalValueSpec::HModel {
        high_growth_rate: 0.10,
        stable_growth_rate: f64::NAN,
        half_life_years: 5.0,
    };
    assert!(dcf.calculate_terminal_value().is_err());
}

#[test]
fn normalized_annual_fcff_drives_growth_perpetuity_tv() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.normalized_annual_fcff = Some(Money::from((400_i64, Currency::USD)));

    let tv = dcf.calculate_terminal_value().expect("terminal value");
    let expected = 400.0 * 1.02 / (0.10 - 0.02);
    assert!(
        (tv - expected).abs() < 1e-9,
        "TV should capitalize the override, expected {expected}, got {tv}"
    );

    // Exit multiples require their own separately authored metric.
    dcf.terminal_value = TerminalValueSpec::ExitMultiple {
        terminal_metric: Some(50.0),
        multiple: 10.0,
    };
    assert!(dcf.calculate_terminal_value().is_err());
    dcf.normalized_annual_fcff = None;
    let tv = dcf.calculate_terminal_value().expect("terminal value");
    assert!((tv - 500.0).abs() < 1e-12);
}

#[test]
fn asymmetric_forecast_periods_use_exact_midpoints_and_independent_terminal_date() {
    let mut dcf = build_simple_dcf_gordon();
    let date = dcf.valuation_date;
    let first_end = date + time::Duration::days(90);
    let final_end = date + time::Duration::days(365);
    dcf.flows = vec![
        (first_end, Money::from((15_i64, Currency::USD))),
        (final_end, Money::from((85_i64, Currency::USD))),
    ];
    dcf.forecast_timing = ForecastTiming::Midpoints {
        period_starts: vec![date, first_end],
    };
    dcf.terminal_date = final_end;
    dcf.normalized_annual_fcff = Some(Money::from((120_i64, Currency::USD)));
    dcf.wacc = 0.08;
    assert_eq!(
        dcf.explicit_flow_discount_years(first_end).unwrap(),
        45.0 / 365.25
    );
    assert_eq!(
        dcf.explicit_flow_discount_years(final_end).unwrap(),
        227.5 / 365.25
    );
    assert_eq!(dcf.terminal_discount_years().unwrap(), 365.0 / 365.25);
    assert!((dcf.calculate_pv_explicit_flows().unwrap() - 95.879998631628).abs() < 1e-10);
    let value = crate::dcf_equity::pricer::pv_with_rf_bump(&dcf, 0.0).unwrap();
    assert!((value - 1984.868391069042).abs() < 1e-9);
    let up = crate::dcf_equity::pricer::pv_with_rf_bump(&dcf, 0.0001).unwrap();
    let down = crate::dcf_equity::pricer::pv_with_rf_bump(&dcf, -0.0001).unwrap();
    assert!(((up - down) / 2.0 + 3.327952064358).abs() < 1e-9);
}

#[test]
fn terminal_date_may_follow_the_explicit_forecast_but_cannot_precede_it() {
    let mut dcf = build_simple_dcf_gordon();
    let receipt = dcf.flows[0].0;
    dcf.terminal_date = receipt + time::Duration::days(2);
    assert_eq!(dcf.terminal_discount_years().unwrap(), 367.0 / 365.25);
    dcf.terminal_date = receipt - time::Duration::days(1);
    assert!(dcf.validate().is_err());
}

#[test]
fn gordon_growth_terminal_value_matches_formula() {
    let dcf = build_simple_dcf_gordon();

    let tv = dcf
        .calculate_terminal_value()
        .expect("terminal value should succeed");

    // TV = FCF_terminal × (1 + g) / (WACC - g)
    let expected_tv = 100.0 * (1.0 + 0.02) / (0.10 - 0.02);
    let diff = (tv - expected_tv).abs();
    assert!(
        diff < 1e-9,
        "terminal value mismatch: got {}, expected {}, diff {}",
        tv,
        expected_tv,
        diff
    );
}

#[test]
fn exit_multiple_terminal_value_matches_product() {
    let valuation_date =
        Date::from_calendar_date(2025, Month::January, 1).expect("valid test valuation date");
    let cf_date =
        Date::from_calendar_date(2026, Month::January, 1).expect("valid test cashflow date");

    let dcf = DiscountedCashFlow {
        id: InstrumentId::new("TEST-DCF-EXIT"),
        currency: Currency::USD,
        flows: vec![(cf_date, Money::from((100_i64, Currency::USD)))],
        wacc: 0.10,
        terminal_value: TerminalValueSpec::ExitMultiple {
            terminal_metric: Some(150.0),
            multiple: 8.0,
        },
        valuation_date,
        forecast_timing: ForecastTiming::DatedReceipts,
        terminal_date: cf_date,
        normalized_annual_fcff: None,
        equity_bridge: EquityBridge::default(),
        shares_outstanding: None,
        dilution_securities: Vec::new(),
        valuation_discounts: None,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::default(),
    };

    let tv = dcf
        .calculate_terminal_value()
        .expect("terminal value should succeed");
    let expected_tv = 150.0 * 8.0;
    assert!(
        (tv - expected_tv).abs() < 1e-9,
        "exit multiple TV mismatch: got {}, expected {}",
        tv,
        expected_tv
    );
}

#[test]
fn npv_gordon_growth_matches_manual_calculation() {
    let dcf = build_simple_dcf_gordon();
    let market = MarketContext::new();
    let as_of = dcf.valuation_date;

    let money = dcf.value(&market, as_of).expect("value should succeed");
    let npv = money.amount();

    // Manual NPV:
    // PV_explicit = 100 / (1.10)^1
    // TV = 100 * (1.02) / (0.10 - 0.02)
    // PV_TV = TV / (1.10)^1
    let pv_explicit = 100.0 / 1.10_f64.powf(1.0);
    let tv = 100.0 * (1.0 + 0.02) / (0.10 - 0.02);
    let pv_tv = tv / 1.10_f64.powf(1.0);
    let expected_npv = pv_explicit + pv_tv;

    let diff = (npv - expected_npv).abs();
    // Allow for minor rounding differences introduced by Money's fixed-point representation.
    assert!(
        diff < 0.1,
        "NPV mismatch: got {}, expected {}, diff {}",
        npv,
        expected_npv,
        diff
    );
}

#[test]
fn npv_errors_when_wacc_not_greater_than_growth() {
    let valuation_date =
        Date::from_calendar_date(2025, Month::January, 1).expect("valid test valuation date");
    let cf_date =
        Date::from_calendar_date(2026, Month::January, 1).expect("valid test cashflow date");

    let dcf = DiscountedCashFlow {
        id: InstrumentId::new("TEST-DCF-BAD-G"),
        currency: Currency::USD,
        flows: vec![(cf_date, Money::from((100_i64, Currency::USD)))],
        // WACC <= stable_growth_rate should be rejected
        wacc: 0.02,
        terminal_value: TerminalValueSpec::GordonGrowth {
            stable_growth_rate: 0.03,
        },
        valuation_date,
        forecast_timing: ForecastTiming::DatedReceipts,
        terminal_date: cf_date,
        normalized_annual_fcff: Some(Money::from((100_i64, Currency::USD))),
        equity_bridge: EquityBridge::default(),
        shares_outstanding: None,
        dilution_securities: Vec::new(),
        valuation_discounts: None,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::default(),
    };

    let market = MarketContext::new();
    let as_of = valuation_date;

    let result = dcf.value(&market, as_of);
    assert!(
        result.is_err(),
        "expected validation error when WACC <= growth"
    );
}

/// A DCF discounts at its own WACC, so it declares no discount-curve
/// dependency; rate sensitivity bumps the risk-free component of the WACC.
#[test]
fn dcf_declares_no_discount_curve_dependency() {
    let dcf = build_simple_dcf_gordon();

    let deps = dcf
        .market_dependencies()
        .expect("market_dependencies should succeed")
        .curves;
    assert!(
        deps.discount_curves.is_empty(),
        "DCF must not force callers to load a curve no computation reads, got {:?}",
        deps.discount_curves
    );
}

fn build_flat_discount_curve(id: &CurveId, as_of: Date, rate: f64) -> DiscountCurve {
    // Simple flat discount curve with a few knots approximating exp(-r t)
    let knots = [(0.0, 1.0), (1.0, (-rate).exp()), (5.0, (-rate * 5.0).exp())];

    DiscountCurve::builder(id.clone())
        .base_date(as_of)
        .knots(knots)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("failed to build flat discount curve")
}

fn build_market_with_flat_curve(as_of: Date, curve_id: &CurveId, rate: f64) -> MarketContext {
    let curve = build_flat_discount_curve(curve_id, as_of, rate);
    MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve")
}

/// the PV must always discount at WACC; loading the
/// named curve in the market context must not change the valuation.
#[test]
fn pv_identical_with_and_without_curve_loaded() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid valuation date");
    let mut dcf = build_simple_dcf_gordon();
    dcf.valuation_date = as_of;

    let market_curve = build_market_with_flat_curve(as_of, &CurveId::new("USD-OIS"), 0.05);
    let pv_curve = dcf.value(&market_curve, as_of).expect("pv with curve");
    let pv_no_curve = dcf
        .value(&MarketContext::new(), as_of)
        .expect("pv without curve");

    assert!(
        (pv_curve.amount() - pv_no_curve.amount()).abs() < 1e-9,
        "PV must not depend on whether the curve is loaded: with={}, without={}",
        pv_curve.amount(),
        pv_no_curve.amount()
    );

    // And the WACC discounting must reconstruct exactly.
    let tv = dcf.calculate_terminal_value().expect("terminal value");
    let expected = dcf.calculate_pv_explicit_flows().unwrap()
        + dcf.discount_terminal_value(tv).expect("terminal pv");
    assert!((pv_no_curve.amount() - expected).abs() < 1e-6);
}

fn build_metric_context(
    dcf: DiscountedCashFlow,
    market: MarketContext,
    as_of: Date,
) -> MetricContext {
    let base_value = dcf.value(&market, as_of).expect("base value");
    let instrument: std::sync::Arc<dyn Instrument> = std::sync::Arc::new(dcf);
    MetricContext::new(
        instrument,
        std::sync::Arc::new(market),
        as_of,
        base_value,
        MetricContext::default_config(),
    )
}

#[test]
fn dcf_theta_metric_computes() {
    let as_of =
        Date::from_calendar_date(2025, Month::January, 1).expect("valid test date for theta");
    let mut dcf = build_simple_dcf_gordon();
    dcf.valuation_date = as_of;

    let market = build_market_with_flat_curve(as_of, &CurveId::new("USD-OIS"), 0.05);
    let mut mctx = build_metric_context(dcf, market, as_of);

    let registry = crate::EQUITY.metrics();

    let results = registry
        .compute(&[MetricId::Theta], &mut mctx)
        .expect("theta metric should compute");

    assert!(
        results.contains_key(&MetricId::Theta),
        "Theta metric should be present for DCF"
    );
}

#[test]
fn dcf_wacc01_metric_computes_and_is_reasonable() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid WACC01 date");
    let mut dcf = build_simple_dcf_gordon();
    dcf.valuation_date = as_of;

    let market = build_market_with_flat_curve(as_of, &CurveId::new("USD-OIS"), 0.05);
    let mut mctx = build_metric_context(dcf, market, as_of);

    let registry = crate::EQUITY.metrics();

    let wacc01_id = MetricId::custom("dcf::wacc01");
    let results = registry
        .compute(std::slice::from_ref(&wacc01_id), &mut mctx)
        .expect("WACC01 metric should compute");

    let wacc01 = *results
        .get(&wacc01_id)
        .expect("WACC01 metric should be present");

    assert!(wacc01 < 0.0, "DCF WACC01 should be negative, got {wacc01}");

    // WACC01 equals the analytic dPV/dWACC per 1bp.
    // one-flow Gordon DCF: PV(w) = F/(1+w)^t + F(1+g)/((w-g)(1+w)^t).
    let f = 100.0;
    let g = 0.02;
    let w = 0.10;
    let t = {
        let mut d = build_simple_dcf_gordon();
        d.valuation_date = as_of;
        d.explicit_flow_discount_years(d.flows[0].0).unwrap()
    };
    let pv = |w: f64| (f + f * (1.0 + g) / (w - g)) / (1.0 + w).powf(t);
    let h = 1e-7;
    let analytic_per_bp = (pv(w + h) - pv(w - h)) / (2.0 * h) / 10_000.0;
    assert!(
        (wacc01 - analytic_per_bp).abs() < 1e-4 * analytic_per_bp.abs(),
        "WACC01 ({wacc01}) should equal analytic dPV/dWACC per bp ({analytic_per_bp})"
    );

    // And it must not depend on whether the curve is loaded.
    let mut dcf2 = build_simple_dcf_gordon();
    dcf2.valuation_date = as_of;
    let mut mctx_no_curve = build_metric_context(dcf2, MarketContext::new(), as_of);
    let results_no_curve = registry
        .compute(std::slice::from_ref(&wacc01_id), &mut mctx_no_curve)
        .expect("WACC01 should compute without curve");
    let wacc01_no_curve = *results_no_curve.get(&wacc01_id).expect("WACC01 present");
    assert!(
        (wacc01 - wacc01_no_curve).abs() < 1e-9,
        "WACC01 must not depend on curve presence: with={wacc01}, without={wacc01_no_curve}"
    );
}

#[test]
fn dcf_cashflow_schedule_emits_projected_explicit_flows() {
    let dcf = build_simple_dcf_gordon();
    let schedule = dcf
        .cashflow_schedule(&MarketContext::new(), dcf.valuation_date)
        .expect("dcf schedule");

    assert_eq!(
        schedule.get_meta().representation,
        finstack_quant_cashflows::builder::CashflowRepresentation::Projected
    );
    assert_eq!(schedule.get_flows().len(), 1);
    assert_eq!(schedule.get_flows()[0].get_date(), dcf.flows[0].0);
    assert_eq!(
        schedule.get_flows()[0].get_amount().amount(),
        dcf.flows[0].1.amount()
    );
    assert_eq!(
        schedule.get_flows()[0].get_amount().currency(),
        Currency::USD
    );
}

//  Mid-year convention tests

#[test]
fn midpoint_timing_changes_only_the_explicit_forecast_pv() {
    let mut dcf = build_simple_dcf_gordon();
    let pv_end = dcf.calculate_pv_explicit_flows().unwrap();
    let terminal_value = dcf.calculate_terminal_value().unwrap();
    let terminal_pv_end = dcf.discount_terminal_value(terminal_value).unwrap();
    dcf.forecast_timing = ForecastTiming::Midpoints {
        period_starts: vec![dcf.valuation_date],
    };
    let expected_pv = 100.0 / 1.1_f64.powf(182.5 / 365.25);
    assert!((dcf.calculate_pv_explicit_flows().unwrap() - expected_pv).abs() < 1e-10);
    assert!(expected_pv > pv_end);
    assert_eq!(
        dcf.discount_terminal_value(terminal_value).unwrap(),
        terminal_pv_end
    );
}

//  Equity bridge tests

#[test]
fn equity_bridge_matches_flat_net_debt_when_simple() {
    // EquityBridge with only total_debt and cash should match a flat net_debt
    let mut dcf_flat = build_simple_dcf_gordon();
    dcf_flat.equity_bridge = EquityBridge {
        total_debt: 50.0,
        ..Default::default()
    };

    let mut dcf_bridge = build_simple_dcf_gordon();
    dcf_bridge.equity_bridge = EquityBridge {
        total_debt: 80.0,
        cash: 30.0,
        ..Default::default()
    };

    let market = MarketContext::new();
    let val_flat = dcf_flat
        .value(&market, dcf_flat.valuation_date)
        .expect("flat");
    let val_bridge = dcf_bridge
        .value(&market, dcf_bridge.valuation_date)
        .expect("bridge");

    assert!(
        (val_flat.amount() - val_bridge.amount()).abs() < 0.1,
        "bridge (debt=80, cash=30, net=50) should match flat net_debt=50: flat={}, bridge={}",
        val_flat.amount(),
        val_bridge.amount()
    );
}

#[test]
fn equity_bridge_with_preferred_and_minority() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.equity_bridge = EquityBridge {
        total_debt: 100.0,
        cash: 20.0,
        preferred_equity: 30.0,
        minority_interest: 10.0,
        non_operating_assets: 5.0,
        other_adjustments: vec![("pension".into(), -15.0)], // negative = reduces equity
    };

    let bridge = &dcf.equity_bridge;
    // net_adjustment = 100 - 20 + 30 + 10 - 5 - (-15) = 130
    let expected = 100.0 - 20.0 + 30.0 + 10.0 - 5.0 + 15.0;
    assert!(
        (bridge.net_adjustment() - expected).abs() < 1e-9,
        "bridge net_adjustment ({}) should be {}",
        bridge.net_adjustment(),
        expected
    );
}

//  H-Model terminal value tests

#[test]
fn h_model_terminal_value_matches_damodaran_formula() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.terminal_value = TerminalValueSpec::HModel {
        high_growth_rate: 0.15,
        stable_growth_rate: 0.03,
        half_life_years: 5.0,
    };

    let tv = dcf
        .calculate_terminal_value()
        .expect("h-model terminal value");

    // Expected: stable_tv + growth_premium
    // stable_tv = 100 * (1.03) / (0.10 - 0.03) = 1471.43
    // growth_premium = 100 * 5.0 * (0.15 - 0.03) / (0.10 - 0.03) = 857.14
    // total = 2328.57
    let last_fcf = 100.0;
    let expected_stable = last_fcf * (1.0 + 0.03) / (0.10 - 0.03);
    let expected_premium = last_fcf * 5.0 * (0.15 - 0.03) / (0.10 - 0.03);
    let expected_tv = expected_stable + expected_premium;

    let diff = (tv - expected_tv).abs();
    assert!(
        diff < 1e-6,
        "H-model TV mismatch: got {}, expected {}, diff {}",
        tv,
        expected_tv,
        diff
    );
}

#[test]
fn h_model_errors_when_wacc_not_greater_than_stable_growth() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.wacc = 0.02;
    dcf.terminal_value = TerminalValueSpec::HModel {
        high_growth_rate: 0.15,
        stable_growth_rate: 0.03,
        half_life_years: 5.0,
    };

    assert!(
        dcf.calculate_terminal_value().is_err(),
        "H-model should error when WACC <= stable_growth_rate"
    );
}

//  Valuation discounts (DLOM / DLOC) tests

#[test]
fn valuation_discounts_reduce_equity_value() {
    let dcf_base = build_simple_dcf_gordon();
    let mut dcf_disc = build_simple_dcf_gordon();
    dcf_disc.valuation_discounts = Some(ValuationDiscounts {
        dlom: Some(0.25),
        dloc: Some(0.15),
        other_discount: None,
    });

    let market = MarketContext::new();
    let val_base = dcf_base
        .value(&market, dcf_base.valuation_date)
        .expect("base value");
    let val_disc = dcf_disc
        .value(&market, dcf_disc.valuation_date)
        .expect("discounted value");

    // Expected: val_disc = val_base * (1 - 0.15) * (1 - 0.25)
    let expected = val_base.amount() * (1.0 - 0.15) * (1.0 - 0.25);
    let diff = (val_disc.amount() - expected).abs();
    assert!(
        diff < 0.1,
        "DLOM/DLOC mismatch: got {}, expected {}, diff {}",
        val_disc.amount(),
        expected,
        diff
    );
}

//  Dilution / per-share tests

#[test]
fn diluted_shares_treasury_stock_method() {
    // Build a DCF with equity value ~1,250 on a small share base
    // so that per-share price is high enough for ITM options.
    let mut dcf = build_simple_dcf_gordon();
    dcf.shares_outstanding = Some(100.0); // 100 shares => ~$12.50/share

    let market = MarketContext::new();
    let equity = dcf
        .value(&market, dcf.valuation_date)
        .expect("value")
        .amount();
    let pps = equity / 100.0;
    assert!(pps > 10.0, "sanity: price per share should be > $10");

    dcf.dilution_securities = vec![
        DilutionSecurity {
            name: "ITM Options".into(),
            quantity: 20.0,
            exercise_price: 5.0, // well in-the-money
        },
        DilutionSecurity {
            name: "OTM Warrants".into(),
            quantity: 10.0,
            exercise_price: 999.0, // far out-of-money
        },
    ];

    let diluted = dcf.diluted_shares(equity).expect("diluted shares");

    // Only the in-the-money options dilute. TSM iterates to the
    // self-consistent diluted price p = E/d, so the fixed point solves
    //   d = basic + q - q * X * d / E
    // => d = (basic + q) / (1 + q * X / E)
    let expected_diluted = (100.0 + 20.0) / (1.0 + 20.0 * 5.0 / equity);

    let diff = (diluted - expected_diluted).abs();
    assert!(
        diff < 1e-6,
        "diluted shares mismatch: got {:.6}, expected {:.6}, diff {:.6}",
        diluted,
        expected_diluted,
        diff
    );

    // Self-consistency: dilution evaluated at the converged diluted
    // price must reproduce the converged share count.
    let converged_price = equity / diluted;
    let reproduced = dcf.diluted_shares_at_price(100.0, converged_price);
    assert!(
        (reproduced - diluted).abs() < 1e-6,
        "fixed point not self-consistent: {reproduced} vs {diluted}"
    );

    // Per-share value should be less than undiluted price
    let eps = dcf.equity_value_per_share(equity).expect("eps");
    assert!(
        eps < pps,
        "diluted EPS ({:.4}) should be less than basic PPS ({:.4})",
        eps,
        pps
    );
}

#[test]
fn large_warrant_issue_solves_piecewise_equity_equation() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.shares_outstanding = Some(100.0);
    dcf.dilution_securities = vec![DilutionSecurity {
        name: "Warrants".into(),
        quantity: 1000.0,
        exercise_price: 5.0,
    }];
    let shares = dcf.diluted_shares(1000.0).expect("diluted shares");
    assert!((shares - 183.33333333333333).abs() < 1e-10);
    assert!((1000.0 / shares - 6000.0 / 1100.0).abs() < 1e-12);
    assert_eq!(dcf.diluted_shares(500.0), Some(100.0));
    dcf.dilution_securities.push(DilutionSecurity {
        name: "RSUs".into(),
        quantity: 20.0,
        exercise_price: 0.0,
    });
    dcf.dilution_securities.push(DilutionSecurity {
        name: "OTM".into(),
        quantity: 1e6,
        exercise_price: 100.0,
    });
    let shares = dcf.diluted_shares(1000.0).expect("multiple classes");
    let price = 1000.0 / shares;
    assert!((120.0 * price + 1000.0 * (price - 5.0).max(0.0) - 1000.0).abs() < 1e-9);
}

#[test]
fn discounts_and_dilution_preserve_signed_negative_common_equity() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.shares_outstanding = Some(100.0);
    dcf.valuation_discounts = Some(ValuationDiscounts {
        dlom: Some(0.20),
        ..ValuationDiscounts::default()
    });
    assert_eq!(dcf.apply_valuation_discounts(-25.0).unwrap(), -25.0);
    assert_eq!(dcf.apply_valuation_discounts(25.0).unwrap(), 20.0);
    assert_eq!(dcf.diluted_shares(-25.0), None);
    assert_eq!(dcf.equity_value_per_share(-25.0), None);
    assert_eq!(dcf.diluted_shares(0.0), Some(100.0));
    assert_eq!(dcf.equity_value_per_share(0.0), Some(0.0));
}

#[test]
fn diluted_shares_none_when_no_shares() {
    let dcf = build_simple_dcf_gordon();
    assert!(dcf.diluted_shares(1000.0).is_none());
    assert!(dcf.equity_value_per_share(1000.0).is_none());
}

//  Serde round-trip tests

#[test]
fn serde_roundtrip_basic_dcf() {
    let dcf = build_simple_dcf_gordon();
    let json = serde_json::to_string(&dcf).expect("serialize");
    let dcf2: DiscountedCashFlow = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(dcf.id, dcf2.id);
    assert_eq!(dcf2.forecast_timing, ForecastTiming::DatedReceipts);
    assert_eq!(dcf2.equity_bridge.net_adjustment(), 0.0);
    assert!(dcf2.shares_outstanding.is_none());
    assert!(dcf2.dilution_securities.is_empty());
    assert!(dcf2.valuation_discounts.is_none());
}

#[test]
fn serde_roundtrip_full_dcf() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.forecast_timing = ForecastTiming::Midpoints {
        period_starts: vec![dcf.valuation_date],
    };
    dcf.equity_bridge = EquityBridge {
        total_debt: 100.0,
        cash: 20.0,
        preferred_equity: 30.0,
        minority_interest: 5.0,
        non_operating_assets: 10.0,
        other_adjustments: vec![("pension".into(), -8.0)],
    };
    dcf.shares_outstanding = Some(1_000_000.0);
    dcf.dilution_securities = vec![DilutionSecurity {
        name: "Options".into(),
        quantity: 50_000.0,
        exercise_price: 10.0,
    }];
    dcf.valuation_discounts = Some(ValuationDiscounts {
        dlom: Some(0.25),
        dloc: Some(0.15),
        other_discount: None,
    });

    let json = serde_json::to_string_pretty(&dcf).expect("serialize");
    let dcf2: DiscountedCashFlow = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(dcf2.forecast_timing, dcf.forecast_timing);
    assert_eq!(dcf2.equity_bridge.other_adjustments.len(), 1);
    assert_eq!(dcf2.shares_outstanding, Some(1_000_000.0));
    assert_eq!(dcf2.dilution_securities.len(), 1);
    assert!(dcf2.valuation_discounts.is_some());
    let disc = dcf2
        .valuation_discounts
        .expect("valuation_discounts should be present after roundtrip");
    assert_eq!(disc.dlom, Some(0.25));
    assert_eq!(disc.dloc, Some(0.15));
}

#[test]
fn old_wire_requires_authored_timing_and_terminal_basis() {
    let mut value = serde_json::to_value(build_simple_dcf_gordon()).unwrap();
    value.as_object_mut().unwrap().remove("forecast_timing");
    assert!(serde_json::from_value::<DiscountedCashFlow>(value).is_err());
}

#[test]
fn validate_rejects_non_finite_equity_bridge() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.equity_bridge.total_debt = f64::NAN;
    let err = dcf.validate().expect_err("NaN total_debt must be rejected");
    assert!(
        err.to_string().contains("equity_bridge.total_debt"),
        "{err}"
    );

    let mut dcf = build_simple_dcf_gordon();
    dcf.equity_bridge.other_adjustments = vec![("pension".into(), f64::INFINITY)];
    assert!(dcf.validate().is_err());
}

fn minimal_dcf_json(extra: &str, terminal_value: &str) -> String {
    format!(
        r#"{{
        "id": "TEST",
        "currency": "USD",
        "flows": [["2026-01-01", 100.0]],
        "wacc": 0.10,
        "terminal_value": {terminal_value},
        "valuation_date": "2025-01-01",
        {extra}
        "attributes": {{"tags": [], "meta": {{}}}}
    }}"#
    )
}

#[test]
// schema-rejection-test: retired `net_debt` (now `equity_bridge`)
fn retired_net_debt_key_is_rejected() {
    let json = minimal_dcf_json(
        r#""net_debt": 50.0, "equity_bridge": {"total_debt": 50.0},"#,
        r#"{"type": "gordon_growth", "stable_growth_rate": 0.02}"#,
    );
    assert!(serde_json::from_str::<DiscountedCashFlow>(&json).is_err());
}

#[test]
// schema-rejection-test: retired `growth_rate` (now `stable_growth_rate`)
fn retired_gordon_growth_rate_key_is_rejected() {
    let json = minimal_dcf_json(
        r#""equity_bridge": {"total_debt": 50.0},"#,
        r#"{"type": "gordon_growth", "growth_rate": 0.02}"#,
    );
    assert!(serde_json::from_str::<DiscountedCashFlow>(&json).is_err());
}

#[test]
fn serde_h_model_terminal_value_roundtrip() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.terminal_value = TerminalValueSpec::HModel {
        high_growth_rate: 0.15,
        stable_growth_rate: 0.03,
        half_life_years: 5.0,
    };

    let json = serde_json::to_string(&dcf).expect("serialize");
    let dcf2: DiscountedCashFlow = serde_json::from_str(&json).expect("deserialize");

    match dcf2.terminal_value {
        TerminalValueSpec::HModel {
            high_growth_rate,
            stable_growth_rate,
            half_life_years,
        } => {
            assert!((high_growth_rate - 0.15).abs() < 1e-10);
            assert!((stable_growth_rate - 0.03).abs() < 1e-10);
            assert!((half_life_years - 5.0).abs() < 1e-10);
        }
        _ => panic!("Expected HModel terminal value after roundtrip"),
    }
}

//  Metric consistency: EV metric matches value() with curve

#[test]
fn ev_metric_consistent_with_value_when_curve_present() {
    let as_of =
        Date::from_calendar_date(2025, Month::January, 1).expect("valid test date for EV metric");
    let mut dcf = build_simple_dcf_gordon();
    dcf.valuation_date = as_of;

    let market = build_market_with_flat_curve(as_of, &CurveId::new("USD-OIS"), 0.05);

    // Get equity from value()
    let equity = dcf.value(&market, as_of).expect("value").amount();
    let bridge = dcf.effective_net_debt();
    let expected_ev = equity + bridge;

    // Get EV from metric
    let mut mctx = build_metric_context(dcf, market, as_of);
    let registry = crate::EQUITY.metrics();

    let results = registry
        .compute(&[MetricId::EnterpriseValue], &mut mctx)
        .expect("EV metric should compute");

    let ev_metric = *results
        .get(&MetricId::EnterpriseValue)
        .expect("EV metric should be present");

    // Allow for small rounding differences
    let diff = (ev_metric - expected_ev).abs();
    assert!(
        diff < 1.0,
        "EV metric ({}) should be close to value()+bridge ({}), diff {}",
        ev_metric,
        expected_ev,
        diff
    );
}

/// the terminal value PV metric discounts at WACC
/// even when the named curve is loaded.
#[test]
fn terminal_value_pv_metric_uses_wacc_even_with_curve_present() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1)
        .expect("valid test date for terminal value pv metric");
    let mut dcf = build_simple_dcf_gordon();
    dcf.valuation_date = as_of;

    let market = build_market_with_flat_curve(as_of, &CurveId::new("USD-OIS"), 0.05);
    let terminal_value = dcf.calculate_terminal_value().expect("terminal value");
    let expected_pv_terminal = dcf
        .discount_terminal_value(terminal_value)
        .expect("wacc-discounted terminal value");

    let mut mctx = build_metric_context(dcf, market, as_of);
    let registry = crate::EQUITY.metrics();

    let results = registry
        .compute(&[MetricId::TerminalValuePV], &mut mctx)
        .expect("TerminalValuePV metric should compute");

    let pv_metric = *results
        .get(&MetricId::TerminalValuePV)
        .expect("TerminalValuePV metric should be present");
    let diff = (pv_metric - expected_pv_terminal).abs();
    assert!(
        diff < 1e-9,
        "TerminalValuePV metric ({}) should match curve-based PV ({}), diff {}",
        pv_metric,
        expected_pv_terminal,
        diff
    );
}

/// Per-share metrics must fail loudly (not emit NaN) when
/// `shares_outstanding` is unset, and compute when it is set.
#[test]
fn per_share_metrics_error_without_shares_outstanding() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let registry = crate::EQUITY.metrics();

    // Without shares_outstanding: both metrics error with a clear message.
    let mut dcf = build_simple_dcf_gordon();
    dcf.valuation_date = as_of;
    let mut mctx = build_metric_context(dcf, MarketContext::new(), as_of);
    for metric in [MetricId::EquityShares, MetricId::EquityPricePerShare] {
        let err = registry
            .compute(std::slice::from_ref(&metric), &mut mctx)
            .expect_err("per-share metric must error without shares_outstanding");
        assert!(
            err.to_string().contains("shares_outstanding"),
            "error should name shares_outstanding, got: {err}"
        );
    }

    // With shares_outstanding: both metrics compute finite values.
    let mut dcf = build_simple_dcf_gordon();
    dcf.valuation_date = as_of;
    dcf.shares_outstanding = Some(100.0);
    let mut mctx = build_metric_context(dcf, MarketContext::new(), as_of);
    let results = registry
        .compute(
            &[MetricId::EquityShares, MetricId::EquityPricePerShare],
            &mut mctx,
        )
        .expect("per-share metrics should compute with shares set");
    for metric in [MetricId::EquityShares, MetricId::EquityPricePerShare] {
        let value = *results.get(&metric).expect("metric present");
        assert!(value.is_finite() && value > 0.0, "{metric:?} = {value}");
    }
}

//  Builder pattern tests

#[test]
fn builder_with_all_new_fields() {
    let valuation_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let cf_date = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");

    let dcf = DiscountedCashFlow::builder()
        .id(InstrumentId::new("BUILDER-TEST"))
        .currency(Currency::USD)
        .flows(vec![(cf_date, Money::from((100_i64, Currency::USD)))])
        .wacc(0.10)
        .terminal_value(TerminalValueSpec::GordonGrowth {
            stable_growth_rate: 0.02,
        })
        .valuation_date(valuation_date)
        .forecast_timing(ForecastTiming::Midpoints {
            period_starts: vec![valuation_date],
        })
        .terminal_date(cf_date)
        .normalized_annual_fcff(Money::from((100_i64, Currency::USD)))
        .equity_bridge(EquityBridge {
            total_debt: 80.0,
            cash: 30.0,
            ..Default::default()
        })
        .shares_outstanding(1_000_000.0)
        .valuation_discounts(ValuationDiscounts {
            dlom: Some(0.20),
            ..Default::default()
        })
        .build()
        .expect("builder should succeed");

    assert!(matches!(
        dcf.forecast_timing,
        ForecastTiming::Midpoints { .. }
    ));
    assert_eq!(dcf.equity_bridge.net_adjustment(), 50.0);
    assert_eq!(dcf.shares_outstanding, Some(1_000_000.0));
    assert!(dcf.valuation_discounts.is_some());
}

//  calculate_terminal_value error cases

#[test]
fn calculate_terminal_value_errors_on_empty_flows() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.flows.clear();
    assert!(
        dcf.calculate_terminal_value().is_err(),
        "should error on empty flows"
    );
}

#[test]
fn discount_terminal_value_errors_on_empty_flows() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.flows.clear();
    assert!(
        dcf.discount_terminal_value(100.0).is_err(),
        "should error on empty flows"
    );
}

#[test]
fn exit_multiple_requires_an_explicit_projection() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.flows.clear();
    dcf.normalized_annual_fcff = None;
    dcf.terminal_value = TerminalValueSpec::ExitMultiple {
        terminal_metric: Some(500.0),
        multiple: 8.0,
    };
    assert!(dcf.calculate_terminal_value().is_err());
}

#[test]
fn h_model_errors_when_high_growth_less_than_stable() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.terminal_value = TerminalValueSpec::HModel {
        high_growth_rate: 0.02,
        stable_growth_rate: 0.05, // high < stable
        half_life_years: 5.0,
    };
    let result = dcf.calculate_terminal_value();
    assert!(
        result.is_err(),
        "H-model should error when high_growth_rate < stable_growth_rate"
    );
    let msg = result
        .expect_err("expected validation error when high_growth_rate < stable_growth_rate")
        .to_string();
    assert!(
        msg.contains("high growth"),
        "Error should mention high_growth_rate: {}",
        msg
    );
}

#[test]
fn h_model_errors_when_half_life_zero_or_negative() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.terminal_value = TerminalValueSpec::HModel {
        high_growth_rate: 0.15,
        stable_growth_rate: 0.03,
        half_life_years: 0.0, // invalid
    };
    assert!(
        dcf.calculate_terminal_value().is_err(),
        "H-model should error when half_life_years = 0"
    );

    dcf.terminal_value = TerminalValueSpec::HModel {
        high_growth_rate: 0.15,
        stable_growth_rate: 0.03,
        half_life_years: -1.0, // invalid
    };
    assert!(
        dcf.calculate_terminal_value().is_err(),
        "H-model should error when half_life_years < 0"
    );
}

#[test]
fn valuation_discounts_rejects_out_of_range() {
    let mut dcf = build_simple_dcf_gordon();
    dcf.valuation_discounts = Some(ValuationDiscounts {
        dlom: Some(1.5), // out of range
        dloc: None,
        other_discount: None,
    });

    let market = MarketContext::new();
    let result = dcf.value(&market, dcf.valuation_date);
    assert!(result.is_err(), "should reject DLOM > 1.0");

    dcf.valuation_discounts = Some(ValuationDiscounts {
        dlom: None,
        dloc: Some(-0.1), // out of range
        other_discount: None,
    });
    let result = dcf.value(&market, dcf.valuation_date);
    assert!(result.is_err(), "should reject negative DLOC");
}
