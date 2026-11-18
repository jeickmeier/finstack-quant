//! Interest rate swap pricing benchmarks.
//!
//! Measures performance of IRS operations:
//! - Present value calculation (with Kahan summation)
//! - DV01 (bump-and-revalue)
//! - Annuity factor calculation
//! - Par rate calculation
//! - OIS compounding (daily accrual)
//!
//! # Numerical Stability Focus
//!
//! Long-dated swaps (30Y+) with quarterly/monthly frequencies can have 120-360+
//! cashflows. The IRS pricer uses **Kahan compensated summation** to prevent
//! floating-point drift. These benchmarks verify that the numerical stability
//! overhead is acceptable.
//!
//! Market Standards Review (Week 5)

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_cashflows::builder::{
    CashFlowSchedule, CashflowRepresentation, FloatingLegCompounding, build_periods,
};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt, DayCount, HolidayCalendar, Tenor, TenorUnit};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::PayReceive;
use finstack_quant_valuations::instruments::pricing::default_rate_calendar_id;
use finstack_quant_valuations::instruments::pricing::overnight::{
    OvernightCouponProjection, OvernightCouponProjectionInput, OvernightProjectionCurve,
    adjust_overnight_accrual_boundaries, project_overnight_coupon,
    resolve_overnight_fixing_calendar,
};
use finstack_quant_valuations::instruments::rates::irs::InterestRateSwap;
use finstack_quant_valuations::instruments::{FixedLegSpec, FloatLegSpec};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::ModelKey;
use finstack_quant_valuations::results::ValuationResult;
#[allow(dead_code, unused_imports, clippy::expect_used, clippy::unwrap_used)]
#[path = "../rates/tests/support/rates.rs"]
mod rates_support;
#[path = "support/record.rs"]
mod record;
mod support;
use rust_decimal_macros::dec;
use serde_json::{Value, json};
use std::hint::black_box;
use time::Month;

fn create_swap(tenor_years: i32) -> InterestRateSwap {
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2025 + tenor_years, Month::January, 1).unwrap();

    rates_support::usd_irs_swap(
        InstrumentId::new(format!("IRS-{}Y", tenor_years)),
        Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        0.04, // 4% fixed rate
        start,
        end,
        PayReceive::Pay,
    )
    .expect("Failed to create swap for benchmark")
}

/// Create a swap with monthly frequency for stress-testing Kahan summation.
///
/// A 50Y monthly swap has 600 periods, which is where numerical precision
/// really matters.
fn create_monthly_swap(tenor_years: i32) -> InterestRateSwap {
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2025 + tenor_years, Month::January, 1).unwrap();

    let disc_id = CurveId::new("USD-OIS");
    let fwd_id = CurveId::new("USD-SOFR-1M");

    InterestRateSwap::builder()
        .id(format!("IRS-{}Y-Monthly", tenor_years).into())
        .notional(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .side(PayReceive::Pay)
        .fixed_leg(FixedLegSpec {
            discount_curve_id: disc_id.clone(),
            rate: dec!(0.04),
            frequency: Tenor::new(1, TenorUnit::Months).expect("valid tenor fixture"), // Monthly fixed
            day_count: finstack_quant_core::dates::DayCount::Act360,
            business_day_convention:
                finstack_quant_core::dates::BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: finstack_quant_core::dates::StubKind::None,
            start,
            end,

            payment_lag_days: 0,
            end_of_month: false,
        })
        .float_leg(FloatLegSpec {
            discount_curve_id: disc_id,
            index_id: fwd_id.as_str().into(),
            forward_curve_id: fwd_id,
            spread_bp: dec!(0.0),
            frequency: Tenor::new(1, TenorUnit::Months).expect("valid tenor fixture"), // Monthly float
            day_count: finstack_quant_core::dates::DayCount::Act360,
            business_day_convention:
                finstack_quant_core::dates::BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: finstack_quant_core::dates::StubKind::None,
            reset_lag_days: 2,
            start,
            end,
            compounding: FloatingLegCompounding::Simple,
            fixing_calendar_id: None,
            payment_lag_days: 0,
            end_of_month: false,
        })
        .build()
        .expect("Failed to create monthly swap for benchmark")
}

