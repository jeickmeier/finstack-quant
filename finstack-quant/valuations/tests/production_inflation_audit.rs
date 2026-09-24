//! Inflation reference observations and market-convention production regressions.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{calendar_by_id, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{
    InflationIndex, InflationInterpolation, InflationLag,
};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, InflationCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::{InflationLinkedBond, InflationSwap, Repo};
use finstack_quant_valuations::market::conventions::{
    ids::{CdsConventionKey, CdsDocClause, InflationSwapConventionId, XccyConventionId},
    ConventionRegistry,
};
use rust_decimal::Decimal;
use time::macros::date;

#[test]
fn b17_fixed_inflation_leg_compounds_one_per_annual_period() {
    let as_of = date!(2024 - 01 - 15);
    let market = MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (40.0, 1.0)])
            .build()
            .expect("discount"),
    );
    let mut swap = InflationSwap::example();
    swap.day_count = ConventionRegistry::try_global()
        .expect("registry")
        .require_inflation_swap(&InflationSwapConventionId::new("USD-CPI"))
        .expect("USD inflation")
        .day_count;
    for (maturity, years) in [(date!(2029 - 01 - 15), 5), (date!(2054 - 01 - 15), 30)] {
        swap.maturity = maturity;
        let actual = swap
            .pv_fixed_leg(&market, as_of)
            .expect("fixed PV")
            .amount();
        let expected = 1_000_000.0 * (1.02_f64.powi(years) - 1.0);
        assert!(
            (actual - expected).abs() < 1e-7,
            "{years}y: {actual} vs {expected}"
        );
    }
}

#[test]
fn b17_month_end_cpi_has_the_same_reference_month_in_index_and_hybrid_sources() {
    let mut bond = InflationLinkedBond::example();
    bond.base_index = 300.0;
    bond.lag = InflationLag::Months(3);
    let index = InflationIndex::new(
        "US-CPI",
        vec![
            (date!(2024 - 10 - 31), 300.0),
            (date!(2024 - 11 - 30), 303.0),
        ],
        Currency::USD,
    )
    .expect("published monthly CPI")
    .with_interpolation(InflationInterpolation::Linear);
    bond.inflation_index_id = "US-CPI".into();
    let historical = MarketContext::new().insert_inflation_index("US-CPI", index);
    let hybrid = historical.clone().insert(
        InflationCurve::builder("US-CPI")
            .base_date(date!(2024 - 11 - 01))
            .base_cpi(303.0)
            .knots([(0.0, 303.0), (1.0, 310.0)])
            .build()
            .expect("projection"),
    );
    let expected = (300.0 + 3.0 * 15.0 / 31.0) / 300.0;
    for market in [&historical, &hybrid] {
        let actual = bond
            .index_ratio_from_market(date!(2025 - 01 - 16), market)
            .expect("reference CPI");
        assert!((actual - expected).abs() < 1e-12, "{actual} vs {expected}");
    }
}

#[test]
fn m20_historical_inflation_requires_an_observation() {
    let as_of = date!(2025 - 01 - 15);
    let market = MarketContext::new()
        .insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (10.0, 1.0)])
                .build()
                .expect("discount"),
        )
        .insert(
            InflationCurve::builder("US-CPI")
                .base_date(as_of)
                .base_cpi(300.0)
                .knots([(0.0, 300.0), (10.0, 360.0)])
                .build()
                .expect("projection"),
        );
    let swap = InflationSwap::example();
    assert!(
        swap.pv_inflation_leg(&market, as_of).is_err(),
        "a past start fixing must not be replaced with the curve's base CPI"
    );
}

#[test]
fn m19_real_duration_requires_the_same_clean_quote_as_real_yield() {
    let mut bond = InflationLinkedBond::example();
    bond.quoted_clean = None;
    assert!(bond.real_duration(date!(2025 - 01 - 15)).is_err());
}

#[test]
fn b20_asian_cds_premiums_use_actual_360() {
    let registry = ConventionRegistry::try_global().expect("registry");
    for currency in [
        Currency::JPY,
        Currency::KRW,
        Currency::AUD,
        Currency::NZD,
        Currency::HKD,
        Currency::SGD,
    ] {
        let convention = registry
            .resolve_cds(&CdsConventionKey {
                currency,
                doc_clause: CdsDocClause::IsdaAs,
            })
            .expect("Asian CDS");
        assert_eq!(convention.day_count, DayCount::Act360, "{currency}");
    }
}

