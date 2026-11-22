use super::*;
use crate::instruments::{GreekBumps, OptionGreekKind, OptionGreeksRequest, Position};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_models::types::{ExerciseStyle, OptionType};
use time::macros::date;

fn market(rate: f64) -> MarketContext {
    MarketContext::new()
        .try_insert(
            DiscountCurve::flat("USD-OIS", date!(2026 - 01 - 01), rate, DayCount::Act365F)
                .expect("discount curve"),
        )
        .expect("insert market curve")
}

fn option(model: FutureOptionModel, option_type: OptionType) -> FutureOptionTerms {
    FutureOptionTerms::builder()
        .underlying("TEST-Z6".to_string())
        .futures_price(100.0)
        .strike(100.0)
        .contracts(1.0)
        .multiplier(10.0)
        .currency(Currency::USD)
        .option_type(option_type)
        .position(Position::Long)
        .exercise_style(ExerciseStyle::European)
        .expiry(date!(2027 - 01 - 01))
        .settlement(FutureOptionSettlement::Cash {
            payment_date: date!(2027 - 01 - 01),
        })
        .model(model)
        .premium_style(FutureOptionPremiumStyle::PremiumPaid)
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .build()
        .expect("option")
}

/// Test volatility in the model's units: 20% lognormal or 5 price points normal.
fn vol(model: FutureOptionModel) -> Option<f64> {
    Some(match model {
        FutureOptionModel::Black76 => 0.20,
        FutureOptionModel::Normal => 5.0,
    })
}

#[test]
fn put_call_parity_holds_for_black_and_normal() {
    let market = market(0.05);
    let as_of = date!(2026 - 01 - 01);
    for model in [FutureOptionModel::Black76, FutureOptionModel::Normal] {
        let call = option(model, OptionType::Call)
            .npv_raw(
                &InstrumentId::new("TEST-FOP"),
                None,
                vol(model),
                &market,
                as_of,
            )
            .expect("call");
        let put = option(model, OptionType::Put)
            .npv_raw(
                &InstrumentId::new("TEST-FOP"),
                None,
                vol(model),
                &market,
                as_of,
            )
            .expect("put");
        assert!((call - put).abs() < 1e-10);
    }
}

#[test]
fn normal_atm_matches_closed_form() {
    let market = market(0.0);
    let as_of = date!(2026 - 01 - 01);
    let option = option(FutureOptionModel::Normal, OptionType::Call);
    let expected = 5.0 / (2.0 * std::f64::consts::PI).sqrt() * 10.0;
    let actual = option
        .npv_raw(
            &InstrumentId::new("TEST-FOP"),
            None,
            vol(FutureOptionModel::Normal),
            &market,
            as_of,
        )
        .expect("normal pv");
    assert!((actual - expected).abs() < 1e-10 * expected);
}

#[test]
fn futures_style_value_is_pnl_against_reference_quote() {
    let market = market(0.05);
    let as_of = date!(2026 - 01 - 01);
    let premium_paid = option(FutureOptionModel::Black76, OptionType::Call);
    let mut futures_style = premium_paid.clone();
    futures_style.premium_style = FutureOptionPremiumStyle::FuturesStyle;
    let paid = premium_paid
        .npv_raw(
            &InstrumentId::new("TEST-FOP"),
            None,
            vol(FutureOptionModel::Black76),
            &market,
            as_of,
        )
        .expect("paid");
    let undiscounted_quote = futures_style
        .live_unit_price(&market, as_of, futures_style.futures_price, 0.20, None)
        .expect("undiscounted quote");
    assert!((undiscounted_quote * futures_style.multiplier / paid - 0.05_f64.exp()).abs() < 1e-10);

    futures_style.option_reference_price = Some(undiscounted_quote);
    let pnl = futures_style
        .npv_raw(
            &InstrumentId::new("TEST-FOP"),
            None,
            vol(FutureOptionModel::Black76),
            &market,
            as_of,
        )
        .expect("margined pnl");
    assert!(pnl.abs() < 1e-10);
}

#[test]
fn futures_style_requires_reference_quote() {
    let mut terms = option(FutureOptionModel::Black76, OptionType::Call);
    terms.premium_style = FutureOptionPremiumStyle::FuturesStyle;
    assert!(terms.validate().is_err());
}