/// Create an OIS swap with daily compounding for benchmarking the complex
/// accrual logic.
fn create_ois_swap(tenor_years: i32) -> InterestRateSwap {
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2025 + tenor_years, Month::January, 1).unwrap();

    let disc_id = CurveId::new("USD-OIS");

    InterestRateSwap::builder()
        .id(format!("OIS-{}Y", tenor_years).into())
        .notional(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .side(PayReceive::Pay)
        .fixed_leg(FixedLegSpec {
            discount_curve_id: disc_id.clone(),
            rate: dec!(0.04),
            frequency: Tenor::new(1, TenorUnit::Years).expect("valid tenor fixture"), // Annual fixed
            day_count: finstack_quant_core::dates::DayCount::Act360,
            business_day_convention:
                finstack_quant_core::dates::BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: finstack_quant_core::dates::StubKind::None,
            start,
            end,

            payment_lag_days: 2,
            end_of_month: false,
        })
        .float_leg(FloatLegSpec {
            discount_curve_id: disc_id.clone(),
            index_id: disc_id.as_str().into(),
            forward_curve_id: disc_id, // Single-curve OIS
            spread_bp: dec!(0.0),
            frequency: Tenor::new(1, TenorUnit::Years).expect("valid tenor fixture"), // Annual payment with daily compounding
            day_count: finstack_quant_core::dates::DayCount::Act360,
            business_day_convention:
                finstack_quant_core::dates::BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: finstack_quant_core::dates::StubKind::None,
            reset_lag_days: 0,
            start,
            end,
            compounding: FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 }, // Compounded in arrears
            fixing_calendar_id: None,
            payment_lag_days: 2,
            end_of_month: false,
        })
        .build()
        .expect("Failed to create OIS swap for benchmark")
}

fn create_market() -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.98),
            (2.0, 0.96),
            (5.0, 0.88),
            (10.0, 0.70),
            (30.0, 0.40),
            (50.0, 0.20),
        ])
        .interp(InterpStyle::Linear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let fwd = ForwardCurve::builder("USD-SOFR-3M")
        .base_date(base)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.995611221459588),
            (0.25, 0.991195240930358),
            (0.375, 0.986752592135641),
            (0.5, 0.982283810160798),
            (0.625, 0.977789431347014),
            (0.75, 0.973269993185277),
            (0.875, 0.968726034210573),
            (1.0, 0.964158093896324),
            (1.125, 0.959574209193318),
            (1.25, 0.954982273933355),
            (1.375, 0.950382612946264),
            (1.5, 0.945775550224929),
            (1.625, 0.941161408888927),
            (1.75, 0.936540511148462),
            (1.875, 0.93191317826863),
            (2.0, 0.927279730533999),
            (2.1875, 0.920324184202576),
            (2.375, 0.913367292399534),
            (2.5625, 0.906409877451143),
            (2.75, 0.899452755535353),
            (2.9375, 0.892496736538075),
            (3.125, 0.885542623912392),
            (3.3125, 0.878591214540736),
            (3.5, 0.871643298600053),
            (3.6875, 0.864699659429976),
            (3.875, 0.857761073404031),
            (4.0625, 0.850828309803906),
            (4.25, 0.843902130696782),
            (4.4375, 0.836983290815769),
            (4.625, 0.830072537443442),
            (4.8125, 0.823170610298508),
            (5.0, 0.81627824142561),
            (5.3125, 0.804840362948709),
            (5.625, 0.793485262077054),
            (5.9375, 0.782213972982446),
            (6.25, 0.771027490817996),
            (6.5625, 0.759926771886431),
            (6.875, 0.74891273382482),
            (7.1875, 0.737986255805403),
            (7.5, 0.727148178752154),
            (7.8125, 0.71639930557274),
            (8.125, 0.705740401405505),
            (8.4375, 0.695172193881119),
            (8.75, 0.684695373398532),
            (9.0625, 0.674310593414836),
            (9.375, 0.664018470748686),
            (9.6875, 0.653819585896884),
            (10.0, 0.643714483363737),
            (11.25, 0.604595697764404),
            (12.5, 0.567632401630785),
            (13.75, 0.532720803704493),
            (15.0, 0.499761147348737),
            (16.25, 0.468657613416832),
            (17.5, 0.439318221663474),
            (18.75, 0.411654731012661),
            (20.0, 0.385582538979713),
            (21.25, 0.361020580528505),
            (22.5, 0.337891226628893),
            (23.75, 0.316120182763365),
            (25.0, 0.29563638761626),
            (26.25, 0.276371912163538),
            (27.5, 0.258261859365997),
            (28.75, 0.241244264654134),
            (30.0, 0.225259997378556),
            (31.25, 0.210293732369713),
            (32.5, 0.196321825395681),
            (33.75, 0.183278211349314),
            (35.0, 0.171101214486481),
            (36.25, 0.159733256796966),
            (37.5, 0.149120585751199),
            (38.75, 0.139213020135473),
            (40.0, 0.129963712773867),
            (41.25, 0.121328929014911),
            (42.5, 0.113267839935589),
            (43.75, 0.105742329284862),
            (45.0, 0.098716813253846),
            (46.25, 0.0921580722204673),
            (47.5, 0.0860350936729815),
            (48.75, 0.080318925569656),
            (50.0, 0.0749825394412265),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    // Monthly forward curve for monthly swaps
    let fwd_1m = ForwardCurve::builder("USD-SOFR-1M")
        .base_date(base)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.995735680640807),
            (0.25, 0.991443070718023),
            (0.375, 0.987122693747407),
            (0.5, 0.982775074871822),
            (0.625, 0.978400740755897),
            (0.75, 0.974000219480797),
            (0.875, 0.969574040439128),
            (1.0, 0.965122734230001),
            (1.125, 0.96065433763699),
            (1.25, 0.956176748166637),
            (1.375, 0.951690287859541),
            (1.5, 0.947195278079959),
            (1.625, 0.942692039478908),
            (1.75, 0.938180891957541),
            (1.875, 0.933662154630828),
            (2.0, 0.929136145791519),
            (2.1875, 0.922339596909496),
            (2.375, 0.915539117740705),
            (2.5625, 0.908735531233421),
            (2.75, 0.901929654788586),
            (2.9375, 0.895122300110174),
            (3.125, 0.888314273058335),
            (3.3125, 0.881506373505352),
            (3.5, 0.874699395194429),
            (3.6875, 0.867894125601357),
            (3.875, 0.861091345799062),
            (4.0625, 0.854291830325076),
            (4.25, 0.847496347051956),
            (4.4375, 0.840705657060653),
            (4.625, 0.833920514516883),
            (4.8125, 0.827141666550488),
            (5.0, 0.820369853137831),
            (5.3125, 0.809127454882066),
            (5.625, 0.797961193431406),
            (5.9375, 0.786872183825467),
            (6.25, 0.775861503188559),
            (6.5625, 0.76493019083878),
            (6.875, 0.754079248413903),
            (7.1875, 0.743309640013785),
            (7.5, 0.732622292358937),
            (7.8125, 0.722018094964979),
            (8.125, 0.71149790033261),
            (8.4375, 0.701062524152792),
            (8.75, 0.690712745526791),
            (9.0625, 0.680449307200737),
            (9.375, 0.670272915814344),
            (9.6875, 0.660184242163456),
            (10.0, 0.650183921476041),
            (11.25, 0.611435802813574),
            (12.5, 0.574772338287592),
            (13.75, 0.540096305125504),
            (15.0, 0.507314069861044),
            (16.25, 0.476335513620964),
            (17.5, 0.447073955279954),
            (18.75, 0.419446072787661),
            (20.0, 0.393371822958022),
            (21.25, 0.368774359997452),
            (22.5, 0.3455799530347),
            (23.75, 0.323717902901575),
            (25.0, 0.303120458400172),
            (26.25, 0.283722732278818),
            (27.5, 0.265462617125723),
            (28.75, 0.248280701376271),
            (30.0, 0.232120185617091),
            (31.25, 0.216969172010832),
            (32.5, 0.202807099597632),
            (33.75, 0.189569417931642),
            (35.0, 0.177195789921751),
            (36.25, 0.165629816816315),
            (37.5, 0.154818781138764),
            (38.75, 0.144713406402389),
            (40.0, 0.135267632509086),
            (41.25, 0.126438405808344),
            (42.5, 0.118185482859558),
            (43.75, 0.110471247003219),
            (45.0, 0.103260536904929),
            (46.25, 0.0965204862907317),
            (47.5, 0.0902203741432862),
            (48.75, 0.0843314846760791),
            (50.0, 0.0788269764474369),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    MarketContext::new().insert(disc).insert(fwd).insert(fwd_1m)
}