#[test]
fn b20_eurusd_mtm_convention_resets_the_usd_leg() {
    use finstack_quant_valuations::instruments::rates::xccy_swap::{
        NotionalExchange, ResettingSide,
    };
    let convention = ConventionRegistry::try_global()
        .expect("registry")
        .require_xccy(&XccyConventionId::new("EUR/USD-XCCY"))
        .expect("EUR/USD");
    assert_eq!(
        convention.notional_exchange,
        NotionalExchange::MtmResetting {
            resetting_side: ResettingSide::Leg2
        }
    );
}

#[test]
fn m21_uk_inflation_uses_a_registered_london_calendar() {
    let convention = ConventionRegistry::try_global()
        .expect("registry")
        .require_inflation_swap(&InflationSwapConventionId::new("UK-RPI"))
        .expect("RPI");
    assert!(
        calendar_by_id(&convention.calendar_id).is_some(),
        "{}",
        convention.calendar_id
    );
}

#[test]
fn m23_repo_conveniences_reject_currencies_without_explicit_defaults() {
    use finstack_quant_valuations::instruments::rates::repo::CollateralSpec;
    let collateral = CollateralSpec::new("GILT", 100.0, "GILT-PRICE");
    let as_of = date!(2025 - 01 - 15);
    assert!(Repo::term(
        "GBP-REPO",
        Money::from((100_000_i64, Currency::GBP)),
        collateral.clone(),
        0.05,
        as_of,
        date!(2025 - 01 - 22),
        "GBP-OIS"
    )
    .is_err());
    // Explicit contract terms remain available for the unsupported preset.
    let repo = Repo::builder()
        .id("GBP-EXPLICIT".into())
        .cash_amount(Money::from((100_000_i64, Currency::GBP)))
        .collateral(collateral)
        .repo_rate(Decimal::new(5, 2))
        .start_date(as_of)
        .maturity(date!(2025 - 01 - 22))
        .day_count(DayCount::Act365F)
        .haircut(0.02)
        .repo_type(finstack_quant_valuations::instruments::rates::repo::RepoType::Term)
        .triparty(false)
        .business_day_convention(finstack_quant_core::dates::BusinessDayConvention::Following)
        .discount_curve_id("GBP-OIS".into())
        .calendar_id_opt(Some("gblo".into()))
        .build();
    assert!(repo.is_ok(), "{repo:?}");
}

#[test]
fn m22_ois_conveniences_match_the_canonical_registry() {
    use finstack_quant_valuations::instruments::IRSConvention;
    for (convention, expected) in [
        (IRSConvention::UsdSofr, DayCount::Act360),
        (IRSConvention::EurEstr, DayCount::Act360),
        (IRSConvention::JpyTonar, DayCount::Act365F),
    ] {
        assert_eq!(convention.fixed_day_count().expect("registry"), expected);
        assert_eq!(
            convention.fixed_frequency().expect("registry"),
            Tenor::annual()
        );
    }
}

#[test]
fn b17_projected_inflation_uses_contract_month_interpolation_weights() {
    use finstack_quant_core::dates::DayCountContext;
    let as_of = date!(2025 - 01 - 15);
    let origin = date!(2024 - 10 - 01);
    let time = |date| {
        DayCount::Act365F
            .year_fraction(origin, date, DayCountContext::default())
            .expect("clock")
    };
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
                .base_date(origin)
                .base_cpi(300.0)
                .knots([
                    (0.0, 300.0),
                    (time(date!(2025 - 11 - 01)), 303.0),
                    (time(date!(2025 - 12 - 01)), 306.0),
                ])
                .build()
                .expect("projection"),
        )
        .insert_inflation_index(
            "US-CPI",
            InflationIndex::new(
                "US-CPI",
                vec![(date!(2024 - 10 - 31), 300.0)],
                Currency::USD,
            )
            .expect("history")
            .with_interpolation(InflationInterpolation::Linear)
            .with_lag(InflationLag::Months(3)),
        );
    let mut swap = InflationSwap::example();
    swap.start_date = as_of;
    swap.maturity = date!(2026 - 02 - 16);
    swap.base_cpi = Some(300.0);
    swap.lag_override = Some(InflationLag::Months(3));
    let actual = swap
        .pv_inflation_leg(&market, as_of)
        .expect("inflation PV")
        .amount();
    let expected = swap.notional.amount() * ((303.0 + 3.0 * 15.0 / 28.0) / 300.0 - 1.0);
    assert!((actual - expected).abs() < 1e-7, "{actual} vs {expected}");
}
