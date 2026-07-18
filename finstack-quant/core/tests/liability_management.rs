//! Behavioral tests for distressed-exchange and LME analytics.

use finstack_quant_core::credit::liability_management::{
    analyze_exchange_offer, analyze_lme, ExchangeType, LmeType, TENDER_RECOMMENDATION_HURDLE,
};

#[test]
fn discount_exchange_matches_reference_economics() {
    let offer = analyze_exchange_offer(45.0, 80.0, 2.0, 0.0, ExchangeType::Discount)
        .expect("valid exchange offer");

    assert_eq!(offer.exchange_type, ExchangeType::Discount);
    assert_eq!(offer.tender_total, 82.0);
    assert_eq!(offer.delta_npv, 37.0);
    assert_eq!(offer.breakeven_recovery, 1.0);
    assert!(offer.tender_recommended);
}

#[test]
fn breakeven_recovery_is_uncapped_below_par_and_defaults_when_hold_out_is_worthless() {
    let below_par = analyze_exchange_offer(100.0, 60.0, 0.0, 5.0, ExchangeType::ParForPar)
        .expect("valid exchange offer");
    assert_eq!(below_par.tender_total, 65.0);
    assert_eq!(below_par.delta_npv, -35.0);
    assert!((below_par.breakeven_recovery - 0.65).abs() < 1e-12);
    assert!(!below_par.tender_recommended);

    let worthless =
        analyze_exchange_offer(0.0, 10.0, 0.0, 0.0, ExchangeType::Uptier).expect("valid offer");
    assert_eq!(worthless.breakeven_recovery, 1.0);
    assert!(worthless.tender_recommended);
}

#[test]
fn tender_hurdle_requires_more_than_two_percent_pickup() {
    let old_pv = 100.0;

    let at_hurdle = analyze_exchange_offer(
        old_pv,
        old_pv * TENDER_RECOMMENDATION_HURDLE,
        0.0,
        0.0,
        ExchangeType::Downtier,
    )
    .expect("valid offer");
    assert!(!at_hurdle.tender_recommended);

    let above_hurdle = analyze_exchange_offer(old_pv, 102.5, 0.0, 0.0, ExchangeType::Downtier)
        .expect("valid offer");
    assert!(above_hurdle.tender_recommended);
}

#[test]
fn exchange_offer_rejects_negative_and_non_finite_amounts() {
    assert!(analyze_exchange_offer(-1.0, 1.0, 0.0, 0.0, ExchangeType::Discount).is_err());
    assert!(analyze_exchange_offer(1.0, -1.0, 0.0, 0.0, ExchangeType::Discount).is_err());
    assert!(analyze_exchange_offer(1.0, 1.0, -1.0, 0.0, ExchangeType::Discount).is_err());
    assert!(analyze_exchange_offer(1.0, 1.0, 0.0, -1.0, ExchangeType::Discount).is_err());
    assert!(analyze_exchange_offer(f64::NAN, 1.0, 0.0, 0.0, ExchangeType::Discount).is_err());
    assert!(analyze_exchange_offer(1.0, f64::INFINITY, 0.0, 0.0, ExchangeType::Discount).is_err());
}

#[test]
fn exchange_type_parsing_accepts_market_shorthand() {
    assert_eq!(
        "par".parse::<ExchangeType>().expect("alias"),
        ExchangeType::ParForPar
    );
    assert_eq!(
        " Par-For-Par ".parse::<ExchangeType>().expect("normalized"),
        ExchangeType::ParForPar
    );
    assert_eq!(
        "UPTIER".parse::<ExchangeType>().expect("case insensitive"),
        ExchangeType::Uptier
    );
    assert_eq!(ExchangeType::Downtier.to_string(), "downtier");
    assert!("mystery".parse::<ExchangeType>().is_err());
}

#[test]
fn open_market_repurchase_matches_reference_economics() {
    let lme = analyze_lme(
        LmeType::OpenMarketRepurchase,
        200_000_000.0,
        0.60,
        0.40,
        Some(25_000_000.0),
    )
    .expect("valid LME");

    assert_eq!(lme.lme_type, LmeType::OpenMarketRepurchase);
    assert_eq!(lme.cost, 48_000_000.0);
    assert_eq!(lme.notional_reduction, 80_000_000.0);
    assert_eq!(lme.discount_capture, 32_000_000.0);
    assert!((lme.discount_capture_pct - 0.40).abs() < 1e-12);
    assert_eq!(lme.remaining_holder_impact_pct, 0.0);

    let leverage = lme.leverage_impact.expect("EBITDA supplied");
    assert_eq!(leverage.pre_total_debt, 200_000_000.0);
    assert_eq!(leverage.post_total_debt, 120_000_000.0);
    assert!((leverage.pre_leverage - 8.0).abs() < 1e-12);
    assert!((leverage.post_leverage - 4.8).abs() < 1e-12);
    assert!((leverage.leverage_reduction - 3.2).abs() < 1e-12);
}