fn create_monthly_market() -> MarketContext {
    // The first monthly coupon resets two days before the January 1 valuation.
    // Preserve the contractual reset lag and supply its exact historical fixing.
    let fixings = ScalarTimeSeries::new(
        "FIXING:USD-SOFR-1M",
        vec![(
            Date::from_calendar_date(2024, Month::December, 30).unwrap(),
            0.034,
        )],
        None,
        finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
        finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
    )
    .expect("valid monthly IRS fixing series");
    create_market().insert_series(fixings)
}

const OIS_METRICS: [MetricId; 4] = [
    MetricId::PvFixed,
    MetricId::PvFloat,
    MetricId::ParRate,
    MetricId::Dv01,
];

#[derive(Clone, Copy)]
enum OisControl {
    Seasoned,
    Lookback,
    ObservationShift,
    Cutoff,
    ArithmeticSpread,
    ExplicitForward,
    DiscountDerivedForward,
}

impl OisControl {
    fn label(self) -> &'static str {
        match self {
            Self::Seasoned => "5Y_seasoned",
            Self::Lookback => "5Y_lookback_2bd",
            Self::ObservationShift => "5Y_observation_shift_2bd",
            Self::Cutoff => "5Y_cutoff_2bd",
            Self::ArithmeticSpread => "5Y_arithmetic_spread_10bp",
            Self::ExplicitForward => "5Y_explicit_forward",
            Self::DiscountDerivedForward => "5Y_discount_derived_forward",
        }
    }
}

fn fixing_calendar(swap: &InterestRateSwap) -> &'static dyn HolidayCalendar {
    resolve_overnight_fixing_calendar(
        swap.float_leg
            .fixing_calendar_id
            .as_deref()
            .or(swap.float_leg.calendar_id.as_deref()),
        swap.notional.currency(),
        swap.id.as_str(),
    )
    .expect("valid OIS fixing calendar")
}

