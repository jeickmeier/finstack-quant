//! Fixed public EquityOption fixture shared by CCM context-bump consumers.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};
use finstack_quant_models::types::{ExerciseStyle, OptionType};
use finstack_quant_valuations::instruments::SettlementType;
use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
use finstack_quant_valuations::instruments::pricing_overrides::MetricPricingOverrides;
use time::{Duration, Month};

/// Valuation date for the fixed 365-day ACT/365F contract.
pub fn as_of() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).expect("valid CCM date")
}

/// Construct a surface-backed European call with explicit FD bump conventions.
pub fn option() -> EquityOption {
    EquityOption::builder()
        .id(InstrumentId::new("CCM-CONTEXT-CALL"))
        .underlying_ticker("CCM-EQUITY".to_string())
        .strike(100.0)
        .option_type(OptionType::Call)
        .exercise_style(ExerciseStyle::European)
        .expiry(as_of() + Duration::days(365))
        .quantity(1.0)
        .currency(Currency::USD)
        .day_count(DayCount::Act365F)
        .settlement(SettlementType::Cash)
        .discount_curve_id(CurveId::new("CCM-USD-OIS"))
        .spot_id("SPX-SPOT".into())
        .vol_surface_id(CurveId::new("CCM-EQ-VOL"))
        .div_yield_id_opt(Some(PriceId::new("CCM-DIV")))
        .metric_pricing_overrides(
            MetricPricingOverrides::default()
                .with_spot_bump_decimal(0.01)
                .with_vol_bump_decimal(0.01)
                .with_rate_bump_bp(1.0),
        )
        .attributes(Default::default())
        .build()
        .expect("valid CCM surface-backed call")
}

/// Construct one used curve and a fixed number of unrelated 40-knot curves.
///
/// # Arguments
///
/// * `unrelated_curves` - Number of unused discount curves; the total is one
///   larger because the option reads `CCM-USD-OIS`. Target-store sizes stay fixed.
pub fn market(unrelated_curves: usize) -> MarketContext {
    let knots: Vec<_> = (0..40)
        .map(|i| {
            let time = i as f64 * 0.5;
            (time, (-0.05 * time).exp())
        })
        .collect();
    let make_curve = |id: String| {
        DiscountCurve::builder(id)
            .base_date(as_of())
            .knots(knots.clone())
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("valid CCM curve")
    };
    let mut market = MarketContext::new().insert(make_curve("CCM-USD-OIS".into()));
    for index in 0..unrelated_curves {
        market.insert_mut(make_curve(format!("CCM-UNUSED-{index}")));
    }
    market.insert_surface_mut(
        VolSurface::from_grid(
            "CCM-EQ-VOL",
            &[0.25, 1.0, 2.0],
            &[80.0, 100.0, 120.0],
            &[0.20; 9],
            Default::default(),
            None,
        )
        .expect("valid CCM volatility surface"),
    );
    market.insert_price_mut(
        "SPX-SPOT",
        MarketScalar::Price(Money::new(100.0, Currency::USD).expect("valid spot")),
    );
    market.insert_price_mut(
        "CCM-DIV",
        MarketScalar::unitless(0.02).expect("valid yield"),
    );
    assert_eq!(market.curve_ids().len(), unrelated_curves + 1);
    market
}

/// Independent closed-form endpoint value for the fixed one-year contract.
///
/// # Arguments
///
/// * `spot` - Positive spot in USD per share at the FD endpoint.
/// * `volatility` - Annualized lognormal decimal volatility at the FD endpoint.
pub fn reference_price(spot: f64, volatility: f64) -> f64 {
    finstack_quant_models::closed_form::bs_price(
        spot,
        100.0,
        0.05,
        0.02,
        volatility,
        1.0,
        OptionType::Call,
    )
    .expect("valid analytic CCM endpoint")
}
