//! Unit tests for equity-option Black-Scholes, tree, lifecycle, and dividend pricing.

use super::black::EquityOptionGreeks;
use super::inputs::adjust_spot_for_discrete_dividends;
use super::*;
use crate::equity_option::types::{EquityOption, EquityOptionExercise};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};
use finstack_quant_models::closed_form::bs_greeks_unchecked;
use finstack_quant_models::types::{ExerciseStyle, OptionMarketParams, OptionType};
use finstack_quant_valuations_core::instruments::pricing_overrides::ThetaDayBasis;
use finstack_quant_valuations_core::instruments::{Instrument, SettlementType};
use finstack_quant_valuations_core::pricer::ModelKey;
use time::Month;

pub(super) fn date(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
        .expect("valid date")
}

pub(super) fn market(as_of: Date, spot: f64, vol: f64, rate: f64, div_yield: f64) -> MarketContext {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (10.0, (-rate * 10.0).exp())])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("curve");
    let surface = VolSurface::builder("SPX-VOL")
        .expiries(&[0.25, 0.5, 1.0, 2.0])
        .secondary_values(&[80.0, 100.0, 120.0, 150.0])
        .secondary_axis(finstack_quant_core::market_data::surfaces::VolSurfaceAxis::Strike)
        .quote_type(finstack_quant_core::market_data::surfaces::VolQuoteType::BlackLognormal)
        .interpolation_mode(finstack_quant_core::market_data::surfaces::VolInterpolationMode::Vol)
        .row(&[vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol])
        .build()
        .expect("surface");

    MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve")
        .insert_surface(surface)
        .insert_price(
            "SPX-SPOT",
            MarketScalar::unitless(spot).expect("valid scalar"),
        )
        .insert_price(
            "SPX-DIV",
            MarketScalar::unitless(div_yield).expect("valid scalar"),
        )
}

pub(super) fn option(
    expiry: Date,
    option_type: OptionType,
    exercise_style: ExerciseStyle,
) -> EquityOption {
    EquityOption::builder()
        .id(InstrumentId::new("EQ-OPT-TEST"))
        .underlying_ticker("SPX".to_string())
        .strike(100.0)
        .option_type(option_type)
        .exercise_style(exercise_style)
        .expiry(expiry)
        .exercise_dates_opt(matches!(exercise_style, ExerciseStyle::Bermudan).then(|| vec![expiry]))
        .quantity(100.0)
        .currency(Currency::USD)
        .day_count(DayCount::Act365F)
        .settlement(SettlementType::Cash)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id("SPX-SPOT".into())
        .vol_surface_id(CurveId::new("SPX-VOL"))
        .div_yield_id_opt(Some(PriceId::new("SPX-DIV")))
        .attributes(Attributes::new())
        .build()
        .expect("equity option")
}