#[test]
fn amend_and_extend_pays_a_fee_without_retiring_par() {
    let lme = analyze_lme(LmeType::AmendAndExtend, 500.0, 0.02, 0.75, Some(100.0))
        .expect("valid amend-and-extend");

    assert!((lme.cost - 7.5).abs() < 1e-12);
    assert_eq!(lme.notional_reduction, 0.0);
    assert!((lme.discount_capture + 7.5).abs() < 1e-12);
    assert_eq!(lme.discount_capture_pct, 0.0);
    assert_eq!(lme.remaining_holder_impact_pct, 0.0);

    let leverage = lme.leverage_impact.expect("EBITDA supplied");
    assert!((leverage.pre_leverage - 5.0).abs() < 1e-12);
    assert!((leverage.post_leverage - 5.0).abs() < 1e-12);
    assert_eq!(leverage.leverage_reduction, 0.0);
}

#[test]
fn dropdown_dilutes_remaining_holders_without_cash_or_par_change() {
    let lme = analyze_lme(LmeType::Dropdown, 400.0, 0.35, 1.0, None).expect("valid dropdown");

    assert_eq!(lme.cost, 0.0);
    assert_eq!(lme.notional_reduction, 0.0);
    assert_eq!(lme.discount_capture, 0.0);
    assert_eq!(lme.discount_capture_pct, 0.0);
    assert!((lme.remaining_holder_impact_pct - 0.35).abs() < 1e-12);
    assert!(lme.leverage_impact.is_none());
}

#[test]
fn non_positive_ebitda_omits_the_leverage_block() {
    let zero = analyze_lme(LmeType::TenderOffer, 100.0, 0.9, 1.0, Some(0.0)).expect("valid tender");
    assert!(zero.leverage_impact.is_none());

    let negative =
        analyze_lme(LmeType::TenderOffer, 100.0, 0.9, 1.0, Some(-5.0)).expect("valid tender");
    assert!(negative.leverage_impact.is_none());
}

#[test]
fn lme_rejects_out_of_range_inputs_per_structure() {
    assert!(analyze_lme(LmeType::TenderOffer, 0.0, 0.5, 1.0, None).is_err());
    assert!(analyze_lme(LmeType::TenderOffer, f64::NAN, 0.5, 1.0, None).is_err());
    assert!(analyze_lme(LmeType::TenderOffer, 100.0, 0.5, 1.5, None).is_err());
    assert!(analyze_lme(LmeType::TenderOffer, 100.0, 0.5, f64::NAN, None).is_err());
    // Price quoted in points rather than as a fraction.
    assert!(analyze_lme(LmeType::OpenMarketRepurchase, 100.0, 60.0, 1.0, None).is_err());
    assert!(analyze_lme(LmeType::OpenMarketRepurchase, 100.0, 0.0, 1.0, None).is_err());
    // Extension fees above 10 points are rejected.
    assert!(analyze_lme(LmeType::AmendAndExtend, 100.0, 0.2, 1.0, None).is_err());
    assert!(analyze_lme(LmeType::Dropdown, 100.0, 1.5, 1.0, None).is_err());
}

#[test]
fn lme_type_parsing_accepts_market_shorthand() {
    for alias in ["open_market", "OMR", "open-market-repurchase"] {
        assert_eq!(
            alias.parse::<LmeType>().expect("alias"),
            LmeType::OpenMarketRepurchase
        );
    }
    assert_eq!(
        "tender".parse::<LmeType>().expect("alias"),
        LmeType::TenderOffer
    );
    for alias in ["A&E", "ae", "amend-and-extend"] {
        assert_eq!(
            alias.parse::<LmeType>().expect("alias"),
            LmeType::AmendAndExtend
        );
    }
    assert_eq!(
        LmeType::OpenMarketRepurchase.to_string(),
        "open_market_repurchase"
    );
    assert!("mystery".parse::<LmeType>().is_err());
}

#[test]
fn enums_round_trip_through_serde_snake_case() {
    let json = serde_json::to_string(&ExchangeType::ParForPar).expect("serialize");
    assert_eq!(json, "\"par_for_par\"");
    let json = serde_json::to_string(&LmeType::AmendAndExtend).expect("serialize");
    assert_eq!(json, "\"amend_and_extend\"");

    let parsed: LmeType = serde_json::from_str("\"open_market_repurchase\"").expect("deserialize");
    assert_eq!(parsed, LmeType::OpenMarketRepurchase);
}
