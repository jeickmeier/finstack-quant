//! Public-API regressions for the September 2026 production quant audit.

#[test]
fn b8_cached_hw_risk_rebuilds_the_active_grid() {
    use finstack_quant_models::rates::hull_white::HullWhiteCalibrationParams;
    use finstack_quant_valuations::instruments::rates::swaption::{
        BermudanSwaption, BermudanSwaptionPricer, BermudanSwaptionPricerConfig,
        BermudanSwaptionTreeValuator, PreparedHullWhiteModel,
    };
    use finstack_quant_valuations::pricer::{ModelKey, PricerRegistry};
    let as_of = date!(2025 - 01 - 01);
    let mut swaption = BermudanSwaption::example();
    swaption.underlying_float_leg.forward_curve_id = swaption.get_discount_curve_id().clone();
    let make_curve = |rate: f64| {
        DiscountCurve::builder(swaption.get_discount_curve_id().clone())
            .base_date(as_of)
            .knots([
                (0.0, 1.0),
                (1.0, (-rate).exp()),
                (20.0, (-rate * 20.0).exp()),
            ])
            .build()
            .expect("curve")
    };
    let mut times = swaption.exercise_times(as_of).expect("exercises");
    times.push(2.123);
    let horizon = swaption.time_to_maturity(as_of).expect("horizon");
    let prepare = |curve: &DiscountCurve, sigma| {
        PreparedHullWhiteModel::prepare(
            HullWhiteCalibrationParams::new(0.12, sigma).expect("parameters"),
            47,
            curve,
            as_of,
            horizon,
            &times,
        )
        .expect("prepare")
    };
    let curve = make_curve(0.03);
    let prepared = prepare(&curve, 0.025);
    let probs = BermudanSwaptionTreeValuator::new(&swaption, &prepared, &curve, as_of)
        .expect("valuator")
        .exercise_probabilities();
    let expected_time =
        probs.iter().map(|(t, p)| t * p).sum::<f64>() / probs.iter().map(|(_, p)| p).sum::<f64>();
    let mut registry = PricerRegistry::new();
    registry
        .register(BermudanSwaptionPricer::tree_with_config(
            BermudanSwaptionPricerConfig {
                prepared_model: Some(prepared),
                ..Default::default()
            },
        ))
        .expect("registry");
    let market = MarketContext::new().insert(curve);
    let result = registry
        .price_with_metrics(
            &swaption,
            ModelKey::HullWhite1F,
            &market,
            as_of,
            &[
                MetricId::Delta,
                MetricId::HwSigmaVega,
                MetricId::Theta,
                MetricId::custom("exercise_probability"),
            ],
            PricingOptions::default(),
        )
        .expect("cached model risk");
    let pv = |rate, sigma| {
        let curve = make_curve(rate);
        let model = prepare(&curve, sigma);
        BermudanSwaptionTreeValuator::new(&swaption, &model, &curve, as_of)
            .expect("bumped valuator")
            .price()
            .expect("price")
    };
    let delta = (pv(0.0301, 0.025) - pv(0.0299, 0.025)) / 0.0002;
    let vega = (pv(0.03, 0.02525) - pv(0.03, 0.02475)) / 0.0005 * 0.01;
    assert!((result.measures["delta"] - delta).abs() < 1e-5);
    assert!((result.measures["hw_sigma_vega"] - vega).abs() < 1e-5);
    assert!(result.measures["theta"].is_finite());
    assert!((result.measures["exercise_probability"] - expected_time).abs() < 1e-12);
}

