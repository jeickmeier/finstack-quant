#![allow(clippy::expect_used, clippy::panic)]

use super::helpers::{date_from_hazard_time, df_asof_to, haz_t, settlement_date, sp_cond_to};
use super::*;
use crate::constants::{credit, ONE_BASIS_POINT};
use crate::instruments::credit_derivatives::cds::{
    CdsValuationConvention, CreditDefaultSwap, PayReceive,
};
use crate::market::conventions::ids::CdsDocClause;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::money::Money;
use rust_decimal::Decimal;
use time::macros::date;
use time::Duration;

fn create_test_cds(
    id: impl Into<String>,
    start_date: Date,
    end_date: Date,
    spread_bp: f64,
    recovery_rate: f64,
) -> CreditDefaultSwap {
    CreditDefaultSwap::new_isda(
        finstack_quant_core::types::InstrumentId::new(id),
        Money::new(10_000_000.0, Currency::USD),
        PayReceive::Pay,
        crate::instruments::credit_derivatives::cds::CDSConvention::IsdaNa,
        Decimal::try_from(spread_bp).expect("valid spread_bp"),
        start_date,
        end_date,
        recovery_rate,
        finstack_quant_core::types::CurveId::new("USD-OIS"),
        finstack_quant_core::types::CurveId::new("TEST-CREDIT"),
    )
    .expect("test CDS creation should succeed")
}

