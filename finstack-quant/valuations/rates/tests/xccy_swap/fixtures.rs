//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::fx::FxMatrix;
use finstack_quant_core::money::fx::SimpleFxProvider;
use finstack_quant_valuations_core::instruments::FloatLegSpec;
pub use finstack_quant_valuations_core::instruments::Instrument;
use rust_decimal::Decimal;
use std::sync::Arc;
use time::Month;

pub fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).unwrap(), day).unwrap()
}

pub const USD_CAL: &str = "usny";
pub const EUR_CAL: &str = "target2";

pub fn market_with_fx() -> MarketContext {
    let base = d(2025, 1, 2);

    let usd_disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots(vec![(0.0, 1.0), (1.0, 0.98), (2.0, 0.96)])
        .interp(InterpStyle::LogLinear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let eur_disc = DiscountCurve::builder("EUR-OIS")
        .base_date(base)
        .knots(vec![(0.0, 1.0), (1.0, 0.985), (2.0, 0.97)])
        .interp(InterpStyle::LogLinear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let usd_fwd = ForwardCurve::flat(
        "USD-SOFR-3M",
        base,
        0.02,
        finstack_quant_core::dates::DayCount::Act365F,
    )
    .unwrap();
    let eur_fwd = ForwardCurve::flat(
        "EUR-EURIBOR-3M",
        base,
        0.015,
        finstack_quant_core::dates::DayCount::Act365F,
    )
    .unwrap();

    let provider = {
        let p = Arc::new(SimpleFxProvider::new());
        p.set_quotes(&[(Currency::EUR, Currency::USD, 1.10)])
            .expect("valid rate");
        p
    };
    let fx = FxMatrix::new(provider, None);

    MarketContext::new()
        .insert(usd_disc)
        .insert(eur_disc)
        .insert(usd_fwd)
        .insert(eur_fwd)
        .insert_fx(fx)
}

pub fn market_without_fx() -> MarketContext {
    let base = d(2025, 1, 2);

    let usd_disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots(vec![(0.0, 1.0), (1.0, 0.98), (2.0, 0.96)])
        .interp(InterpStyle::LogLinear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let eur_disc = DiscountCurve::builder("EUR-OIS")
        .base_date(base)
        .knots(vec![(0.0, 1.0), (1.0, 0.985), (2.0, 0.97)])
        .interp(InterpStyle::LogLinear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let usd_fwd = ForwardCurve::flat(
        "USD-SOFR-3M",
        base,
        0.02,
        finstack_quant_core::dates::DayCount::Act365F,
    )
    .unwrap();
    let eur_fwd = ForwardCurve::flat(
        "EUR-EURIBOR-3M",
        base,
        0.015,
        finstack_quant_core::dates::DayCount::Act365F,
    )
    .unwrap();

    MarketContext::new()
        .insert(usd_disc)
        .insert(eur_disc)
        .insert(usd_fwd)
        .insert(eur_fwd)
}

pub fn leg_usd_receive(
    start: Date,
    end: Date,
) -> finstack_quant_valuations_rates::xccy_swap::XccySwapLeg {
    finstack_quant_valuations_rates::xccy_swap::XccySwapLeg {
        notional: finstack_quant_core::money::Money::new(1_000_000.0, Currency::USD)
            .expect("valid money fixture"),
        side: finstack_quant_valuations_core::instruments::PayReceive::Receive,
        leg: FloatLegSpec {
            forward_curve_id: finstack_quant_core::types::CurveId::new("USD-SOFR-3M"),
            discount_curve_id: finstack_quant_core::types::CurveId::new("USD-OIS"),
            start,
            end,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            calendar_id: Some(USD_CAL.into()),
            reset_lag_days: 0,
            compounding: Default::default(),
            fixing_calendar_id: None,
            end_of_month: false,
        },
    }
}

pub fn leg_eur_pay(
    start: Date,
    end: Date,
) -> finstack_quant_valuations_rates::xccy_swap::XccySwapLeg {
    finstack_quant_valuations_rates::xccy_swap::XccySwapLeg {
        notional: finstack_quant_core::money::Money::new(900_000.0, Currency::EUR)
            .expect("valid money fixture"),
        side: finstack_quant_valuations_core::instruments::PayReceive::Pay,
        leg: FloatLegSpec {
            forward_curve_id: finstack_quant_core::types::CurveId::new("EUR-EURIBOR-3M"),
            discount_curve_id: finstack_quant_core::types::CurveId::new("EUR-OIS"),
            start,
            end,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            calendar_id: Some(EUR_CAL.into()),
            reset_lag_days: 0,
            compounding: Default::default(),
            fixing_calendar_id: None,
            end_of_month: false,
        },
    }
}

/// Market context with curves extending to 15 years for long-dated swap tests.
pub fn market_with_extended_curves() -> MarketContext {
    let base = d(2025, 1, 2);

    // Extended discount curves out to 15 years
    let usd_disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots(vec![
            (0.0, 1.0),
            (1.0, 0.98),
            (2.0, 0.96),
            (5.0, 0.90),
            (10.0, 0.80),
            (15.0, 0.70),
        ])
        .interp(InterpStyle::LogLinear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let eur_disc = DiscountCurve::builder("EUR-OIS")
        .base_date(base)
        .knots(vec![
            (0.0, 1.0),
            (1.0, 0.985),
            (2.0, 0.97),
            (5.0, 0.92),
            (10.0, 0.85),
            (15.0, 0.78),
        ])
        .interp(InterpStyle::LogLinear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    // Extended forward curves
    let usd_fwd = ForwardCurve::builder("USD-SOFR-3M")
        .base_date(base)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.3125, 0.993720967907132),
            (0.625, 0.987384933040048),
            (0.9375, 0.980993492353815),
            (1.25, 0.974548248723029),
            (1.5625, 0.96805081027945),
            (1.875, 0.961502789751398),
            (2.1875, 0.954905803805283),
            (2.5, 0.948261472389648),
            (2.8125, 0.941571418082069),
            (3.125, 0.934837265439304),
            (3.4375, 0.928060640351021),
            (3.75, 0.921243169397475),
            (4.0625, 0.914386479211468),
            (4.375, 0.907492195844948),
            (4.6875, 0.900561944140562),
            (5.0, 0.893597347108516),
            (5.3125, 0.886600025309047),
            (5.625, 0.879571596240839),
            (5.9375, 0.87251367373568),
            (6.25, 0.865427867359675),
            (6.5625, 0.858315781821313),
            (6.875, 0.851179016386663),
            (7.1875, 0.84401916430201),
            (7.5, 0.836837812224172),
            (7.8125, 0.829636539658803),
            (8.125, 0.822416918406909),
            (8.4375, 0.815180512019858),
            (8.75, 0.807928875263107),
            (9.0625, 0.800663553588897),
            (9.375, 0.79338608261814),
            (9.6875, 0.786097987631719),
            (10.0, 0.778800783071405),
            (10.3125, 0.771518574802765),
            (10.625, 0.764274604057694),
            (10.9375, 0.757069075082385),
            (11.25, 0.749902185717813),
            (11.5625, 0.74277412742936),
            (11.875, 0.735685085337393),
            (12.1875, 0.728635238248791),
            (12.5, 0.72162475868939),
            (12.8125, 0.714653812937354),
            (13.125, 0.707722561057421),
            (13.4375, 0.700831156936041),
            (13.75, 0.69397974831737),
            (14.0625, 0.687168476840105),
            (14.375, 0.680397478075151),
            (14.6875, 0.673666881564094),
            (15.0, 0.666976810858474),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let eur_fwd = ForwardCurve::builder("EUR-EURIBOR-3M")
        .base_date(base)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.3125, 0.995274870589815),
            (0.625, 0.990475337197317),
            (0.9375, 0.985602693725241),
            (1.25, 0.980658249139539),
            (1.5625, 0.975643326885253),
            (1.875, 0.970559264298252),
            (2.1875, 0.965407412013174),
            (2.5, 0.960189133367952),
            (2.8125, 0.954905803805283),
            (3.125, 0.949558810271416),
            (3.4375, 0.944149550612607),
            (3.75, 0.938679432969625),
            (4.0625, 0.933149875170665),
            (4.375, 0.92756230412303),
            (4.6875, 0.921918155203946),
            (5.0, 0.916218871650878),
            (5.3125, 0.91046590395169),
            (5.625, 0.90466070923503),
            (5.9375, 0.898804750661266),
            (6.25, 0.892899496814353),
            (6.5625, 0.886946421094956),
            (6.875, 0.880947001115197),
            (7.1875, 0.874902718095351),
            (7.5, 0.868815056262843),
            (7.8125, 0.86268550225387),
            (8.125, 0.856515544517991),
            (8.4375, 0.850306672726003),
            (8.75, 0.844060377181432),
            (9.0625, 0.83777814823595),
            (9.375, 0.831461475709036),
            (9.6875, 0.825111848312186),
            (10.0, 0.818730753077982),
            (10.3125, 0.812335540567882),
            (10.625, 0.805943057285814),
            (10.9375, 0.799554027904529),
            (11.25, 0.793169170768443),
            (11.5625, 0.786789197776123),
            (11.875, 0.780414814265582),
            (12.1875, 0.774046718902403),
            (12.5, 0.767685603570725),
            (12.8125, 0.761332153267083),
            (13.125, 0.754987045997131),
            (13.4375, 0.748650952675256),
            (13.75, 0.742324537027097),
            (14.0625, 0.73600845549497),
            (14.375, 0.72970335714622),
            (14.6875, 0.723409883584501),
            (15.0, 0.717128668863986),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let provider = {
        let p = Arc::new(SimpleFxProvider::new());
        p.set_quotes(&[(Currency::EUR, Currency::USD, 1.10)])
            .expect("valid rate");
        p
    };
    let fx = FxMatrix::new(provider, None);

    MarketContext::new()
        .insert(usd_disc)
        .insert(eur_disc)
        .insert(usd_fwd)
        .insert(eur_fwd)
        .insert_fx(fx)
}