#[test]
fn black_greeks_keep_bumped_futures_prices_positive() {
    let market = market(0.0);
    let as_of = date!(2026 - 01 - 01);
    let mut terms = option(FutureOptionModel::Black76, OptionType::Call);
    terms.futures_price = 1.0e-8;
    terms.strike = 1.0e-8;
    assert!(
        terms
            .cash_delta(None, vol(FutureOptionModel::Black76), &market, as_of)
            .expect("delta")
            .is_finite()
    );
    assert!(
        terms
            .cash_gamma(None, vol(FutureOptionModel::Black76), &market, as_of)
            .expect("gamma")
            .is_finite()
    );
}

#[test]
fn american_normal_lattice_does_not_undervalue_european() {
    let market = market(0.08);
    let as_of = date!(2026 - 01 - 01);
    let european = option(FutureOptionModel::Normal, OptionType::Put);
    let mut american = european.clone();
    american.exercise_style = ExerciseStyle::American;
    let european_pv = european
        .npv_raw(
            &InstrumentId::new("TEST-FOP"),
            None,
            vol(FutureOptionModel::Normal),
            &market,
            as_of,
        )
        .expect("european");
    let american_pv = american
        .npv_raw(
            &InstrumentId::new("TEST-FOP"),
            None,
            vol(FutureOptionModel::Normal),
            &market,
            as_of,
        )
        .expect("american");
    assert!(american_pv + 1e-10 >= european_pv);
}

#[test]
fn delivered_future_keeps_directional_delta_after_exercise() {
    let market = market(0.0);
    let mut option = option(FutureOptionModel::Black76, OptionType::Call);
    option.expiry = date!(2026 - 06 - 01);
    option.futures_price = 104.0;
    option.strike = 100.0;
    option.settlement = FutureOptionSettlement::Future {
        underlying_last_trading_date: date!(2026 - 12 - 30),
        underlying_settlement_date: date!(2026 - 12 - 31),
        underlying_settlement_price: None,
    };
    option.exercise =
        Some(FutureOptionExercise::new(date!(2026 - 06 - 01), 102.0).expect("exercise"));
    let as_of = date!(2026 - 06 - 02);
    assert_eq!(
        option
            .npv_raw(&InstrumentId::new("TEST-FOP"), None, None, &market, as_of)
            .expect("pnl"),
        40.0
    );
    assert_eq!(
        option
            .cash_delta(None, None, &market, as_of)
            .expect("delta"),
        10.0
    );
}

#[test]
fn live_option_requires_market_quote_implied_volatility() {
    let market = market(0.0);
    let option = option(FutureOptionModel::Black76, OptionType::Call);
    let error = option
        .npv_raw(
            &InstrumentId::new("TEST-FOP"),
            None,
            None,
            &market,
            date!(2026 - 01 - 01),
        )
        .expect_err("live option without a volatility quote");
    assert!(
        error
            .to_string()
            .contains("instrument_pricing_overrides.market_quotes.implied_volatility")
    );
}

#[test]
fn terms_reject_retired_volatility_key() {
    let mut value = serde_json::to_value(FutureOptionTerms::example().expect("example"))
        .expect("serialize terms");
    // schema-rejection-test
    value["volatility"] = serde_json::json!(0.2);
    let error = serde_json::from_value::<FutureOptionTerms>(value)
        .expect_err("retired terms.volatility must be rejected");
    assert!(error.to_string().contains("unknown field"), "{error}");
}