#[test]
fn m2_lsmc_partial_coupon_and_spread_match_cashflows() {
    use finstack_quant_valuations::instruments::rates::swaption::{
        BermudanSwaption, BermudanSwaptionPricer, BermudanSwaptionPricerConfig,
    };
    use finstack_quant_valuations::pricer::Pricer;
    use rust_decimal::Decimal;
    let as_of = date!(2025 - 01 - 01);
    let exercise = date!(2026 - 04 - 01);
    let mut swaption = BermudanSwaption::example();
    swaption.bermudan_schedule.exercise_dates = vec![exercise];
    let fixed = &mut swaption.underlying_fixed_leg;
    fixed.start = date!(2026 - 01 - 01);
    fixed.end = date!(2028 - 01 - 01);
    fixed.frequency = Tenor::annual();
    fixed.day_count = DayCount::Act365F;
    fixed.business_day_convention = BusinessDayConvention::Unadjusted;
    fixed.payment_lag_days = 0;
    fixed.rate = Decimal::new(2, 2);
    let float = &mut swaption.underlying_float_leg;
    float.start = fixed.start;
    float.end = fixed.end;
    float.frequency = fixed.frequency;
    float.day_count = fixed.day_count;
    float.business_day_convention = fixed.business_day_convention;
    float.payment_lag_days = 0;
    float.reset_lag_days = 0;
    float.forward_curve_id = fixed.discount_curve_id.clone();
    float.spread_bp = Decimal::from(100);
    swaption
        .instrument_pricing_overrides
        .model_config
        .hw1f_mean_reversion = Some(0.05);
    swaption
        .instrument_pricing_overrides
        .model_config
        .hw1f_sigma = Some(1e-7);
    let market = MarketContext::new().insert(
        DiscountCurve::builder(swaption.get_discount_curve_id().clone())
            .base_date(as_of)
            .knots([(0.0, 1.0), (10.0, (-0.4_f64).exp())])
            .build()
            .expect("curve"),
    );
    let mut expected = 0.0;
    let mut start = exercise;
    for end in [date!(2027 - 01 - 01), date!(2028 - 01 - 01)] {
        let tau = (end - start).whole_days() as f64 / 365.0;
        let t = (end - as_of).whole_days() as f64 / 365.0;
        expected += ((0.04 * tau).exp() - 1.0 + (0.01 - 0.02) * tau) * (-0.04 * t).exp();
        start = end;
    }
    expected *= swaption.notional.amount();
    let mut config = BermudanSwaptionPricerConfig::default();
    config.mc.num_paths = 256;
    let actual = BermudanSwaptionPricer::lsmc_with_config(config)
        .price_dyn(&swaption, &market, as_of)
        .expect("price")
        .value
        .amount();
    assert!(
        (actual - expected).abs() < 0.1,
        "actual={actual}, expected={expected}"
    );
}

use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::{cashflow::CFKind, currency::Currency, money::Money};
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, CommitmentFeeBase, DdtlSpec, DrawEvent, RateSpec, TermLoan,
};
use finstack_quant_valuations::instruments::Bond;
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

#[test]
fn b11_icma_annuity_uses_quasi_coupon_periods_for_eom_stub() {
    use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::fixed_leg_annuity;
    let start = date!(2024 - 01 - 31);
    let dates = [start, date!(2024 - 02 - 29), date!(2024 - 08 - 31)];
    let curve = DiscountCurve::builder("FLAT")
        .base_date(start)
        .knots([(0.0, 1.0), (2.0, 1.0)])
        .build()
        .expect("curve");
    let annuity = fixed_leg_annuity(
        &curve,
        DayCount::ActActIsma,
        Some(Tenor::semi_annual()),
        &dates,
    )
    .expect("ICMA annuity");
    // First stub: 29 days over the Aug-31/Feb-29 reference's 182 days.
    assert!((annuity - (29.0 / 364.0 + 0.5)).abs() < 1e-12);
}

#[test]
fn b11_icma_annuity_retains_regular_grid_for_back_stub() {
    use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::fixed_leg_annuity;
    let dates = [
        date!(2024 - 02 - 29),
        date!(2024 - 08 - 31),
        date!(2024 - 11 - 30),
    ];
    let curve = DiscountCurve::builder("FLAT")
        .base_date(dates[0])
        .knots([(0.0, 1.0), (2.0, 1.0)])
        .build()
        .expect("curve");
    let value = fixed_leg_annuity(
        &curve,
        DayCount::ActActIsma,
        Some(Tenor::semi_annual()),
        &dates,
    )
    .expect("ICMA annuity");
    assert!((value - (0.5 + 91.0 / 362.0)).abs() < 1e-12, "{value}");
}