fn create_ois_control(control: OisControl) -> (InterestRateSwap, MarketContext, Date) {
    let mut swap = create_ois_swap(5);
    let mut market = create_market();
    let mut as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let mut shift_days = 0;
    match control {
        OisControl::Seasoned => {
            as_of = Date::from_calendar_date(2025, Month::July, 1).unwrap();
        }
        OisControl::Lookback => {
            swap.float_leg.compounding =
                FloatingLegCompounding::CompoundedInArrears { lookback_days: 2 };
            shift_days = 2;
        }
        OisControl::ObservationShift => {
            swap.float_leg.compounding =
                FloatingLegCompounding::CompoundedWithObservationShift { shift_days: 2 };
            shift_days = 2;
        }
        OisControl::Cutoff => {
            swap.float_leg.compounding =
                FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 2 };
        }
        OisControl::ArithmeticSpread => swap.float_leg.spread_bp = dec!(10),
        OisControl::ExplicitForward => {
            swap.float_leg.forward_curve_id = CurveId::new("USD-SOFR-OIS");
            let forward = ForwardCurve::builder("USD-SOFR-OIS")
                .base_date(as_of)
                .day_count(DayCount::Act360)
                .knots([
                    (0.0, 1.0),
                    (0.125, 0.995611221459588),
                    (0.25, 0.991195240930358),
                    (0.375, 0.986752592135641),
                    (0.5, 0.982283810160798),
                    (0.625, 0.977789431347014),
                    (0.75, 0.973269993185277),
                    (0.875, 0.968726034210573),
                    (1.0, 0.964158093896324),
                    (1.125, 0.959574209193318),
                    (1.25, 0.954982273933355),
                    (1.375, 0.950382612946264),
                    (1.5, 0.945775550224929),
                    (1.625, 0.941161408888927),
                    (1.75, 0.936540511148462),
                    (1.875, 0.93191317826863),
                    (2.0, 0.927279730533999),
                    (2.1875, 0.920324184202576),
                    (2.375, 0.913367292399534),
                    (2.5625, 0.906409877451143),
                    (2.75, 0.899452755535353),
                    (2.9375, 0.892496736538075),
                    (3.125, 0.885542623912392),
                    (3.3125, 0.878591214540736),
                    (3.5, 0.871643298600053),
                    (3.6875, 0.864699659429976),
                    (3.875, 0.857761073404031),
                    (4.0625, 0.850828309803906),
                    (4.25, 0.843902130696782),
                    (4.4375, 0.836983290815769),
                    (4.625, 0.830072537443442),
                    (4.8125, 0.823170610298508),
                    (5.0, 0.81627824142561),
                    (5.3125, 0.804840362948709),
                    (5.625, 0.793485262077054),
                    (5.9375, 0.782213972982446),
                    (6.25, 0.771027490817996),
                    (6.5625, 0.759926771886431),
                    (6.875, 0.74891273382482),
                    (7.1875, 0.737986255805403),
                    (7.5, 0.727148178752154),
                    (7.8125, 0.71639930557274),
                    (8.125, 0.705740401405505),
                    (8.4375, 0.695172193881119),
                    (8.75, 0.684695373398532),
                    (9.0625, 0.674310593414836),
                    (9.375, 0.664018470748686),
                    (9.6875, 0.653819585896884),
                    (10.0, 0.643714483363737),
                ])
                .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("valid explicit overnight forward curve");
            market = market.insert(forward);
        }
        OisControl::DiscountDerivedForward => {
            swap.float_leg.forward_curve_id = CurveId::new("USD-SOFR-OIS");
            let disc = market.get_discount("USD-OIS").expect("OIS discount curve");
            let forward = ForwardCurve::from_discount_curve("USD-SOFR-OIS", disc.as_ref(), None);
            market = market.insert(forward);
        }
    }

    // Required historical data is fixture setup, not a diagnostic projection.
    // Resolve shifted dates with the same business calendar as the instrument.
    if matches!(
        control,
        OisControl::Seasoned | OisControl::Lookback | OisControl::ObservationShift
    ) {
        let calendar = fixing_calendar(&swap);
        let (start, _) = adjust_overnight_accrual_boundaries(
            swap.float_leg.start,
            swap.float_leg.end,
            swap.float_leg.business_day_convention,
            calendar,
        )
        .expect("valid adjusted OIS boundaries");
        let mut date = start
            .add_business_days(-shift_days, calendar)
            .expect("valid first observation date");
        let mut values = Vec::new();
        while date < as_of {
            values.push((date, 0.05));
            date = date
                .add_business_days(1, calendar)
                .expect("valid next fixing date");
        }
        market = market.insert_series(
            ScalarTimeSeries::new(
                "FIXING:USD-OIS",
                values,
                None,
                finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
                finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
            )
            .expect("valid exact historical OIS fixings"),
        );
    }
    swap.validate().expect("valid OIS control instrument");
    (swap, market, as_of)
}