fn create_test_curves() -> (DiscountCurve, HazardCurve) {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date"))
        .knots(vec![(0.0, 1.0), (1.0, 0.95), (5.0, 0.80), (10.0, 0.65)])
        .build()
        .expect("should succeed");

    let credit = HazardCurve::builder("TEST-CREDIT")
        .base_date(Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date"))
        .recovery_rate(0.40)
        .knots(vec![(1.0, 0.02), (3.0, 0.03), (5.0, 0.04), (10.0, 0.05)])
        .par_spreads(vec![
            (1.0, 100.0),
            (3.0, 150.0),
            (5.0, 200.0),
            (10.0, 250.0),
        ])
        .build()
        .expect("should succeed");

    (disc, credit)
}

#[test]
fn test_enhanced_protection_leg() {
    let (disc, credit) = create_test_curves();
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
    let cds = create_test_cds("TEST-CDS", as_of, as_of.add_months(60), 100.0, 0.40);
    let pricer = CDSPricer::new();
    let protection_pv = pricer
        .pv_protection_leg(&cds, &disc, &credit, as_of)
        .expect("should succeed");
    assert!(protection_pv.amount() > 0.0);
}

#[test]
fn test_accrual_on_default() {
    let (disc, credit) = create_test_curves();
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
    let cds = create_test_cds("TEST-CDS", as_of, as_of.add_months(60), 100.0, 0.40);
    let pricer_with = CDSPricer::new();
    let pricer_without = CDSPricer::with_config(CDSPricerConfig {
        include_accrual: false,
        ..Default::default()
    });
    let pv_with = pricer_with
        .pv_premium_leg(&cds, &disc, &credit, as_of)
        .expect("should succeed");
    let pv_without = pricer_without
        .pv_premium_leg(&cds, &disc, &credit, as_of)
        .expect("should succeed");
    assert!(pv_with.amount() > pv_without.amount());
}

#[test]
fn premium_leg_scales_linearly_with_notional_when_accrual_on_default_enabled() {
    let (disc, credit) = create_test_curves();
    let as_of = Date::from_calendar_date(2025, time::Month::February, 15).expect("valid date");
    let pricer = CDSPricer::new();

    let mut cds_unit = create_test_cds(
        "TEST-CDS-UNIT",
        date!(2024 - 12 - 20),
        date!(2028 - 03 - 20),
        100.0,
        0.40,
    );
    cds_unit.notional = Money::new(1.0, Currency::USD);

    let mut cds_large = cds_unit.clone();
    cds_large.id = finstack_quant_core::types::InstrumentId::new("TEST-CDS-LARGE");
    cds_large.notional = Money::new(1_000_000.0, Currency::USD);

    let pv_unit = pricer
        .pv_premium_leg_raw(&cds_unit, &disc, &credit, as_of)
        .expect("unit notional premium leg");
    let pv_large = pricer
        .pv_premium_leg_raw(&cds_large, &disc, &credit, as_of)
        .expect("large notional premium leg");

    let scaled_unit = pv_unit * cds_large.notional.amount();
    assert!(
        (pv_large - scaled_unit).abs() < 1e-8,
        "premium leg PV should scale with notional, unit={pv_unit}, large={pv_large}, scaled_unit={scaled_unit}"
    );
}

#[test]
fn test_par_spread_calculation() {
    let (disc, credit) = create_test_curves();
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
    let cds = create_test_cds("TEST-CDS", as_of, as_of.add_months(60), 0.0, 0.40);
    let pricer = CDSPricer::new();
    let par_spread = pricer
        .par_spread(&cds, &disc, &credit, as_of)
        .expect("should succeed");
    assert!(par_spread > 0.0 && par_spread < 2000.0);
    let mut cds_at_par = cds;
    cds_at_par.premium.spread_bp = Decimal::try_from(par_spread).expect("valid par_spread");
    let npv = pricer
        .npv_full(&cds_at_par, &disc, &credit, as_of)
        .expect("should succeed");
    // A CDS at par spread should have near-zero NPV. Tolerance of $5000
    // (~5bp on $10M) accounts for accrual-on-default and discrete quarterly
    // premium schedule vs. continuous protection leg.
    assert!(
        npv.abs() < 5000.0,
        "CDS at par spread should have near-zero NPV, got {npv}"
    );
}

#[test]
fn test_settlement_delay_reduces_protection_pv() {
    let (disc, credit) = create_test_curves();
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
    let mut cds0 = create_test_cds("CDS-0D", as_of, as_of.add_months(60), 100.0, 0.40);
    let mut cds20 = cds0.clone();
    cds0.protection.settlement_delay = 0;
    cds20.protection.settlement_delay = 20;
    let pricer = CDSPricer::new();
    let pv0 = pricer
        .pv_protection_leg(&cds0, &disc, &credit, as_of)
        .expect("should succeed")
        .amount();
    let pv20 = pricer
        .pv_protection_leg(&cds20, &disc, &credit, as_of)
        .expect("should succeed")
        .amount();
    assert!(pv20 < pv0);
}

#[test]
fn test_par_spread_full_premium_option_runs() {
    let (disc, credit) = create_test_curves();
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
    let cds = create_test_cds("CDS-PAR", as_of, as_of.add_months(60), 0.0, 0.40);
    let pricer_ra = CDSPricer::new();
    let pricer_full = CDSPricer::with_config(CDSPricerConfig {
        par_spread_uses_full_premium: true,
        ..Default::default()
    });
    let s1 = pricer_ra
        .par_spread(&cds, &disc, &credit, as_of)
        .expect("should succeed");
    let s2 = pricer_full
        .par_spread(&cds, &disc, &credit, as_of)
        .expect("should succeed");
    assert!(s1.is_finite() && s2.is_finite());
}

// ─── Restructuring clause / doc_clause tests ───────────────────────

#[test]
fn test_xr14_regression_matches_baseline() {
    // Xr14 (no restructuring) should produce the same output as a CDS without
    // any explicit doc_clause, since the default convention (IsdaNa) maps to Xr14.
    let (disc, credit) = create_test_curves();
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");

    let cds_baseline = create_test_cds("CDS-BASELINE", as_of, as_of.add_months(60), 100.0, 0.40);

    let mut cds_xr14 = create_test_cds("CDS-XR14", as_of, as_of.add_months(60), 100.0, 0.40);
    cds_xr14.doc_clause = Some(CdsDocClause::Xr14);

    let pricer = CDSPricer::new();

    let pv_baseline = pricer
        .pv_protection_leg_raw(&cds_baseline, &disc, &credit, as_of)
        .expect("should succeed");
    let pv_xr14 = pricer
        .pv_protection_leg_raw(&cds_xr14, &disc, &credit, as_of)
        .expect("should succeed");

    // Both should be identical since IsdaNa convention defaults to Xr14
    assert!(
        (pv_baseline - pv_xr14).abs() < 1e-10,
        "Xr14 should match baseline (IsdaNa default). Baseline={}, Xr14={}",
        pv_baseline,
        pv_xr14,
    );
}

#[test]
fn test_doc_clause_does_not_affect_protection_pv() {
    // The pricer no longer carries a restructuring-uplift heuristic; protection
    // PV is identical for all clauses on the same hazard curve. (A future
    // calibrated restructuring-hazard model would split the hazard curve, not
    // multiply by a per-clause scalar.)
    let (disc, credit) = create_test_curves();
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");

    let clauses = [
        CdsDocClause::Xr14,
        CdsDocClause::Mr14,
        CdsDocClause::Mm14,
        CdsDocClause::Cr14,
    ];

    let pricer = CDSPricer::new();
    let mut pvs = Vec::new();
    for clause in &clauses {
        let mut cds = create_test_cds("CDS-TEST", as_of, as_of.add_months(60), 100.0, 0.40);
        cds.doc_clause = Some(*clause);
        pvs.push(
            pricer
                .pv_protection_leg_raw(&cds, &disc, &credit, as_of)
                .expect("should succeed"),
        );
    }

    for pv in &pvs[1..] {
        assert!((pv - pvs[0]).abs() < 1e-10);
    }
}

#[test]
fn test_doc_clause_effective_defaults() {
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");

    // No explicit doc_clause with IsdaNa convention -> Xr14
    let cds_na = create_test_cds("CDS-NA", as_of, as_of.add_months(60), 100.0, 0.40);
    assert_eq!(cds_na.doc_clause_effective(), CdsDocClause::Xr14);

    // Explicit Cr14 should override convention default
    let mut cds_cr14 = create_test_cds("CDS-CR14", as_of, as_of.add_months(60), 100.0, 0.40);
    cds_cr14.doc_clause = Some(CdsDocClause::Cr14);
    assert_eq!(cds_cr14.doc_clause_effective(), CdsDocClause::Cr14);

    // Meta-clause IsdaEu should resolve to Mm14
    let mut cds_eu = create_test_cds("CDS-EU", as_of, as_of.add_months(60), 100.0, 0.40);
    cds_eu.doc_clause = Some(CdsDocClause::IsdaEu);
    assert_eq!(cds_eu.doc_clause_effective(), CdsDocClause::Mm14);
}

#[test]
fn test_doc_clause_serde_roundtrip() {
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");

    // With doc_clause set
    let mut cds_with = create_test_cds("CDS-SERDE", as_of, as_of.add_months(60), 100.0, 0.40);
    cds_with.doc_clause = Some(CdsDocClause::Cr14);
    let json = serde_json::to_string(&cds_with).expect("serialize should succeed");
    assert!(
        json.contains("doc_clause"),
        "JSON should contain doc_clause field"
    );
    let deser: CreditDefaultSwap = serde_json::from_str(&json).expect("deserialize should succeed");
    assert_eq!(deser.doc_clause, Some(CdsDocClause::Cr14));

    // Without doc_clause (None) - should not appear in JSON (skip_serializing_if)
    let cds_without = create_test_cds("CDS-SERDE-NONE", as_of, as_of.add_months(60), 100.0, 0.40);
    let json_without = serde_json::to_string(&cds_without).expect("serialize should succeed");
    assert!(
        !json_without.contains("doc_clause"),
        "JSON should NOT contain doc_clause when None"
    );
    let deser_without: CreditDefaultSwap =
        serde_json::from_str(&json_without).expect("deserialize should succeed");
    assert_eq!(deser_without.doc_clause, None);
}

#[test]
fn test_doc_clause_default_when_omitted() {
    // Existing construction without doc_clause should still work
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
    let cds = create_test_cds("CDS-COMPAT", as_of, as_of.add_months(60), 100.0, 0.40);
    assert_eq!(cds.doc_clause, None);

    // Builder pattern should also work without doc_clause
    let cds_built = CreditDefaultSwap::builder()
        .id(finstack_quant_core::types::InstrumentId::new("CDS-BUILDER"))
        .notional(Money::new(10_000_000.0, Currency::USD))
        .side(PayReceive::Pay)
        .convention(crate::instruments::credit_derivatives::cds::CDSConvention::IsdaNa)
        .premium(
            crate::instruments::common_impl::parameters::legs::PremiumLegSpec {
                start: as_of,
                end: as_of.add_months(60),
                frequency: finstack_quant_core::dates::Tenor::quarterly(),
                stub: finstack_quant_core::dates::StubKind::ShortFront,
                bdc: finstack_quant_core::dates::BusinessDayConvention::ModifiedFollowing,
                calendar_id: Some("nyse".to_string()),
                day_count: finstack_quant_core::dates::DayCount::Act360,
                spread_bp: Decimal::try_from(100.0).expect("valid"),
                discount_curve_id: finstack_quant_core::types::CurveId::new("USD-OIS"),
            },
        )
        .protection(
            crate::instruments::common_impl::parameters::legs::ProtectionLegSpec {
                credit_curve_id: finstack_quant_core::types::CurveId::new("TEST-CREDIT"),
                recovery_rate: 0.40,
                settlement_delay: 3,
            },
        )
        .build()
        .expect("builder should succeed without doc_clause");
    assert_eq!(cds_built.doc_clause, None);
    assert_eq!(cds_built.doc_clause_effective(), CdsDocClause::Xr14);
}

#[test]
fn test_doc_clause_serde_deserializes_without_field() {
    // Simulate old serialized data by serializing a CDS, stripping the
    // doc_clause field from JSON, and verifying it still deserializes.
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
    let cds = create_test_cds("CDS-OLD", as_of, as_of.add_months(60), 100.0, 0.40);
    let json = serde_json::to_string(&cds).expect("serialize should succeed");

    // The JSON should not contain "doc_clause" since it is None
    assert!(
        !json.contains("doc_clause"),
        "Baseline CDS JSON should not contain doc_clause"
    );

    // Deserialize and verify omitted fields use defaults
    let deser: CreditDefaultSwap =
        serde_json::from_str(&json).expect("Should deserialize old JSON without doc_clause field");
    assert_eq!(deser.doc_clause, None);
    assert_eq!(deser.doc_clause_effective(), CdsDocClause::Xr14);
}

#[test]
fn test_schedule_generation_respects_isda_flag_and_calendar_availability() {
    let start = Date::from_calendar_date(2025, time::Month::July, 1).expect("valid date");
    let end = Date::from_calendar_date(2026, time::Month::July, 1).expect("valid date");
    let cds = create_test_cds("CDS-SCHED", start, end, 100.0, 0.40);

    let isda = CDSPricer::new();
    let adjusted_schedule = isda
        .generate_isda_schedule(&cds)
        .expect("adjusted ISDA schedule");

    let mut cds_no_calendar = cds;
    cds_no_calendar.premium.calendar_id = None;
    let unadjusted_schedule = isda
        .generate_isda_schedule(&cds_no_calendar)
        .expect("unadjusted ISDA schedule");

    let sep_20 = Date::from_calendar_date(2025, time::Month::September, 20).expect("valid date");
    let sep_22 = Date::from_calendar_date(2025, time::Month::September, 22).expect("valid date");
    assert!(
        unadjusted_schedule.contains(&sep_20),
        "calendar-less ISDA schedule should keep weekend IMM dates"
    );
    assert!(
        adjusted_schedule.contains(&sep_22),
        "calendar-aware ISDA schedule should adjust weekend IMM dates"
    );
    assert!(
        !adjusted_schedule.contains(&sep_20),
        "calendar-aware ISDA schedule should not keep the unadjusted weekend date"
    );
}

#[test]
fn test_premium_leg_per_bp_matches_risky_annuity_without_accrual_on_default() {
    let (disc, credit) = create_test_curves();
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
    let cds = create_test_cds("CDS-PER-BP", as_of, as_of.add_months(60), 100.0, 0.40);

    let without_aod = CDSPricer::with_config(CDSPricerConfig {
        include_accrual: false,
        ..Default::default()
    });
    let risky_annuity = without_aod
        .risky_annuity(&cds, &disc, &credit, as_of)
        .expect("risky annuity");
    let per_bp_without_aod = without_aod
        .premium_leg_pv_per_bp(&cds, &disc, &credit, as_of)
        .expect("premium leg per bp");
    assert!(
        (per_bp_without_aod - risky_annuity * ONE_BASIS_POINT).abs() < 1e-14,
        "premium leg per bp without AoD should equal risky annuity × 1bp"
    );

    let with_aod = CDSPricer::new();
    let per_bp_with_aod = with_aod
        .premium_leg_pv_per_bp(&cds, &disc, &credit, as_of)
        .expect("premium leg per bp with AoD");
    assert!(
        per_bp_with_aod > per_bp_without_aod,
        "including AoD should increase premium leg PV per bp"
    );
}

#[test]
fn test_full_premium_par_spread_is_below_risky_annuity_par_spread() {
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(vec![(0.0, 1.0), (1.0, 0.975), (5.0, 0.86), (10.0, 0.72)])
        .build()
        .expect("discount curve");
    let credit = HazardCurve::builder("TEST-CREDIT")
        .base_date(as_of)
        .recovery_rate(0.40)
        .knots(vec![(0.25, 0.08), (1.0, 0.12), (3.0, 0.16), (5.0, 0.20)])
        .build()
        .expect("hazard curve");
    let mut cds = create_test_cds("CDS-PAR-FULL", as_of, as_of.add_months(60), 100.0, 0.40);
    cds.valuation_convention = CdsValuationConvention::IsdaDirty;

    let isda = CDSPricer::new();
    let full_premium = CDSPricer::with_config(CDSPricerConfig {
        par_spread_uses_full_premium: true,
        ..Default::default()
    });

    let isda_spread = isda
        .par_spread(&cds, &disc, &credit, as_of)
        .expect("ISDA par spread");
    let full_spread = full_premium
        .par_spread(&cds, &disc, &credit, as_of)
        .expect("full-premium par spread");

    assert!(isda_spread.is_finite() && full_spread.is_finite());
    assert!(
        full_spread < isda_spread,
        "including AoD in the denominator should reduce the par spread"
    );
}

#[test]
fn test_npv_full_combines_dated_and_market_quote_upfronts() {
    use crate::instruments::common_impl::traits::Instrument;

    let (disc, credit) = create_test_curves();
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
    let mut cds = create_test_cds("CDS-UPFRONT", as_of, as_of.add_months(60), 100.0, 0.40);
    cds.valuation_convention = CdsValuationConvention::IsdaDirty;
    let pricer = CDSPricer::new();

    let base_npv = pricer
        .npv_full(&cds, &disc, &credit, as_of)
        .expect("base npv");

    let dated_upfront_date = as_of.add_months(6);
    let dated_upfront_amount = 150_000.0;
    let quote_adjustment = Money::new(25_000.0, Currency::USD);
    cds.upfront = Some((
        dated_upfront_date,
        Money::new(dated_upfront_amount, Currency::USD),
    ));
    cds.instrument_pricing_overrides
        .market_quotes
        .upfront_payment = Some(quote_adjustment);

    let dated_df = disc
        .df_between_dates(as_of, dated_upfront_date)
        .expect("discount factor");
    let expected = base_npv - dated_upfront_amount * dated_df - quote_adjustment.amount();
    let npv_with_upfront = pricer
        .npv_full(&cds, &disc, &credit, as_of)
        .expect("npv with upfront");
    assert!(
        (npv_with_upfront - expected).abs() < 1e-8,
        "dated upfront and direct PV adjustment should combine additively"
    );

    let market = MarketContext::new().insert(disc).insert(credit);
    let npv_via_value_raw = cds.value_raw(&market, as_of).expect("value_raw npv");
    assert!(
        (npv_via_value_raw - npv_with_upfront).abs() < 1e-12,
        "Instrument::value_raw should match the direct-curve pricer.npv_full"
    );
}

#[test]
fn test_time_and_settlement_helpers_match_curve_and_calendar_conventions() {
    let (disc, credit) = create_test_curves();
    let base_date = disc.base_date();
    let one_year = base_date.add_months(12);

    let expected_haz_t = credit
        .day_count()
        .year_fraction(
            credit.base_date(),
            one_year,
            finstack_quant_core::dates::DayCountContext::default(),
        )
        .expect("hazard year fraction");
    assert!(
        (haz_t(&credit, one_year).expect("haz_t") - expected_haz_t).abs() < 1e-12,
        "haz_t should respect the hazard curve day-count"
    );

    assert_eq!(
        date_from_hazard_time(&credit, -1.0),
        credit.base_date(),
        "negative hazard times should clamp to the curve base date"
    );
    let days_per_year: f64 = match credit.day_count() {
        DayCount::Act365F => 365.0,
        DayCount::Act360 | DayCount::Thirty360 | DayCount::ThirtyE360 => 360.0,
        DayCount::Bus252 => 252.0,
        _ => 365.25,
    };
    let hazard_time = 1.25_f64;
    let expected_date =
        credit.base_date() + Duration::days((hazard_time * days_per_year).round() as i64);
    assert_eq!(date_from_hazard_time(&credit, hazard_time), expected_date);

    // 3 business days at 252 bdays/year ≈ 4.35 calendar days → rounds to 4.
    // base_date is 2025-01-01 (Wednesday), so +4 calendar days lands on
    // Sunday 2025-01-05; the fallback must then roll forward to the next
    // weekday — Monday 2025-01-06 — because a settlement date is by
    // definition a business day.
    let fallback_settlement = settlement_date(base_date, 3, None, 252.0).expect("fallback");
    assert!(
        !fallback_settlement.is_weekend(),
        "fallback settlement must not land on a weekend, got {fallback_settlement:?}"
    );
    let raw = base_date + Duration::days(4);
    let expected = if raw.is_weekend() {
        let mut d = raw;
        while d.is_weekend() {
            d += Duration::days(1);
        }
        d
    } else {
        raw
    };
    assert_eq!(
        fallback_settlement, expected,
        "fallback settlement = 4 calendar days rolled forward off weekends"
    );

    let nyse =
        finstack_quant_core::dates::fx::resolve_calendar(Some("nyse")).expect("nyse calendar");
    let friday =
        Date::from_calendar_date(2025, time::Month::January, 3).expect("valid Friday date");
    let monday =
        Date::from_calendar_date(2025, time::Month::January, 6).expect("valid Monday date");
    assert_eq!(
        settlement_date(friday, 1, Some(nyse.as_holiday_calendar()), 252.0)
            .expect("calendar settlement"),
        monday,
        "calendar-aware settlement should advance by business days"
    );
}

#[test]
fn test_discount_survival_and_default_density_helpers_cover_boundary_cases() {
    let (disc, credit) = create_test_curves();
    let as_of = disc.base_date();
    let one_year = as_of.add_months(12);

    assert_eq!(
        df_asof_to(&disc, as_of, one_year).expect("df"),
        disc.df_between_dates(as_of, one_year)
            .expect("df between dates")
    );

    let t_asof = haz_t(&credit, as_of).expect("haz_t as_of");
    let t_one_year = haz_t(&credit, one_year).expect("haz_t future");
    let expected_conditional_survival = credit.sp(t_one_year) / credit.sp(t_asof);
    assert!(
        (sp_cond_to(&credit, as_of, one_year).expect("conditional survival")
            - expected_conditional_survival)
            .abs()
            < 1e-12
    );

    let mut late_as_of = as_of;
    while credit.sp(haz_t(&credit, late_as_of).expect("haz_t late"))
        > credit::SURVIVAL_PROBABILITY_FLOOR
    {
        late_as_of = late_as_of.add_months(600);
    }
    assert_eq!(
        sp_cond_to(&credit, late_as_of, late_as_of.add_months(12))
            .expect("conditional survival after effective default"),
        0.0,
        "conditional survival should floor to zero after effective default"
    );
}

// ── Forward-starting CDS tests ──────────────────────────────────────

/// Helper: create a forward-starting CDS with a specified protection effective date.
fn create_forward_start_cds(
    id: impl Into<String>,
    start_date: Date,
    end_date: Date,
    spread_bp: f64,
    recovery_rate: f64,
    protection_effective_date: Option<Date>,
) -> CreditDefaultSwap {
    let mut cds = create_test_cds(id, start_date, end_date, spread_bp, recovery_rate);
    cds.protection_effective_date = protection_effective_date;
    cds.validate().expect("forward-start CDS should validate");
    cds
}

#[test]
fn test_forward_start_none_matches_spot_cds() {
    let (disc, credit) = create_test_curves();
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
    let end = as_of.add_months(60);

    let spot_cds = create_test_cds("CDS-SPOT", as_of, end, 100.0, 0.40);
    let fwd_none = create_forward_start_cds("CDS-FWD-NONE", as_of, end, 100.0, 0.40, None);

    let pricer = CDSPricer::new();

    let spot_prot = pricer
        .pv_protection_leg_raw(&spot_cds, &disc, &credit, as_of)
        .expect("should succeed");
    let fwd_prot = pricer
        .pv_protection_leg_raw(&fwd_none, &disc, &credit, as_of)
        .expect("should succeed");

    let spot_prem = pricer
        .pv_premium_leg_raw(&spot_cds, &disc, &credit, as_of)
        .expect("should succeed");
    let fwd_prem = pricer
        .pv_premium_leg_raw(&fwd_none, &disc, &credit, as_of)
        .expect("should succeed");

    assert!(
        (spot_prot - fwd_prot).abs() < 1e-10,
        "None protection_effective_date should match spot: spot={spot_prot}, fwd={fwd_prot}",
    );
    assert!(
        (spot_prem - fwd_prem).abs() < 1e-10,
        "None protection_effective_date should match spot premium: spot={spot_prem}, fwd={fwd_prem}",
    );
}

#[test]
fn test_forward_start_lower_protection_pv_same_premium_pv() {
    let (disc, credit) = create_test_curves();
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
    let end = as_of.add_months(60);
    let fwd_date = as_of.add_months(24);

    let spot_cds = create_test_cds("CDS-SPOT", as_of, end, 100.0, 0.40);
    let fwd_cds = create_forward_start_cds("CDS-FWD", as_of, end, 100.0, 0.40, Some(fwd_date));

    let pricer = CDSPricer::new();

    let spot_prot = pricer
        .pv_protection_leg_raw(&spot_cds, &disc, &credit, as_of)
        .expect("should succeed");
    let fwd_prot = pricer
        .pv_protection_leg_raw(&fwd_cds, &disc, &credit, as_of)
        .expect("should succeed");

    assert!(
        fwd_prot < spot_prot,
        "Forward-start protection PV ({fwd_prot}) should be less than spot ({spot_prot})",
    );
    assert!(
        fwd_prot > 0.0,
        "Forward-start protection PV should still be positive"
    );

    let spot_prem = pricer
        .pv_premium_leg_raw(&spot_cds, &disc, &credit, as_of)
        .expect("should succeed");
    let fwd_prem = pricer
        .pv_premium_leg_raw(&fwd_cds, &disc, &credit, as_of)
        .expect("should succeed");

    assert!(
        (spot_prem - fwd_prem).abs() < 1e-10,
        "Premium leg should be identical: spot={spot_prem}, fwd={fwd_prem}",
    );
}

#[test]
fn test_forward_start_protection_at_end_near_zero() {
    let (disc, credit) = create_test_curves();
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
    let end = as_of.add_months(60);

    let fwd_cds = create_forward_start_cds("CDS-FWD-END", as_of, end, 100.0, 0.40, Some(end));

    let pricer = CDSPricer::new();
    let prot_pv = pricer
        .pv_protection_leg_raw(&fwd_cds, &disc, &credit, as_of)
        .expect("should succeed");

    assert!(
        prot_pv.abs() < 1.0,
        "Protection PV should be ~0 when effective_date = end, got {prot_pv}",
    );
}

#[test]
fn test_forward_start_invalid_before_premium_start() {
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
    let end = as_of.add_months(60);
    let before_start = Date::from_calendar_date(2024, time::Month::June, 1).expect("valid date");

    let mut cds = create_test_cds("CDS-BAD", as_of, end, 100.0, 0.40);
    cds.protection_effective_date = Some(before_start);
    let result = cds.validate();
    assert!(
        result.is_err(),
        "protection_effective_date before premium start should fail"
    );
}

#[test]
fn test_forward_start_invalid_after_premium_end() {
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
    let end = as_of.add_months(60);
    let after_end = end.add_months(12);

    let mut cds = create_test_cds("CDS-BAD", as_of, end, 100.0, 0.40);
    cds.protection_effective_date = Some(after_end);
    let result = cds.validate();
    assert!(
        result.is_err(),
        "protection_effective_date after premium end should fail"
    );
}

#[test]
fn test_protection_start_helper() {
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
    let end = as_of.add_months(60);
    let fwd_date = as_of.add_months(24);

    let spot = create_test_cds("CDS-SPOT", as_of, end, 100.0, 0.40);
    assert_eq!(spot.protection_start(), as_of);

    let mut fwd = spot;
    fwd.protection_effective_date = Some(fwd_date);
    assert_eq!(fwd.protection_start(), fwd_date);
}