#[test]
fn expiry_requires_official_observation_across_pv_and_greeks() {
    let market = market(0.0);
    let option = option(FutureOptionModel::Normal, OptionType::Call);
    let error = option
        .npv_raw(
            &InstrumentId::new("TEST-FOP"),
            None,
            vol(FutureOptionModel::Normal),
            &market,
            option.expiry,
        )
        .expect_err("missing expiry observation on PV");
    assert!(
        error
            .to_string()
            .contains("requires an exercise/expiry observation")
    );
    for res in [
        option.cash_delta(None, vol(FutureOptionModel::Normal), &market, option.expiry),
        option.cash_gamma(None, vol(FutureOptionModel::Normal), &market, option.expiry),
        option.cash_vega(None, vol(FutureOptionModel::Normal), &market, option.expiry),
        option.cash_theta(None, vol(FutureOptionModel::Normal), &market, option.expiry),
    ] {
        let err = res.expect_err("missing expiry observation on Greek");
        assert!(
            err.to_string()
                .contains("requires an exercise/expiry observation"),
            "{err}"
        );
    }

    // Past terminal settlement date returns 0 across PV and Greeks.
    let past = date!(2027 - 01 - 02);
    assert_eq!(
        option
            .npv_raw(&InstrumentId::new("TEST-FOP"), None, None, &market, past)
            .expect("past terminal PV"),
        0.0
    );
    assert_eq!(
        option
            .cash_delta(None, None, &market, past)
            .expect("past terminal delta"),
        0.0
    );
    assert_eq!(
        option
            .cash_gamma(None, None, &market, past)
            .expect("past terminal gamma"),
        0.0
    );
    assert_eq!(
        option
            .cash_vega(None, None, &market, past)
            .expect("past terminal vega"),
        0.0
    );
    assert_eq!(
        option
            .cash_theta(None, None, &market, past)
            .expect("past terminal theta"),
        0.0
    );
}