fn projection_record(projection: &OvernightCouponProjection) -> Value {
    json!({
        "rate": projection.rate,
        "accrual_year_fraction": projection.accrual_year_fraction,
        "compound_factor": projection.compound_factor,
        "parallel_forward_sensitivity": projection.parallel_forward_sensitivity,
        "parallel_forward_second_sensitivity": projection.parallel_forward_second_sensitivity,
        "fixing_date": projection.fixing_date.to_string(),
        "observation_exposures": projection.observation_exposures.iter().map(|observation| {
            json!({
                "observation_start": observation.observation_start.to_string(),
                "observation_end": observation.observation_end.to_string(),
                "projected_rate": observation.projected_rate,
                "rate_accrual_year_fraction": observation.rate_accrual_year_fraction,
                "factor_accrual_year_fraction": observation.factor_accrual_year_fraction,
                "coupon_forward_derivative": observation.coupon_forward_derivative,
            })
        }).collect::<Vec<_>>(),
    })
}

fn capture_swap(
    case: &str,
    swap: &InterestRateSwap,
    market: &MarketContext,
    as_of: Date,
    requested: &[MetricId],
    output: impl FnOnce() -> Value,
) {
    // Schedule construction, daily diagnostics and serialization belong only to
    // the explicit capture pass. They must not inflate a PV memory measurement.
    record::capture(case, || {
        let schedule = swap
            .cashflow_schedule(market, as_of)
            .expect("capture swap cashflow schedule");
        let float = &swap.float_leg;
        let is_overnight = !matches!(
            float.compounding,
            FloatingLegCompounding::Simple | FloatingLegCompounding::SimpleAverage
        );
        let periods = build_periods(float.schedule_params(
            float.calendar_or_weekends(),
            if is_overnight {
                None
            } else {
                Some(float.reset_lag_days)
            },
            swap.adjust_float_accrual_dates,
        ))
        .expect("capture floating periods");
        let discount = market
            .get_discount(&float.discount_curve_id)
            .expect("capture OIS discount curve");
        let forward = market.get_forward(&float.forward_curve_id).ok();
        let calendar = fixing_calendar(swap);
        let fixings = market
            .get_series(format!("FIXING:{}", float.forward_curve_id))
            .ok();
        let mut raw_observation_count = 0;
        let period_records: Vec<_> = periods
            .iter()
            .filter(|period| period.payment_date > as_of)
            .map(|period| {
                let projection = if is_overnight {
                    let (start, end) = adjust_overnight_accrual_boundaries(
                        period.accrual_start,
                        period.accrual_end,
                        float.business_day_convention,
                        calendar,
                    )
                    .expect("capture adjusted OIS coupon");
                    let projection = project_overnight_coupon(OvernightCouponProjectionInput {
                        curve: forward
                            .as_ref()
                            .map_or(OvernightProjectionCurve::Discount(&discount), |curve| {
                                OvernightProjectionCurve::Forward(curve)
                            }),
                        fixings,
                        fixing_id: float.forward_curve_id.as_str(),
                        as_of,
                        accrual_start: start,
                        accrual_end: end,
                        end_is_termination_date: None,
                        day_count: float.day_count,
                        coupon_frequency: Some(float.frequency),
                        coupon_period: if float.day_count == DayCount::Act365L {
                            (period.accrual_start, period.accrual_end)
                        } else {
                            (start, end)
                        },
                        compounding: &float.compounding,
                        fixing_calendar: calendar,
                        compounded_spread: 0.0,
                        need_observation_exposures: true,
                    })
                    .expect("capture full daily OIS projection");
                    raw_observation_count += projection.observation_exposures.len();
                    json!({
                        "adjusted_accrual_start": start.to_string(),
                        "adjusted_accrual_end": end.to_string(),
                        "projection": projection_record(&projection),
                    })
                } else {
                    Value::Null
                };
                json!({
                    "accrual_start": period.accrual_start.to_string(),
                    "accrual_end": period.accrual_end.to_string(),
                    "payment_date": period.payment_date.to_string(),
                    "reset_date": period.reset_date.map(|date| date.to_string()),
                    "accrual_year_fraction": period.accrual_year_fraction,
                    "overnight": projection,
                })
            })
            .collect();
        let fixed_periods: Vec<_> = build_periods(
            swap.fixed_leg
                .schedule_params(swap.adjust_fixed_accrual_dates),
        )
        .expect("capture fixed periods")
        .into_iter()
        .filter(|period| period.payment_date > as_of)
        .map(|period| {
            json!({
                "accrual_start": period.accrual_start.to_string(),
                "accrual_end": period.accrual_end.to_string(),
                "payment_date": period.payment_date.to_string(),
                "accrual_year_fraction": period.accrual_year_fraction,
            })
        })
        .collect();
        json!({
            "fixture": {
                "instrument": swap,
                "market": market,
                "as_of": as_of.to_string(),
                "model": swap.default_model(),
                "requested_metrics": requested,
                "dimensions": {
                    "fixed_coupons": fixed_periods.len(),
                    "floating_coupons": period_records.len(),
                    "daily_observations": raw_observation_count,
                    "unique_exported_fixings": schedule.get_meta().projected_fixings.len(),
                    "fixed_payment_calendar": swap.fixed_leg.calendar_id.as_deref()
                        .unwrap_or(finstack_quant_core::dates::WEEKENDS_ONLY_ID),
                    "floating_payment_calendar": float.calendar_or_weekends(),
                    "overnight_fixing_calendar": if is_overnight {
                        float.fixing_calendar_id.as_deref().or(float.calendar_id.as_deref())
                            .or(default_rate_calendar_id(swap.notional.currency()))
                    } else { None },
                    "projection_curve_source": if forward.is_some() { "forward" } else { "discount" },
                    "dv01_bump_bp": 1.0,
                },
            },
            "output": output(),
            "diagnostics": {
                "cashflows": schedule,
                "fixed_periods": fixed_periods,
                "floating_periods": period_records,
            },
        })
    });
}