#[test]
fn b11_icma_bond_duration_and_convexity_use_yield_clock() {
    use finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec;
    let start = date!(2024 - 02 - 29);
    let mut bond = Bond::fixed(
        "ICMA-RISK",
        Money::from((100_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.04).expect("rate"),
        start,
        date!(2025 - 02 - 28),
        StubKind::None,
        "FLAT",
    )
    .expect("bond");
    bond.settlement_convention = None;
    if let CashflowSpec::Fixed(spec) = &mut bond.cashflow_spec {
        spec.schedule.day_count = DayCount::ActActIsma;
        spec.schedule.end_of_month = true;
        spec.schedule.frequency = Tenor::semi_annual();
        spec.schedule.business_day_convention = BusinessDayConvention::Unadjusted;
    }
    bond.instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price = Some(100.0);
    let curve = DiscountCurve::builder("FLAT")
        .base_date(start)
        .knots([(0.0, 1.0), (2.0, 1.0)])
        .build()
        .expect("curve");
    let market = MarketContext::new().insert(curve);
    for (as_of, first_time, dirty) in [(start, 0.5, 100.0), (date!(2024 - 05 - 31), 0.25, 101.0)] {
        let result = bond
            .price_with_metrics(
                &market,
                as_of,
                &[
                    MetricId::Ytm,
                    MetricId::DurationMac,
                    MetricId::Convexity,
                    MetricId::ZSpread,
                ],
                PricingOptions::default(),
            )
            .expect("ICMA risk");
        let base = 1.0 + result.measures["ytm"] / 2.0;
        let last_time = first_time + 0.5;
        let pv_first = 2.0 / base.powf(2.0 * first_time);
        let pv_last = 102.0 / base.powf(2.0 * last_time);
        let expected_duration = (first_time * pv_first + last_time * pv_last) / dirty;
        let expected_convexity = (first_time * (first_time + 0.5) * pv_first
            + last_time * (last_time + 0.5) * pv_last)
            / base.powi(2)
            / dirty
            / 100.0;
        assert!((result.measures["duration_mac"] - expected_duration).abs() < 1e-9);
        assert!((result.measures["convexity"] - expected_convexity).abs() < 1e-10);
        let spread_price = finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::price_from_z_spread(
            &bond, &market, as_of, result.measures["z_spread"]).expect("Z-spread reprice");
        assert!((spread_price - dirty).abs() < 1e-8);
    }
}

#[test]
fn b12_dm_inversion_applies_settlement_once() {
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::price_from_dm;
    use finstack_quant_valuations::instruments::fixed_income::bond::DiscountMarginCalculator;
    use finstack_quant_valuations::metrics::{MetricCalculator, MetricContext};
    use std::sync::Arc;
    let as_of = date!(2025 - 01 - 01);
    let mut bond = Bond::floating(
        "DM-SETTLEMENT",
        Money::from((1_000_000_i64, Currency::USD)),
        "USD-SOFR-3M",
        150,
        date!(2025 - 01 - 03),
        date!(2028 - 01 - 03),
        Tenor::quarterly(),
        DayCount::Act360,
        "USD-OIS",
    )
    .expect("bond");
    assert!(bond.settlement_convention.is_some());
    if let finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec::Floating(
        spec,
    ) = &mut bond.cashflow_spec
    {
        spec.rate_spec.reset_lag_days = 0;
    }
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, 0.85)])
        .build()
        .expect("curve");
    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.03), (5.0, 0.03)])
        .build()
        .expect("forward");
    let market = MarketContext::new().insert(disc).insert(fwd);
    let target = 0.015;
    let dirty = price_from_dm(&bond, &market, as_of, target).expect("DM price");
    let quotes = finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::compute_quotes(
        &bond, &market, as_of,
        finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::BondQuoteInput::DiscountMargin(target),
        PricingOptions::default()).expect("DM quote normalization");
    assert!((quotes.dirty_price_currency - dirty).abs() < 1e-8);
    assert!(quotes.asw_par.is_none() && quotes.asw_market.is_none());
    let mut quoted = bond;
    quoted
        .instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price = Some(dirty / 10_000.0);
    let mut context = MetricContext::new(
        Arc::new(quoted),
        Arc::new(market),
        as_of,
        Money::from((1_000_000_i64, Currency::USD)),
        MetricContext::default_config(),
    );
    let dm = DiscountMarginCalculator
        .calculate(&mut context)
        .expect("DM solve");
    assert!((dm - target).abs() < 1e-9, "expected {target}, got {dm}");
}

