//! Economic boundary regressions from the margin pipeline audit.

use finstack_quant_core::{
    cashflow::CFKind, currency::Currency, market_data::term_structures::DiscountCurve,
    money::Money, types::CreditRating,
};
use finstack_quant_margin::{
    metrics::MarginUtilization,
    regulatory::{
        frtb::{
            curvature::curvature_charge, delta::delta_charge, drc::drc_charge, vega::vega_charge,
            CorrelationScenario, DrcAssetType, DrcPosition, DrcSector, DrcSeniority, FrtbRiskClass,
            FrtbSensitivities,
        },
        sa_ccr::{
            SaCcrAssetClass, SaCcrEngine, SaCcrNettingSetConfig, SaCcrSupervisoryCategory,
            SaCcrTrade,
        },
    },
    types::generate_margin_cashflows,
    xva::{cva::compute_fva, types::ExposureProfile},
    CollateralAssetClass, CsaSpec, HaircutImCalculator, NettingSetId, RepoMarginSpec,
    ScheduleAssetClass, ScheduleImCalculator, SimmCalculator, SimmSensitivities, SimmVersion,
    VmCalculator,
};

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("finite fixture")
}

#[test]
fn vm_cashflows_follow_desk_direction_on_both_sides() {
    let csa = CsaSpec::usd_regulatory().expect("registry");
    let calc = VmCalculator::new(csa);
    let date = time::Date::from_calendar_date(2025, time::Month::January, 10).expect("date");
    for (exposure, expected) in [
        (1_000_000.0, CFKind::VariationMarginReceive),
        (-1_000_000.0, CFKind::VariationMarginPay),
    ] {
        let calls = calc
            .generate_margin_calls(&[(date, usd(exposure))], usd(0.0))
            .expect("calls");
        let flows = finstack_quant_margin::types::margin_calls_to_cashflows(&calls);
        assert_eq!(flows.len(), 1);
        assert_eq!(flows[0].kind, expected);
        assert!(flows[0].date > date);
    }
}

#[test]
fn negative_exposure_excess_is_returned_with_rounding_down() {
    let mut csa = CsaSpec::usd_regulatory().expect("registry");
    csa.vm_params.mta = usd(0.0);
    csa.vm_params.rounding = usd(10_000.0);
    let amount = csa
        .vm_params
        .calculate_margin_call(usd(-1_000_000.0), usd(-1_155_000.0))
        .expect("call");
    assert_eq!(amount, usd(150_000.0));
}

#[test]
fn csa_rejects_cross_currency_independent_amount_at_calculation() {
    let mut csa = CsaSpec::usd_regulatory().expect("registry");
    csa.vm_params.independent_amount = Money::new(1_000_000.0, Currency::EUR).expect("money");
    assert!(csa.validate().is_err());
    assert!(csa
        .vm_params
        .calculate_margin_call(usd(0.0), usd(0.0))
        .is_err());
}

#[test]
fn utilization_zero_requirement_survives_json_roundtrip() {
    let original = MarginUtilization::new(usd(10.0), usd(0.0)).expect("utilization");
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: MarginUtilization = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, original);
    assert!(restored.is_adequate());
}

#[test]
fn haircut_enforces_maturity_rating_and_contractual_fx_addon() {
    let date = time::Date::from_calendar_date(2025, time::Month::January, 10).expect("date");
    let calc = HaircutImCalculator::us_treasuries().expect("registry");
    assert!(calc
        .calculate_for_collateral(
            usd(1e6),
            &CollateralAssetClass::GovernmentBonds,
            false,
            date
        )
        .is_err());
    let calc = calc
        .with_collateral_terms(10.0, Some(CreditRating::AAA))
        .expect("terms");
    let plain = calc
        .calculate_for_collateral(
            usd(1e6),
            &CollateralAssetClass::GovernmentBonds,
            false,
            date,
        )
        .expect("eligible");
    let fx = calc
        .calculate_for_collateral(usd(1e6), &CollateralAssetClass::GovernmentBonds, true, date)
        .expect("eligible");
    assert_eq!(plain.amount, usd(15_000.0));
    assert_eq!(
        fx.amount, plain.amount,
        "the Treasury schedule elects zero FX add-on"
    );
    assert!(calc
        .calculate_for_collateral(
            usd(-1e6),
            &CollateralAssetClass::GovernmentBonds,
            false,
            date
        )
        .is_err());
    assert!(calc
        .calculate_for_collateral(usd(1e6), &CollateralAssetClass::Equity, false, date)
        .is_err());
}

