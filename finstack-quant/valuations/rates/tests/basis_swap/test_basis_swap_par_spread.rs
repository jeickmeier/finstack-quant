//! Comprehensive par spread calculation tests for basis swaps.
//!
//! Tests verify that computed par spreads correctly set NPV to zero and
//! validate the mathematical relationship between par spread, annuity, and PV.

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
        .knots(vec![(0.0, 1.0), (0.5, 0.99), (1.0, 0.98), (2.0, 0.96)])
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
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    MarketContext::new().insert(disc).insert(f3m).insert(f1m)
}

#[test]
fn par_spread_preserves_end_of_month_on_sloped_curves() {
    let as_of = d(2025, 1, 1);
    let ctx = MarketContext::new()
        .insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (1.0, 0.97), (3.0, 0.88)])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .unwrap(),
        )
        .insert(
            ForwardCurve::builder("USD-SOFR-3M")
                .base_date(as_of)
                .day_count(DayCount::Act360)
                .knots([
                    (0.0, 1.0),
                    (0.125, 0.99843872006759),
                    (0.25, 0.996257022469171),
                    (0.375, 0.993458986176422),
                    (0.5, 0.990049833749168),
                    (0.625, 0.986035915092277),
                    (0.75, 0.981424687747777),
                    (0.875, 0.976224693807818),
                    (1.0, 0.970445533548508),
                    (1.125, 0.964248487954782),
                    (1.25, 0.957791658626643),
                    (1.375, 0.951080806514218),
                    (1.5, 0.944121890386422),
                    (1.625, 0.936921058024467),
                    (1.75, 0.929484637184243),
                    (1.875, 0.921819126346146),
                    (2.0, 0.913931185271228),
                    (2.125, 0.905827625382865),
                    (2.25, 0.897515399993304),
                    (2.375, 0.889001594394688),
                    (2.5, 0.880293415834221),
                    (2.625, 0.871398183393264),
                    (2.75, 0.86232331779014),
                    (2.875, 0.853076331126421),
                    (3.0, 0.843664816596384),
                ])
                .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .unwrap(),
        )
        .insert(
            ForwardCurve::builder("USD-SOFR-1M")
                .base_date(as_of)
                .day_count(DayCount::Act360)
                .knots([
                    (0.0, 1.0),
                    (0.125, 0.997230404924721),
                    (0.25, 0.993924779237912),
                    (0.375, 0.990088508326157),
                    (0.5, 0.985727827010081),
                    (0.625, 0.980849802659613),
                    (0.75, 0.975462316108346),
                    (0.875, 0.969574040439128),
                    (1.0, 0.963194417720822),
                    (1.125, 0.956445710698142),
                    (1.25, 0.949447540368186),
                    (1.375, 0.942206089190876),
                    (1.5, 0.934727720616027),
                    (1.625, 0.927018969780986),
                    (1.75, 0.919086534013519),
                    (1.875, 0.910937263159219),
                    (2.0, 0.902578149752926),
                    (2.125, 0.894016319053851),
                    (2.25, 0.8852590189642),
                    (2.375, 0.876313609851173),
                    (2.5, 0.867187554292255),
                    (2.625, 0.857888406763654),
                    (2.75, 0.848423803291716),
                    (2.875, 0.838801451086983),
                    (3.0, 0.8290291181804),
                ])
                .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .unwrap(),
        );
    let mut swap = BasisSwap::example().unwrap();
    for leg in [&mut swap.primary_leg, &mut swap.reference_leg] {
        leg.start = d(2025, 4, 30);
        leg.end = d(2027, 4, 30);
        leg.frequency = Tenor::monthly();
        leg.end_of_month = true;
        leg.day_count = DayCount::Act360;
        leg.stub = StubKind::ShortBack;
        leg.calendar_id = Some(CALENDAR_ID.into());
        leg.spread_bp = Decimal::ZERO;
    }
    let result = swap
        .price_with_metrics(&ctx, as_of, &[MetricId::BasisParSpread], Default::default())
        .unwrap();
    swap.primary_leg.spread_bp =
        Decimal::try_from(result.measures[MetricId::BasisParSpread.as_str()]).unwrap();
    let residual = swap.value(&ctx, as_of).unwrap().amount();
    assert!(residual.abs() < 1e-5, "par residual: {residual}");

    // Verify this fixture distinguishes EOM from a same-day-of-month roll;
    // otherwise a false EOM-preservation test could pass by coincidence.
    swap.primary_leg.end_of_month = false;
    assert!(swap.value(&ctx, as_of).unwrap().amount().abs() > 1.0);
}