#[test]
fn fixed_date_cash_american_matches_european_and_recorded_exercise() {
    let as_of = date!(2026 - 01 - 01);
    let id = InstrumentId::new("CASH-SETTLEMENT");
    for model in [FutureOptionModel::Black76, FutureOptionModel::Normal] {
        for kind in [OptionType::Call, OptionType::Put] {
            for rate in [-0.05, 0.0, 0.05] {
                let market = market(rate);
                for payment_date in [date!(2027 - 01 - 01), date!(2027 - 02 - 01)] {
                    for premium_style in [
                        FutureOptionPremiumStyle::PremiumPaid,
                        FutureOptionPremiumStyle::FuturesStyle,
                    ] {
                        for position in [Position::Long, Position::Short] {
                            let mut terms = option(model, kind);
                            terms.futures_price = if kind == OptionType::Call {
                                110.0
                            } else {
                                90.0
                            };
                            terms.position = position;
                            terms.premium_style = premium_style;
                            terms.option_reference_price = Some(1.25);
                            terms.settlement = FutureOptionSettlement::Cash { payment_date };
                            for sigma in [0.0, 1e-10, vol(model).expect("vol")] {
                                terms.exercise_style = ExerciseStyle::European;
                                let european = terms
                                    .npv_raw(&id, None, Some(sigma), &market, as_of)
                                    .expect("European");
                                terms.exercise_style = ExerciseStyle::American;
                                let american = terms
                                    .npv_raw(&id, None, Some(sigma), &market, as_of)
                                    .expect("American");
                                assert!((american - european).abs() < 1e-12);
                                if sigma == 0.0 {
                                    let df =
                                        if premium_style == FutureOptionPremiumStyle::PremiumPaid {
                                            market
                                                .get_discount("USD-OIS")
                                                .expect("curve")
                                                .df_between_dates(as_of, payment_date)
                                                .expect("df")
                                        } else {
                                            1.0
                                        };
                                    let reference = if premium_style
                                        == FutureOptionPremiumStyle::FuturesStyle
                                    {
                                        1.25
                                    } else {
                                        0.0
                                    };
                                    let expected = position.sign() * 10.0 * (10.0 * df - reference);
                                    assert!((american - expected).abs() < 1e-10);
                                    let mut exercised = terms.clone();
                                    exercised.exercise = Some(
                                        FutureOptionExercise::new(as_of, terms.futures_price)
                                            .expect("exercise"),
                                    );
                                    let settled = exercised
                                        .npv_raw(&id, None, None, &market, as_of)
                                        .expect("recorded");
                                    assert!((settled - american).abs() < 1e-12);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn future_delivery_american_zero_vol_retains_exercise_economics() {
    let as_of = date!(2026 - 01 - 01);
    let id = InstrumentId::new("FUTURE-DELIVERY");
    for model in [FutureOptionModel::Black76, FutureOptionModel::Normal] {
        for kind in [OptionType::Call, OptionType::Put] {
            for rate in [-0.05, 0.0, 0.05] {
                let market = market(rate);
                for premium_style in [
                    FutureOptionPremiumStyle::PremiumPaid,
                    FutureOptionPremiumStyle::FuturesStyle,
                ] {
                    let mut terms = option(model, kind);
                    terms.futures_price = if kind == OptionType::Call {
                        110.0
                    } else {
                        90.0
                    };
                    terms.premium_style = premium_style;
                    terms.option_reference_price = Some(1.25);
                    terms.settlement = FutureOptionSettlement::Future {
                        underlying_last_trading_date: date!(2027 - 02 - 01),
                        underlying_settlement_date: date!(2027 - 02 - 02),
                        underlying_settlement_price: None,
                    };
                    let european = terms
                        .npv_raw(&id, None, Some(0.0), &market, as_of)
                        .expect("European");
                    terms.exercise_style = ExerciseStyle::American;
                    let american = terms
                        .npv_raw(&id, None, Some(0.0), &market, as_of)
                        .expect("American");
                    let tiny = terms
                        .npv_raw(&id, None, Some(1e-6), &market, as_of)
                        .expect("tiny sigma");
                    let df = terms.pricing_discount_factor(&market, as_of).expect("df");
                    let expected = terms
                        .position_value_from_unit_quote(10.0 * df.max(1.0))
                        .expect("scaled value");
                    assert!((american - expected).abs() < 1e-10);
                    assert!(
                        (american - tiny).abs() < 1e-5,
                        "{model:?}: zero={american}, tiny={tiny}"
                    );
                    assert!(american >= european - 1e-10);
                    if rate > 0.0 && premium_style == FutureOptionPremiumStyle::PremiumPaid {
                        assert!(american > european);
                    }
                }
            }
        }
    }
}

#[test]
fn delivered_future_delta_stops_when_official_settlement_fixes_the_mark() {
    let market = market(0.0);
    let id = InstrumentId::new("DELIVERED");
    for kind in [OptionType::Call, OptionType::Put] {
        for position in [Position::Long, Position::Short] {
            let mut terms = option(FutureOptionModel::Black76, kind);
            terms.position = position;
            terms.expiry = date!(2026 - 06 - 01);
            terms.futures_price = 105.0;
            let official_exercise = if kind == OptionType::Call {
                110.0
            } else {
                90.0
            };
            terms.exercise =
                Some(FutureOptionExercise::new(terms.expiry, official_exercise).expect("exercise"));
            terms.settlement = FutureOptionSettlement::Future {
                underlying_last_trading_date: date!(2026 - 06 - 05),
                underlying_settlement_date: date!(2026 - 06 - 10),
                underlying_settlement_price: Some(108.0),
            };
            for as_of in [
                date!(2026 - 06 - 04),
                date!(2026 - 06 - 05),
                date!(2026 - 06 - 06),
                date!(2026 - 06 - 10),
                date!(2026 - 06 - 11),
            ] {
                let delta = terms.cash_delta(None, None, &market, as_of).expect("delta");
                let mut up = terms.clone();
                let mut down = terms.clone();
                up.futures_price += 0.01;
                down.futures_price -= 0.01;
                let fd = (up.npv_raw(&id, None, None, &market, as_of).expect("up")
                    - down.npv_raw(&id, None, None, &market, as_of).expect("down"))
                    / 0.02;
                assert!((delta - fd).abs() < 1e-8);
                if as_of > date!(2026 - 06 - 05) {
                    assert_eq!(delta, 0.0);
                } else {
                    assert_eq!(delta, terms.exercise_direction() * 10.0);
                }
            }
            if let FutureOptionSettlement::Future {
                underlying_settlement_price,
                ..
            } = &mut terms.settlement
            {
                *underlying_settlement_price = None;
            }
            assert!(
                terms
                    .cash_delta(None, None, &market, date!(2026 - 06 - 06))
                    .is_err()
            );
            assert!(
                terms
                    .npv_raw(&id, None, None, &market, date!(2026 - 06 - 06))
                    .is_err()
            );
        }
    }
}

#[test]
fn non_positive_discount_factor_is_rejected() {
    use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};

    let base_date = date!(2026 - 01 - 01);
    let as_of = date!(2026 - 07 - 02);
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (0.5, 1.0e-200), (1.0, 1.0e200)])
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Flat)
        .day_count(DayCount::Act365F)
        .build()
        .expect("curve");
    let bad_market = MarketContext::new().try_insert(curve).expect("insert");
    let terms = option(FutureOptionModel::Black76, OptionType::Call);
    let err = terms
        .npv_raw(
            &InstrumentId::new("BAD-DF"),
            None,
            Some(0.20),
            &bad_market,
            as_of,
        )
        .expect_err("overflowing between-date discount factor must fail");
    assert!(
        err.to_string().contains("Invalid relative discount factor")
            || err.to_string().contains("finite and positive"),
        "{err}"
    );
}

#[test]
fn non_finite_greek_triggers_validation_error() {
    let market = market(0.0);
    let as_of = date!(2026 - 01 - 01);
    let mut terms = option(FutureOptionModel::Normal, OptionType::Call);
    terms.contracts = f64::MAX / 2.0;
    terms.multiplier = 1.0e10;
    let err = terms
        .cash_delta(None, vol(FutureOptionModel::Normal), &market, as_of)
        .expect_err("overflowing greek must fail");
    assert!(err.to_string().contains("non-finite"), "{err}");
}

#[test]
fn option_greeks_honors_custom_spot_and_vol_bumps() {
    let market = market(0.03);
    let as_of = date!(2026 - 01 - 01);
    let mut terms = option(FutureOptionModel::Black76, OptionType::Call);
    terms.strike = 100.0;
    let vol = Some(0.10);

    let small_delta = terms
        .option_greeks(
            None,
            vol,
            &market,
            as_of,
            &OptionGreeksRequest {
                greek: OptionGreekKind::Delta,
                base_pv: None,
                bumps: GreekBumps {
                    spot_bump_decimal: 1e-4,
                    ..GreekBumps::default()
                },
            },
        )
        .expect("small delta")
        .delta
        .expect("delta");
    let wide_delta = terms
        .option_greeks(
            None,
            vol,
            &market,
            as_of,
            &OptionGreeksRequest {
                greek: OptionGreekKind::Delta,
                base_pv: None,
                bumps: GreekBumps {
                    spot_bump_decimal: 0.25,
                    ..GreekBumps::default()
                },
            },
        )
        .expect("wide delta")
        .delta
        .expect("delta");
    assert!((small_delta - wide_delta).abs() > 1e-3);

    let small_vega = terms
        .option_greeks(
            None,
            vol,
            &market,
            as_of,
            &OptionGreeksRequest {
                greek: OptionGreekKind::Vega,
                base_pv: None,
                bumps: GreekBumps {
                    vol_bump_decimal: 1e-4,
                    ..GreekBumps::default()
                },
            },
        )
        .expect("small vega")
        .vega
        .expect("vega");
    let wide_vega = terms
        .option_greeks(
            None,
            vol,
            &market,
            as_of,
            &OptionGreeksRequest {
                greek: OptionGreekKind::Vega,
                base_pv: None,
                bumps: GreekBumps {
                    vol_bump_decimal: 0.15,
                    ..GreekBumps::default()
                },
            },
        )
        .expect("wide vega")
        .vega
        .expect("vega");
    // Both report vega per +0.01 vol point, differing only by convexity over the bump width.
    assert!((small_vega - wide_vega).abs() < 20.0);
    assert!((small_vega - wide_vega).abs() > 1e-3);

    assert!(
        terms
            .option_greeks(
                None,
                vol,
                &market,
                as_of,
                &OptionGreeksRequest {
                    greek: OptionGreekKind::Delta,
                    base_pv: None,
                    bumps: GreekBumps {
                        spot_bump_decimal: 0.0,
                        ..GreekBumps::default()
                    },
                },
            )
            .is_err()
    );
    assert!(
        terms
            .option_greeks(
                None,
                vol,
                &market,
                as_of,
                &OptionGreeksRequest {
                    greek: OptionGreekKind::Vega,
                    base_pv: None,
                    bumps: GreekBumps {
                        vol_bump_decimal: -0.01,
                        ..GreekBumps::default()
                    },
                },
            )
            .is_err()
    );
}