fn delayed_draw_loan() -> TermLoan {
    let mut loan = TermLoan::example().expect("valid example");
    loan.issue_date = date!(2025 - 01 - 01);
    loan.maturity = date!(2026 - 01 - 01);
    loan.notional_limit = Money::from((1_200_000_i64, Currency::USD));
    loan.frequency = Tenor::quarterly();
    loan.day_count = DayCount::Act360;
    loan.business_day_convention = BusinessDayConvention::Unadjusted;
    loan.stub = StubKind::None;
    loan.rate = RateSpec::Fixed { rate_bp: 500 };
    loan.amortization = AmortizationSpec::Linear {
        start: loan.issue_date,
        end: loan.maturity,
    };
    loan.ddtl = Some(DdtlSpec {
        commitment_limit: loan.notional_limit,
        availability_start: loan.issue_date,
        availability_end: date!(2025 - 09 - 01),
        draws: vec![
            DrawEvent {
                date: date!(2025 - 02 - 01),
                amount: Money::from((600_000_i64, Currency::USD)),
            },
            DrawEvent {
                date: date!(2025 - 08 - 01),
                amount: Money::from((300_000_i64, Currency::USD)),
            },
        ],
        commitment_step_downs: vec![],
        usage_fee_bp: 50.0,
        commitment_fee_bp: 0.0,
        fee_base: CommitmentFeeBase::Undrawn,
        oid_policy: None,
    });
    loan
}

#[test]
fn b14_ddtl_linear_amortizes_each_draw_over_its_remaining_dates() {
    let loan = delayed_draw_loan();
    let schedule = loan
        .cashflow_schedule(&MarketContext::new(), loan.issue_date)
        .expect("cashflows");
    let repayments: Vec<_> = schedule
        .get_flows()
        .iter()
        .filter(|flow| flow.kind == CFKind::Amortization && flow.amount.amount() > 0.0)
        .map(|flow| flow.amount.amount())
        .collect();
    assert_eq!(repayments, vec![150_000.0, 150_000.0, 300_000.0, 300_000.0]);
}

#[test]
fn b14_ddtl_usage_fee_begins_at_the_draw_date() {
    let loan = delayed_draw_loan();
    let schedule = loan
        .cashflow_schedule(&MarketContext::new(), loan.issue_date)
        .expect("cashflows");
    let first_fee: f64 = schedule
        .get_flows()
        .iter()
        .filter(|flow| flow.kind == CFKind::Fee && flow.date == date!(2025 - 04 - 01))
        .map(|flow| flow.amount.amount())
        .sum();
    assert!((first_fee - 600_000.0 * 0.005 * 59.0 / 360.0).abs() < 1e-8);
}

#[test]
fn b14_ddtl_original_principal_excludes_future_draws() {
    let mut loan = delayed_draw_loan();
    loan.amortization = AmortizationSpec::PercentOfOriginalNotional { bp: 500 };
    let schedule = loan
        .cashflow_schedule(&MarketContext::new(), loan.issue_date)
        .expect("cashflows");
    let first = schedule
        .get_flows()
        .iter()
        .find(|flow| flow.kind == CFKind::Amortization)
        .expect("first repayment");
    assert_eq!(first.amount.amount(), 30_000.0);
}

#[test]
fn m28_model_yield_does_not_repeat_settlement_carry() {
    let mut loan = delayed_draw_loan();
    loan.ddtl = None;
    loan.amortization = AmortizationSpec::None;
    loan.day_count = DayCount::Act365F;
    loan.rate = RateSpec::Fixed { rate_bp: 0 };
    loan.calendar_id = None;
    loan.settlement_days = 7;
    let as_of = date!(2025 - 03 - 28);
    let curve = DiscountCurve::builder(loan.discount_curve_id.as_str())
        .base_date(as_of)
        .knots([(0.0, 1.0), (2.0, (-0.1_f64).exp())])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("flat continuous curve");
    let result = loan
        .price_with_metrics(
            &MarketContext::new().insert(curve),
            as_of,
            &[MetricId::Ytm],
            PricingOptions::default(),
        )
        .expect("yield");
    let yield_value = result.measures["ytm"];
    assert!(
        (yield_value - 0.05_f64.exp_m1()).abs() < 1e-9,
        "model settlement yield {yield_value} must equal annualized flat curve rate"
    );
}