#[test]
fn par_spread_preserves_separate_fixing_calendar() {
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;

    // May 26 is a holiday in the payment calendar but a business day in the
    // explicitly selected fixing calendar. Both exact observations are present
    // so silently dropping the fixing calendar causes a measurable wrong price.
    let as_of = d(2025, 5, 27);
    let ctx = market()
        .insert_series(
            ScalarTimeSeries::new(
                "FIXING:USD-SOFR-3M",
                vec![(d(2025, 5, 23), 0.01), (d(2025, 5, 26), 0.04)],
                None,
                finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
                finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
            )
            .unwrap(),
        )
        .insert_series(
            ScalarTimeSeries::new(
                "FIXING:USD-SOFR-1M",
                vec![(d(2025, 5, 23), 0.03)],
                None,
                finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
                finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
            )
            .unwrap(),
        );
    let mut swap = BasisSwap::example().unwrap();
    for leg in [&mut swap.primary_leg, &mut swap.reference_leg] {
        leg.start = as_of;
        leg.end = d(2025, 8, 27);
        leg.frequency = Tenor::quarterly();
        leg.day_count = DayCount::Act360;
        leg.calendar_id = Some(CALENDAR_ID.into());
        leg.reset_lag_days = 1;
        leg.spread_bp = Decimal::ZERO;
    }
    swap.primary_leg.fixing_calendar_id = Some("weekends_only".into());
    let result = swap
        .price_with_metrics(&ctx, as_of, &[MetricId::BasisParSpread], Default::default())
        .unwrap();
    let par = result.measures[MetricId::BasisParSpread.as_str()];
    // Same accrual/payment convention gives an exact spread of 3% - 4%.
    assert!((par + 100.0).abs() < 1e-9, "par spread: {par}");
    swap.primary_leg.spread_bp = Decimal::try_from(par).unwrap();
    assert!(swap.value(&ctx, as_of).unwrap().amount().abs() < 1e-7);
    assert_eq!(
        swap.primary_leg.fixing_calendar_id.as_deref(),
        Some("weekends_only")
    );
}

#[test]
fn par_spread_zeros_npv() {
    // Test that applying the computed par spread results in zero NPV
    let ctx = market();
    let as_of = d(2025, 1, 2);

    // Create swap with zero spread
    let swap = BasisSwap::new(
        "PAR-TEST",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-3M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-1M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
    )
    .expect("valid basis swap");

    // Compute par spread
    let res = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::BasisParSpread],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();
    let par_spread_bp = res.measures[MetricId::BasisParSpread.as_str()];

    // Create new swap with par spread applied
    let swap_at_par = BasisSwap::new(
        "PAR-TEST-APPLIED",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-3M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::try_from(par_spread_bp).expect("valid literal"),
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-1M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
    )
    .expect("valid basis swap");

    let npv = swap_at_par.value(&ctx, as_of).unwrap();

    // NPV should be very close to zero (within $1 tolerance)
    assert!(
        npv.amount().abs() < 1.0,
        "NPV with par spread should be near zero, got {}",
        npv.amount()
    );
}

#[test]
fn par_spread_formula_validation() {
    // Validate: par_spread * annuity * notional = PV_difference
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new(
        "FORMULA-TEST",
        Money::new(5_000_000.0, USD).expect("valid money fixture"),
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-3M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2027, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-1M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2027, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
    )
    .expect("valid basis swap");

    let res = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[
                MetricId::BasisParSpread,
                MetricId::AnnuityPrimary,
                MetricId::PvPrimary,
                MetricId::PvReference,
            ],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();

    let par_spread_bp = res.measures[MetricId::BasisParSpread.as_str()];
    let par_spread_decimal = par_spread_bp / 1e4;
    let annuity = res.measures[MetricId::AnnuityPrimary.as_str()];
    let pv_primary = res.measures[MetricId::PvPrimary.as_str()];
    let pv_reference = res.measures[MetricId::PvReference.as_str()];
    let notional = swap.notional.amount();

    // par_spread (decimal) * annuity * notional should equal (pv_reference - pv_primary)
    let computed_pv_diff = par_spread_decimal * annuity * notional;
    let actual_pv_diff = pv_reference - pv_primary;

    assert!(
        (computed_pv_diff - actual_pv_diff).abs() < 1.0,
        "Par spread formula mismatch: computed {} vs actual {}",
        computed_pv_diff,
        actual_pv_diff
    );
}

