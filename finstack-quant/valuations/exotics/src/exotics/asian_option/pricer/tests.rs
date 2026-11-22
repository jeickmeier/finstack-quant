use super::*;
use crate::exotics::asian_option::AsianOption;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::{
    VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};
use finstack_quant_models::closed_form::{AsianFixingSchedule, geometric_asian_price};
use finstack_quant_models::monte_carlo::payoff::asian::AveragingMethod;
use finstack_quant_models::types::{OptionMarketParams, OptionType};
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::pricer::ModelKey;
use time::Month;

fn date(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
        .expect("valid date")
}

fn market(as_of: Date, spot: f64, vol: f64, rate: f64, div_yield: f64) -> MarketContext {
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (5.0, (-rate * 5.0).exp())])
        .interp(InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");

    let vol_surface = VolSurface::builder("SPX-VOL")
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .expiries(&[0.25, 0.5, 1.0, 2.0])
        .secondary_values(&[80.0, 90.0, 100.0, 110.0, 120.0])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .build()
        .expect("vol surface");

    MarketContext::new()
        .try_insert(discount)
        .expect("insert market curve")
        .insert_surface(vol_surface)
        .insert_price(
            "SPX-SPOT",
            MarketScalar::unitless(spot).expect("valid scalar"),
        )
        .insert_price(
            "SPX-DIV",
            MarketScalar::unitless(div_yield).expect("valid scalar"),
        )
}

fn market_without_vol(as_of: Date, spot: f64, rate: f64, div_yield: f64) -> MarketContext {
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (5.0, (-rate * 5.0).exp())])
        .interp(InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");

    MarketContext::new()
        .try_insert(discount)
        .expect("insert market curve")
        .insert_price(
            "SPX-SPOT",
            MarketScalar::unitless(spot).expect("valid scalar"),
        )
        .insert_price(
            "SPX-DIV",
            MarketScalar::unitless(div_yield).expect("valid scalar"),
        )
}

fn asian_option(
    averaging: AveragingMethod,
    option_type: OptionType,
    expiry: Date,
    strike: f64,
    fixing_dates: Vec<Date>,
) -> AsianOption {
    AsianOption::builder()
        .id(InstrumentId::new("ASIAN-TEST"))
        .underlying_ticker("SPX".to_string())
        .strike(strike)
        .option_type(option_type)
        .averaging_method(averaging)
        .expiry(expiry)
        .fixing_dates(fixing_dates)
        .quantity(1.0)
        .currency(Currency::USD)
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id("SPX-SPOT".into())
        .vol_surface_id(CurveId::new("SPX-VOL"))
        .div_yield_id_opt(Some(PriceId::new("SPX-DIV")))
        .attributes(Default::default())
        .build()
        .expect("asian option")
}

#[test]
fn geometric_pricer_matches_kemna_vorst_call_benchmark() {
    let as_of = date(2025, 1, 2);
    let expiry = date(2026, 1, 2);
    let fixing_dates = vec![
        date(2025, 4, 2),
        date(2025, 7, 2),
        date(2025, 10, 2),
        date(2026, 1, 2),
    ];

    let spot = 100.0;
    let strike = 100.0;
    let vol = 0.20;
    let rate = 0.05;
    let div_yield = 0.00;

    let market = market(as_of, spot, vol, rate, div_yield);
    let option = asian_option(
        AveragingMethod::Geometric,
        OptionType::Call,
        expiry,
        strike,
        fixing_dates.clone(),
    );

    let pv = option.value(&market, as_of).expect("asian pv").amount();

    let t = option
        .day_count
        .year_fraction(as_of, expiry, DayCountContext::default())
        .expect("year fraction");
    // Benchmark on the ACTUAL fixing times (calendar quarters are not
    // exactly uniform under ACT/365F), matching the pricer's schedule-
    // aware formula.
    let times: Vec<f64> = fixing_dates
        .iter()
        .map(|d| {
            option
                .day_count
                .year_fraction(as_of, *d, DayCountContext::default())
                .expect("year fraction")
        })
        .collect();
    let expected = finstack_quant_models::closed_form::geometric_asian_price_times(
        spot,
        strike,
        div_yield,
        vol,
        &AsianFixingSchedule {
            t_expiry: t,
            df: (-rate * t).exp(),
            fixing_times: &times,
        },
        OptionType::Call,
    )
    .expect("valid schedule");
    // And it must stay close to the uniform-grid Kemna-Vorst benchmark.
    let kv = geometric_asian_price(
        &OptionMarketParams {
            spot,
            strike,
            rate,
            volatility: vol,
            time_to_expiry: t,
            dividend_yield: div_yield,
            option_type: OptionType::Call,
        },
        fixing_dates.len(),
    );
    let expected_money = Money::new(expected, Currency::USD)
        .expect("valid money fixture")
        .amount();

    assert!((pv - expected_money).abs() < 1e-12);
    assert!((pv - kv).abs() < 0.05, "pv {pv} far from Kemna-Vorst {kv}");
}

/// `model_config.mc_antithetic` must reach the Asian Monte Carlo engine
/// through the shared `merged_path_config`: toggling it changes the
/// simulated streams, so the PV moves, while both estimates stay within
/// Monte Carlo noise of the discrete-fixing geometric (Kemna-Vorst) price.
///
/// Tolerance: the discounted ATM 1y geometric-Asian payoff has a standard
/// deviation below 7, so 20,000 estimators give a standard error below
/// 0.05; 0.2 is four standard errors.
#[test]
fn asian_honours_mc_antithetic() {
    let as_of = date(2025, 1, 2);
    let expiry = date(2026, 1, 2);
    let fixing_dates = vec![
        date(2025, 4, 2),
        date(2025, 7, 2),
        date(2025, 10, 2),
        date(2026, 1, 2),
    ];
    let (spot, strike, vol, rate) = (100.0, 100.0, 0.20, 0.05);
    let market = market(as_of, spot, vol, rate, 0.0);
    let mut option = asian_option(
        AveragingMethod::Geometric,
        OptionType::Call,
        expiry,
        strike,
        fixing_dates.clone(),
    );
    option.instrument_pricing_overrides.model_config.mc_paths = Some(20_000);

    let pv_with = |antithetic: bool| {
        let mut inst = option.clone();
        inst.instrument_pricing_overrides.model_config.mc_antithetic = Some(antithetic);
        AsianOptionMcPricer::new()
            .price_internal(&inst, &market, as_of)
            .expect("mc price")
            .amount()
    };
    let plain = pv_with(false);
    let antithetic = pv_with(true);

    let t = option
        .day_count
        .year_fraction(as_of, expiry, DayCountContext::default())
        .expect("year fraction");
    let times: Vec<f64> = fixing_dates
        .iter()
        .map(|d| {
            option
                .day_count
                .year_fraction(as_of, *d, DayCountContext::default())
                .expect("year fraction")
        })
        .collect();
    let reference = finstack_quant_models::closed_form::geometric_asian_price_times(
        spot,
        strike,
        0.0,
        vol,
        &AsianFixingSchedule {
            t_expiry: t,
            df: (-rate * t).exp(),
            fixing_times: &times,
        },
        OptionType::Call,
    )
    .expect("valid schedule");

    assert_ne!(
        plain.to_bits(),
        antithetic.to_bits(),
        "mc_antithetic must change the simulated streams"
    );
    for pv in [plain, antithetic] {
        assert!(
            (pv - reference).abs() < 0.2,
            "MC geometric Asian {pv} outside 4 standard errors of {reference}"
        );
    }
}