#[test]
fn heterogeneous_schedule_uses_trade_rates_before_one_ngr() {
    let date = time::Date::from_calendar_date(2025, time::Month::January, 10).expect("date");
    let trades = [
        (usd(100.0), usd(1e6), ScheduleAssetClass::InterestRate, 1.0),
        (usd(-100.0), usd(1e6), ScheduleAssetClass::Equity, 1.0),
    ];
    let result = ScheduleImCalculator::bcbs_standard()
        .expect("registry")
        .calculate_netting_set_with_ngr(&trades, date)
        .expect("IM")
        .expect("nonempty");
    assert_eq!(result.amount, usd((10_000.0 + 150_000.0) * 0.4));
}

#[test]
fn simm_preserves_ir_direction_and_does_not_erase_unrelated_equities() {
    let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry");
    let mut same = SimmSensitivities::new(Currency::USD);
    same.add_ir_delta(Currency::USD, "5Y", 1000.0);
    same.add_ir_delta(Currency::EUR, "5Y", 1000.0);
    let mut opposite = same.clone();
    opposite.add_ir_delta(Currency::EUR, "5Y", -2000.0);
    let positive = calc
        .calculate_from_sensitivities_parts(&same, Currency::USD)
        .expect("IM")
        .0;
    let hedged = calc
        .calculate_from_sensitivities_parts(&opposite, Currency::USD)
        .expect("IM")
        .0;
    assert!(hedged < positive);
    assert!(calc
        .calculate_from_sensitivities_parts(&same, Currency::EUR)
        .is_err());
    let mut equity = SimmSensitivities::new(Currency::USD);
    equity.add_equity_delta("AAPL", 100_000.0);
    equity.add_equity_delta("MSFT", -100_000.0);
    let date = time::Date::from_calendar_date(2025, time::Month::January, 10).expect("date");
    let result = calc
        .calculate_from_sensitivities(&equity, Currency::USD, date)
        .expect("indicative IM");
    assert!(result.amount.amount() > 0.0);
    assert!(result.approximation);
}

#[test]
fn opening_exposure_is_not_halved_in_funding_integral() {
    let date = time::Date::from_calendar_date(2025, time::Month::January, 10).expect("date");
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(date)
        .knots([(0.0, 1.0), (2.0, 1.0)])
        .build()
        .expect("curve");
    for times in [vec![1.0], vec![0.0, 1.0]] {
        let n = times.len();
        let profile = ExposureProfile {
            times,
            mtm_values: vec![1e6; n],
            epe: vec![1e6; n],
            ene: vec![0.0; n],
            diagnostics: None,
        };
        let fva = compute_fva(&profile, &curve, 100.0, 0.0).expect("FVA");
        assert!((fva - 10_000.0).abs() < 1e-8);
    }
}

#[test]
fn drc_respects_subordination_maturity_and_direction_of_offsets() {
    let long = DrcPosition {
        maturity_years: 1.0,
        issuer: "ACME".into(),
        jtd_amount: 1e6,
        rating_bucket: 4,
        sector: DrcSector::Corporate,
        seniority: DrcSeniority::Subordinated,
        asset_type: DrcAssetType::Corporate,
        pnl_adjustment: 0.0,
    };
    assert_eq!(
        drc_charge(std::slice::from_ref(&long)).expect("DRC"),
        60_000.0
    );
    let mut short = long.clone();
    short.jtd_amount = -1e6;
    short.seniority = DrcSeniority::SeniorUnsecured;
    assert!(
        drc_charge(&[long.clone(), short]).expect("DRC") > 0.0,
        "a senior short cannot fully offset a junior long"
    );
    let mut short_dated = long;
    short_dated.maturity_years = 0.1;
    assert_eq!(drc_charge(&[short_dated]).expect("DRC"), 15_000.0);
}

#[test]
fn frtb_equity_small_cap_vega_and_curvature_match_basel() {
    let mut sens = FrtbSensitivities::new(Currency::USD);
    sens.add_equity_vega("SMALL", 9, "1Y", 1e6);
    assert_eq!(
        vega_charge(FrtbRiskClass::Equity, &sens, CorrelationScenario::Medium),
        1e6
    );
    sens.add_equity_curvature("A", 1, 100.0, 0.0);
    sens.add_equity_curvature("B", 1, 100.0, 0.0);
    for (scenario, rho) in [
        (CorrelationScenario::Medium, 0.15_f64.powi(2)),
        (CorrelationScenario::High, 1.25 * 0.15_f64.powi(2)),
    ] {
        let actual = curvature_charge(FrtbRiskClass::Equity, &sens, scenario);
        assert!((actual - (20_000.0 + 20_000.0 * rho).sqrt()).abs() < 1e-9);
    }
}

