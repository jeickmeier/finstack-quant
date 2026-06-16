//! Tests for swaption core types and schedule utilities.

#![allow(clippy::unwrap_used)]

use crate::swaption::common::*;
use finstack_quant_core::dates::DayCountContext;
use finstack_quant_core::dates::{Tenor, TenorUnit};
use finstack_quant_valuations::instruments::pricing_overrides::VolSurfaceExtrapolation;
use finstack_quant_valuations::instruments::rates::swaption::SABRParameters;
use finstack_quant_valuations::instruments::rates::swaption::{
    BermudanSchedule, BermudanSwaption, CashSettlementMethod, Swaption, SwaptionExercise,
    SwaptionSettlement, VolatilityModel,
};
use time::macros::date;

#[test]
fn test_swaption_settlement_and_exercise_from_str() {
    let physical: SwaptionSettlement = "physical".parse().unwrap();
    let cash: SwaptionSettlement = "CASH".parse().unwrap();
    assert_eq!(physical.to_string(), "physical");
    assert_eq!(cash.to_string(), "cash");

    let european: SwaptionExercise = "european".parse().unwrap();
    let bermudan: SwaptionExercise = "BERMUDAN".parse().unwrap();
    let american: SwaptionExercise = "american".parse().unwrap();
    assert_eq!(european.to_string(), "european");
    assert_eq!(bermudan.to_string(), "bermudan");
    assert_eq!(american.to_string(), "american");

    assert!("unknown".parse::<SwaptionSettlement>().is_err());
    assert!("invalid".parse::<SwaptionExercise>().is_err());
}

#[test]
fn test_vol_model_and_cash_settlement_method_aliases() {
    assert_eq!(
        "black".parse::<VolatilityModel>().unwrap(),
        VolatilityModel::Black
    );
    assert_eq!(
        "black76".parse::<VolatilityModel>().unwrap(),
        VolatilityModel::Black
    );
    assert_eq!(
        "bachelier".parse::<VolatilityModel>().unwrap(),
        VolatilityModel::Normal
    );
    assert!("mystery".parse::<VolatilityModel>().is_err());

    assert_eq!(
        CashSettlementMethod::default(),
        CashSettlementMethod::IsdaParPar
    );
    assert_eq!(CashSettlementMethod::ParYield.to_string(), "par_yield");
    assert_eq!(
        "isda_par_par".parse::<CashSettlementMethod>().unwrap(),
        CashSettlementMethod::IsdaParPar
    );
    assert_eq!(
        "zero_coupon".parse::<CashSettlementMethod>().unwrap(),
        CashSettlementMethod::ZeroCoupon
    );
    assert!("not_real".parse::<CashSettlementMethod>().is_err());
}

#[test]
fn test_bermudan_schedule_sort_and_lockout() {
    let d1 = date!(2026 - 06 - 01);
    let d2 = date!(2026 - 01 - 01);
    let d3 = date!(2026 - 12 - 01);
    let schedule = BermudanSchedule::new(vec![d1, d2, d3])
        .with_lockout(date!(2026 - 03 - 01))
        .with_notice_days(2);

    assert_eq!(schedule.exercise_dates, vec![d2, d1, d3]);
    assert_eq!(schedule.notice_days, 2);

    let effective = schedule.effective_dates();
    assert_eq!(effective, vec![d1, d3]);
    assert_eq!(schedule.num_exercises(), 2);
}

#[test]
fn test_bermudan_schedule_co_terminal_excludes_maturity() {
    let first_ex = date!(2026 - 01 - 01);
    let swap_end = date!(2027 - 01 - 01);
    let schedule = BermudanSchedule::co_terminal(first_ex, swap_end, Tenor::semi_annual())
        .expect("valid Bermudan schedule");

    assert!(!schedule.exercise_dates.is_empty());
    assert!(schedule.exercise_dates.first().unwrap() >= &first_ex);
    assert!(schedule.exercise_dates.last().unwrap() < &swap_end);
}