#[test]
fn geometric_pricer_matches_kemna_vorst_put_benchmark() {
    let as_of = date(2025, 1, 2);
    let expiry = date(2026, 1, 2);
    let fixing_dates = vec![date(2025, 6, 2), date(2026, 1, 2)];

    let spot = 100.0;
    let strike = 110.0;
    let vol = 0.25;
    let rate = 0.03;
    let div_yield = 0.01;

    let market = market(as_of, spot, vol, rate, div_yield);
    let option = asian_option(
        AveragingMethod::Geometric,
        OptionType::Put,
        expiry,
        strike,
        fixing_dates.clone(),
    );

    let pv = option.value(&market, as_of).expect("asian pv").amount();

    let t = option
        .day_count
        .year_fraction(as_of, expiry, DayCountContext::default())
        .expect("year fraction");
    let times: Vec<f64> = fixing_dates
        .iter()
        .map(|d| {
            option
                .day_count
                .year_fraction(as_of, *d, DayCountContext::default())
                .expect("year fraction")
        })
        .collect();
    let expected = finstack_quant_models::closed_form::geometric_asian_price_times(
        spot,
        strike,
        div_yield,
        vol,
        &AsianFixingSchedule {
            t_expiry: t,
            df: (-rate * t).exp(),
            fixing_times: &times,
        },
        OptionType::Put,
    )
    .expect("valid schedule");
    let kv = geometric_asian_price(
        &OptionMarketParams {
            spot,
            strike,
            rate,
            volatility: vol,
            time_to_expiry: t,
            dividend_yield: div_yield,
            option_type: OptionType::Put,
        },
        fixing_dates.len(),
    );
    let expected_money = Money::new(expected, Currency::USD)
        .expect("valid money fixture")
        .amount();

    assert!((pv - expected_money).abs() < 1e-12);
    // With only 2 fixings the calendar schedule (≈0.41y, 1y) deviates
    // visibly from the uniform grid (0.5y, 1y); the prices stay close
    // but not equal.
    assert!((pv - kv).abs() < 0.5, "pv {pv} far from Kemna-Vorst {kv}");
}