fn assert_usd(value: &Money) {
    assert!(value.amount().is_finite());
    assert_eq!(value.currency(), Currency::USD);
}

fn assert_metrics(result: &ValuationResult, requested: &[MetricId]) {
    assert_usd(&result.value);
    assert!(result.measures.values().all(|value| value.is_finite()));
    for metric in requested {
        assert!(
            result.measures.contains_key(metric),
            "missing metric {metric}"
        );
    }
}

fn bench_swap_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("swap_pv");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [2, 5, 10, 30].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                let market = create_market();
                let swap = create_swap(*tenor);
                assert_eq!(swap.default_model(), ModelKey::Discounting);
                assert_eq!(swap.float_leg.compounding, FloatingLegCompounding::Simple);
                support::bench(
                    b,
                    || {
                        swap.value(black_box(&market), black_box(as_of))
                            .expect("IRS PV")
                    },
                    |value| {
                        assert_usd(value);
                        if *tenor == 5 {
                            capture_swap(
                                "swap_pv/5Y",
                                &swap,
                                &market,
                                as_of,
                                &[],
                                || json!({"value": value}),
                            );
                        }
                    },
                );
            },
        );
    }
    group.finish();
}

/// Benchmark long-dated swaps with many periods to stress-test Kahan summation.
///
/// A 50Y monthly swap has 600 fixed + 600 float = 1200 cashflows.
/// This is where numerical precision really matters.
fn bench_swap_pv_kahan_stress(c: &mut Criterion) {
    let mut group = c.benchmark_group("swap_pv_kahan_stress");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Long-dated monthly swaps: tests Kahan summation with many periods
    for tenor in [10, 30, 50].iter() {
        let periods = tenor * 12 * 2; // fixed + float periods
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y_monthly_{}periods", tenor, periods)),
            tenor,
            |b, _| {
                let market = create_monthly_market();
                let swap = create_monthly_swap(*tenor);
                assert_eq!(swap.default_model(), ModelKey::Discounting);
                assert_eq!(swap.float_leg.reset_lag_days, 2);
                assert_eq!(swap.float_leg.compounding, FloatingLegCompounding::Simple);
                let missing_fixing = swap
                    .value(&create_market(), as_of)
                    .expect_err("monthly IRS must reject a missing historical fixing");
                assert!(missing_fixing.to_string().contains("FIXING:USD-SOFR-1M"));
                support::bench(
                    b,
                    || {
                        swap.value(black_box(&market), black_box(as_of))
                            .expect("monthly IRS PV")
                    },
                    assert_usd,
                );
            },
        );
    }
    group.finish();
}

/// Benchmark OIS swaps with daily compounding.
///
/// OIS pricing involves daily rate accumulation which is computationally
/// more expensive than simple term-rate swaps.
fn bench_ois_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("ois_pv");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [1, 2, 5, 10].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                let market = create_market();
                let swap = create_ois_swap(*tenor);
                assert_eq!(swap.default_model(), ModelKey::Discounting);
                assert_eq!(
                    swap.float_leg.compounding,
                    FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 }
                );
                assert_eq!(
                    swap.float_leg.forward_curve_id,
                    swap.float_leg.discount_curve_id
                );
                support::bench(
                    b,
                    || {
                        swap.value(black_box(&market), black_box(as_of))
                            .expect("OIS PV")
                    },
                    |value| {
                        assert_usd(value);
                        capture_swap(
                            &format!("ois_pv/{tenor}Y"),
                            &swap,
                            &market,
                            as_of,
                            &[],
                            || json!({"value": value}),
                        );
                    },
                );
            },
        );
    }
    group.finish();
}