#[test]
fn par_spread_with_existing_spread() {
    // Test par spread calculation when primary leg already has a spread
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new(
        "EXISTING-SPREAD-TEST",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-3M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::from(10), // 10bp existing spread
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-1M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
    )
    .expect("valid basis swap");

    let res = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::BasisParSpread],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();
    let par_spread = res.measures[MetricId::BasisParSpread.as_str()];

    // Par spread should be finite and represent the additional spread needed
    assert!(par_spread.is_finite());
    // Since we already have 10bp spread, par spread should be different from zero-spread case
}

#[test]
fn par_spread_inverted_curves() {
    // Test with inverted forward curves (shorter tenor > longer tenor)
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 1.0), (1.0, 0.98), (2.0, 0.96)])
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
            (0.125, 0.996898617535531),
            (0.25, 0.99382257503179),
            (0.375, 0.990771688570422),
            (0.5, 0.987745775625648),
            (0.625, 0.984744655052645),
            (0.75, 0.981768147076161),
            (0.875, 0.978816073279207),
            (1.0, 0.975888256591839),
            (1.125, 0.972984521279971),
            (1.25, 0.970104692934284),
            (1.375, 0.967248598459202),
            (1.5, 0.964416066062004),
            (1.625, 0.961606925242009),
            (1.75, 0.958821006779859),
            (1.875, 0.956058142726888),
            (2.0, 0.953318166394589),
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
            (0.125, 0.996277759337218),
            (0.25, 0.992584950550794),
            (0.375, 0.98892131033638),
            (0.5, 0.985286577957363),
            (0.625, 0.981680495224815),
            (0.75, 0.978102806480492),
            (0.875, 0.974553258579005),
            (1.0, 0.97103160087091),
            (1.125, 0.967537585185889),
            (1.25, 0.964070965816049),
            (1.375, 0.960631499499325),
            (1.5, 0.957218945403018),
            (1.625, 0.953833065107428),
            (1.75, 0.950473622589581),
            (1.875, 0.947140384207036),
            (2.0, 0.943833118681787),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let ctx = MarketContext::new().insert(disc).insert(f3m).insert(f1m);

    let swap = BasisSwap::new(
        "INVERTED-TEST",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-3M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-1M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
    )
    .expect("valid basis swap");

    let res = swap
        .price_with_metrics(
            &ctx,
            d(2025, 1, 2),
            &[MetricId::BasisParSpread],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();
    let par_spread = res.measures[MetricId::BasisParSpread.as_str()];

    assert!(par_spread.is_finite());
    // With inverted curves (1M higher than 3M), par spread should be positive
    // to compensate for receiving lower 3M rate
}

#[test]
fn par_spread_long_maturity() {
    // Test par spread for longer-dated swaps (5 years)
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new(
        "LONG-MAT-TEST",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-3M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2030, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-1M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2030, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
    )
    .expect("valid basis swap");

    let res = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::BasisParSpread, MetricId::AnnuityPrimary],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();
    let par_spread = res.measures[MetricId::BasisParSpread.as_str()];
    let annuity = res.measures[MetricId::AnnuityPrimary.as_str()];

    assert!(par_spread.is_finite());
    assert!(annuity > 0.0);
    // Annuity should be larger for longer maturities
}

#[test]
fn par_spread_sign_convention() {
    // Verify par spread sign convention: positive spread added to primary leg
    let ctx = market();
    let as_of = d(2025, 1, 2);

    // If 3M rate > 1M rate, primary leg receives more, so negative spread needed
    // If 1M rate > 3M rate, primary leg receives less, so positive spread needed
    let swap = BasisSwap::new(
        "SIGN-TEST",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-3M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-1M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
    )
    .expect("valid basis swap");

    let res = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[
                MetricId::BasisParSpread,
                MetricId::PvPrimary,
                MetricId::PvReference,
            ],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();

    let par_spread = res.measures[MetricId::BasisParSpread.as_str()];
    let pv_primary = res.measures[MetricId::PvPrimary.as_str()];
    let pv_reference = res.measures[MetricId::PvReference.as_str()];

    // If PV_primary < PV_reference, need positive spread to increase primary leg value
    // If PV_primary > PV_reference, need negative spread to decrease primary leg value
    if pv_primary < pv_reference {
        assert!(
            par_spread > 0.0,
            "Expected positive par spread when primary < reference"
        );
    } else if pv_primary > pv_reference {
        assert!(
            par_spread < 0.0,
            "Expected negative par spread when primary > reference"
        );
    }
}