#[test]
fn turnbull_wakeman_respects_fully_realized_average_payoff() {
    let as_of = date(2025, 7, 1);
    let expiry = date(2025, 12, 31);
    let fixing_dates = vec![
        date(2025, 1, 31),
        date(2025, 2, 28),
        date(2025, 3, 31),
        date(2025, 4, 30),
        date(2025, 5, 31),
        date(2025, 6, 30),
    ];

    let mut option = asian_option(
        AveragingMethod::Arithmetic,
        OptionType::Call,
        expiry,
        100.0,
        fixing_dates.clone(),
    );
    option.past_fixings = fixing_dates
        .iter()
        .copied()
        .zip([102.0, 101.0, 103.0, 104.0, 100.0, 105.0])
        .collect();

    let market = market(as_of, 100.0, 0.20, 0.05, 0.0);
    let pv = option.value(&market, as_of).expect("asian pv").amount();

    let average = [102.0, 101.0, 103.0, 104.0, 100.0, 105.0]
        .iter()
        .sum::<f64>()
        / fixing_dates.len() as f64;
    let payoff = (average - 100.0).max(0.0);
    let df = market
        .get_discount("USD-OIS")
        .expect("discount")
        .df(option
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction"))
        .expect("df");
    let expected_money = Money::new(payoff * df, Currency::USD)
        .expect("valid money fixture")
        .amount();

    assert!((pv - expected_money).abs() < 1e-12);
}

#[test]
fn mc_pricer_expired_uses_realized_arithmetic_average() {
    let as_of = date(2025, 6, 30);
    let fixing_dates = vec![date(2025, 4, 30), date(2025, 5, 31), date(2025, 6, 30)];
    let mut option = asian_option(
        AveragingMethod::Arithmetic,
        OptionType::Call,
        as_of,
        100.0,
        fixing_dates.clone(),
    );
    option.past_fixings = fixing_dates
        .iter()
        .copied()
        .zip([90.0, 110.0, 120.0])
        .collect();

    let pv = AsianOptionMcPricer::new()
        .price_internal(&option, &market(as_of, 999.0, 0.20, 0.05, 0.0), as_of)
        .expect("expired MC price")
        .amount();

    let expected = ((90.0_f64 + 110.0 + 120.0) / 3.0 - 100.0).max(0.0);
    assert!((pv - expected).abs() < 1e-12);
}

#[test]
fn mc_pricer_expired_without_fixings_errors() {
    let as_of = date(2025, 6, 30);
    let option = asian_option(
        AveragingMethod::Geometric,
        OptionType::Put,
        as_of,
        100.0,
        vec![date(2025, 3, 31), date(2025, 6, 30)],
    );

    let err = AsianOptionMcPricer::new()
        .price_internal(&option, &market(as_of, 80.0, 0.20, 0.05, 0.0), as_of)
        .expect_err("expired Asian option must require realized fixings");
    assert!(err.to_string().contains("missing a past fixing"));
}

#[test]
fn mc_price_dyn_wraps_market_data_errors() {
    let as_of = date(2025, 1, 2);
    let expiry = date(2025, 7, 2);
    let option = asian_option(
        AveragingMethod::Arithmetic,
        OptionType::Call,
        expiry,
        100.0,
        vec![date(2025, 3, 2), expiry],
    );

    let err = AsianOptionMcPricer::new()
        .price_dyn(&option, &market_without_vol(as_of, 100.0, 0.05, 0.0), as_of)
        .expect_err("missing vol surface should be wrapped");
    assert!(err.to_string().contains("SPX-VOL"));
}

#[test]
fn analytical_geometric_expired_uses_realized_average_and_ignores_spot() {
    let as_of = date(2025, 6, 30);
    let fixing_dates = vec![date(2025, 5, 31), as_of];
    let mut option = asian_option(
        AveragingMethod::Geometric,
        OptionType::Call,
        as_of,
        100.0,
        fixing_dates.clone(),
    );
    option.past_fixings = fixing_dates.iter().copied().zip([100.0, 121.0]).collect();

    let pricer = AsianOptionAnalyticalGeometricPricer;
    let low_spot_pv = pricer
        .price_dyn(&option, &market(as_of, 50.0, 0.20, 0.05, 0.0), as_of)
        .expect("expired geometric Asian price at low current spot")
        .value
        .amount();
    let high_spot_pv = pricer
        .price_dyn(&option, &market(as_of, 999.0, 0.20, 0.05, 0.0), as_of)
        .expect("expired geometric Asian price at high current spot")
        .value
        .amount();

    // sqrt(100 * 121) = 110, so the realized call payoff is exactly 10.
    assert!(
        (low_spot_pv - 10.0).abs() < 1e-12,
        "expired geometric payoff should be 10, got {low_spot_pv}"
    );
    assert!(
        (high_spot_pv - low_spot_pv).abs() < 1e-12,
        "expired geometric payoff must ignore current spot: low={low_spot_pv}, high={high_spot_pv}"
    );
}

#[test]
fn analytical_geometric_expired_without_fixings_errors() {
    let as_of = date(2025, 6, 30);
    let option = asian_option(
        AveragingMethod::Geometric,
        OptionType::Call,
        as_of,
        100.0,
        vec![date(2025, 3, 31), date(2025, 6, 30)],
    );

    let err = AsianOptionAnalyticalGeometricPricer
        .price_dyn(&option, &market(as_of, 125.0, 0.20, 0.05, 0.0), as_of)
        .expect_err("expired Asian option must require realized fixings");
    assert!(err.to_string().contains("missing a past fixing"));
}

#[test]
fn analytical_geometric_price_dyn_wraps_market_data_errors() {
    let as_of = date(2025, 1, 2);
    let expiry = date(2025, 7, 2);
    let option = asian_option(
        AveragingMethod::Geometric,
        OptionType::Call,
        expiry,
        100.0,
        vec![date(2025, 3, 2), expiry],
    );

    let err = AsianOptionAnalyticalGeometricPricer
        .price_dyn(&option, &market_without_vol(as_of, 100.0, 0.05, 0.0), as_of)
        .expect_err("missing vol surface should be wrapped");
    assert!(err.to_string().contains("SPX-VOL"));
}

#[test]
fn analytical_geometric_prices_seasoned_option() {
    let as_of = date(2025, 7, 1);
    let expiry = date(2025, 12, 31);
    let fixing_dates = vec![
        date(2025, 1, 31),
        date(2025, 3, 31),
        date(2025, 6, 30),
        date(2025, 9, 30),
        expiry,
    ];

    let mut option = asian_option(
        AveragingMethod::Geometric,
        OptionType::Call,
        expiry,
        100.0,
        fixing_dates.clone(),
    );
    option.past_fixings = fixing_dates
        .iter()
        .take(3)
        .copied()
        .zip([101.0, 103.0, 102.0])
        .collect();

    let res = AsianOptionAnalyticalGeometricPricer
        .price_dyn(&option, &market(as_of, 100.0, 0.20, 0.05, 0.0), as_of)
        .expect("seasoned geometric analytical pricing should succeed");
    assert!(res.value.amount() > 0.0 && res.value.amount().is_finite());
}

#[test]
fn turnbull_wakeman_all_fixings_past_put_discounts_deterministic_payoff() {
    let as_of = date(2025, 7, 1);
    let expiry = date(2025, 12, 31);
    let fixing_dates = vec![
        date(2025, 1, 31),
        date(2025, 2, 28),
        date(2025, 3, 31),
        date(2025, 4, 30),
        date(2025, 5, 31),
        date(2025, 6, 30),
    ];

    let mut option = asian_option(
        AveragingMethod::Arithmetic,
        OptionType::Put,
        expiry,
        100.0,
        fixing_dates.clone(),
    );
    option.past_fixings = fixing_dates
        .iter()
        .copied()
        .zip([92.0, 95.0, 97.0, 96.0, 94.0, 98.0])
        .collect();

    let market = market(as_of, 100.0, 0.20, 0.05, 0.0);
    let pv = AsianOptionSemiAnalyticalTwPricer
        .price_dyn(&option, &market, as_of)
        .expect("deterministic TW price")
        .value
        .amount();

    let average =
        [92.0, 95.0, 97.0, 96.0, 94.0, 98.0].iter().sum::<f64>() / fixing_dates.len() as f64;
    let payoff = (100.0 - average).max(0.0);
    let df = market
        .get_discount("USD-OIS")
        .expect("discount")
        .df(option
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction"))
        .expect("df");
    let expected = Money::new(payoff * df, Currency::USD)
        .expect("valid money fixture")
        .amount();

    assert!((pv - expected).abs() < 1e-12);
}

#[test]
fn turnbull_wakeman_negative_effective_strike_call_uses_forward_average_branch() {
    let as_of = date(2025, 4, 2);
    let expiry = date(2025, 10, 2);
    let fixing_dates = vec![
        date(2025, 1, 2),
        date(2025, 2, 2),
        date(2025, 3, 2),
        date(2025, 4, 2),
        date(2025, 5, 2),
        date(2025, 6, 2),
        date(2025, 7, 2),
        date(2025, 8, 2),
        date(2025, 9, 2),
        date(2025, 10, 2),
    ];

    let mut option = asian_option(
        AveragingMethod::Arithmetic,
        OptionType::Call,
        expiry,
        100.0,
        fixing_dates.clone(),
    );
    option.past_fixings = fixing_dates
        .iter()
        .take(4)
        .copied()
        .zip([250.0, 240.0, 260.0, 255.0])
        .collect();

    let market = market(as_of, 100.0, 0.20, 0.05, 0.01);
    let pv = AsianOptionSemiAnalyticalTwPricer
        .price_dyn(&option, &market, as_of)
        .expect("deep ITM TW price")
        .value
        .amount();

    let disc_curve = market.get_discount("USD-OIS").expect("discount");
    let df_expiry = disc_curve
        .df(option
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction"))
        .expect("df");
    let spot = 100.0;
    let q = 0.01;
    let sum_past = [250.0, 240.0, 260.0, 255.0].iter().sum::<f64>();
    let n = fixing_dates.len() as f64;
    let mut sum_fwd = 0.0;
    for date in fixing_dates.iter().copied().filter(|d| *d > as_of) {
        let t_i = option
            .day_count
            .year_fraction(as_of, date, DayCountContext::default())
            .expect("fixing year fraction");
        let df_i = disc_curve
            .df_between_dates(as_of, date)
            .expect("date-based discount factor");
        sum_fwd += spot * (-q * t_i).exp() / df_i;
    }
    let expected_avg = (sum_past + sum_fwd) / n;
    let expected = Money::new(
        (expected_avg - option.strike).max(0.0) * df_expiry,
        Currency::USD,
    )
    .expect("valid money fixture")
    .amount();

    assert!((pv - expected).abs() < 1e-12);
}

#[test]
fn turnbull_wakeman_negative_effective_strike_put_clamps_to_zero() {
    let as_of = date(2025, 4, 2);
    let expiry = date(2025, 10, 2);
    let fixing_dates = vec![
        date(2025, 1, 2),
        date(2025, 2, 2),
        date(2025, 3, 2),
        date(2025, 4, 2),
        date(2025, 5, 2),
        date(2025, 6, 2),
        date(2025, 7, 2),
        date(2025, 8, 2),
        date(2025, 9, 2),
        date(2025, 10, 2),
    ];

    let mut option = asian_option(
        AveragingMethod::Arithmetic,
        OptionType::Put,
        expiry,
        100.0,
        fixing_dates.clone(),
    );
    option.past_fixings = fixing_dates
        .iter()
        .take(4)
        .copied()
        .zip([250.0, 240.0, 260.0, 255.0])
        .collect();

    let pv = AsianOptionSemiAnalyticalTwPricer
        .price_dyn(&option, &market(as_of, 100.0, 0.20, 0.05, 0.01), as_of)
        .expect("deep OTM TW put should price")
        .value
        .amount();

    assert_eq!(pv, 0.0);
}

#[test]
fn mc_pricer_expired_without_fixings_rejects_price_scalar_fallback() {
    let as_of = date(2025, 6, 30);
    let option = asian_option(
        AveragingMethod::Arithmetic,
        OptionType::Call,
        as_of,
        100.0,
        vec![date(2025, 3, 31), date(2025, 6, 30)],
    );

    let market = market(as_of, 80.0, 0.20, 0.05, 0.0).insert_price(
        "SPX-SPOT",
        MarketScalar::price(Money::from((125_i64, Currency::USD))),
    );

    let err = AsianOptionMcPricer::new()
        .price_internal(&option, &market, as_of)
        .expect_err("price scalar must not replace past fixing history");
    assert!(err.to_string().contains("missing a past fixing"));
}

#[test]
fn turnbull_wakeman_price_dyn_wraps_market_data_errors() {
    let as_of = date(2025, 1, 2);
    let expiry = date(2025, 7, 2);
    let option = asian_option(
        AveragingMethod::Arithmetic,
        OptionType::Put,
        expiry,
        100.0,
        vec![date(2025, 3, 2), expiry],
    );

    let err = AsianOptionSemiAnalyticalTwPricer
        .price_dyn(&option, &market_without_vol(as_of, 100.0, 0.05, 0.0), as_of)
        .expect_err("missing vol surface should be wrapped");
    assert!(err.to_string().contains("SPX-VOL"));
}

/// Distinct fixing times are inserted exactly even on a coarse base grid;
/// no contractual observation is lost or shifted to an arbitrary free slot.
#[test]
fn w04_close_fixings_map_to_distinct_steps() {
    let as_of = date(2025, 1, 1);
    // Twelve monthly fixings within one year.
    let fixing_dates: Vec<Date> = (1..=12).map(|m| date(2025, m, 15)).collect();
    let t = DayCount::Act365F
        .year_fraction(as_of, date(2026, 1, 1), DayCountContext::default())
        .expect("year fraction");

    // A deliberately coarse base grid (4 steps) cannot resolve 12 monthly
    // fixings — the naive round()+dedup() would merge several of them.
    let grid =
        map_fixings_to_steps(&fixing_dates, DayCount::Act365F, as_of, t, 4).expect("fixing grid");

    assert_eq!(
        grid.fixing_steps.len(),
        12,
        "all 12 distinct monthly fixings must survive as distinct steps; \
         got {} — the coarse-grid round()+dedup() merged some",
        grid.fixing_steps.len()
    );
    // Steps must be strictly increasing (distinct) and within the grid.
    for w in grid.fixing_steps.windows(2) {
        assert!(
            w[1] > w[0],
            "fixing steps must be strictly increasing/distinct, got {:?}",
            grid.fixing_steps
        );
    }
    assert!(
        grid.fixing_steps
            .iter()
            .all(|&s| s >= 1 && s <= grid.time_grid.num_steps()),
        "every fixing step must lie in [1, num_steps={}]",
        grid.time_grid.num_steps()
    );
}

/// W-04: when fixings are well separated the helper does not blow up the
/// grid — it returns at least the base step count and keeps each fixing
/// distinct.
#[test]
fn w04_well_separated_fixings_keep_base_grid() {
    let as_of = date(2025, 1, 1);
    let fixing_dates = vec![date(2025, 4, 1), date(2025, 8, 1), date(2026, 1, 1)];
    let t = DayCount::Act365F
        .year_fraction(as_of, date(2026, 1, 1), DayCountContext::default())
        .expect("year fraction");
    let grid =
        map_fixings_to_steps(&fixing_dates, DayCount::Act365F, as_of, t, 252).expect("fixing grid");
    assert_eq!(grid.fixing_steps.len(), 3);
    assert!(grid.time_grid.num_steps() >= 252);
}

/// W-05: the control-variate moments must stay accurate even when the
/// per-path payoffs carry a large constant offset that would swamp a naive
/// sum of squared deviations.
#[test]
fn w05_compensated_moments_are_accurate_with_large_offset() {
    // Values with a huge offset: naive Σ(x-mean)² loses precision badly.
    let offset = 1e9;
    let xs: Vec<f64> = (0..10_000).map(|i| offset + (i % 7) as f64).collect();
    let ys: Vec<f64> = (0..10_000).map(|i| offset + (i % 5) as f64).collect();

    let moments = paired_moments(&xs, &ys);
    let var_x = moments
        .get_variance_x()
        .expect("finite variance_x")
        .expect("at least two observations");
    let cov = moments
        .get_covariance()
        .expect("finite covariance")
        .expect("at least two observations");

    // Reference variance of the repeating pattern 0..7 (independent of the
    // offset). Compute it directly on the de-offset values.
    let centered: Vec<f64> = xs.iter().map(|v| v - offset).collect();
    let ref_mean = centered.iter().sum::<f64>() / centered.len() as f64;
    let ref_var = centered.iter().map(|v| (v - ref_mean).powi(2)).sum::<f64>()
        / (centered.len() as f64 - 1.0);

    assert!(
        (var_x - ref_var).abs() < 1e-6,
        "compensated variance {var_x} must match reference {ref_var} \
         despite the 1e9 offset"
    );
    assert!(cov.is_finite(), "compensated covariance must be finite");
}

/// W-07: `seasoned_geometric_asian_control` must reduce to the standard
/// Kemna-Vorst geometric Asian value when the option is unseasoned
/// (`hist_count == 0`) and the fixings are equally spaced — the standard
/// closed form is the special case of the seasoned formula.
#[test]
fn w07_seasoned_geometric_control_reduces_to_kemna_vorst_unseasoned() {
    let spot = 100.0;
    let strike = 100.0;
    let r = 0.05_f64;
    let q = 0.0;
    let sigma = 0.20;
    let t = 1.0_f64;
    let n = 12usize;
    let df = (-r * t).exp();

    // Equally spaced fixings t_i = i*T/n, i = 1..n.
    let future_times: Vec<f64> = (1..=n).map(|i| i as f64 * t / n as f64).collect();

    let seasoned_call = seasoned_geometric_asian_control(
        &OptionMarketParams {
            spot,
            strike,
            rate: r,
            volatility: sigma,
            time_to_expiry: t,
            dividend_yield: q,
            option_type: OptionType::Call,
        },
        df,
        0.0,
        0,
        &future_times,
        None,
        None,
    )
    .expect("finite seasoned geometric call");
    let kv_call = geometric_asian_price(
        &OptionMarketParams {
            spot,
            strike,
            rate: r,
            volatility: sigma,
            time_to_expiry: t,
            dividend_yield: q,
            option_type: OptionType::Call,
        },
        n,
    );
    assert!(
        (seasoned_call - kv_call).abs() < 1e-9,
        "unseasoned seasoned-control call {seasoned_call} must equal \
         Kemna-Vorst {kv_call}"
    );

    let seasoned_put = seasoned_geometric_asian_control(
        &OptionMarketParams {
            spot,
            strike,
            rate: r,
            volatility: sigma,
            time_to_expiry: t,
            dividend_yield: q,
            option_type: OptionType::Put,
        },
        df,
        0.0,
        0,
        &future_times,
        None,
        None,
    )
    .expect("finite seasoned geometric put");
    let kv_put = geometric_asian_price(
        &OptionMarketParams {
            spot,
            strike,
            rate: r,
            volatility: sigma,
            time_to_expiry: t,
            dividend_yield: q,
            option_type: OptionType::Put,
        },
        n,
    );
    assert!(
        (seasoned_put - kv_put).abs() < 1e-9,
        "unseasoned seasoned-control put {seasoned_put} must equal \
         Kemna-Vorst {kv_put}"
    );
}

/// W-07: with past fixings folded in, the seasoned geometric control must
/// stay finite, non-negative, and respond correctly to the realized
/// geometric average — high past fixings lift a call's value.
#[test]
fn w07_seasoned_geometric_control_uses_history() {
    let spot = 100.0;
    let strike = 100.0;
    let r = 0.05;
    let q = 0.0;
    let sigma = 0.20;
    let df = (-r * 0.5_f64).exp();
    // Three future fixings in the back half of the year.
    let future_times = vec![0.6, 0.8, 1.0];

    // hist_prod_log = Σ ln(S_past) for two past fixings.
    let low_hist = 2.0 * 90.0_f64.ln();
    let high_hist = 2.0 * 130.0_f64.ln();

    let call_low = seasoned_geometric_asian_control(
        &OptionMarketParams {
            spot,
            strike,
            rate: r,
            volatility: sigma,
            time_to_expiry: 1.0,
            dividend_yield: q,
            option_type: OptionType::Call,
        },
        df,
        low_hist,
        2,
        &future_times,
        None,
        None,
    )
    .expect("finite seasoned call");
    let call_high = seasoned_geometric_asian_control(
        &OptionMarketParams {
            spot,
            strike,
            rate: r,
            volatility: sigma,
            time_to_expiry: 1.0,
            dividend_yield: q,
            option_type: OptionType::Call,
        },
        df,
        high_hist,
        2,
        &future_times,
        None,
        None,
    )
    .expect("finite seasoned call");

    assert!(call_low.is_finite() && call_low >= 0.0);
    assert!(call_high.is_finite() && call_high >= 0.0);
    assert!(
        call_high > call_low,
        "a seasoned geometric Asian call with higher realized past fixings \
         ({call_high}) must be worth more than one with lower past fixings \
         ({call_low})"
    );
}

#[test]
fn coincident_model_times_preserve_all_asian_fixings() {
    let as_of = date(2025, 1, 1);
    let expiry = date(2025, 1, 31);
    let curves = market(as_of, 100.0, 0.0, 0.0, 0.0);
    for averaging in [AveragingMethod::Arithmetic, AveragingMethod::Geometric] {
        for option_type in [OptionType::Call, OptionType::Put] {
            let strike = if option_type == OptionType::Call {
                95.0
            } else {
                105.0
            };
            let mut option = asian_option(
                averaging,
                option_type,
                expiry,
                strike,
                vec![as_of, date(2025, 1, 30), expiry],
            );
            option.day_count = DayCount::ThirtyE360;
            option.past_fixings = vec![(as_of, 90.0)];
            option.instrument_pricing_overrides.model_config.mc_paths = Some(16);
            let average = match averaging {
                AveragingMethod::Arithmetic => (90.0 + 100.0 + 100.0) / 3.0,
                AveragingMethod::Geometric => ((90.0_f64.ln() + 2.0 * 100.0_f64.ln()) / 3.0).exp(),
            };
            let expected = if option_type == OptionType::Call {
                average - strike
            } else {
                strike - average
            };
            let actual = crate::EXOTICS
                .pricers()
                .price_with_metrics(
                    &option,
                    ModelKey::MonteCarloGBM,
                    &curves,
                    as_of,
                    &[],
                    finstack_quant_valuations_core::instruments::PricingOptions::default(),
                )
                .expect("MC price")
                .value
                .amount();
            assert!(
                (actual - expected).abs() < 1e-10,
                "{averaging:?} {option_type:?}: {actual} vs {expected}"
            );
        }
    }
}

#[test]
fn asian_grid_contains_exact_close_fixings_and_equal_time_weights() {
    let as_of = date(2025, 1, 1);
    let dates = [date(2025, 12, 30), date(2025, 12, 31)];
    let grid = map_fixings_to_steps(&dates, DayCount::Act365F, as_of, 1.0, 2).expect("grid");
    for (&fixing, &time) in dates.iter().zip(grid.future_times().iter()) {
        let expected = DayCount::Act365F
            .year_fraction(as_of, fixing, DayCountContext::default())
            .expect("time");
        assert!((time - expected).abs() < 1e-14);
    }
    let expiry = date(2025, 1, 31);
    let t = DayCount::ThirtyE360
        .year_fraction(as_of, expiry, DayCountContext::default())
        .expect("time");
    let grid = map_fixings_to_steps(
        &[date(2025, 1, 30), expiry],
        DayCount::ThirtyE360,
        as_of,
        t,
        2,
    )
    .expect("grid");
    assert_eq!(grid.fixing_steps.len(), 2);
    assert_eq!(grid.fixing_steps[0], grid.fixing_steps[1]);
}

#[test]
fn geometric_asian_uses_dated_forward_and_separate_payment_discount() {
    let as_of = date(2025, 1, 1);
    let fixing = date(2025, 7, 2);
    let expiry = date(2026, 1, 1);
    let mut option = asian_option(
        AveragingMethod::Geometric,
        OptionType::Call,
        expiry,
        95.0,
        vec![fixing],
    );
    option.instrument_pricing_overrides.model_config.mc_paths = Some(16);
    for terminal_df in [0.9, 0.8] {
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (0.5, 1.0), (1.0, terminal_df), (2.0, 0.7)])
            .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("curve");
        let curves = market(as_of, 100.0, 0.0, 0.0, 0.0)
            .try_insert(curve)
            .expect("insert market curve");
        let expected = 5.0 * terminal_df;
        let actual = AsianOptionAnalyticalGeometricPricer
            .price_dyn(&option, &curves, as_of)
            .expect("analytical")
            .value
            .amount();
        let mc = crate::EXOTICS
            .pricers()
            .price_with_metrics(
                &option,
                ModelKey::MonteCarloGBM,
                &curves,
                as_of,
                &[],
                finstack_quant_valuations_core::instruments::PricingOptions::default(),
            )
            .expect("MC")
            .value
            .amount();
        assert!((actual - expected).abs() < 1e-10, "{actual} vs {expected}");
        assert!((mc - expected).abs() < 1e-10, "{mc} vs {expected}");
    }
}

#[test]
fn coincident_asian_times_retain_each_dated_forward() {
    let as_of = date(2025, 1, 1);
    let expiry = date(2025, 1, 31);
    let earlier = date(2025, 1, 30);
    let curves = market(as_of, 100.0, 0.0, 0.05, 0.0);
    let discount = curves.get_discount("USD-OIS").expect("curve");
    let first = 100.0 / discount.df_between_dates(as_of, earlier).expect("first DF");
    let second = 100.0 / discount.df_between_dates(as_of, expiry).expect("second DF");
    let payment_df = discount
        .df_between_dates(as_of, expiry)
        .expect("payment DF");
    assert_ne!(first, second);
    for averaging in [AveragingMethod::Arithmetic, AveragingMethod::Geometric] {
        let mut option = asian_option(
            averaging,
            OptionType::Call,
            expiry,
            95.0,
            vec![as_of, earlier, expiry],
        );
        option.day_count = DayCount::ThirtyE360;
        option.past_fixings = vec![(as_of, 90.0)];
        option.instrument_pricing_overrides.model_config.mc_paths = Some(16);
        let average = match averaging {
            AveragingMethod::Arithmetic => (90.0 + first + second) / 3.0,
            AveragingMethod::Geometric => ((90.0_f64.ln() + first.ln() + second.ln()) / 3.0).exp(),
        };
        let expected = (average - 95.0) * payment_df;
        let actual = crate::EXOTICS
            .pricers()
            .price_with_metrics(
                &option,
                ModelKey::MonteCarloGBM,
                &curves,
                as_of,
                &[],
                finstack_quant_valuations_core::instruments::PricingOptions::default(),
            )
            .expect("MC")
            .value
            .amount();
        assert!(
            (actual - expected).abs() < 1e-10,
            "{averaging:?}: {actual} vs {expected}"
        );
    }
}

#[test]
fn heston_asian_preserves_coincident_dates_and_dated_forwards() {
    let as_of = date(2025, 1, 1);
    let expiry = date(2025, 1, 31);
    let earlier = date(2025, 1, 30);
    let curves = market(as_of, 100.0, 0.0, 0.05, 0.0)
        .insert_price(
            "HESTON_KAPPA",
            MarketScalar::unitless(2.0).expect("valid scalar"),
        )
        .insert_price(
            "HESTON_THETA",
            MarketScalar::unitless(1e-20).expect("valid scalar"),
        )
        .insert_price(
            "HESTON_SIGMA_V",
            MarketScalar::unitless(1e-12).expect("valid scalar"),
        )
        .insert_price(
            "HESTON_RHO",
            MarketScalar::unitless(0.0).expect("valid scalar"),
        )
        .insert_price(
            "HESTON_V0",
            MarketScalar::unitless(1e-20).expect("valid scalar"),
        );
    let discount = curves.get_discount("USD-OIS").expect("curve");
    let first = 100.0 / discount.df_between_dates(as_of, earlier).expect("first DF");
    let second = 100.0 / discount.df_between_dates(as_of, expiry).expect("second DF");
    let payment_df = discount
        .df_between_dates(as_of, expiry)
        .expect("payment DF");
    let pricer = crate::exotics::asian_option::heston_mc_pricer::AsianOptionHestonMcPricer::new();
    for averaging in [AveragingMethod::Arithmetic, AveragingMethod::Geometric] {
        let mut option = asian_option(
            averaging,
            OptionType::Call,
            expiry,
            95.0,
            vec![as_of, earlier, expiry],
        );
        option.day_count = DayCount::ThirtyE360;
        option.past_fixings = vec![(as_of, 90.0)];
        option.instrument_pricing_overrides.model_config.mc_paths = Some(16);
        let average = match averaging {
            AveragingMethod::Arithmetic => (90.0 + first + second) / 3.0,
            AveragingMethod::Geometric => ((90.0_f64.ln() + first.ln() + second.ln()) / 3.0).exp(),
        };
        let expected = (average - 95.0) * payment_df;
        let actual = pricer
            .price_dyn(&option, &curves, as_of)
            .expect("Heston MC")
            .value
            .amount();
        assert!(
            (actual - expected).abs() < 1e-6,
            "{averaging:?}: {actual} vs {expected}"
        );
    }
}

#[test]
fn positive_vol_coincident_fixing_control_matches_log_moment_oracle_and_mc() {
    let as_of = date(2025, 1, 1);
    let expiry = date(2025, 1, 31);
    let first = date(2025, 1, 30);
    let sigma = 0.35;
    let mut option = asian_option(
        AveragingMethod::Geometric,
        OptionType::Call,
        expiry,
        95.0,
        vec![as_of, first, expiry],
    );
    option.day_count = DayCount::ThirtyE360;
    option.past_fixings = vec![(as_of, 90.0)];
    let curves = market(as_of, 100.0, sigma, 0.04, 0.0);
    let discount = curves.get_discount("USD-OIS").expect("curve");
    let df = discount
        .df_between_dates(as_of, expiry)
        .expect("payment DF");
    let f1 = 100.0 / discount.df_between_dates(as_of, first).expect("first DF");
    let f2 = 100.0 / df;
    let t = 29.0 / 360.0;
    // Both future fixings share the same Brownian increment: their sum
    // has variance 4*sigma^2*T, rather than 2*sigma^2*T.
    let variance = 4.0 * sigma * sigma * t / 9.0;
    let mean_log = (90.0_f64.ln() + f1.ln() + f2.ln()) / 3.0 - sigma * sigma * t / 3.0;
    let expected_g = (mean_log + 0.5 * variance).exp();
    let d2 = (mean_log - option.strike.ln()) / variance.sqrt();
    let expected = df
        * (expected_g
            * finstack_quant_core::math::special_functions::norm_cdf(d2 + variance.sqrt())
                .expect("valid normal variate")
            - option.strike
                * finstack_quant_core::math::special_functions::norm_cdf(d2)
                    .expect("valid normal variate"));
    let grid =
        map_fixings_to_steps(&option.fixing_dates, option.day_count, as_of, t, 2).expect("grid");
    let times = grid.future_times();
    let (drift, multipliers) =
        asian_drift_and_fixing_multipliers(&option, &curves, as_of, 0.0).expect("carry");
    let control = seasoned_geometric_asian_control(
        &OptionMarketParams {
            spot: 100.0,
            strike: option.strike,
            rate: -df.ln() / t,
            volatility: sigma,
            time_to_expiry: t,
            dividend_yield: 0.0,
            option_type: OptionType::Call,
        },
        df,
        90.0_f64.ln(),
        1,
        &times,
        Some(&drift),
        Some(&multipliers),
    )
    .expect("finite seasoned control");
    assert!(
        (control - expected).abs() < 1e-12,
        "control={control}, oracle={expected}"
    );
    let process = GbmProcess::new(GbmParams::new(-df.ln() / t, 0.0, sigma).expect("GBM"))
        .with_drift_schedule(std::sync::Arc::new(drift));
    let payoff = Asian::with_history(
        finstack_quant_models::types::OptionType::Call,
        option.strike,
        1.0,
        AveragingMethod::Geometric,
        grid.fixing_steps,
        finstack_quant_models::monte_carlo::payoff::asian::AsianFixingHistory {
            sum: 90.0,
            log_sum: 90.0_f64.ln(),
            count: 1,
        },
    )
    .expect("payoff")
    .with_fixing_multipliers(&multipliers)
    .expect("dated scaling");
    let config = PathDependentPricerConfig {
        num_paths: 40_000,
        seed: 20260930,
        use_parallel: false,
        use_sobol: false,
        antithetic: false,
        ..Default::default()
    };
    let result = PathDependentPricer::new(config)
        .price(&process, 100.0, grid.time_grid, &payoff, Currency::USD, df)
        .expect("MC");
    assert!(
        (result.mean.amount() - expected).abs() < 5.0 * result.stderr,
        "MC={}, oracle={expected}, sampling stderr={}",
        result.mean.amount(),
        result.stderr
    );
}

#[test]
fn nonflat_multifixing_geometric_asian_matches_joint_normal_moment_oracle() {
    let as_of = date(2025, 1, 1);
    let expiry = date(2026, 1, 1);
    let t1 = 91.0 / 365.0;
    let t2 = 273.0 / 365.0;
    let (sigma, q) = (0.30, 0.01);
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (t1, 0.99), (t2, 0.90), (1.0, 0.85)])
        .interp(InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("curve");
    let curves = market(as_of, 100.0, sigma, 0.0, q)
        .try_insert(curve)
        .expect("insert market curve");
    // For two distinct observations, Cov(log S1, log S2)=sigma^2*t1.
    let f1 = 100.0 * (-q * t1).exp() / 0.99;
    let f2 = 100.0 * (-q * t2).exp() / 0.90;
    let mean_log = 0.5 * (f1.ln() + f2.ln()) - 0.25 * sigma * sigma * (t1 + t2);
    let variance = 0.25 * sigma * sigma * (3.0 * t1 + t2);
    let expected_g = (mean_log + 0.5 * variance).exp();
    let d2 = (mean_log - 100.0_f64.ln()) / variance.sqrt();
    let d1 = d2 + variance.sqrt();
    for option_type in [OptionType::Call, OptionType::Put] {
        let option = asian_option(
            AveragingMethod::Geometric,
            option_type,
            expiry,
            100.0,
            vec![date(2025, 4, 2), date(2025, 10, 1)],
        );
        let expected = 0.85
            * match option_type {
                OptionType::Call => {
                    expected_g
                        * finstack_quant_core::math::special_functions::norm_cdf(d1)
                            .expect("valid normal variate")
                        - 100.0
                            * finstack_quant_core::math::special_functions::norm_cdf(d2)
                                .expect("valid normal variate")
                }
                OptionType::Put => {
                    100.0
                        * finstack_quant_core::math::special_functions::norm_cdf(-d2)
                            .expect("valid normal variate")
                        - expected_g
                            * finstack_quant_core::math::special_functions::norm_cdf(-d1)
                                .expect("valid normal variate")
                }
            };
        let actual = AsianOptionAnalyticalGeometricPricer
            .price_dyn(&option, &curves, as_of)
            .expect("analytic")
            .value
            .amount();
        assert!(
            (actual - expected).abs() < 1e-11,
            "{option_type:?}: {actual} vs {expected}"
        );
    }
}

