//! Smoke tests for Calibration v2 -- exercises the v2 engine on simple USD setups.
//!
//! Not a parity test against external references. See `tests/golden/calibration/`
//! for external-reference goldens.

use crate::finstack_test_utils::calibration as cal_utils;
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::math::interp::ExtrapolationPolicy;
use finstack_core::types::IndexId;
use finstack_core::HashMap;
use finstack_valuations::calibration::api::engine;
use finstack_valuations::calibration::api::market_datum::MarketDatum;
use finstack_valuations::calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, DiscountCurveParams, ForwardCurveParams,
    StepParams,
};
use finstack_valuations::calibration::{CalibrationConfig, CalibrationMethod};
use finstack_valuations::market::quotes::ids::{Pillar, QuoteId};
use finstack_valuations::market::quotes::market_quote::MarketQuote;
use finstack_valuations::market::quotes::rates::RateQuote;
use time::Month;

use super::tolerances;

#[test]
fn test_v2_simple_usd_calibration() {
    let base_date = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let currency = Currency::USD;

    // 1. Create Quotes
    let mut quotes = Vec::new();

    // Deposit (Discount) - market standard: tenor-based from spot, not an absolute T+1 date.
    quotes.push(MarketQuote::Rates(RateQuote::Deposit {
        id: QuoteId::new("DEP-1M"),
        index: IndexId::new("USD-Deposit"),
        pillar: Pillar::Tenor(Tenor::parse("1M").unwrap()),
        rate: 0.05,
    }));

    // OIS Swaps (Discount) - market standard: use >= 1Y for OIS par swaps.
    let tenors = vec![("1Y", 0.0525), ("2Y", 0.0535), ("5Y", 0.0540)];
    for (tenor, rate) in tenors {
        quotes.push(MarketQuote::Rates(RateQuote::Swap {
            id: QuoteId::new(format!("SWAP-{tenor}")),
            index: IndexId::new("USD-OIS"),
            pillar: Pillar::Tenor(Tenor::parse(tenor).unwrap()),
            rate,
            spread_decimal: None,
        }));
    }

    // Forward Quotes (3M FRAs) - ensure start/end are strictly after base_date.
    let fwd_quotes = vec![
        MarketQuote::Rates(RateQuote::Fra {
            id: QuoteId::new("FRA-1"), // Simplified ID
            index: IndexId::new("USD-LIBOR-3M"),
            start: Pillar::Date(base_date + time::Duration::days(90)),
            end: Pillar::Date(base_date + time::Duration::days(180)),
            rate: 0.0530,
        }),
        MarketQuote::Rates(RateQuote::Fra {
            id: QuoteId::new("FRA-2"),
            index: IndexId::new("USD-LIBOR-3M"),
            start: Pillar::Date(base_date + time::Duration::days(180)),
            end: Pillar::Date(base_date + time::Duration::days(270)),
            rate: 0.0540,
        }),
    ];

    let mut market_data: Vec<MarketDatum> = Vec::new();
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("usd_ois".to_string(), cal_utils::quote_set_ids(&quotes));
    quote_sets.insert("usd_3m".to_string(), cal_utils::quote_set_ids(&fwd_quotes));
    cal_utils::extend_market_data(&mut market_data, &quotes);
    cal_utils::extend_market_data(&mut market_data, &fwd_quotes);

    // 2. Build Plan
    let plan = CalibrationPlan {
        id: "test_plan".to_string(),
        description: None,
        quote_sets,
        settings: CalibrationConfig {
            use_parallel: true,
            solver: finstack_valuations::calibration::SolverConfig::brent_default()
                .with_tolerance(1e-12)
                .with_max_iterations(250),
            ..Default::default()
        },
        steps: vec![
            CalibrationStep {
                id: "step_1".to_string(),
                quote_set: "usd_ois".to_string(),
                params: StepParams::Discount(DiscountCurveParams {
                    curve_id: "USD-OIS".into(),
                    currency,
                    base_date,
                    method: CalibrationMethod::Bootstrap,
                    interpolation: Default::default(),
                    extrapolation: ExtrapolationPolicy::FlatForward,
                    pricing_discount_id: None,
                    pricing_forward_id: None,
                    conventions: Default::default(),
                }),
            },
            CalibrationStep {
                id: "step_2".to_string(),
                quote_set: "usd_3m".to_string(),
                params: StepParams::Forward(ForwardCurveParams {
                    curve_id: "USD-3M".into(),
                    currency,
                    base_date,
                    tenor_years: 0.25,
                    discount_curve_id: "USD-OIS".into(),
                    method: CalibrationMethod::Bootstrap,
                    interpolation: Default::default(),
                    conventions: Default::default(),
                }),
            },
        ],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: "finstack.calibration/2".to_string(),
        plan,
        market_data,
        prior_market: Vec::new(),
    };

    // 3. Execute
    let result = engine::execute(&envelope).expect("Calibration failed");

    // Forward rate checks might need adjustment if rate changes due to different date
    // But since market data is synthetic/flat-ish, it should be robust.

    // 4. Verify
    assert!(result.result.report.success);

    let context =
        MarketContext::try_from(result.result.final_market).expect("Failed to restore context");

    // Check Discount Curve
    let discount = context
        .get_discount("USD-OIS")
        .expect("Discount curve missing");
    let df_1y = discount.df(1.0);
    assert!(df_1y < 1.0 && df_1y > 0.9, "Reasonable DF");

    // Check Forward Curve
    let forward = context
        .get_forward("USD-3M")
        .expect("Forward curve missing");
    let fwd_0 = forward.rate(0.0);
    assert!(
        (fwd_0 - 0.0530).abs() < tolerances::FWD_RATE_ABS_TOL,
        "Spot forward should match first FRA"
    );
}