#[test]
fn m28_coupon_date_purchase_uses_balance_after_settled_amortization() {
    let mut loan = delayed_draw_loan();
    loan.ddtl = None;
    loan.notional_limit = Money::from((1_000_000_i64, Currency::USD));
    loan.amortization = AmortizationSpec::PercentOfOriginalNotional { bp: 2500 };
    loan.day_count = DayCount::Act365F;
    loan.calendar_id = None;
    loan.settlement_days = 2;
    loan.instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price = Some(100.0);
    let as_of = date!(2025 - 03 - 28);
    let settlement = loan.settlement_date(as_of).expect("settlement");
    assert_eq!(settlement, date!(2025 - 04 - 01));
    let curve = DiscountCurve::builder(loan.discount_curve_id.as_str())
        .base_date(as_of)
        .knots([(0.0, 1.0), (2.0, 0.9)])
        .build()
        .expect("curve");
    let result = loan
        .price_with_metrics(
            &MarketContext::new().insert(curve),
            as_of,
            &[MetricId::Ytm],
            PricingOptions::default(),
        )
        .expect("yield");
    let ytm = result.measures["ytm"];
    let expected_flows = [
        (
            date!(2025 - 07 - 01),
            250_000.0 + 750_000.0 * 0.05 * 91.0 / 365.0,
        ),
        (
            date!(2025 - 10 - 01),
            250_000.0 + 500_000.0 * 0.05 * 92.0 / 365.0,
        ),
        (
            date!(2026 - 01 - 01),
            250_000.0 + 250_000.0 * 0.05 * 92.0 / 365.0,
        ),
    ];
    let purchase_value: f64 = expected_flows
        .iter()
        .map(|(date, amount)| {
            amount / (1.0 + ytm).powf((*date - settlement).whole_days() as f64 / 365.0)
        })
        .sum();
    // XIRR normalizes by the largest cashflow and accepts a 1e-8 residual.
    assert!(
        (purchase_value - 750_000.0).abs() / 750_000.0 < 1e-8,
        "implied purchase value {purchase_value}"
    );
}

#[test]
fn b1_bond_builder_requires_contractual_issue_date() {
    let bond = Bond::example().expect("valid example bond");
    let error = Bond::builder()
        .id(bond.id)
        .notional(bond.notional)
        .maturity(bond.maturity)
        .cashflow_spec(bond.cashflow_spec)
        .discount_curve_id(bond.discount_curve_id)
        .build()
        .expect_err("a missing issue date must not shorten the bond");
    assert!(error
        .to_string()
        .contains("missing required field 'issue_date'"));
}

#[test]
fn b2_cap_auto_uses_normal_quote_metadata_and_rejects_black_mismatch() {
    use finstack_quant_core::market_data::{
        surfaces::{VolQuoteType, VolSurface},
        term_structures::ForwardCurve,
    };
    use finstack_quant_valuations::instruments::rates::cap_floor::{
        CapFloor, CapFloorVolType, RateOptionType,
    };
    let as_of = date!(2024 - 01 - 02);
    let market = MarketContext::new()
        .insert(
            DiscountCurve::builder("DISC")
                .base_date(as_of)
                .knots([(0.0, 1.0), (5.0, 1.0)])
                .build()
                .expect("discount"),
        )
        .insert(
            ForwardCurve::builder("FWD", 0.25)
                .base_date(as_of)
                .day_count(DayCount::Act360)
                .knots([(0.0, 0.02), (5.0, 0.02)])
                .build()
                .expect("forward"),
        )
        .insert_surface(
            VolSurface::builder("NORMAL")
                .quote_type(VolQuoteType::Normal)
                .expiries(&[1.0])
                .strikes(&[0.02])
                .row(&[0.008])
                .build()
                .expect("surface"),
        );
    let mut option = CapFloor::new(
        "CAP-NORMAL",
        RateOptionType::Caplet,
        Money::from((1_000_000_i64, Currency::USD)),
        0.02,
        date!(2025 - 01 - 02),
        date!(2025 - 04 - 02),
        Some(Tenor::quarterly()),
        DayCount::Act360,
        "DISC",
        "FWD",
        "NORMAL",
    )
    .expect("cap");
    option.business_day_convention = BusinessDayConvention::Unadjusted;
    option.vol_type = CapFloorVolType::Normal;
    let normal = option
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta, MetricId::Gamma, MetricId::Vega],
            PricingOptions::default(),
        )
        .expect("normal");
    // Independent Bachelier formula, using the curve's simple term forward.
    let fwd = market.get_forward("FWD").expect("curve");
    let forward = fwd
        .rate_between(366.0 / 360.0, 456.0 / 360.0)
        .expect("forward");
    let t: f64 = 366.0 / 365.0;
    let std_dev = 0.008 * t.sqrt();
    let d = (forward - 0.02) / std_dev;
    let phi = (-0.5 * d * d).exp() / std::f64::consts::TAU.sqrt();
    let cdf = finstack_quant_core::math::norm_cdf(d);
    let scale = 1_000_000.0 * 0.25;
    let expected = scale * ((forward - 0.02) * cdf + std_dev * phi);
    assert!((normal.value.amount() - expected).abs() < 1e-7);
    assert!((normal.measures["delta"] - scale * cdf).abs() < 1e-7);
    assert!((normal.measures["gamma"] - scale * phi / std_dev).abs() < 1e-5);
    assert!((normal.measures["vega"] - scale * t.sqrt() * phi * 0.01).abs() < 1e-7);
    option.vol_type = CapFloorVolType::Auto;
    let auto = option
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta, MetricId::Gamma, MetricId::Vega],
            PricingOptions::default(),
        )
        .expect("auto");
    assert!((auto.value.amount() - normal.value.amount()).abs() < 1e-8);
    for key in ["delta", "gamma", "vega"] {
        assert!(
            (auto.measures[key] - normal.measures[key]).abs() < 1e-8,
            "{key}"
        );
    }
    option.vol_type = CapFloorVolType::Lognormal;
    assert!(option.value(&market, as_of).is_err());
}

