use super::*;
use crate::FloatLegSpec;
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations_core::instruments::{
    Instrument, NotionalExchange, PayReceive, ResettingSide,
};
use rust_decimal::Decimal;
use time::Month;

fn no_fallback_swap() -> XccySwap {
    let mut swap = XccySwap::example().expect("example");
    swap.allow_calendar_fallback = false;
    swap
}

fn date(year: i32, month: Month, day: u8) -> Date {
    Date::from_calendar_date(year, month, day).expect("valid test date")
}

#[test]
fn xccy_swap_cashflow_provider_emits_multi_currency_flows() {
    let as_of = date(2025, Month::January, 1);
    let market = MarketContext::new()
        .try_insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots(vec![(0.0, 1.0), (1.0, 0.95)])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("usd curve"),
        )
        .expect("insert market curve")
        .try_insert(
            DiscountCurve::builder("EUR-OIS")
                .base_date(as_of)
                .knots(vec![(0.0, 1.0), (1.0, 0.97)])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("eur curve"),
        )
        .expect("insert market curve")
        .try_insert(
            ForwardCurve::flat(
                "USD-SOFR-3M",
                as_of,
                0.04,
                finstack_quant_core::dates::DayCount::Act365F,
            )
            .expect("usd forward"),
        )
        .expect("insert market curve")
        .try_insert(
            ForwardCurve::flat(
                "EUR-EURIBOR-3M",
                as_of,
                0.03,
                finstack_quant_core::dates::DayCount::Act365F,
            )
            .expect("eur forward"),
        )
        .expect("insert market curve");

    let start = date(2025, Month::January, 2);
    let end = date(2026, Month::January, 2);
    let mut swap = XccySwap::new(
        "XCCY-CF",
        XccySwapLeg {
            notional: Money::from((1_000_000_i64, Currency::USD)),
            side: PayReceive::Receive,
            leg: FloatLegSpec {
                index_id: "USD-SOFR-3M".into(),
                forward_curve_id: CurveId::new("USD-SOFR-3M"),
                discount_curve_id: CurveId::new("USD-OIS"),
                start,
                end,
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                stub: StubKind::ShortFront,
                spread_bp: Decimal::ZERO,
                payment_lag_days: 0,
                calendar_id: None,
                reset_lag_days: 0,
                compounding: Default::default(),
                fixing_calendar_id: None,
                end_of_month: false,
            },
        },
        XccySwapLeg {
            notional: Money::from((900_000_i64, Currency::EUR)),
            side: PayReceive::Pay,
            leg: FloatLegSpec {
                index_id: "EUR-EURIBOR-3M".into(),
                forward_curve_id: CurveId::new("EUR-EURIBOR-3M"),
                discount_curve_id: CurveId::new("EUR-OIS"),
                start,
                end,
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                stub: StubKind::ShortFront,
                spread_bp: Decimal::ZERO,
                payment_lag_days: 0,
                calendar_id: None,
                reset_lag_days: 0,
                compounding: Default::default(),
                fixing_calendar_id: None,
                end_of_month: false,
            },
        },
        Currency::USD,
    );
    swap.allow_calendar_fallback = true;

    let flows = swap
        .dated_cashflows(&market, as_of)
        .expect("xccy contractual schedule should build");

    assert!(
        flows.len() >= 6,
        "xccy swap should emit principal and coupon flows"
    );
    assert!(
        flows
            .iter()
            .any(|(_, money)| money.currency() == Currency::USD)
    );
    assert!(
        flows
            .iter()
            .any(|(_, money)| money.currency() == Currency::EUR)
    );
}