#[test]
fn test_swaption_cash_annuity_zero_forward_and_invalid_freq() {
    let (_, expiry, swap_start, swap_end) = standard_dates();
    let mut swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    swaption.settlement = SwaptionSettlement::Cash;

    let expected = swaption
        .day_count
        .year_fraction(
            swaption.swap_start,
            swaption.swap_end,
            DayCountContext::default(),
        )
        .unwrap();
    let annuity = swaption.cash_annuity_par_yield(0.0).unwrap();
    assert_approx_eq(annuity, expected, 1e-8, "cash annuity zero rate");

    swaption.fixed_freq = Tenor::new(0, TenorUnit::Months);
    assert!(swaption.cash_annuity_par_yield(0.02).is_err());
}

#[test]
fn test_resolve_volatility_priority_and_greek_inputs_expired() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let mut swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    let market = create_flat_market(as_of, 0.03, 0.2);
    let forward = swaption.forward_swap_rate(&market, as_of).unwrap();
    let t = swaption
        .day_count
        .year_fraction(as_of, swaption.expiry, DayCountContext::default())
        .unwrap();

    let surface_vol = swaption.resolve_volatility(&market, forward, t).unwrap();
    assert_approx_eq(surface_vol, 0.2, 1e-12, "surface vol");

    swaption.pricing_overrides = swaption
        .pricing_overrides
        .clone()
        .with_implied_vol(0.35)
        .with_vol_surface_extrapolation(VolSurfaceExtrapolation::Clamp);
    let override_vol = swaption.resolve_volatility(&market, forward, t).unwrap();
    assert_approx_eq(override_vol, 0.35, 1e-12, "override vol");

    let sabr_params = SABRParameters::rates_standard(0.2, 0.5, -0.25).unwrap();
    swaption = swaption.with_sabr(sabr_params);
    let sabr_vol = swaption.resolve_volatility(&market, forward, t).unwrap();
    assert!(sabr_vol.is_finite(), "sabr vol should be finite");
    assert!(
        (sabr_vol - surface_vol).abs() > 1e-6,
        "SABR vol should override surface vol"
    );

    let expired = create_standard_payer_swaption(
        date!(2023 - 01 - 01),
        date!(2023 - 01 - 01),
        date!(2028 - 01 - 01),
        0.05,
    );
    let none_inputs = expired.greek_inputs(&market, as_of).unwrap();
    assert!(none_inputs.is_none());
}

#[test]
fn test_sabr_parameter_constructors_and_internal_conversion() {
    let plain = SABRParameters::new(0.2, 0.7, 0.4, -0.2).unwrap();
    assert_eq!(plain.shift, None);

    let shifted = SABRParameters::new_with_shift(0.2, 0.5, 0.4, -0.2, 0.01).unwrap();
    assert_eq!(shifted.shift, Some(0.01));

    let eq = SABRParameters::equity_standard(0.2, 0.5, -0.3).unwrap();
    let rates = SABRParameters::rates_standard(0.2, 0.5, -0.3).unwrap();
    let normal = SABRParameters::normal(0.2, 0.5, -0.3).unwrap();
    let lognormal = SABRParameters::lognormal(0.2, 0.5, -0.3).unwrap();
    assert_eq!(eq.beta, 1.0);
    assert_eq!(rates.beta, 0.5);
    assert_eq!(normal.beta, 0.0);
    assert_eq!(lognormal.beta, 1.0);
}

#[test]
fn test_swaption_example_and_builder_helpers() {
    let example = Swaption::example();
    assert_eq!(example.exercise_style, SwaptionExercise::European);
    assert_eq!(example.settlement, SwaptionSettlement::Cash);
    assert_eq!(
        example.cash_settlement_method,
        CashSettlementMethod::IsdaParPar
    );

    let updated = example
        .with_settlement(SwaptionSettlement::Cash)
        .with_option_type(finstack_quant_valuations::instruments::OptionType::Put)
        .with_cash_settlement_method(CashSettlementMethod::ZeroCoupon)
        .with_calendar("nyse");
    assert_eq!(updated.settlement, SwaptionSettlement::Cash);
    assert_eq!(
        updated.option_type,
        finstack_quant_valuations::instruments::OptionType::Put
    );
    assert_eq!(
        updated.cash_settlement_method,
        CashSettlementMethod::ZeroCoupon
    );
    assert_eq!(
        updated.calendar_id.as_ref().map(|id| id.as_str()),
        Some("nyse")
    );
}

