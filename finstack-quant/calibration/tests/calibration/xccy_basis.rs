//! Public-engine coverage for foreign-curve conventions and basis extraction.

use crate::calibration::calibration_support as cal_utils;
use finstack_quant_calibration::api::schema::{StepParams, XccyBasisParams};
use finstack_quant_calibration::quotes::ids::{Pillar, QuoteId};
use finstack_quant_calibration::quotes::market_quote::MarketQuote;
use finstack_quant_calibration::quotes::rates::RateQuote;
use finstack_quant_calibration::{CalibrationConfig, CalibrationMethod, RatesStepConventions};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use time::macros::date;

fn params(day_count: DayCount) -> StepParams {
    StepParams::XccyBasis(XccyBasisParams {
        curve_id: "EUR-OIS".into(),
        currency: Currency::EUR,
        base_date: date!(2025 - 01 - 02),
        fx_spot: 1.1,
        domestic_discount_id: "USD-OIS".into(),
        method: CalibrationMethod::Bootstrap,
        interpolation: InterpStyle::LogLinear,
        extrapolation: ExtrapolationPolicy::FlatZero,
        conventions: RatesStepConventions {
            curve_day_count: Some(day_count),
            ois_compounding: None,
        },
        basis_spread_curve_id: Some("EUR-USD-BASIS".into()),
    })
}

fn market(day_count: DayCount, base: Date) -> MarketContext {
    let denominator = if day_count == DayCount::Act360 {
        360.0
    } else {
        365.0
    };
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .day_count(day_count)
        .knots([
            (0.0, 1.0),
            (10.0, (-0.02_f64 * denominator / 365.0 * 10.0).exp()),
        ])
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::FlatZero)
        .build()
        .expect("domestic discount curve");
    MarketContext::new().insert(discount)
}

fn deposit(rate: f64) -> MarketQuote {
    MarketQuote::Rates(RateQuote::Deposit {
        id: QuoteId::new("EUR-DEP-1Y"),
        index: "EUR-ESTR-OIS".into(),
        pillar: Pillar::Tenor("1Y".parse().expect("tenor")),
        rate,
    })
}

#[test]
fn xccy_basis_builds_signed_spread_output_with_default_discount_interpolation() {
    let base = date!(2025 - 01 - 02);
    for (rate, expected_sign) in [(0.01, -1.0), (0.03, 1.0)] {
        let (context, report) = cal_utils::execute_step(
            &params(DayCount::Act365F),
            &[deposit(rate)],
            &market(DayCount::Act365F, base),
            &CalibrationConfig::default(),
        )
        .expect("zero and negative basis spreads must build");
        assert!(report.success);
        let spread = context
            .get_basis_spread("EUR-USD-BASIS")
            .expect("basis curve");
        assert_eq!(spread.interp_style(), InterpStyle::Linear);
        assert_eq!(spread.spread(0.0), 0.0);
        assert!(spread.spread(1.0) * expected_sign > 0.0);
    }
}

#[test]
fn xccy_basis_preserves_foreign_and_spread_curve_day_counts() {
    let base = date!(2025 - 01 - 02);
    let (context, report) = cal_utils::execute_step(
        &params(DayCount::Act360),
        &[deposit(0.03)],
        &market(DayCount::Act365F, base),
        &CalibrationConfig::default(),
    )
    .expect("Act360 foreign curve");
    assert!(report.success);
    let foreign = context.get_discount("EUR-OIS").expect("foreign curve");
    let spread = context
        .get_basis_spread("EUR-USD-BASIS")
        .expect("basis curve");
    assert_eq!(foreign.day_count(), DayCount::Act360);
    assert_eq!(spread.day_count(), DayCount::Act360);
    // ESTR deposits start two TARGET business days after 2 January and mature
    // one year later, so the pillar date is 6 January 2026.
    let maturity = date!(2026 - 01 - 06);
    let time = (maturity - base).whole_days() as f64 / 360.0;
    assert!((foreign.knots()[1] - time).abs() < 1e-12);
    assert!(
        (foreign.df_between_dates(base, maturity).expect("dated DF") - foreign.dfs()[1]).abs()
            < 1e-12
    );
    let expected = (context
        .get_discount("USD-OIS")
        .expect("domestic")
        .df_between_dates(base, maturity)
        .expect("domestic dated DF")
        / foreign.dfs()[1])
        .ln()
        / time;
    assert!((spread.spread(time) - expected).abs() < 1e-12);
}

#[test]
fn xccy_basis_is_invariant_to_domestic_curve_clock_and_base() {
    let base = date!(2025 - 01 - 02);
    let mut spreads = Vec::new();
    for domestic_day_count in [DayCount::Act365F, DayCount::Act360] {
        for domestic_base in [base, date!(2024 - 10 - 01)] {
            let (context, report) = cal_utils::execute_step(
                &params(DayCount::Act365F),
                &[deposit(0.03)],
                &market(domestic_day_count, domestic_base),
                &CalibrationConfig::default(),
            )
            .expect("economically equivalent domestic curve");
            assert!(report.success);
            spreads.push(
                context
                    .get_basis_spread("EUR-USD-BASIS")
                    .expect("basis")
                    .spread(1.0),
            );
        }
    }
    assert!(spreads
        .iter()
        .all(|spread| (spread - spreads[0]).abs() < 1e-12));
}

#[test]
fn xccy_basis_prices_ordinary_single_curve_swap_quotes() {
    let base = date!(2025 - 01 - 02);
    let quote = MarketQuote::Rates(RateQuote::Swap {
        id: QuoteId::new("EUR-SWAP-1Y"),
        index: "EUR-ESTR-OIS".into(),
        pillar: Pillar::Tenor("1Y".parse().expect("tenor")),
        rate: 0.03,
        spread_decimal: None,
    });
    let (_, report) = cal_utils::execute_step(
        &params(DayCount::Act365F),
        &[quote],
        &market(DayCount::Act365F, base),
        &CalibrationConfig::default(),
    )
    .expect("swap must have a projection curve role");
    assert!(report.success);
    let residual_ratio = report.residuals["step-0:EUR-SWAP-1Y:tolerance_ratio"];
    assert!(residual_ratio.is_finite());
    assert!(residual_ratio.abs() < 1.0);
}

#[test]
fn xccy_basis_rejects_unimplemented_global_solve() {
    let mut step = params(DayCount::Act365F);
    if let StepParams::XccyBasis(p) = &mut step {
        p.method = CalibrationMethod::GlobalSolve {
            use_analytical_jacobian: false,
        };
    }
    let error = cal_utils::execute_step(
        &step,
        &[deposit(0.03)],
        &market(DayCount::Act365F, date!(2025 - 01 - 02)),
        &CalibrationConfig::default(),
    )
    .expect_err("unsupported method must not silently bootstrap");
    assert!(error.to_string().contains("supports only Bootstrap"));
}