#[test]
fn base_value_fails_loud_when_fx_matrix_is_missing() {
    // Reproduces the audit scenario: USD/EUR XCCY with EUR reporting,
    // market context has both curves but NO FxMatrix. Pre-flight
    // reachability must reject up front with a message naming the
    // instrument id and both leg currencies, NOT a generic NotFound
    // surfaced from inside the per-cashflow loop.
    let as_of = date(2025, Month::January, 1);
    let market = MarketContext::new()
        .try_insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots(vec![(0.0, 1.0), (1.0, 0.95)])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("usd curve"),
        )
        .expect("insert market curve")
        .try_insert(
            DiscountCurve::builder("EUR-OIS")
                .base_date(as_of)
                .knots(vec![(0.0, 1.0), (1.0, 0.97)])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("eur curve"),
        )
        .expect("insert market curve")
        .try_insert(
            ForwardCurve::flat(
                "USD-SOFR-3M",
                as_of,
                0.04,
                finstack_quant_core::dates::DayCount::Act365F,
            )
            .expect("usd forward"),
        )
        .expect("insert market curve")
        .try_insert(
            ForwardCurve::flat(
                "EUR-EURIBOR-3M",
                as_of,
                0.03,
                finstack_quant_core::dates::DayCount::Act365F,
            )
            .expect("eur forward"),
        )
        .expect("insert market curve");

    let start = date(2025, Month::January, 2);
    let end = date(2026, Month::January, 2);
    let mut swap = XccySwap::new(
        "XCCY-NOFX",
        XccySwapLeg {
            notional: Money::from((1_000_000_i64, Currency::USD)),
            side: PayReceive::Receive,
            leg: FloatLegSpec {
                index_id: "USD-SOFR-3M".into(),
                forward_curve_id: CurveId::new("USD-SOFR-3M"),
                discount_curve_id: CurveId::new("USD-OIS"),
                start,
                end,
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                stub: StubKind::ShortFront,
                spread_bp: Decimal::ZERO,
                payment_lag_days: 0,
                calendar_id: None,
                reset_lag_days: 0,
                compounding: Default::default(),
                fixing_calendar_id: None,
                end_of_month: false,
            },
        },
        XccySwapLeg {
            notional: Money::from((900_000_i64, Currency::EUR)),
            side: PayReceive::Pay,
            leg: FloatLegSpec {
                index_id: "EUR-EURIBOR-3M".into(),
                forward_curve_id: CurveId::new("EUR-EURIBOR-3M"),
                discount_curve_id: CurveId::new("EUR-OIS"),
                start,
                end,
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                stub: StubKind::ShortFront,
                spread_bp: Decimal::ZERO,
                payment_lag_days: 0,
                calendar_id: None,
                reset_lag_days: 0,
                compounding: Default::default(),
                fixing_calendar_id: None,
                end_of_month: false,
            },
        },
        Currency::EUR, // reporting != either leg directly
    );
    swap.allow_calendar_fallback = true;

    let err = swap
        .base_value(&market, as_of)
        .expect_err("missing FxMatrix must be rejected pre-flight");
    let msg = format!("{err}");
    assert!(
        msg.contains("XCCY-NOFX"),
        "error must name the instrument id, got: {msg}"
    );
    assert!(
        msg.contains("fx_matrix") || msg.contains("FX path"),
        "error must explain that FX is required, got: {msg}"
    );
}

#[test]
// schema-rejection-test: leg-level allow_calendar_fallback moved to XccySwap.
fn xccy_leg_rejects_leg_level_allow_calendar_fallback() {
    let swap = XccySwap::example().expect("example");
    let mut json = serde_json::to_value(&swap.leg1).expect("serialize leg");
    json["allow_calendar_fallback"] = serde_json::Value::Bool(true);
    let err = serde_json::from_value::<XccySwapLeg>(json)
        .expect_err("leg-level allow_calendar_fallback must be rejected");
    assert!(err.to_string().contains("allow_calendar_fallback"), "{err}");
}

#[test]
fn resetting_side_fromstr_display_roundtrip() {
    use std::str::FromStr;
    for side in [ResettingSide::Leg1, ResettingSide::Leg2] {
        let s = side.to_string();
        let parsed = ResettingSide::from_str(&s).expect("roundtrip parse");
        assert_eq!(side, parsed, "roundtrip failed for {s}");
    }
    for noncanonical in ["leg_1", "Leg1", " leg1"] {
        assert!(ResettingSide::from_str(noncanonical).is_err());
    }
    assert!(ResettingSide::from_str("garbage").is_err());
}