#[test]
fn equity_option_default_equals_black76_for_pv_and_raw() {
    let as_of = date(2025, 1, 1);
    let option = option(date(2026, 1, 1), OptionType::Call, ExerciseStyle::European);
    let market = market(as_of, 100.0, 0.20, 0.03, 0.01);
    let registry = crate::EQUITY.pricers();

    let default = option
        .price_with_metrics(
            &market,
            as_of,
            &[],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .expect("default equity-option price");
    let black76 = registry
        .price_with_metrics(
            &option,
            ModelKey::Black76,
            &market,
            as_of,
            &[],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .expect("Black76 equity-option price");
    let default_raw = option
        .value_raw(&market, as_of)
        .expect("default equity-option raw price");
    let black76_raw = registry
        .price_raw(&option, ModelKey::Black76, &market, as_of)
        .expect("Black76 equity-option raw price");

    assert_eq!(default.value, black76.value);
    assert_eq!(default_raw, black76_raw);
}

#[test]
fn expiry_requires_observed_lifecycle_state() {
    let expiry = date(2025, 6, 20);
    let option = option(expiry, OptionType::Call, ExerciseStyle::European);
    let error = compute_pv(&option, &market(expiry, 120.0, 0.2, 0.05, 0.0), expiry)
        .expect_err("expiry without an observation must fail");
    assert!(error.to_string().contains("exercise/expiry observation"));
}

#[test]
fn cash_exercise_remains_until_payment() {
    let expiry = date(2025, 6, 20);
    let settlement = date(2025, 6, 23);
    let mut option = option(expiry, OptionType::Call, ExerciseStyle::European);
    option.exercise = Some(EquityOptionExercise::new(expiry, 120.0, settlement, true));
    let market = market(expiry, 121.0, 0.2, 0.05, 0.0);

    let pv = compute_pv(&option, &market, expiry)
        .expect("fixed cash payoff")
        .amount();
    let df = market
        .get_discount("USD-OIS")
        .expect("discount curve")
        .df_between_dates(expiry, settlement)
        .expect("settlement discount factor");
    assert!((pv - 20.0 * 100.0 * df).abs() < 1e-9);

    let after_settlement = date(2025, 6, 24);
    assert_eq!(
        compute_pv(&option, &market, after_settlement)
            .expect("settled option")
            .amount(),
        0.0
    );
}

#[test]
fn physical_exercise_marks_delivery_obligation() {
    let expiry = date(2025, 6, 20);
    let settlement = date(2025, 6, 23);
    let mut option = option(expiry, OptionType::Call, ExerciseStyle::European);
    option.settlement = SettlementType::Physical;
    option.exercise = Some(EquityOptionExercise::new(expiry, 120.0, settlement, true));
    let market = market(expiry, 121.0, 0.2, 0.05, 0.0);
    let df = market
        .get_discount("USD-OIS")
        .expect("discount curve")
        .df_between_dates(expiry, settlement)
        .expect("settlement discount factor");

    let pv = compute_pv(&option, &market, expiry)
        .expect("physical delivery mark")
        .amount();
    assert!((pv - (121.0 - 100.0 * df) * 100.0).abs() < 1e-9);
    let greeks = compute_greeks(&option, &market, expiry).expect("delivery risk");
    assert!((greeks.delta - 100.0).abs() < 1e-12);
}

#[test]
fn american_discrete_dividend_preserves_pre_exercise_spot() {
    let as_of = date(2025, 1, 2);
    let expiry = date(2026, 1, 2);
    let mut american = option(expiry, OptionType::Call, ExerciseStyle::American);
    american.strike = 50.0;
    american.discrete_dividends = vec![(date(2025, 2, 3), 20.0)];
    american.validate().expect("valid discrete-dividend option");
    let mut european = american.clone();
    european.exercise_style = ExerciseStyle::European;
    let market = market(as_of, 100.0, 0.2, 0.05, 0.0);

    let american_pv = compute_pv(&american, &market, as_of)
        .expect("American discrete-dividend price")
        .amount();
    let european_pv = compute_pv(&european, &market, as_of)
        .expect("European escrowed-dividend price")
        .amount();

    assert!(american_pv >= 50.0 * american.quantity);
    assert!(
        american_pv > european_pv,
        "large near-term dividend should create an early-exercise premium"
    );
}

#[test]
fn theta_day_basis_is_explicit_and_configurable() {
    let as_of = date(2025, 1, 2);
    let expiry = date(2026, 1, 2);
    let calendar = option(expiry, OptionType::Call, ExerciseStyle::European);
    let mut trading = calendar.clone();
    trading.metric_pricing_overrides.theta_day_basis = Some(ThetaDayBasis::Trading252);
    let market = market(as_of, 100.0, 0.2, 0.05, 0.01);

    let calendar_theta = compute_greeks(&calendar, &market, as_of)
        .expect("calendar theta")
        .theta;
    let trading_theta = compute_greeks(&trading, &market, as_of)
        .expect("trading theta")
        .theta;
    assert!((trading_theta / calendar_theta - 365.0 / 252.0).abs() < 1e-12);
}

#[test]
fn test_adjust_spot_for_discrete_dividends_single() {
    // Stock at $100, dividend of $2 in 0.25 years, r = 5%
    let s_adj = adjust_spot_for_discrete_dividends(100.0, 0.05, &[(0.25, 2.0)])
        .expect("valid adjusted spot");
    // PV(div) = 2 × e^{-0.05×0.25} ≈ 1.9751
    assert!((s_adj - 98.0248).abs() < 0.01);
}

#[test]
fn test_adjust_spot_for_discrete_dividends_multiple() {
    let s_adj = adjust_spot_for_discrete_dividends(100.0, 0.05, &[(0.25, 1.5), (0.5, 1.5)])
        .expect("valid adjusted spot");
    let expected = 100.0 - 1.5 * (-0.05 * 0.25_f64).exp() - 1.5 * (-0.05 * 0.5_f64).exp();
    assert!((s_adj - expected).abs() < 1e-10);
}

#[test]
fn test_adjust_spot_for_discrete_dividends_rejects_nonpositive_result() {
    let error = adjust_spot_for_discrete_dividends(1.0, 0.01, &[(0.1, 50.0)])
        .expect_err("dividend PV above spot must fail");
    assert!(
        error
            .to_string()
            .contains("escrowed-dividend model invalid")
    );
}

#[test]
fn test_adjust_spot_for_discrete_dividends_empty() {
    let s_adj = adjust_spot_for_discrete_dividends(100.0, 0.05, &[]).expect("unchanged spot");
    assert!((s_adj - 100.0).abs() < 1e-12);
}

#[test]
fn test_adjust_spot_for_discrete_dividends_skips_past() {
    // Dividend at t=0 or negative should be skipped
    let s_adj = adjust_spot_for_discrete_dividends(100.0, 0.05, &[(0.0, 5.0), (-0.1, 3.0)])
        .expect("past dividends ignored");
    assert!((s_adj - 100.0).abs() < 1e-12);
}

/// Escrowed-dividend rho must include the `∂S*/∂r` chain-rule term.
///
/// With discrete dividends the BS inputs use `S* = S − Σ D·e^{−r·t}`, which
/// depends on `r`. The analytic rho from `compute_greeks` must therefore
/// match a finite-difference rho computed by bumping the discount-curve
/// rate (which re-derives `S*` at the bumped rate). Before the fix, rho
/// held `S*` fixed and disagreed with the FD rho by `delta·∂S*/∂r`.
#[test]
fn escrowed_dividend_rho_includes_spot_rate_sensitivity() {
    let as_of = date(2025, 1, 1);
    let expiry = date(2026, 1, 1); // ~1y
    let mut opt = option(expiry, OptionType::Call, ExerciseStyle::European);
    // A sizeable dividend mid-life makes ∂S*/∂r materially non-zero.
    opt.discrete_dividends = vec![(date(2025, 7, 1), 8.0)];

    let base_rate = 0.04;
    let analytic = compute_greeks(&opt, &market(as_of, 100.0, 0.20, base_rate, 0.0), as_of)
        .expect("analytic greeks")
        .rho;

    // Central finite-difference rho of the full PV over the curve rate.
    // compute_pv re-derives r (and hence S*) from the curve, so this FD
    // captures the ∂S*/∂r contribution that the analytic rho must match.
    let h = 1e-4; // 1bp in rate space
    let pv_up = compute_pv(&opt, &market(as_of, 100.0, 0.20, base_rate + h, 0.0), as_of)
        .expect("pv up")
        .amount();
    let pv_dn = compute_pv(&opt, &market(as_of, 100.0, 0.20, base_rate - h, 0.0), as_of)
        .expect("pv dn")
        .amount();
    // analytic rho is per 1bp; FD slope per unit-rate * 0.0001.
    let fd_rho = (pv_up - pv_dn) / (2.0 * h) * 0.0001;

    let denom = analytic.abs().max(fd_rho.abs()).max(1e-9);
    assert!(
        (analytic - fd_rho).abs() / denom < 5e-3,
        "escrowed-dividend rho must match FD rho of the full PV (which \
         re-derives S* at the bumped rate): analytic={analytic} fd={fd_rho}"
    );

    // And it must NOT equal the naive rho that holds S* fixed.
    let inputs =
        collect_inputs(&opt, &market(as_of, 100.0, 0.20, base_rate, 0.0), as_of).expect("inputs");
    let naive = bs_greeks_unchecked(
        &OptionMarketParams {
            spot: inputs.spot,
            strike: opt.strike,
            rate: inputs.r,
            volatility: inputs.sigma,
            time_to_expiry: inputs.t_vol,
            dividend_yield: inputs.q,
            option_type: opt.option_type,
        },
        opt.metric_pricing_overrides.theta_days_per_year(),
    )
    .rho_r
        * opt.quantity
        / 100.0; // `BsGreeks::rho_r` is per 1%; compare per 1bp.
    assert!(
        (analytic - naive).abs() / denom > 1e-3,
        "the ∂S*/∂r correction must move rho away from the S*-fixed value: \
         analytic={analytic} naive={naive}"
    );
}

#[test]
fn cash_settlement_has_zero_delta_at_expiry() {
    let as_of = date(2025, 1, 1);
    let mut call = option(as_of, OptionType::Call, ExerciseStyle::European);
    let mut put = option(as_of, OptionType::Put, ExerciseStyle::European);
    call.exercise = Some(EquityOptionExercise::new(as_of, 100.0, as_of, true));
    put.exercise = Some(EquityOptionExercise::new(as_of, 100.0, as_of, true));
    let curves = market(as_of, 100.0, 0.20, 0.03, 0.01);

    let call_greeks = compute_greeks(&call, &curves, as_of).expect("call greeks");
    let put_greeks = compute_greeks(&put, &curves, as_of).expect("put greeks");

    assert_eq!(call_greeks, EquityOptionGreeks::default());
    assert_eq!(put_greeks, EquityOptionGreeks::default());
}

/// Short-dated tree FD gamma must be well-conditioned.
///
/// An American call on a non-dividend-paying underlying is never optimally
/// exercised early, so its price (and gamma) equals the European value.
/// For a short-dated near-ATM option the analytic BS gamma is therefore a
/// reliable oracle. With the old 1%-of-spot gamma bump the tree second
/// difference is noise-dominated and gamma drifts well off the analytic
/// value; the wider `σ√t`-scaled bump keeps it close.
#[test]
fn short_dated_tree_gamma_is_well_conditioned() {
    let as_of = date(2025, 1, 1);
    // ~3-week expiry: short enough that a 1%-of-spot bump is noise-prone.
    let expiry = date(2025, 1, 22);
    let mut american = option(expiry, OptionType::Call, ExerciseStyle::American);
    american
        .instrument_pricing_overrides
        .model_config
        .tree_steps = Some(201);
    // Zero dividend yield => American call == European call.
    let curves = market(as_of, 100.0, 0.20, 0.03, 0.0);

    let tree_greeks = compute_greeks(&american, &curves, as_of).expect("tree greeks");

    // Analytic European gamma with the same inputs.
    let inputs = collect_inputs(&american, &curves, as_of).expect("inputs");
    let analytic = bs_greeks_unchecked(
        &OptionMarketParams {
            spot: inputs.spot,
            strike: american.strike,
            rate: inputs.r,
            volatility: inputs.sigma,
            time_to_expiry: inputs.t_vol,
            dividend_yield: inputs.q,
            option_type: american.option_type,
        },
        american.metric_pricing_overrides.theta_days_per_year(),
    )
    .gamma
        * american.quantity;

    assert!(
        analytic > 0.0 && tree_greeks.gamma > 0.0,
        "gamma must be positive: analytic={analytic} tree={}",
        tree_greeks.gamma
    );
    let rel_err = (tree_greeks.gamma - analytic).abs() / analytic;
    assert!(
        rel_err < 0.05,
        "short-dated tree gamma must track analytic gamma within 5%: \
         analytic={analytic} tree={} rel_err={rel_err}",
        tree_greeks.gamma
    );
}

#[test]
fn test_american_call_tree_path_prices_above_european() {
    let as_of = date(2025, 1, 1);
    let expiry = date(2025, 7, 1);
    let mut european = option(expiry, OptionType::Call, ExerciseStyle::European);
    let mut american = option(expiry, OptionType::Call, ExerciseStyle::American);
    european
        .instrument_pricing_overrides
        .model_config
        .tree_steps = Some(51);
    american
        .instrument_pricing_overrides
        .model_config
        .tree_steps = Some(51);
    let curves = market(as_of, 105.0, 0.22, 0.03, 0.01);

    let european_pv = compute_pv(&european, &curves, as_of).expect("european pv");
    let american_pv = compute_pv(&american, &curves, as_of).expect("american pv");

    assert!(american_pv.amount().is_finite());
    // Exercise dominance compares the same discretization. LR's finite
    // European approximation can lie either side of the analytic value.
    let inputs::EquityOptionInputs {
        spot,
        r: rate,
        q,
        sigma,
        t_vol: t,
    } = collect_inputs(&european, &curves, as_of).expect("inputs");
    let mut params = finstack_quant_models::types::OptionMarketParams {
        spot,
        strike: european.strike,
        rate,
        volatility: sigma,
        time_to_expiry: t,
        dividend_yield: 0.0,
        option_type: finstack_quant_models::types::OptionType::Call,
    };
    params.dividend_yield = q;
    let tree_european = finstack_quant_models::trees::binomial_tree::BinomialTree::new(
        51,
        finstack_quant_models::trees::binomial_tree::TreeType::LeisenReimer,
    )
    .price_european(&params)
    .expect("European lattice")
        * european.quantity;
    assert!(american_pv.amount() >= tree_european);
    assert!((tree_european - european_pv.amount()).abs() < 0.01 * european.quantity);
}

#[test]
fn test_bermudan_schedule_filters_invalid_dates_before_tree_pricing() {
    let as_of = date(2025, 1, 1);
    let expiry = date(2025, 7, 1);
    let mut filtered = option(expiry, OptionType::Put, ExerciseStyle::Bermudan);
    let mut noisy = option(expiry, OptionType::Put, ExerciseStyle::Bermudan);
    filtered
        .instrument_pricing_overrides
        .model_config
        .tree_steps = Some(51);
    noisy.instrument_pricing_overrides.model_config.tree_steps = Some(51);
    filtered.exercise_dates = Some(vec![date(2025, 3, 1), date(2025, 5, 1)]);
    noisy.exercise_dates = Some(vec![
        as_of,
        date(2024, 12, 15),
        date(2025, 3, 1),
        date(2025, 5, 1),
        date(2025, 8, 1),
    ]);
    let curves = market(as_of, 95.0, 0.25, 0.03, 0.0);

    let filtered_pv = compute_pv(&filtered, &curves, as_of).expect("filtered bermudan pv");
    let noisy_pv = compute_pv(&noisy, &curves, as_of).expect("noisy bermudan pv");

    assert!((filtered_pv.amount() - noisy_pv.amount()).abs() < 1e-10);
}

#[test]
fn canonical_theta_uses_calendar_365_basis() {
    let as_of = date(2025, 1, 1);
    let expiry = date(2026, 1, 1);
    let curves = market(as_of, 100.0, 0.20, 0.03, 0.0);
    let option = option(expiry, OptionType::Call, ExerciseStyle::European);
    let theta = finstack_quant_valuations_core::instruments::OptionGreeksProvider::option_greeks(
        &option,
        &curves,
        as_of,
        &finstack_quant_valuations_core::instruments::OptionGreeksRequest {
            greek: finstack_quant_valuations_core::instruments::OptionGreekKind::Theta,
            base_pv: None,
            bumps: finstack_quant_valuations_core::instruments::GreekBumps::default(),
        },
    )
    .map(|greeks| greeks.theta)
    .expect("theta")
    .expect("supported");
    let inputs = collect_inputs(&option, &curves, as_of).expect("inputs");
    let expected = bs_greeks_unchecked(
        &OptionMarketParams {
            spot: inputs.spot,
            strike: option.strike,
            rate: inputs.r,
            volatility: inputs.sigma,
            time_to_expiry: inputs.t_vol,
            dividend_yield: inputs.q,
            option_type: option.option_type,
        },
        365.0,
    )
    .theta
        * option.quantity;

    assert!((theta - expected).abs() < 1e-12);
}

#[test]
fn option_rejects_spot_price_in_wrong_currency() {
    let as_of = date(2025, 1, 1);
    let expiry = date(2026, 1, 1);
    let curves = market(as_of, 100.0, 0.20, 0.03, 0.0).insert_price(
        "SPX-SPOT",
        MarketScalar::price(Money::from((100_i64, Currency::EUR))),
    );
    let option = option(expiry, OptionType::Call, ExerciseStyle::European);

    assert!(matches!(
        compute_pv(&option, &curves, as_of),
        Err(finstack_quant_core::Error::CurrencyMismatch {
            expected: Currency::USD,
            actual: Currency::EUR,
        })
    ));
}

#[test]
fn post_expiry_value_and_greeks_are_zero_without_market_data() {
    let expiry = date(2025, 1, 1);
    let as_of = date(2025, 1, 2);
    let mut option = option(expiry, OptionType::Call, ExerciseStyle::European);
    option.exercise = Some(EquityOptionExercise::new(expiry, 100.0, expiry, false));
    let empty = MarketContext::new();
    let pv = compute_pv(&option, &empty, as_of).expect("post-expiry PV");
    let greeks = compute_greeks(&option, &empty, as_of).expect("post-expiry greeks");
    assert_eq!(pv.amount(), 0.0);
    assert_eq!(greeks.delta, 0.0);
    assert_eq!(greeks.gamma, 0.0);
    assert_eq!(greeks.vega, 0.0);
}