#[test]
fn missing_discount_curve_surfaces_as_not_found_for_both_averaging_methods() {
    let as_of = date(2025, 1, 2);
    let curves = market(as_of, 100.0, 0.2, 0.03, 0.0);
    for averaging in [AveragingMethod::Geometric, AveragingMethod::Arithmetic] {
        let mut option = asian_option(
            averaging,
            OptionType::Call,
            date(2026, 1, 2),
            100.0,
            vec![date(2025, 7, 1), date(2026, 1, 2)],
        );
        option.discount_curve_id = CurveId::new("MISSING-OIS");
        let err = option.value(&curves, as_of).expect_err("missing curve");
        assert!(
            matches!(
                err,
                finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound { .. })
            ),
            "{averaging:?}: missing discount curve must stay NotFound, got {err:?}"
        );
    }
}

fn quarterly_asian(averaging: AveragingMethod) -> AsianOption {
    let expiry = date(2026, 1, 2);
    asian_option(
        averaging,
        OptionType::Call,
        expiry,
        100.0,
        vec![
            date(2025, 4, 2),
            date(2025, 7, 2),
            date(2025, 10, 2),
            expiry,
        ],
    )
}

#[test]
fn spot_price_in_another_currency_is_rejected() {
    let as_of = date(2025, 1, 2);
    let eur_spot = market(as_of, 100.0, 0.20, 0.05, 0.0).insert_price(
        "SPX-SPOT",
        MarketScalar::price(Money::new(100.0, Currency::EUR).expect("money")),
    );
    let arithmetic = quarterly_asian(AveragingMethod::Arithmetic);
    let geometric = quarterly_asian(AveragingMethod::Geometric);

    let mismatch = |err: finstack_quant_core::Error| {
        assert!(
            matches!(
                err,
                finstack_quant_core::Error::CurrencyMismatch {
                    expected: Currency::USD,
                    actual: Currency::EUR,
                }
            ),
            "{err:?}"
        );
    };
    mismatch(
        AsianOptionMcPricer::new()
            .price_internal(&arithmetic, &eur_spot, as_of)
            .expect_err("MC spot"),
    );
    mismatch(
        arithmetic
            .theta_observed_state(&eur_spot, as_of, date(2025, 4, 3))
            .err()
            .expect("theta spot"),
    );
    let registry_errors = [
        AsianOptionAnalyticalGeometricPricer
            .price_dyn(&geometric, &eur_spot, as_of)
            .expect_err("geometric spot")
            .to_string(),
        AsianOptionSemiAnalyticalTwPricer
            .price_dyn(&arithmetic, &eur_spot, as_of)
            .expect_err("TW spot")
            .to_string(),
        super::super::heston_mc_pricer::AsianOptionHestonMcPricer::new()
            .price_dyn(&arithmetic, &eur_spot, as_of)
            .expect_err("Heston spot")
            .to_string(),
        arithmetic
            .value(&eur_spot, as_of)
            .expect_err("default model spot")
            .to_string(),
    ];
    for err in registry_errors {
        assert!(err.contains("Currency mismatch"), "{err}");
    }
}

#[test]
fn analytic_pricers_honour_the_implied_volatility_override() {
    let as_of = date(2025, 1, 2);
    let surface_20 = market(as_of, 100.0, 0.20, 0.05, 0.01);
    let surface_40 = market(as_of, 100.0, 0.40, 0.05, 0.01);
    let geometric_pricer = AsianOptionAnalyticalGeometricPricer;
    let tw_pricer = AsianOptionSemiAnalyticalTwPricer;
    let price = |pricer: &dyn Pricer, option: &AsianOption, curves: &MarketContext| {
        pricer
            .price_dyn(option, curves, as_of)
            .expect("price")
            .value
            .amount()
    };

    for (pricer, averaging) in [
        (&geometric_pricer as &dyn Pricer, AveragingMethod::Geometric),
        (&tw_pricer as &dyn Pricer, AveragingMethod::Arithmetic),
    ] {
        let mut option = quarterly_asian(averaging);
        let expected = price(pricer, &option, &surface_40);
        option
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.40);
        let actual = price(pricer, &option, &surface_20);
        assert!(
            (actual - expected).abs() < 1e-12,
            "{averaging:?}: override {actual} vs 40% surface {expected}"
        );
    }
}