#[test]
fn b3_shifted_swaption_prices_shifted_positive_rates() {
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_models::volatility::sabr::SabrParameters;
    use finstack_quant_valuations::instruments::rates::swaption::{Swaption, SwaptionParams};
    let as_of = date!(2024 - 01 - 02);
    let curve = DiscountCurve::builder("DISC")
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, 1.0)])
        .build()
        .expect("discount");
    let market = MarketContext::new().insert(curve).insert(
        ForwardCurve::builder("FWD", 0.25)
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.02), (10.0, 0.02)])
            .build()
            .expect("forward"),
    );
    let params = SwaptionParams::payer(
        Money::from((1_000_000_i64, Currency::USD)),
        0.02,
        date!(2025 - 01 - 02),
        date!(2025 - 01 - 02),
        date!(2030 - 01 - 02),
    )
    .expect("params");
    let mut option = Swaption::new("SHIFTED", &params, "DISC", "FWD", "VOL");
    let base = option
        .price_black(&market, 0.3, as_of)
        .expect("black")
        .amount();
    option.sabr_params =
        Some(SabrParameters::new_with_shift(0.03, 0.5, 0.5, -0.2, 0.03).expect("sabr"));
    let shifted = option
        .price_black(&market, 0.3, as_of)
        .expect("shifted")
        .amount();
    assert!(
        shifted > base * 2.0,
        "shift must affect positive rates too: base={base}, shifted={shifted}"
    );
    option
        .instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.3);
    let inputs = option
        .greek_inputs(&market, as_of)
        .expect("inputs")
        .expect("live option");
    let forward = inputs.forward + 0.03;
    let strike = 0.05;
    let std_dev = 0.3 * inputs.time_to_expiry.sqrt();
    let d1 = (forward / strike).ln() / std_dev + 0.5 * std_dev;
    let d2 = d1 - std_dev;
    let phi = (-0.5 * d1 * d1).exp() / std::f64::consts::TAU.sqrt();
    let cdf = finstack_quant_core::math::norm_cdf;
    let scale = 1_000_000.0 * inputs.annuity;
    let expected = scale * (forward * cdf(d1) - strike * cdf(d2));
    assert!((shifted - expected).abs() < 1e-7);
    option
        .instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price = Some(expected);
    let risk = option
        .price_with_metrics(
            &market,
            as_of,
            &[
                MetricId::Delta,
                MetricId::Gamma,
                MetricId::Vega,
                MetricId::ImpliedVol,
            ],
            PricingOptions::default(),
        )
        .expect("shifted Greeks and inversion");
    assert!((risk.measures["delta"] - scale * cdf(d1)).abs() < 1e-7);
    assert!((risk.measures["gamma"] - scale * phi / (forward * std_dev)).abs() < 1e-5);
    assert!(
        (risk.measures["vega"] - scale * forward * inputs.time_to_expiry.sqrt() * phi * 0.01).abs()
            < 1e-7
    );
    assert!((risk.measures["implied_vol"] - 0.3).abs() < 1e-9);
    assert!(option.price_black(&market, f64::NAN, as_of).is_err());
}