fn bench_ois_standard_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("ois_standard_metrics");
    for tenor in [1, 2, 5, 10] {
        group.bench_function(format!("{tenor}Y"), |b| {
            let swap = create_ois_swap(tenor);
            let market = create_market();
            let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
            assert_eq!(swap.default_model(), ModelKey::Discounting);
            support::bench(
                b,
                || {
                    swap.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&OIS_METRICS),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                    .expect("standard OIS metrics")
                },
                |result| {
                    assert_metrics(result, &OIS_METRICS);
                    capture_swap(
                        &format!("ois_standard_metrics/{tenor}Y"),
                        &swap,
                        &market,
                        as_of,
                        &OIS_METRICS,
                        || json!({"value": result.value, "metrics": result.measures}),
                    );
                },
            );
        });
    }
    group.finish();
}

fn bench_ois_controls_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("ois_controls_pv");
    for control in [
        OisControl::Seasoned,
        OisControl::Lookback,
        OisControl::ObservationShift,
        OisControl::Cutoff,
        OisControl::ArithmeticSpread,
        OisControl::ExplicitForward,
        OisControl::DiscountDerivedForward,
    ] {
        group.bench_function(control.label(), |b| {
            let (swap, market, as_of) = create_ois_control(control);
            assert_eq!(swap.default_model(), ModelKey::Discounting);
            support::bench(
                b,
                || {
                    swap.value(black_box(&market), black_box(as_of))
                        .expect("OIS control PV")
                },
                |value| {
                    assert_usd(value);
                    capture_swap(
                        &format!("ois_controls_pv/{}", control.label()),
                        &swap,
                        &market,
                        as_of,
                        &[],
                        || json!({"value": value}),
                    );
                },
            );
        });
    }
    group.finish();
}

fn bench_ois_controls_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("ois_controls_metrics");
    for control in [OisControl::Seasoned, OisControl::ExplicitForward] {
        group.bench_function(control.label(), |b| {
            let (swap, market, as_of) = create_ois_control(control);
            assert_eq!(swap.default_model(), ModelKey::Discounting);
            support::bench(
                b,
                || {
                    swap.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&OIS_METRICS),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                    .expect("standard OIS control metrics")
                },
                |result| {
                    assert_metrics(result, &OIS_METRICS);
                    capture_swap(
                        &format!("ois_controls_metrics/{}", control.label()),
                        &swap,
                        &market,
                        as_of,
                        &OIS_METRICS,
                        || json!({"value": result.value, "metrics": result.measures}),
                    );
                },
            );
        });
    }
    group.finish();
}

fn assert_ois_schedule(schedule: &CashFlowSchedule) {
    assert_eq!(
        schedule.get_meta().representation,
        CashflowRepresentation::Projected
    );
    assert_eq!(schedule.get_flows().len(), 10);
    assert_eq!(
        schedule
            .get_flows()
            .iter()
            .filter(|flow| flow.get_kind() == CFKind::Floating)
            .count(),
        5
    );
    assert!(!schedule.get_meta().projected_fixings.is_empty());
    for flow in schedule.get_flows() {
        assert_usd(&flow.get_amount());
        assert!(flow.get_accrual_factor().is_finite());
        assert!(flow.get_rate().is_none_or(f64::is_finite));
    }
}

fn bench_ois_cashflow_schedule(c: &mut Criterion) {
    let mut group = c.benchmark_group("ois_cashflow_schedule");
    group.bench_function("5Y", |b| {
        let swap = create_ois_swap(5);
        let market = create_market();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        assert_eq!(swap.default_model(), ModelKey::Discounting);
        support::bench(
            b,
            || {
                swap.cashflow_schedule(black_box(&market), black_box(as_of))
                    .expect("full OIS cashflow schedule")
            },
            |schedule| {
                assert_ois_schedule(schedule);
                capture_swap(
                    "ois_cashflow_schedule/5Y",
                    &swap,
                    &market,
                    as_of,
                    &[],
                    || json!({"cashflows": schedule}),
                );
            },
        );
    });
    group.finish();
}

