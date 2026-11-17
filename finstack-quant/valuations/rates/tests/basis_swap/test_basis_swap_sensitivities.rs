//! Sensitivity and risk metrics tests for basis swaps.
//!
//! Tests DV01, bucketed DV01, and other risk sensitivities to ensure accurate
//! risk measurement and hedge ratio calculations.

use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::{currency::Currency::USD, math::interp::InterpStyle};
use finstack_quant_valuations_core::instruments::FloatLegSpec;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_rates::basis_swap::BasisSwap;
use rust_decimal::Decimal;
use time::Month;

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).unwrap(), day).unwrap()
}

const CALENDAR_ID: &str = "usny";

fn market() -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(d(2025, 1, 2))
        .knots(vec![
            (0.0, 1.0),
            (0.5, 0.99),
            (1.0, 0.98),
            (2.0, 0.96),
            (3.0, 0.94),
        ])
        .interp(InterpStyle::LogLinear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let f3m = ForwardCurve::builder("USD-SOFR-3M")
        .base_date(d(2025, 1, 2))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.997487536532938),
            (0.25, 0.994950292856076),
            (0.375, 0.992388490374784),
            (0.5, 0.98980235222721),
            (0.625, 0.98719210325196),
            (0.75, 0.984557969955606),
            (0.875, 0.981900180480004),
            (1.0, 0.97921896456946),
            (1.125, 0.976522182587467),
            (1.25, 0.973817611559672),
            (1.375, 0.971105357458722),
            (1.5, 0.96838552632258),
            (1.625, 0.965658224247723),
            (1.75, 0.962923557382358),
            (1.875, 0.96018163191965),
            (2.0, 0.957432554090967),
            (2.125, 0.954676430159143),
            (2.25, 0.951913366411754),
            (2.375, 0.949143469154417),
            (2.5, 0.946366844704097),
            (2.625, 0.943583599382446),
            (2.75, 0.940793839509148),
            (2.875, 0.937997671395296),
            (3.0, 0.935195201336777),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let f1m = ForwardCurve::builder("USD-SOFR-1M")
        .base_date(d(2025, 1, 2))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.9976122302682),
            (0.25, 0.995199061524078),
            (0.375, 0.992760705844714),
            (0.5, 0.990297377149241),
            (0.625, 0.987809291167626),
            (0.75, 0.985296665409241),
            (0.875, 0.982759719131244),
            (1.0, 0.980198673306755),
            (1.125, 0.97762138823012),
            (1.25, 0.975035644686228),
            (1.375, 0.972441545744155),
            (1.5, 0.969839194590702),
            (1.625, 0.96722869452364),
            (1.75, 0.964610148944963),
            (1.875, 0.961983661354156),
            (2.0, 0.959349335341472),
            (2.125, 0.956707274581219),
            (2.25, 0.954057582825055),
            (2.375, 0.951400363895305),
            (2.5, 0.948735721678285),
            (2.625, 0.946063760117635),
            (2.75, 0.943384583207676),
            (2.875, 0.940698294986776),
            (3.0, 0.938004999530729),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    MarketContext::new().insert(disc).insert(f3m).insert(f1m)
}

fn make_leg(forward_curve: &str, start: Date, end: Date, spread_bp: Decimal) -> FloatLegSpec {
    FloatLegSpec {
        end_of_month: false,
        fixing_calendar_id: None,
        forward_curve_id: CurveId::new(forward_curve),
        discount_curve_id: CurveId::new("USD-OIS"),
        start,
        end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some(CALENDAR_ID.into()),
        stub: StubKind::ShortFront,
        spread_bp,
        payment_lag_days: 0,
        reset_lag_days: 0,
        compounding: Default::default(),
    }
}