#[test]
fn m4_inflation_zero_strike_requires_normal_quotes_without_fabricated_conversion() {
    use finstack_quant_core::market_data::{
        scalars::InflationLag,
        surfaces::{VolQuoteType, VolSurface},
        term_structures::InflationCurve,
    };
    use finstack_quant_valuations::instruments::rates::inflation_cap_floor::{
        InflationCapFloor, InflationCapFloorType,
    };
    use finstack_quant_valuations::pricer::ModelKey;
    let as_of = date!(2024 - 01 - 15);
    let mut option = InflationCapFloor::example();
    option.option_type = InflationCapFloorType::Floorlet;
    option.start_date = date!(2025 - 01 - 15);
    option.maturity = date!(2026 - 01 - 15);
    option.strike = rust_decimal::Decimal::ZERO;
    option.lag_override = Some(InflationLag::None);
    let market = MarketContext::new()
        .insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (5.0, 1.0)])
                .build()
                .expect("discount"),
        )
        .insert(
            InflationCurve::builder("US-CPI")
                .base_date(as_of)
                .base_cpi(300.0)
                .knots([(0.0, 300.0), (1.0, 306.0), (2.0, 312.12), (5.0, 331.224)])
                .build()
                .expect("inflation"),
        )
        .insert_surface(
            VolSurface::builder("USD-INFL-VOL")
                .expiries(&[1.0, 2.0])
                .strikes(&[0.0, 0.02])
                .row(&[0.2, 0.2])
                .row(&[0.2, 0.2])
                .build()
                .expect("black surface"),
        );
    assert!(option
        .npv_with_model(&market, as_of, ModelKey::Black76)
        .is_err());
    let market = market.insert_surface(
        VolSurface::builder("USD-INFL-VOL")
            .quote_type(VolQuoteType::Normal)
            .expiries(&[1.0, 2.0])
            .strikes(&[0.0, 0.02])
            .row(&[0.01, 0.01])
            .row(&[0.01, 0.01])
            .build()
            .expect("normal surface"),
    );
    assert!(
        option
            .npv_with_model(&market, as_of, ModelKey::Normal)
            .expect("normal floor")
            .amount()
            > 0.0
    );
}

#[test]
fn m3_prepared_hw_model_rejects_stale_curve_and_date() {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_models::rates::hull_white::HullWhiteCalibrationParams;
    use finstack_quant_valuations::instruments::rates::swaption::BermudanSwaptionTreeValuator;
    use finstack_quant_valuations::instruments::rates::swaption::{
        BermudanSwaption, PreparedHullWhiteModel,
    };
    let as_of = date!(2025 - 01 - 01);
    let mut swaption = BermudanSwaption::example();
    swaption.underlying_float_leg.forward_curve_id = swaption.get_discount_curve_id().clone();
    let curve = |rate: f64| {
        DiscountCurve::builder(swaption.get_discount_curve_id().clone())
            .base_date(as_of)
            .knots([(0.0, 1.0), (20.0, (-rate * 20.0).exp())])
            .build()
            .expect("curve")
    };
    let original = curve(0.03);
    let times = swaption.exercise_times(as_of).expect("times");
    let model = PreparedHullWhiteModel::prepare(
        HullWhiteCalibrationParams::new(0.05, 0.01).expect("params"),
        30,
        &original,
        as_of,
        swaption.time_to_maturity(as_of).expect("maturity"),
        &times,
    )
    .expect("model");
    assert!(BermudanSwaptionTreeValuator::new(&swaption, &model, &original, as_of).is_ok());
    assert!(
        BermudanSwaptionTreeValuator::new(&swaption, &model, &curve(0.04), as_of).is_err(),
        "stale curve accepted"
    );
    assert!(
        BermudanSwaptionTreeValuator::new(&swaption, &model, &original, date!(2025 - 01 - 02))
            .is_err(),
        "stale valuation date accepted"
    );
    let unaligned = PreparedHullWhiteModel::prepare(
        HullWhiteCalibrationParams::new(0.05, 0.01).expect("params"),
        7,
        &original,
        as_of,
        swaption.time_to_maturity(as_of).expect("maturity"),
        &[],
    )
    .expect("unaligned model");
    assert!(
        BermudanSwaptionTreeValuator::new(&swaption, &unaligned, &original, as_of).is_err(),
        "unsupported exercise was rounded to a grid node"
    );
    let short = PreparedHullWhiteModel::prepare(
        HullWhiteCalibrationParams::new(0.05, 0.01).expect("params"),
        30,
        &original,
        as_of,
        times[times.len() - 1] + 0.1,
        &times,
    )
    .expect("short model");
    assert!(
        BermudanSwaptionTreeValuator::new(&swaption, &short, &original, as_of).is_err(),
        "cashflows beyond the prepared horizon were accepted"
    );
}

