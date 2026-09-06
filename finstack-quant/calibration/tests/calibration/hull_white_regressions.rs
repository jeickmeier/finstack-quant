use finstack_quant_calibration::api::{
    engine, market_datum::MarketDatum, prior_market::PriorMarketObject, schema::*,
};
use finstack_quant_calibration::hull_white::SwapFrequency;
use finstack_quant_calibration::quotes::{ids::QuoteId, vol::VolQuote};
use finstack_quant_core::{
    currency::Currency,
    dates::{Date, DayCount},
    market_data::{
        scalars::MarketScalar,
        surfaces::VolQuoteType,
        term_structures::{DiscountCurve, ForwardCurve},
    },
    math::interp::InterpStyle,
};

fn cap_envelope(prior_market: Vec<PriorMarketObject>, dual_curve: bool) -> CalibrationEnvelope {
    let base_date = Date::from_ordinal_date(2025, 1).expect("base date");
    CalibrationEnvelope::new(
        CalibrationPlan {
            id: "cap-test".into(),
            description: None,
            settings: Default::default(),
            quote_sets: [("caps".into(), vec![QuoteId::new("cap")])]
                .into_iter()
                .collect(),
            steps: vec![CalibrationStep {
                id: "hw".into(),
                quote_set: "caps".into(),
                params: StepParams::CapFloorHullWhite(CapFloorHullWhiteStepParams {
                    discount_curve_id: "D".into(),
                    forward_curve_id: if dual_curve { "F" } else { "D" }.into(),
                    currency: Currency::USD,
                    base_date,
                    fixed_kappa: Some(0.0342),
                    initial_kappa: None,
                    initial_sigma: None,
                    payment_frequency: SwapFrequency::Quarterly,
                    volatility_mode: HullWhiteVolatilityMode::Scalar,
                }),
            }],
        },
        vec![MarketDatum::VolQuote(VolQuote::CapFloorVol {
            id: QuoteId::new("cap"),
            expiry: Date::from_ordinal_date(2030, 1).expect("expiry"),
            strike: 0.0365,
            vol: 0.01,
            quote_type: VolQuoteType::Normal,
            is_cap: true,
        })],
        prior_market,
    )
}

// The same dated discount factors, represented on different curve clocks.
fn discount(dc: DayCount, shift_years: u32) -> DiscountCurve {
    let base = Date::from_ordinal_date(2025, 1).expect("base date")
        - time::Duration::days(i64::from(shift_years) * 365);
    let scale = if dc == DayCount::Act360 {
        365.0 / 360.0
    } else {
        1.0
    };
    let shift = f64::from(shift_years);
    let log_df = |t: f64| -0.03 * t - 0.001 * t * t;
    let mut knots = vec![(0.0, 1.0)];
    for t in [0.0, 5.0, 10.0] {
        if t + shift > 0.0 {
            knots.push(((t + shift) * scale, (log_df(t) - log_df(-shift)).exp()));
        }
    }
    DiscountCurve::builder("D")
        .base_date(base)
        .day_count(dc)
        .knots(knots)
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("discount")
}

fn projection(dc: DayCount, shift_years: u32) -> ForwardCurve {
    let base = Date::from_ordinal_date(2025, 1).expect("base date")
        - time::Duration::days(i64::from(shift_years) * 365);
    let scale = if dc == DayCount::Act360 {
        365.0 / 360.0
    } else {
        1.0
    };
    let tenor = 0.25 * scale;
    let rate = ((0.04_f64 * 0.25).exp() - 1.0) / tenor;
    ForwardCurve::builder("F", tenor)
        .base_date(base)
        .day_count(dc)
        .knots([(0.0, rate), (12.0 * scale, rate)])
        .build()
        .expect("projection")
}

fn fitted_sigma(envelope: &CalibrationEnvelope) -> f64 {
    let result = engine::execute(envelope).expect("calibration");
    assert!(result.result.report.success);
    let MarketScalar::Unitless(sigma) = result.result.final_market.prices["D_CAPFLOOR_HW1F_SIGMA"]
    else {
        unreachable!("sigma is unitless");
    };
    sigma
}

#[test]
fn cap_floor_fit_is_invariant_to_discount_and_projection_clocks() {
    let clocks = [DayCount::Act365F, DayCount::Act360];
    let shifts = [0, 1];

    let mut single_curve = None;
    for dc in clocks {
        for shift in shifts {
            let sigma = fitted_sigma(&cap_envelope(
                vec![PriorMarketObject::DiscountCurve(discount(dc, shift))],
                false,
            ));
            let expected = *single_curve.get_or_insert(sigma);
            assert!(
                (sigma - expected).abs() < 1e-9,
                "discount={dc:?}/{shift}: {sigma} != {expected}"
            );
        }
    }

    let mut dual_curve = None;
    for dc in clocks {
        for shift in shifts {
            for forward_dc in clocks {
                for forward_shift in shifts {
                    let sigma = fitted_sigma(&cap_envelope(
                        vec![
                            PriorMarketObject::DiscountCurve(discount(dc, shift)),
                            PriorMarketObject::ForwardCurve(projection(forward_dc, forward_shift)),
                        ],
                        true,
                    ));
                    let expected = *dual_curve.get_or_insert(sigma);
                    assert!(
                        (sigma - expected).abs() < 1e-9,
                        "discount={dc:?}/{shift} forward={forward_dc:?}/{forward_shift}: {sigma} != {expected}"
                    );
                }
            }
        }
    }
}

#[test]
fn cap_floor_plan_rejects_conflicting_quotes_in_any_order() {
    for vols in [[0.005, 0.015, 0.010], [0.005, 0.010, 0.015]] {
        let mut env = cap_envelope(
            vec![PriorMarketObject::DiscountCurve(
                DiscountCurve::builder("D")
                    .base_date(Date::from_ordinal_date(2025, 1).expect("base date"))
                    .knots([(0.0, 1.0), (10.0, (-0.3_f64).exp())])
                    .interp(InterpStyle::LogLinear)
                    .build()
                    .expect("flat curve"),
            )],
            false,
        );
        let maturity = 1826.0 / 365.0;
        let accrual = maturity / 20.0;
        let strike = ((0.03_f64 * accrual).exp() - 1.0) / accrual;
        env.market_data = vols
            .into_iter()
            .enumerate()
            .map(|(index, vol)| {
                MarketDatum::VolQuote(VolQuote::CapFloorVol {
                    id: QuoteId::new(format!("cap-{index}")),
                    expiry: Date::from_ordinal_date(2030, 1).expect("expiry"),
                    strike,
                    vol,
                    quote_type: VolQuoteType::Normal,
                    is_cap: true,
                })
            })
            .collect();
        env.plan.quote_sets.insert(
            "caps".into(),
            (0..3).map(|i| QuoteId::new(format!("cap-{i}"))).collect(),
        );
        assert!(
            engine::execute(&env).is_err(),
            "strict acceptance must reject conflicting quotes"
        );
        env.plan.settings.fail_on_bad_fit = false;
        let result = engine::execute(&env).expect("diagnostic result");
        assert!(!result.result.report.success);
        assert_eq!(result.result.step_reports["hw"].residuals.len(), 3);
    }
}