fn bench_overnight_projection_controls(c: &mut Criterion) {
    let mut group = c.benchmark_group("overnight_projection_controls");
    group.bench_function("1Y_compounded_spread_10bp", |b| {
        let swap = create_ois_swap(1);
        let market = create_market();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        let discount = market.get_discount("USD-OIS").expect("OIS discount curve");
        let calendar = fixing_calendar(&swap);
        let (accrual_start, accrual_end) = adjust_overnight_accrual_boundaries(
            swap.float_leg.start,
            swap.float_leg.end,
            swap.float_leg.business_day_convention,
            calendar,
        )
        .expect("valid first annual OIS coupon boundaries");
        // This is explicitly a shared-projector benchmark, not an IRS spread.
        // The IRS's spread_bp is arithmetic spread outside the daily product.
        support::bench(
            b,
            || {
                project_overnight_coupon(black_box(OvernightCouponProjectionInput {
                    curve: OvernightProjectionCurve::Discount(&discount),
                    fixings: None,
                    fixing_id: "USD-OIS",
                    as_of,
                    accrual_start,
                    accrual_end,
                    end_is_termination_date: None,
                    day_count: swap.float_leg.day_count,
                    coupon_frequency: Some(swap.float_leg.frequency),
                    coupon_period: (accrual_start, accrual_end),
                    compounding: &swap.float_leg.compounding,
                    fixing_calendar: calendar,
                    compounded_spread: 0.001,
                    need_observation_exposures: false,
                }))
                .expect("overnight projection with a compounded spread")
            },
            |projection| {
                assert!(projection.rate.is_finite());
                assert!(projection.compound_factor.is_finite());
                assert!(projection.compound_factor > 0.0);
                assert!(projection.accrual_year_fraction.is_finite());
                assert!(projection.accrual_year_fraction > 0.0);
                assert!(projection.parallel_forward_sensitivity.is_finite());
                assert!(projection.parallel_forward_second_sensitivity.is_finite());
                record::capture(
                    "overnight_projection_controls/1Y_compounded_spread_10bp",
                    || {
                        json!({
                            "fixture": {
                                "instrument": swap,
                                "market": market,
                                "as_of": as_of.to_string(),
                                "model": swap.default_model(),
                                "requested_metrics": [],
                                "projection_input": {
                                    "curve_source": "discount",
                                    "curve_id": "USD-OIS",
                                    "fixing_id": "USD-OIS",
                                    "fixings": null,
                                    "accrual_start": accrual_start.to_string(),
                                    "accrual_end": accrual_end.to_string(),
                                    "coupon_period": [accrual_start.to_string(), accrual_end.to_string()],
                                    "day_count": swap.float_leg.day_count,
                                    "coupon_frequency": swap.float_leg.frequency,
                                    "compounding": swap.float_leg.compounding,
                                    "compounded_spread": 0.001,
                                    "need_observation_exposures": false,
                                },
                                "dimensions": {
                                    "floating_coupons": 1,
                                    "daily_observations": projection.observation_exposures.len(),
                                    "overnight_fixing_calendar": default_rate_calendar_id(Currency::USD),
                                },
                            },
                            "output": projection_record(projection),
                        })
                    },
                );
            },
        );
    });
    group.finish();
}

fn bench_swap_dv01(c: &mut Criterion) {
    let mut group = c.benchmark_group("swap_dv01");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [2, 5, 10, 30].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                let market = create_market();
                let swap = create_swap(*tenor);
                assert_eq!(swap.default_model(), ModelKey::Discounting);
                support::bench(
                    b,
                    || {
                        swap.price_with_metrics(
                            black_box(&market),
                            black_box(as_of),
                            black_box(&[MetricId::Dv01]),
                            finstack_quant_valuations::instruments::PricingOptions::default(),
                        )
                        .expect("IRS DV01")
                    },
                    |result| assert_metrics(result, &[MetricId::Dv01]),
                );
            },
        );
    }
    group.finish();
}

fn bench_swap_par_rate(c: &mut Criterion) {
    let mut group = c.benchmark_group("swap_par_rate");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [2, 5, 10, 30].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                let market = create_market();
                let swap = create_swap(*tenor);
                assert_eq!(swap.default_model(), ModelKey::Discounting);
                support::bench(
                    b,
                    || {
                        swap.price_with_metrics(
                            black_box(&market),
                            black_box(as_of),
                            black_box(&[MetricId::ParRate, MetricId::Annuity]),
                            finstack_quant_valuations::instruments::PricingOptions::default(),
                        )
                        .expect("IRS par rate and annuity")
                    },
                    |result| assert_metrics(result, &[MetricId::ParRate, MetricId::Annuity]),
                );
            },
        );
    }
    group.finish();
}

/// Benchmark the combined market-standard metric request used by IRS callers.
fn swap_standard_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("swap_standard_metrics");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let metrics = [
        MetricId::PvFixed,
        MetricId::PvFloat,
        MetricId::ParRate,
        MetricId::Dv01,
    ];

    group.bench_function("5Y", |b| {
        let market = create_market();
        let swap = create_swap(5);
        assert_eq!(swap.default_model(), ModelKey::Discounting);
        support::bench(
            b,
            || {
                swap.price_with_metrics(
                    black_box(&market),
                    black_box(as_of),
                    black_box(&metrics),
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .expect("standard IRS metrics should price successfully")
            },
            |result| assert_metrics(result, &metrics),
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_swap_pv,
    bench_swap_pv_kahan_stress,
    bench_ois_pv,
    bench_ois_standard_metrics,
    bench_ois_controls_pv,
    bench_ois_controls_metrics,
    bench_ois_cashflow_schedule,
    bench_overnight_projection_controls,
    bench_swap_dv01,
    bench_swap_par_rate,
    swap_standard_metrics
);
criterion_main!(benches);