#[test]
fn test_bermudan_swaption_schedule_and_conversion() {
    let as_of = date!(2024 - 01 - 01);
    let swap_start = date!(2025 - 01 - 01);
    let swap_end = date!(2030 - 01 - 01);
    let first_ex = date!(2026 - 01 - 01);

    let swaption = BermudanSwaption::new_payer(
        "BERM-TEST",
        finstack_quant_core::money::Money::new(
            1_000_000.0,
            finstack_quant_core::currency::Currency::USD,
        ),
        0.03,
        swap_start,
        swap_end,
        BermudanSchedule::co_terminal(first_ex, swap_end, Tenor::semi_annual())
            .expect("valid Bermudan schedule"),
        "USD-OIS",
        "USD-SOFR-3M",
        "USD-SWPNVOL",
    )
    .expect("valid literal strike");

    let (payment_dates, accruals) = swaption.build_swap_schedule(as_of).unwrap();
    assert_eq!(payment_dates.len(), accruals.len());
    assert!(!payment_dates.is_empty());

    let payment_times = swaption.payment_times(as_of).unwrap();
    assert_eq!(payment_times.len(), payment_dates.len());

    let euro = swaption.to_european().unwrap();
    assert_eq!(euro.expiry, first_ex);
    assert_eq!(euro.swap_start, first_ex);
    assert_eq!(euro.swap_end, swap_end);
    assert_eq!(euro.vol_model, VolatilityModel::Black);

    let market = create_flat_market(as_of, 0.03, 0.2);
    let disc = market.get_discount("USD_OIS").unwrap();
    let annuity_early = swaption
        .remaining_annuity(disc.as_ref(), as_of, first_ex)
        .unwrap();
    let later_ex = date!(2027 - 01 - 01);
    let annuity_late = swaption
        .remaining_annuity(disc.as_ref(), as_of, later_ex)
        .unwrap();
    assert!(annuity_early > annuity_late);
}

#[test]
fn test_bermudan_builder_helpers_and_time_accessors() {
    let as_of = date!(2024 - 01 - 01);
    let swap_start = date!(2025 - 01 - 01);
    let swap_end = date!(2030 - 01 - 01);
    let first_ex = date!(2026 - 01 - 01);
    let schedule = BermudanSchedule::co_terminal(first_ex, swap_end, Tenor::semi_annual()).unwrap();

    let berm = BermudanSwaption::new_receiver(
        "BERM-BUILDER",
        finstack_quant_core::money::Money::new(
            2_000_000.0,
            finstack_quant_core::currency::Currency::USD,
        ),
        0.031,
        swap_start,
        swap_end,
        schedule.clone(),
        "USD-OIS",
        "USD-SOFR-3M",
        "USD-SWPNVOL",
    )
    .unwrap()
    .with_fixed_freq(Tenor::annual())
    .with_float_freq(Tenor::semi_annual())
    .with_day_count(finstack_quant_core::dates::DayCount::Act365F)
    .with_settlement(SwaptionSettlement::Cash)
    .with_calendar("nyse");

    assert_eq!(
        berm.first_exercise(),
        schedule.exercise_dates.first().copied()
    );
    assert_eq!(
        berm.last_exercise(),
        schedule.exercise_dates.last().copied()
    );
    assert!(berm.time_to_first_exercise(as_of).unwrap() > 0.0);
    assert!(berm.time_to_maturity(as_of).unwrap() > berm.time_to_first_exercise(as_of).unwrap());
    assert_eq!(berm.settlement, SwaptionSettlement::Cash);
    assert_eq!(berm.fixed_freq, Tenor::annual());
    assert_eq!(berm.float_freq, Tenor::semi_annual());
}