#[test]
fn notional_exchange_serde_mtm_resetting_roundtrip() {
    let original = NotionalExchange::MtmResetting {
        resetting_side: ResettingSide::Leg1,
    };
    let json = serde_json::to_string(&original).expect("serialise");
    assert_eq!(json, r#"{"mtm_resetting":{"resetting_side":"leg1"}}"#);
    let parsed: NotionalExchange = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(original, parsed);
}

#[test]
fn partition_legs_returns_constant_then_resetting() {
    let swap = XccySwap::example()
        .expect("example")
        .with_notional_exchange(NotionalExchange::MtmResetting {
            resetting_side: ResettingSide::Leg2, // EUR leg resets
        });
    let (constant, resetting) = swap
        .partition_legs(ResettingSide::Leg2)
        .expect("partition succeeds");
    assert_eq!(constant.notional.currency(), Currency::USD);
    assert_eq!(resetting.notional.currency(), Currency::EUR);

    // Symmetrically, when leg1 resets, leg2 (EUR) is constant.
    let (constant_l1, resetting_l1) = swap
        .partition_legs(ResettingSide::Leg1)
        .expect("partition succeeds");
    assert_eq!(constant_l1.notional.currency(), Currency::EUR);
    assert_eq!(resetting_l1.notional.currency(), Currency::USD);
}

#[test]
fn partition_legs_errors_when_legs_share_currency() {
    // Force both legs to USD to exercise the same-currency guard inside
    // `partition_legs`. `validate()` should also reject this shape, but this test
    // pins the guard at the helper level since `partition_legs` is the primary
    // contract used by Task 7's PV path.
    let mut swap = XccySwap::example().expect("example");
    swap.leg2.notional = finstack_quant_core::money::Money::from((1_i64, Currency::USD));

    let err = swap
        .partition_legs(ResettingSide::Leg2)
        .expect_err("same-currency legs must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("different currencies") && msg.contains("USD"),
        "expected currency-mismatch error mentioning USD; got: {msg}"
    );
}

#[test]
fn validate_rejects_mtm_reset_with_misaligned_leg_schedules() {
    use finstack_quant_core::money::Money;

    let start = Date::from_calendar_date(2025, time::Month::January, 2).expect("valid date");
    let end = Date::from_calendar_date(2030, time::Month::January, 2).expect("valid date");
    let start_off = Date::from_calendar_date(2025, time::Month::February, 3).expect("valid date");

    let leg1 = XccySwapLeg {
        notional: Money::from((9_200_000_i64, Currency::EUR)),
        side: PayReceive::Receive,
        leg: FloatLegSpec {
            index_id: "EUR-EURIBOR-3M".into(),
            forward_curve_id: CurveId::new("EUR-EURIBOR-3M"),
            discount_curve_id: CurveId::new("EUR-OIS"),
            start,
            end,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            calendar_id: None,
            reset_lag_days: 0,
            compounding: Default::default(),
            fixing_calendar_id: None,
            end_of_month: false,
        },
    };
    let mut leg2 = leg1.clone();
    leg2.notional = Money::from((10_000_000_i64, Currency::USD));
    leg2.side = PayReceive::Pay;
    leg2.leg.forward_curve_id = CurveId::new("USD-SOFR-3M");
    leg2.leg.discount_curve_id = CurveId::new("USD-OIS");
    leg2.leg.start = start_off; // misaligned start

    let mut swap = XccySwap::new("MTM-MISALIGNED", leg1, leg2, Currency::USD)
        .with_notional_exchange(NotionalExchange::MtmResetting {
            resetting_side: ResettingSide::Leg1,
        });
    swap.allow_calendar_fallback = true;

    let err = swap
        .validate()
        .expect_err("misaligned schedules must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("MtmResetting") && msg.contains("schedule"),
        "expected MtM-reset schedule-alignment error, got: {msg}"
    );
}

#[test]
fn validate_accepts_mtm_reset_with_leg_specific_frequencies() {
    use finstack_quant_core::money::Money;

    let start = Date::from_calendar_date(2025, time::Month::January, 2).expect("valid date");
    let end = Date::from_calendar_date(2030, time::Month::January, 2).expect("valid date");

    let leg1 = XccySwapLeg {
        notional: Money::from((9_200_000_i64, Currency::EUR)),
        side: PayReceive::Receive,
        leg: FloatLegSpec {
            index_id: "EUR-EURIBOR-3M".into(),
            forward_curve_id: CurveId::new("EUR-EURIBOR-3M"),
            discount_curve_id: CurveId::new("EUR-OIS"),
            start,
            end,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            calendar_id: None,
            reset_lag_days: 0,
            compounding: Default::default(),
            fixing_calendar_id: None,
            end_of_month: false,
        },
    };
    let mut leg2 = leg1.clone();
    leg2.notional = Money::from((10_000_000_i64, Currency::USD));
    leg2.side = PayReceive::Pay;
    leg2.leg.forward_curve_id = CurveId::new("USD-SOFR-3M");
    leg2.leg.discount_curve_id = CurveId::new("USD-OIS");
    leg2.leg.frequency = Tenor::semi_annual();

    let mut swap = XccySwap::new("MTM-FREQ", leg1, leg2, Currency::USD).with_notional_exchange(
        NotionalExchange::MtmResetting {
            resetting_side: ResettingSide::Leg1,
        },
    );
    swap.allow_calendar_fallback = true;

    swap.validate()
        .expect("each MtM leg owns its coupon frequency");
}

#[test]
fn validate_rejects_same_currency_legs_without_mtm_reset() {
    let mut swap = XccySwap::example()
        .expect("example")
        .with_notional_exchange(NotionalExchange::InitialAndFinal);
    swap.leg2.notional = finstack_quant_core::money::Money::from((1_i64, Currency::USD));

    let err = swap
        .validate()
        .expect_err("same-currency legs must be rejected by validate()");
    let msg = err.to_string();
    assert!(
        msg.contains("different currencies") && msg.contains("USD"),
        "expected same-currency error mentioning USD; got: {msg}"
    );
}

#[test]
fn validate_accepts_well_formed_mtm_resetting_swap() {
    // The canonical example swap has aligned schedules, matching frequencies, and
    // different currencies. Wrapping it in MtmResetting must pass `validate()`.
    let swap = XccySwap::example()
        .expect("example")
        .with_notional_exchange(NotionalExchange::MtmResetting {
            resetting_side: ResettingSide::Leg2,
        });
    swap.validate()
        .expect("well-formed MtmResetting swap should pass validate");
}

#[test]
fn scenario_price_shock_applies_through_the_override_channel() {
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use std::sync::Arc;

    let base = Date::from_calendar_date(2024, Month::January, 3).expect("base date");
    let curve = |id: &str, rate: f64| {
        DiscountCurve::builder(CurveId::new(id))
            .base_date(base)
            .knots(vec![(0.0, 1.0), (5.0, (-rate * 5.0).exp())])
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("discount curve")
    };
    let forward = |id: &str, rate: f64| {
        ForwardCurve::flat(
            CurveId::new(id),
            base,
            rate,
            finstack_quant_core::dates::DayCount::Act365F,
        )
        .expect("forward curve")
    };
    let provider = Arc::new(SimpleFxProvider::new());
    provider
        .set_quotes(&[(Currency::EUR, Currency::USD, 1.10)])
        .expect("set EUR/USD rate");
    let market = MarketContext::new()
        .try_insert(curve("USD-OIS", 0.02))
        .expect("insert market curve")
        .try_insert(curve("EUR-OIS", 0.01))
        .expect("insert market curve")
        .try_insert(forward("USD-SOFR-3M", 0.02))
        .expect("insert market curve")
        .try_insert(forward("EUR-EURIBOR-3M", 0.01))
        .expect("insert market curve")
        .insert_fx(FxMatrix::new(provider, None));

    let swap = XccySwap::example().expect("example");
    let unshocked = swap.value(&market, base).expect("unshocked value");
    let mut shocked_swap = swap.clone();
    shocked_swap
        .get_scenario_pricing_overrides_mut()
        .expect("XccySwap exposes scenario overrides")
        .scenario_price_shock_decimal = Some(-0.05);
    let shocked = shocked_swap.value(&market, base).expect("shocked value");
    // Reference: ScenarioPricingOverrides scales the PV by (1 + shock). The
    // shock is applied to the raw PV before Money's decimal storage, so the
    // two sides can differ in the last ulp (1e-12 relative).
    let expected = unshocked.amount() * (1.0 + -0.05);
    assert!(
        (shocked.amount() - expected).abs() <= 1.0e-12 * expected.abs(),
        "shocked {} vs expected {expected}",
        shocked.amount()
    );
    assert!(unshocked.amount().abs() > 1.0, "non-trivial base PV");

    let json = serde_json::to_value(&shocked_swap).expect("serialize");
    assert_eq!(
        json["scenario_pricing_overrides"]["scenario_price_shock_decimal"],
        -0.05
    );
    let unshocked_json = serde_json::to_value(&swap).expect("serialize");
    assert!(unshocked_json.get("instrument_pricing_overrides").is_none());
    assert!(unshocked_json.get("metric_pricing_overrides").is_none());
    assert!(unshocked_json.get("scenario_pricing_overrides").is_none());
}

#[test]
fn base_value_dispatches_mtm_resetting_to_pricing_mtm() {
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use std::sync::Arc;
    use time::Month;

    let base = Date::from_calendar_date(2024, Month::January, 3).expect("base date");

    // Build minimal curves matching the IDs used by XccySwap::example().expect("example").
    let usd_disc = DiscountCurve::builder(CurveId::new("USD-OIS"))
        .base_date(base)
        .knots(vec![(0.0, 1.0), (5.0, (-0.02_f64 * 5.0).exp())])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("usd disc");
    let eur_disc = DiscountCurve::builder(CurveId::new("EUR-OIS"))
        .base_date(base)
        .knots(vec![(0.0, 1.0), (5.0, (-0.01_f64 * 5.0).exp())])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("eur disc");
    let usd_fwd = ForwardCurve::flat(
        CurveId::new("USD-SOFR-3M"),
        base,
        0.02,
        finstack_quant_core::dates::DayCount::Act365F,
    )
    .expect("usd fwd");
    let eur_fwd = ForwardCurve::flat(
        CurveId::new("EUR-EURIBOR-3M"),
        base,
        0.01,
        finstack_quant_core::dates::DayCount::Act365F,
    )
    .expect("eur fwd");

    let provider = Arc::new(SimpleFxProvider::new());
    provider
        .set_quotes(&[(Currency::EUR, Currency::USD, 1.10)])
        .expect("set EUR/USD rate");
    let fx = FxMatrix::new(provider, None);

    let ctx = MarketContext::new()
        .try_insert(usd_disc)
        .expect("insert market curve")
        .try_insert(eur_disc)
        .expect("insert market curve")
        .try_insert(usd_fwd)
        .expect("insert market curve")
        .try_insert(eur_fwd)
        .expect("insert market curve")
        .insert_fx(fx);

    let swap = XccySwap::example()
        .expect("example")
        .with_notional_exchange(NotionalExchange::MtmResetting {
            resetting_side: ResettingSide::Leg2,
        });

    // The PV should be a finite number; we are not asserting the exact value here.
    // Task 9 will do CIP-invariance.
    let pv = swap
        .base_value(&ctx, base)
        .expect("MtM-reset PV should compute");
    assert!(
        pv.amount().is_finite(),
        "MtM-reset PV must be finite, got {}",
        pv.amount()
    );
    assert_eq!(pv.currency(), Currency::USD);
}

#[test]
fn overnight_estr_leg_matches_shared_compounding_engine() {
    use finstack_quant_cashflows::builder::FloatingLegCompounding;
    use finstack_quant_core::dates::calendar_by_id_strict;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_valuations_core::instruments::pricing::overnight::{
        OvernightCouponProjectionInput, OvernightProjectionCurve, project_overnight_coupon,
    };

    let start = Date::from_calendar_date(2025, time::Month::January, 2).expect("date");
    let end = Date::from_calendar_date(2025, time::Month::April, 2).expect("date");
    let fwd = ForwardCurve::flat(
        "EUR-ESTR-OIS",
        start,
        0.03,
        finstack_quant_core::dates::DayCount::Act365F,
    )
    .expect("estr forward");
    let compounding = FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };
    let period = finstack_quant_cashflows::builder::SchedulePeriod {
        accrual_start: start,
        accrual_end: end,
        payment_date: end,
        reset_date: Some(start),
        accrual_year_fraction: 0.25,
        unadjusted_start: start,
        unadjusted_end: end,
    };
    let leg = XccySwapLeg {
        notional: Money::from((1_000_000_i64, Currency::EUR)),
        side: PayReceive::Pay,
        leg: FloatLegSpec {
            index_id: "EUR-ESTR-OIS".into(),
            forward_curve_id: CurveId::new("EUR-ESTR-OIS"),
            discount_curve_id: CurveId::new("EUR-OIS"),
            start,
            end,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            calendar_id: Some("target2".into()),
            reset_lag_days: 0,
            compounding,
            fixing_calendar_id: None,
            end_of_month: false,
        },
    };
    let mut mismatched = period;
    mismatched.accrual_year_fraction = 0.50;
    let projected = no_fallback_swap()
        .projected_leg_period(
            &leg,
            &crate::swap_legs::ResolvedProjectionCurve::Forward(std::sync::Arc::new(fwd.clone())),
            None,
            &mismatched,
            start,
            crate::xccy_swap::types::XccyPeriodProjectionArgs {
                end_is_termination_date: false,
                projected_fixings: None,
            },
        )
        .expect("overnight xccy period");
    let calendar = calendar_by_id_strict("target2").expect("target2");
    let expected = project_overnight_coupon(OvernightCouponProjectionInput {
        curve: OvernightProjectionCurve::Forward(&fwd),
        fixings: None,
        fixing_id: "EUR-ESTR-OIS",
        as_of: start,
        accrual_start: start,
        accrual_end: end,
        end_is_termination_date: None,
        day_count: DayCount::Act360,
        coupon_frequency: Some(Tenor::quarterly()),
        coupon_period: (start, end),
        compounding: &compounding,
        fixing_calendar: calendar,
        compounded_spread: 0.0,
        need_observation_exposures: false,
    })
    .expect("shared engine");
    assert!(
        (projected.rate - expected.rate).abs() < 1e-12,
        "xccy overnight rate {} != shared engine {}",
        projected.rate,
        expected.rate
    );
    let amount = projected.unsigned_coupon(leg.notional.amount(), 0.0);
    let expected_amount = 1_000_000.0 * (expected.compound_factor - 1.0);
    assert!(
        (amount - expected_amount).abs() < 1e-6,
        "xccy overnight amount {amount} != N*(CF-1) {expected_amount}"
    );
    let wrong_schedule_amount = projected.rate * 1_000_000.0 * mismatched.accrual_year_fraction;
    assert!(
        (amount - wrong_schedule_amount).abs() > 1.0,
        "overnight coupon must not use the unadjusted schedule year fraction"
    );
}