#[test]
fn saccr_categories_mpor_and_unmargined_cap_match_prescribed_stages() {
    let as_of = time::Date::from_calendar_date(2025, time::Month::January, 1).expect("date");
    let end = time::Date::from_calendar_date(2026, time::Month::January, 1).expect("date");
    let config = SaCcrNettingSetConfig::unmargined(NettingSetId::bilateral("A", "CSA"), 0.0, as_of);
    let mut trade = SaCcrTrade {
        trade_id: "POWER".into(),
        asset_class: SaCcrAssetClass::Commodity,
        supervisory_category: Some(SaCcrSupervisoryCategory::CommodityElectricity),
        option_maturity_date: None,
        notional: 1e6,
        start_date: as_of,
        end_date: end,
        underlier: "POWER_NORTH".into(),
        hedging_set: "ENERGY".into(),
        direction: 1.0,
        supervisory_delta: 1.0,
        mtm: 0.0,
        is_option: false,
        option_type: None,
    };
    let engine = SaCcrEngine::default();
    let power = engine
        .calculate_ead(&config, std::slice::from_ref(&trade))
        .expect("power");
    assert!((power.add_on_aggregate - 400_000.0).abs() < 1e-8);
    trade.asset_class = SaCcrAssetClass::Credit;
    trade.supervisory_category = Some(SaCcrSupervisoryCategory::CreditBbb);
    let credit = engine
        .calculate_ead(&config, std::slice::from_ref(&trade))
        .expect("credit");
    let expected = 1e6 * 0.0054 * (1.0 - (-0.05_f64).exp()) / 0.05;
    assert!((credit.add_on_aggregate - expected).abs() < 1e-8);
    let mut margined =
        SaCcrNettingSetConfig::margined(config.netting_set_id, 0.0, 1e6, 0.0, 0.0, 10, as_of);
    assert_eq!(
        engine.calculate_ead(&margined, &[]).expect("empty").ead,
        0.0
    );
    margined.mpor_days = 1;
    assert!(engine.calculate_ead(&margined, &[trade]).is_err());
}

#[test]
fn frtb_basis_and_equity_repo_keep_distinct_risk_factors() {
    let mut sens = FrtbSensitivities::new(Currency::USD);
    sens.add_csr_nonsec_delta("BANK", 3, "5Y", "bond", 100.0);
    sens.add_csr_nonsec_delta("BANK", 3, "5Y", "cds", -100.0);
    let charge = delta_charge(FrtbRiskClass::CsrNonSec, &sens, CorrelationScenario::Medium);
    assert!((charge - (2.0_f64 * 500.0_f64.powi(2) * (1.0 - 0.999)).sqrt()).abs() < 1e-8);
    sens.add_equity_repo_delta("BANK", 1, 100.0);
    let repo = delta_charge(FrtbRiskClass::Equity, &sens, CorrelationScenario::Medium);
    assert!((repo - 55.0).abs() < 1e-8);
}

#[test]
fn repo_margin_call_settles_after_weekend_and_preserves_call_date() {
    let call_date = time::Date::from_calendar_date(2025, time::Month::January, 10).expect("date");
    let settlement = time::Date::from_calendar_date(2025, time::Month::January, 13).expect("date");
    let mut spec = RepoMarginSpec::mark_to_market(1.02, 0.0).expect("terms");
    spec.settlement_lag = 1;
    let calendar = finstack_quant_core::dates::calendar_by_id("usny").expect("calendar");
    let flows = generate_margin_cashflows(
        &spec,
        usd(1e6),
        &[(call_date, 900_000.0)],
        Currency::USD,
        calendar,
    )
    .expect("flows");
    assert_eq!(flows[0].date, settlement);
}

#[test]
fn schedule_rejects_invalid_maturity_at_every_entry_point() {
    let calc = ScheduleImCalculator::bcbs_standard().expect("registry");
    let date = time::Date::from_calendar_date(2025, time::Month::January, 1).expect("date");
    for maturity in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(calc.clone().with_maturity(maturity).is_err());
        assert!(calc
            .rate(ScheduleAssetClass::InterestRate, maturity)
            .is_err());
        assert!(calc
            .calculate_for_notional(usd(1e6), ScheduleAssetClass::InterestRate, maturity, date)
            .is_err());
    }
}