#[test]
fn incremental_par_spread_sign_convention() {
    // Test the sign convention for IncrementalParSpreadCalculator:
    // - Positive: Current spread is below par (primary leg receiver is losing)
    // - Negative: Current spread is above par (primary leg receiver is gaining)
    // - Zero: Swap is at par
    let ctx = market();
    let as_of = d(2025, 1, 2);

    // First, create a swap with zero spread to find the par spread
    let zero_spread_swap = BasisSwap::new(
        "INC-SIGN-ZERO",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-3M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-1M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
    )
    .expect("valid basis swap");

    let res = zero_spread_swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::BasisParSpread],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();
    let par_spread_bp = res.measures[MetricId::BasisParSpread.as_str()];

    // Case 1: Current spread BELOW par -> Positive incremental
    // (need to add more spread to reach par)
    let below_par_spread = par_spread_bp - 5.0; // 5bp below par
    let swap_below_par = BasisSwap::new(
        "INC-SIGN-BELOW",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-3M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::try_from(below_par_spread).expect("valid literal"),
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-1M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
    )
    .expect("valid basis swap");

    let res_below = swap_below_par
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::IncrementalParSpread],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();
    let inc_spread_below = res_below.measures[MetricId::IncrementalParSpread.as_str()];

    assert!(
        inc_spread_below > 0.0,
        "Expected positive incremental spread when current spread is below par, got {:.2}bp",
        inc_spread_below
    );
    // Should be approximately 5bp since we're 5bp below par
    assert!(
        (inc_spread_below - 5.0).abs() < 1.0,
        "Incremental spread should be ~5bp, got {:.2}bp",
        inc_spread_below
    );

    // Case 2: Current spread ABOVE par -> Negative incremental
    // (would need to reduce spread to reach par)
    let above_par_spread = par_spread_bp + 5.0; // 5bp above par
    let swap_above_par = BasisSwap::new(
        "INC-SIGN-ABOVE",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-3M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::try_from(above_par_spread).expect("valid literal"),
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-1M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
    )
    .expect("valid basis swap");

    let res_above = swap_above_par
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::IncrementalParSpread],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();
    let inc_spread_above = res_above.measures[MetricId::IncrementalParSpread.as_str()];

    assert!(
        inc_spread_above < 0.0,
        "Expected negative incremental spread when current spread is above par, got {:.2}bp",
        inc_spread_above
    );
    // Should be approximately -5bp since we're 5bp above par
    assert!(
        (inc_spread_above + 5.0).abs() < 1.0,
        "Incremental spread should be ~-5bp, got {:.2}bp",
        inc_spread_above
    );

    // Case 3: Current spread AT par -> Zero incremental
    let swap_at_par = BasisSwap::new(
        "INC-SIGN-AT-PAR",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-3M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::try_from(par_spread_bp).expect("valid literal"),
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-1M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
    )
    .expect("valid basis swap");

    let res_at_par = swap_at_par
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::IncrementalParSpread],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();
    let inc_spread_at_par = res_at_par.measures[MetricId::IncrementalParSpread.as_str()];

    assert!(
        inc_spread_at_par.abs() < 0.5,
        "Expected near-zero incremental spread when at par, got {:.2}bp",
        inc_spread_at_par
    );
}

#[test]
fn zero_notional_par_spread_is_rejected_at_construction() {
    let leg = |forward_curve_id: &str| FloatLegSpec {
        end_of_month: false,
        fixing_calendar_id: None,
        index_id: forward_curve_id.into(),
        forward_curve_id: CurveId::new(forward_curve_id),
        discount_curve_id: CurveId::new("USD-OIS"),
        start: d(2025, 1, 2),
        end: d(2026, 1, 2),
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some(CALENDAR_ID.into()),
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO,
        payment_lag_days: 0,
        reset_lag_days: 0,
        compounding: Default::default(),
    };
    assert!(
        BasisSwap::new(
            "ZERO-NOTIONAL-PAR",
            Money::new(0.0, USD).expect("valid money fixture"),
            leg("USD-SOFR-3M"),
            leg("USD-SOFR-1M"),
        )
        .is_err()
    );
}