#[test]
fn overnight_unresolvable_calendar_without_fallback_errors() {
    use finstack_quant_cashflows::builder::FloatingLegCompounding;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;

    let start = Date::from_calendar_date(2025, time::Month::January, 2).expect("date");
    let end = Date::from_calendar_date(2025, time::Month::April, 2).expect("date");
    let fwd = ForwardCurve::flat(
        "EUR-ESTR-OIS",
        start,
        0.03,
        finstack_quant_core::dates::DayCount::Act365F,
    )
    .expect("estr forward");
    let period = finstack_quant_cashflows::builder::SchedulePeriod {
        accrual_start: start,
        accrual_end: end,
        payment_date: end,
        reset_date: Some(start),
        accrual_year_fraction: 0.25,
        unadjusted_start: start,
        unadjusted_end: end,
    };
    let mut leg = XccySwapLeg {
        notional: Money::from((1_000_000_i64, Currency::EUR)),
        side: PayReceive::Pay,
        leg: FloatLegSpec {
            index_id: "EUR-ESTR-OIS".into(),
            forward_curve_id: CurveId::new("EUR-ESTR-OIS"),
            discount_curve_id: CurveId::new("EUR-OIS"),
            start,
            end,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            calendar_id: Some("not-a-calendar".into()),
            reset_lag_days: 0,
            compounding: FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
            fixing_calendar_id: None,
            end_of_month: false,
        },
    };
    let err = no_fallback_swap()
        .projected_leg_period(
            &leg,
            &crate::swap_legs::ResolvedProjectionCurve::Forward(std::sync::Arc::new(fwd.clone())),
            None,
            &period,
            start,
            crate::xccy_swap::types::XccyPeriodProjectionArgs {
                end_is_termination_date: false,
                projected_fixings: None,
            },
        )
        .expect_err("unresolvable calendar must fail");
    assert!(
        err.to_string().contains("calendar"),
        "expected calendar error, got {err}"
    );

    let disc = finstack_quant_core::market_data::term_structures::DiscountCurve::builder("EUR-OIS")
        .base_date(start)
        .knots(vec![(0.0, 1.0), (1.0, 0.97)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount");
    let market = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(fwd)
        .expect("insert market curve");
    let swap = XccySwap::new(
        "EUR-OIS-BAD-CAL",
        leg.clone(),
        {
            leg.notional = Money::from((1_000_000_i64, Currency::USD));
            leg.leg.forward_curve_id = CurveId::new("USD-SOFR-3M");
            leg.leg.discount_curve_id = CurveId::new("USD-OIS");
            leg.leg.compounding = FloatingLegCompounding::Simple;
            leg.leg.calendar_id = Some("usny".into());
            leg
        },
        Currency::USD,
    );
    let err = swap
        .base_value(&market, start)
        .expect_err("pricing must reject the unresolvable overnight calendar");
    assert!(
        err.to_string().contains("calendar"),
        "expected calendar error from base_value, got {err}"
    );
}

#[test]
fn validate_rejects_simple_compounding_on_overnight_index() {
    let mut swap = XccySwap::example().expect("example");
    swap.leg1.leg.index_id = "USD-SOFR-OIS".into();
    swap.leg1.leg.forward_curve_id = CurveId::new("USD-SOFR-OIS");
    swap.leg1.leg.compounding = finstack_quant_cashflows::builder::FloatingLegCompounding::Simple;
    let err = swap
        .validate()
        .expect_err("Simple on USD-SOFR-OIS must fail");
    assert!(
        format!("{err}").contains("Overnight RFR"),
        "expected overnight/Simple rejection, got {err}"
    );
}

#[test]
fn xccy_swap_expired_mtm_returns_zero_without_fx_and_has_expiry() {
    let start = Date::from_calendar_date(2024, time::Month::January, 2).expect("date");
    let end = Date::from_calendar_date(2025, time::Month::January, 2).expect("date");
    let mut swap = XccySwap::new(
        "XCCY-EXPIRED-MTM",
        XccySwapLeg {
            notional: Money::from((1_000_000_i64, Currency::USD)),
            side: PayReceive::Receive,
            leg: FloatLegSpec {
                index_id: "USD-SOFR-3M".into(),
                forward_curve_id: CurveId::new("USD-SOFR-3M"),
                discount_curve_id: CurveId::new("USD-OIS"),
                start,
                end,
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                calendar_id: None,
                stub: StubKind::ShortFront,
                reset_lag_days: 0,
                fixing_calendar_id: None,
                end_of_month: false,
                spread_bp: Decimal::ZERO,
                compounding: Default::default(),
                payment_lag_days: 0,
            },
        },
        XccySwapLeg {
            notional: Money::from((900_000_i64, Currency::EUR)),
            side: PayReceive::Pay,
            leg: FloatLegSpec {
                index_id: "EUR-EURIBOR-3M".into(),
                forward_curve_id: CurveId::new("EUR-EURIBOR-3M"),
                discount_curve_id: CurveId::new("EUR-OIS"),
                start,
                end,
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                calendar_id: None,
                stub: StubKind::ShortFront,
                reset_lag_days: 0,
                fixing_calendar_id: None,
                end_of_month: false,
                spread_bp: Decimal::ZERO,
                compounding: Default::default(),
                payment_lag_days: 0,
            },
        },
        Currency::USD,
    )
    .with_notional_exchange(NotionalExchange::MtmResetting {
        resetting_side: finstack_quant_valuations_core::instruments::ResettingSide::Leg2,
    });
    swap.allow_calendar_fallback = true;

    assert_eq!(swap.expiry(), Some(end));
    let pv = swap
        .value(
            &MarketContext::new(),
            Date::from_calendar_date(2026, time::Month::January, 2).expect("after"),
        )
        .expect("expired MtM XccySwap returns 0 without live curves/FX");
    assert_eq!(pv.amount(), 0.0);
}