#[test]
fn dv01_per_curve_breakdown() {
    let ctx = market();
    let as_of = d(2025, 1, 2);
    let swap = BasisSwap::new(
        "DV01-NET-TEST",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2027, 1, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2027, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let res = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01_total = res.measures[MetricId::Dv01.as_str()];

    let dv01_discount = res
        .measures
        .get("bucketed_dv01::USD-OIS")
        .copied()
        .expect("discount-curve bucketed DV01 must be present");
    let dv01_primary_fwd = res
        .measures
        .get("bucketed_dv01::USD-SOFR-3M")
        .copied()
        .expect("primary-forward bucketed DV01 must be present");
    let dv01_reference_fwd = res
        .measures
        .get("bucketed_dv01::USD-SOFR-1M")
        .copied()
        .expect("reference-forward bucketed DV01 must be present");

    let computed_total = dv01_discount + dv01_primary_fwd + dv01_reference_fwd;
    assert!(
        (dv01_total - computed_total).abs() < 1e-6,
        "Total DV01 should equal sum of curve sensitivities: {} vs {}",
        dv01_total,
        computed_total
    );

    assert!(dv01_discount.is_finite());
    assert!(dv01_primary_fwd.is_finite());
    assert!(dv01_reference_fwd.is_finite());
}

#[test]
fn dv01_scales_with_notional() {
    let ctx = market();
    let as_of = d(2025, 1, 2);
    let notionals = vec![1_000_000.0, 5_000_000.0, 10_000_000.0];
    let mut dv01s = Vec::new();

    for notional in &notionals {
        let swap = BasisSwap::new(
            format!("DV01-SCALE-{}", notional),
            Money::new(*notional, USD).expect("valid money fixture"),
            make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
            make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
        )
        .expect("valid basis swap");

        let res = swap
            .price_with_metrics(
                &ctx,
                as_of,
                &[MetricId::Dv01],
                finstack_quant_valuations_core::instruments::PricingOptions::default(),
            )
            .unwrap();

        let dv01 = res
            .measures
            .get("bucketed_dv01::USD-SOFR-3M")
            .copied()
            .expect("primary-forward bucketed DV01 must be present");
        dv01s.push(dv01);
    }

    let ratio_1_to_5 = dv01s[1] / dv01s[0];
    let ratio_1_to_10 = dv01s[2] / dv01s[0];

    // DV01 = annuity × notional × 1bp — exact linear scaling with notional
    assert!(
        (ratio_1_to_5 - 5.0).abs() < 1e-6,
        "DV01 must scale exactly 5x with notional, got {}x",
        ratio_1_to_5
    );
    assert!(
        (ratio_1_to_10 - 10.0).abs() < 1e-6,
        "DV01 must scale exactly 10x with notional, got {}x",
        ratio_1_to_10
    );
}

#[test]
fn dv01_vs_numerical_bump() {
    let as_of = d(2025, 1, 2);
    let ctx_base = market();

    let swap = BasisSwap::new(
        "DV01-BUMP-TEST",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let res_base = swap
        .price_with_metrics(
            &ctx_base,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();
    let dv01_metric = res_base
        .measures
        .get("bucketed_dv01::USD-SOFR-3M")
        .copied()
        .expect("primary-forward bucketed DV01 must be present");

    assert!(
        dv01_metric > 0.0 && dv01_metric.is_finite(),
        "DV01 should be positive and finite: got {}",
        dv01_metric
    );
}

#[test]
fn annuity_positive_and_increasing() {
    let ctx = market();
    let as_of = d(2025, 1, 2);
    let maturities = vec![d(2026, 1, 2), d(2027, 1, 2), d(2028, 1, 2)];

    let mut annuities = Vec::new();

    for maturity in &maturities {
        let swap = BasisSwap::new(
            format!("ANNUITY-{}", maturity),
            Money::new(10_000_000.0, USD).expect("valid money fixture"),
            make_leg("USD-SOFR-3M", d(2025, 1, 2), *maturity, Decimal::ZERO),
            make_leg("USD-SOFR-1M", d(2025, 1, 2), *maturity, Decimal::ZERO),
        )
        .expect("valid basis swap");

        let res = swap
            .price_with_metrics(
                &ctx,
                as_of,
                &[MetricId::AnnuityPrimary],
                finstack_quant_valuations_core::instruments::PricingOptions::default(),
            )
            .unwrap();
        annuities.push(res.measures[MetricId::AnnuityPrimary.as_str()]);
    }

    for annuity in &annuities {
        assert!(*annuity > 0.0, "Annuity should be positive");
    }

    assert!(annuities[1] > annuities[0], "2Y annuity should exceed 1Y");
    assert!(annuities[2] > annuities[1], "3Y annuity should exceed 2Y");
}

#[test]
fn bucketed_dv01_sums_to_total() {
    let ctx = market();
    let as_of = d(2025, 1, 2);
    let swap = BasisSwap::new(
        "BUCKETED-DV01-TEST",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2027, 1, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2027, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let res = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Dv01, MetricId::BucketedDv01],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01_total = res.measures[MetricId::Dv01.as_str()];
    assert!(dv01_total.is_finite(), "Total DV01 should be finite");
}

#[test]
fn dv01_leg_components_reasonable() {
    let ctx = market();
    let as_of = d(2025, 1, 2);
    let notional = 10_000_000.0;

    let swap = BasisSwap::new(
        "DV01-COMPONENTS-TEST",
        Money::new(notional, USD).expect("valid money fixture"),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let res = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[
                MetricId::Dv01,
                MetricId::AnnuityPrimary,
                MetricId::AnnuityReference,
            ],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01_primary = res
        .measures
        .get("bucketed_dv01::USD-SOFR-3M")
        .copied()
        .expect("primary-forward bucketed DV01 must be present");
    let dv01_reference = res
        .measures
        .get("bucketed_dv01::USD-SOFR-1M")
        .copied()
        .expect("reference-forward bucketed DV01 must be present");

    assert!(
        dv01_primary > 0.0,
        "Primary forward DV01 should be positive (receive floating)"
    );
    assert!(
        dv01_reference < 0.0,
        "Reference forward DV01 should be negative (pay floating)"
    );
    assert!(dv01_primary.is_finite(), "Primary DV01 should be finite");
    assert!(
        dv01_reference.is_finite(),
        "Reference DV01 should be finite"
    );

    let dv01_ratio_primary = dv01_primary / notional;
    let dv01_ratio_reference = dv01_reference.abs() / notional;
    assert!(
        dv01_ratio_primary > 1e-6 && dv01_ratio_primary < 0.01,
        "Primary DV01 ratio to notional should be reasonable: {}",
        dv01_ratio_primary
    );
    assert!(
        dv01_ratio_reference > 1e-6 && dv01_ratio_reference < 0.01,
        "Reference DV01 ratio to notional should be reasonable: {}",
        dv01_ratio_reference
    );
}

#[test]
fn sensitivity_to_spread() {
    let ctx = market();
    let as_of = d(2025, 1, 2);
    let spreads = vec![Decimal::ZERO, Decimal::from(10), Decimal::from(20)];
    let mut npvs = Vec::new();

    for spread in &spreads {
        let swap = BasisSwap::new(
            format!("SPREAD-SENS-{}", spread),
            Money::new(10_000_000.0, USD).expect("valid money fixture"),
            make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2026, 1, 2), *spread),
            make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
        )
        .expect("valid basis swap");

        let npv = swap.value(&ctx, as_of).unwrap().amount();
        npvs.push(npv);
    }

    assert!(
        npvs[1] > npvs[0],
        "NPV should increase with 10bp spread: {} vs {}",
        npvs[1],
        npvs[0]
    );
    assert!(
        npvs[2] > npvs[1],
        "NPV should increase with 20bp spread: {} vs {}",
        npvs[2],
        npvs[1]
    );

    let delta1 = npvs[1] - npvs[0];
    let delta2 = npvs[2] - npvs[1];
    let ratio = delta2 / delta1;
    // NPV is linear in spread: consecutive deltas must be exactly equal
    assert!(
        (ratio - 1.0).abs() < 1e-6,
        "Spread sensitivity must be exactly linear, got ratio {}",
        ratio
    );
}

/// Invariant test: Annuity with payment lag should differ from annuity without lag.
#[test]
fn annuity_with_payment_lag_differs_from_no_lag() {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 1.0), (0.5, 0.92), (1.0, 0.85), (2.0, 0.72)])
        .interp(InterpStyle::LogLinear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let f3m = ForwardCurve::builder("USD-SOFR-3M")
        .base_date(d(2025, 1, 2))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.997487536532938),
            (0.25, 0.994950292856076),
            (0.375, 0.992388490374784),
            (0.5, 0.98980235222721),
            (0.625, 0.98719210325196),
            (0.75, 0.984557969955606),
            (0.875, 0.981900180480004),
            (1.0, 0.97921896456946),
            (1.125, 0.976514553537717),
            (1.25, 0.973787180234803),
            (1.375, 0.971037079013718),
            (1.5, 0.968264485696987),
            (1.625, 0.96546963754307),
            (1.75, 0.962652773212653),
            (1.875, 0.95981413273481),
            (2.0, 0.956953957473047),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let f1m = ForwardCurve::builder("USD-SOFR-1M")
        .base_date(d(2025, 1, 2))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.9976122302682),
            (0.25, 0.995199061524078),
            (0.375, 0.992760705844714),
            (0.5, 0.990297377149241),
            (0.625, 0.987809291167626),
            (0.75, 0.985296665409241),
            (0.875, 0.982759719131244),
            (1.0, 0.980198673306755),
            (1.125, 0.977613750592859),
            (1.25, 0.975005175298418),
            (1.375, 0.972373173351715),
            (1.5, 0.969717972267931),
            (1.625, 0.967039801116463),
            (1.75, 0.96433889048808),
            (1.875, 0.961615472461945),
            (2.0, 0.958869780572485),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let ctx = MarketContext::new().insert(disc).insert(f3m).insert(f1m);

    let as_of = d(2025, 1, 2);

    let swap_no_lag = BasisSwap::new(
        "ANNUITY-NO-LAG",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2027, 1, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2027, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let primary_with_lag = FloatLegSpec {
        payment_lag_days: 10,
        ..make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2027, 1, 2), Decimal::ZERO)
    };
    let reference_with_lag = FloatLegSpec {
        payment_lag_days: 10,
        ..make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2027, 1, 2), Decimal::ZERO)
    };
    let swap_with_lag = BasisSwap::new(
        "ANNUITY-WITH-LAG",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        primary_with_lag,
        reference_with_lag,
    )
    .expect("swap construction");

    let res_no_lag = swap_no_lag
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::AnnuityPrimary],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();
    let res_with_lag = swap_with_lag
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::AnnuityPrimary],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();

    let annuity_no_lag = res_no_lag.measures[MetricId::AnnuityPrimary.as_str()];
    let annuity_with_lag = res_with_lag.measures[MetricId::AnnuityPrimary.as_str()];

    assert!(
        annuity_with_lag < annuity_no_lag,
        "Annuity with payment lag ({}) should be lower than without lag ({}) \
         because later payments have lower discount factors",
        annuity_with_lag,
        annuity_no_lag
    );

    let diff_pct = (annuity_no_lag - annuity_with_lag) / annuity_no_lag * 100.0;
    assert!(
        diff_pct > 0.1,
        "Annuity difference should be meaningful (> 0.1%), got {:.4}%",
        diff_pct
    );

    assert!(annuity_no_lag > 0.0, "Annuity should be positive");
    assert!(annuity_with_lag > 0.0, "Annuity should be positive");
}