#[test]
fn m11_hw_greeks_use_the_active_parameters_and_tree_grid() {
    use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
    use finstack_quant_valuations::instruments::rates::swaption::BermudanSwaption;
    let as_of = date!(2025 - 01 - 01);
    let mut swaption = BermudanSwaption::example();
    swaption.underlying_float_leg.forward_curve_id = swaption.get_discount_curve_id().clone();
    swaption
        .instrument_pricing_overrides
        .model_config
        .hw1f_mean_reversion = Some(0.12);
    swaption
        .instrument_pricing_overrides
        .model_config
        .hw1f_sigma = Some(0.025);
    swaption
        .instrument_pricing_overrides
        .model_config
        .tree_steps = Some(70);
    let curve_id = swaption.get_discount_curve_id().clone();
    let market = MarketContext::new().insert(
        DiscountCurve::builder(curve_id.clone())
            .base_date(as_of)
            .knots([(0.0, 1.0), (20.0, (-0.6_f64).exp())])
            .build()
            .expect("curve"),
    );
    let base = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta, MetricId::HwSigmaVega],
            PricingOptions::default(),
        )
        .expect("risk");
    let up = market
        .bump([MarketBump::Curve {
            id: curve_id.clone(),
            spec: BumpSpec::parallel_bp(1.0),
        }])
        .expect("up");
    let down = market
        .bump([MarketBump::Curve {
            id: curve_id,
            spec: BumpSpec::parallel_bp(-1.0),
        }])
        .expect("down");
    let delta = (swaption
        .price_with_metrics(&up, as_of, &[], PricingOptions::default())
        .expect("up price")
        .value
        .amount()
        - swaption
            .price_with_metrics(&down, as_of, &[], PricingOptions::default())
            .expect("down price")
            .value
            .amount())
        / 0.0002;
    assert!(
        (base.measures["delta"] - delta).abs() < 1e-5,
        "reported={}, actual={delta}",
        base.measures["delta"]
    );
    let mut vol_up = swaption.clone();
    vol_up.instrument_pricing_overrides.model_config.hw1f_sigma = Some(0.025 * 1.01);
    let mut vol_down = swaption;
    vol_down
        .instrument_pricing_overrides
        .model_config
        .hw1f_sigma = Some(0.025 * 0.99);
    let vega = (vol_up
        .price_with_metrics(&market, as_of, &[], PricingOptions::default())
        .expect("vol up")
        .value
        .amount()
        - vol_down
            .price_with_metrics(&market, as_of, &[], PricingOptions::default())
            .expect("vol down")
            .value
            .amount())
        / (2.0 * 0.025 * 0.01)
        * 0.01;
    assert!((base.measures["hw_sigma_vega"] - vega).abs() < 1e-5);
}

#[test]
fn b13_unpaid_principal_remains_exposed_to_credit_recovery() {
    use finstack_quant_core::market_data::term_structures::HazardCurve;
    use finstack_quant_core::types::Rate;
    use finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec;
    let issue = date!(2024 - 03 - 01);
    let maturity = date!(2025 - 03 - 01); // Saturday; paid on Monday March 3.
    let mut bond = Bond::fixed(
        "UNPAID",
        Money::from((100_i64, Currency::USD)),
        Rate::from_decimal(0.05).expect("rate"),
        issue,
        maturity,
        StubKind::None,
        "D",
    )
    .expect("bond");
    bond.settlement_convention = None;
    bond.credit_curve_id = Some("H".into());
    if let CashflowSpec::Fixed(spec) = &mut bond.cashflow_spec {
        spec.schedule.frequency = Tenor::annual();
        spec.schedule.day_count = DayCount::Act365F;
        spec.schedule.business_day_convention = BusinessDayConvention::Following;
        spec.schedule.calendar_id = "weekends_only".into();
    }
    let market = MarketContext::new()
        .insert(
            DiscountCurve::builder("D")
                .base_date(maturity)
                .knots([(0.0, 1.0), (1.0, 1.0)])
                .build()
                .expect("discount"),
        )
        .insert(
            HazardCurve::builder("H")
                .base_date(maturity)
                .recovery_rate(0.4)
                .knots([(0.0, 0.2), (1.0, 0.2)])
                .build()
                .expect("hazard"),
        );
    let survival = (-0.2_f64 * 2.0 / 365.0).exp();
    let expected = 105.0 * survival + 40.0 * (1.0 - survival);
    let actual = bond.value(&market, maturity).expect("price").amount();
    assert!(
        (actual - expected).abs() < 1e-8,
        "actual={actual}, expected={expected}"
    );
}
