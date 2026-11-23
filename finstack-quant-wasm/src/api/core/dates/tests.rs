use super::calendar::*;
use super::context::*;
use super::day_count::*;
use super::tenor::*;
use finstack_quant_core::dates::{
    DayCount as RustDayCount, DayCountContextState, Tenor as RustTenor,
};

// -- JsDayCount -----------------------------------------------------------

#[test]
fn daycount_constructors() {
    let day_count = JsDayCount::act360();
    assert_eq!(day_count.to_string(), "act_360");
    let day_count = JsDayCount::act365f();
    assert_eq!(day_count.to_string(), "act_365f");
    let day_count = JsDayCount::thirty360();
    assert_eq!(day_count.to_string(), "30_360");
    let day_count = JsDayCount::thirty_e360();
    assert_eq!(day_count.to_string(), "30e_360");
    let day_count = JsDayCount::thirty_e360_isda();
    assert_eq!(day_count.to_string(), "30e_360_isda");
    let day_count = JsDayCount::act_act();
    assert_eq!(day_count.to_string(), "act_act");
    let day_count = JsDayCount::act_act_isma();
    assert_eq!(day_count.to_string(), "act_act_isma");
    let day_count = JsDayCount::act_act_afb();
    assert_eq!(day_count.to_string(), "act_act_afb");
    let day_count = JsDayCount::thirty360_it();
    assert_eq!(day_count.to_string(), "30_360_it");
    let day_count = JsDayCount::bus252();
    assert_eq!(day_count.to_string(), "bus_252");
}

// -- JsTenor --------------------------------------------------------------

#[test]
fn tenor_factories() {
    assert_eq!(JsTenor::daily().count(), 1);
    assert_eq!(JsTenor::weekly().count(), 1);
    assert_eq!(JsTenor::monthly().count(), 1);
    assert_eq!(JsTenor::quarterly().count(), 3);
    assert_eq!(JsTenor::semi_annual().count(), 6);
    assert_eq!(JsTenor::annual().count(), 1);
}

#[test]
fn tenor_to_string() {
    let t = JsTenor::quarterly();
    let s = t.to_string();
    assert!(s.contains('M') || s.contains('Q'), "got: {s}");
}

// -- Free functions -----------------------------------------------------

#[test]
fn available_calendars_not_empty() {
    let cals = available_calendars();
    assert!(!cals.is_empty());
}

#[test]
fn tenor_weekly_to_string() {
    let t = JsTenor::weekly();
    let s = t.to_string();
    assert!(!s.is_empty());
}

#[test]
fn tenor_semi_annual_years() {
    let t = JsTenor::semi_annual();
    assert!((t.to_years_approx() - 0.5).abs() < 0.01);
}

#[test]
fn tenor_annual_years() {
    let t = JsTenor::annual();
    assert!((t.to_years_approx() - 1.0).abs() < 0.01);
}

#[test]
fn tenor_daily_years() {
    let t = JsTenor::daily();
    assert!(t.to_years_approx() < 0.01);
}

// -- Boundary tests ------------------------------------------------
// Error paths through wasm-bindgen create JsValue, which panics on
// native targets.  Test the underlying Rust types instead.

#[test]
fn create_date_invalid_month() {
    assert!(time::Month::try_from(13_u8).is_err());
    assert!(time::Month::try_from(0_u8).is_err());
}

#[test]
fn create_date_invalid_day() {
    assert!(finstack_quant_core::dates::create_date(2024, time::Month::February, 30).is_err());
}

#[test]
fn date_from_epoch_days_extreme() {
    assert!(finstack_quant_core::dates::date_from_epoch_days(i32::MAX).is_err());
    assert!(finstack_quant_core::dates::date_from_epoch_days(i32::MIN).is_err());
}

#[test]
fn daycount_nl365_factory_and_canonical_parse() {
    assert_eq!(JsDayCount::nl365().to_string(), "nl_365");
    assert_eq!(
        " Act_Act_Isma "
            .parse::<RustDayCount>()
            .expect("normalized"),
        RustDayCount::ActActIsma
    );
    assert_eq!(
        " ACT_360 ".parse::<RustDayCount>().expect("normalized"),
        RustDayCount::Act360
    );
    assert!("act360".parse::<RustDayCount>().is_err());
}

#[test]
fn tenor_members_pass_through_rust() {
    let t = JsTenor {
        inner: RustTenor::parse("3M").expect("3M"),
    };
    assert_eq!(t.unit(), "M");
    assert_eq!(t.months(), Some(3));
    assert_eq!(t.days(), None);
    assert!((t.payments_per_year().expect("monthly ratio") - 4.0).abs() < 1e-12);
    assert_eq!(t.to_days_approx(), t.inner.to_days_approx() as f64);
    assert_eq!(JsTenor::biweekly().days(), Some(14));
    assert_eq!(JsTenor::bimonthly().months(), Some(2));
}

#[test]
fn day_count_context_getters_read_the_rust_state() {
    let start =
        finstack_quant_core::dates::create_date(2025, time::Month::January, 15).expect("start");
    let end = finstack_quant_core::dates::create_date(2025, time::Month::July, 15).expect("end");
    let ctx = JsDayCountContext {
        inner: DayCountContextState::try_new(
            Some("nyse".into()),
            Some(RustTenor::semi_annual()),
            Some(252),
            Some((start, end)),
            Some(true),
        )
        .expect("state"),
    };
    assert_eq!(ctx.calendar_id().as_deref(), Some("nyse"));
    assert_eq!(
        ctx.frequency().map(|t| t.to_string()).as_deref(),
        Some("6M")
    );
    assert_eq!(ctx.bus_basis(), Some(252));
    assert_eq!(
        ctx.coupon_period().as_deref(),
        Some(
            &[
                finstack_quant_core::dates::days_since_epoch(start),
                finstack_quant_core::dates::days_since_epoch(end)
            ][..]
        )
    );
    assert_eq!(ctx.end_is_termination_date(), Some(true));
    let back: DayCountContextState =
        serde_json::from_str(&ctx.to_json().expect("json")).expect("parse");
    assert_eq!(back, ctx.inner);
}

#[test]
fn daycount_invalid_string() {
    assert!("not_a_daycount".parse::<RustDayCount>().is_err());
}

#[test]
fn tenor_invalid_string() {
    assert!(RustTenor::parse("").is_err());
    assert!(RustTenor::parse("XYZ").is_err());
}