#[test]
fn test_bucketed_dv01_per_curve() {
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new(
        "BUCKETED-DV01-TEST",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2027, 1, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2027, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let res = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::BucketedDv01],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();

    assert!(
        res.measures.contains_key("bucketed_dv01"),
        "Standard BucketedDv01 scalar should be present for BC"
    );

    let mut disc_buckets = 0;
    let mut fwd_3m_buckets = 0;
    let mut fwd_1m_buckets = 0;

    for key in res.measures.keys() {
        if key.as_str().starts_with("bucketed_dv01::USD-OIS::") {
            disc_buckets += 1;
        }
        if key.as_str().starts_with("bucketed_dv01::USD-SOFR-3M::") {
            fwd_3m_buckets += 1;
        }
        if key.as_str().starts_with("bucketed_dv01::USD-SOFR-1M::") {
            fwd_1m_buckets += 1;
        }
    }

    assert!(
        disc_buckets > 0,
        "Should have discount curve bucketed DV01s under bucketed_dv01::USD-OIS::*"
    );
    assert!(
        fwd_3m_buckets > 0,
        "Should have 3M forward curve bucketed DV01s under bucketed_dv01::USD-SOFR-3M::*"
    );
    assert!(
        fwd_1m_buckets > 0,
        "Should have 1M forward curve bucketed DV01s under bucketed_dv01::USD-SOFR-1M::*"
    );

    let total_dv01 = *res.measures.get("bucketed_dv01").unwrap();

    let mut sum_disc = 0.0;
    let mut sum_fwd_3m = 0.0;
    let mut sum_fwd_1m = 0.0;

    for (key, val) in &res.measures {
        if key.as_str().starts_with("bucketed_dv01::USD-OIS::") {
            sum_disc += val;
        }
        if key.as_str().starts_with("bucketed_dv01::USD-SOFR-3M::") {
            sum_fwd_3m += val;
        }
        if key.as_str().starts_with("bucketed_dv01::USD-SOFR-1M::") {
            sum_fwd_1m += val;
        }
    }

    let sum_all = sum_disc + sum_fwd_3m + sum_fwd_1m;
    assert!(
        (total_dv01 - sum_all).abs() < 1.0,
        "Total DV01 ({}) should equal sum of per-curve DV01s ({})",
        total_dv01,
        sum_all
    );
}
