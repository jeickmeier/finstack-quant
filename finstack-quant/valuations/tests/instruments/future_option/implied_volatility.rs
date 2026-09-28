//! Futures options read their flat volatility from
//! `instrument_pricing_overrides.market_quotes.implied_volatility`.
//!
//! Every wrapper is priced against Black-76 and Bachelier closed forms coded
//! here. Both sides evaluate the same closed form in f64, so they agree to
//! rounding; 1e-10 relative leaves only floating-point residue.

use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::{norm_cdf, norm_pdf};
use finstack_quant_valuations::instruments::{
    CommodityFutureOption, EquityFutureOption, ExerciseStyle, FutureOptionModel,
    FutureOptionPremiumStyle, FutureOptionSettlement, FutureOptionTerms, FxFutureOption,
    Instrument, InstrumentPricingOverrides, InterestRateFutureOption, VolatilityIndexFutureOption,
};
use time::macros::date;

const RATE: f64 = 0.03;
const TOLERANCE: f64 = 1e-10;

fn as_of() -> Date {
    date!(2026 - 01 - 02)
}

fn market(curve_id: &str) -> MarketContext {
    MarketContext::new().insert(
        DiscountCurve::builder(curve_id)
            .base_date(as_of())
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (2.0, (-2.0 * RATE).exp())])
            .build()
            .expect("discount curve"),
    )
}

/// European, premium-paid, cash-settled call terms shared by every wrapper.
fn european_terms(terms: &FutureOptionTerms, model: FutureOptionModel) -> FutureOptionTerms {
    FutureOptionTerms {
        exercise_style: ExerciseStyle::European,
        premium_style: FutureOptionPremiumStyle::PremiumPaid,
        settlement: FutureOptionSettlement::Cash {
            payment_date: terms.expiry,
        },
        exercise: None,
        model,
        ..terms.clone()
    }
}

/// Closed-form PV coded independently of the pricer.
fn closed_form(terms: &FutureOptionTerms, market: &MarketContext, sigma: f64) -> f64 {
    let t = terms
        .day_count
        .year_fraction(as_of(), terms.expiry, DayCountContext::default())
        .expect("year fraction");
    let df = market
        .get_discount(&terms.discount_curve_id)
        .expect("curve")
        .df_between_dates(as_of(), terms.expiry)
        .expect("df");
    let (f, k) = (terms.futures_price, terms.strike);
    let call = match terms.model {
        FutureOptionModel::Black76 => {
            let std_dev = sigma * t.sqrt();
            let d1 = ((f / k).ln() + 0.5 * std_dev * std_dev) / std_dev;
            f * norm_cdf(d1) - k * norm_cdf(d1 - std_dev)
        }
        FutureOptionModel::Normal => {
            let std_dev = sigma * t.sqrt();
            let d = (f - k) / std_dev;
            (f - k) * norm_cdf(d) + std_dev * norm_pdf(d)
        }
    };
    df * call * terms.contracts * terms.multiplier
}

macro_rules! check_wrapper {
    ($ty:ty) => {{
        let example = <$ty>::example().expect("example");
        let market = market(example.terms.discount_curve_id.as_str());
        for (model, sigma) in [
            (FutureOptionModel::Black76, 0.25),
            (FutureOptionModel::Normal, 6.0),
        ] {
            let mut option = example.clone();
            option.terms = european_terms(&example.terms, model);
            option.instrument_pricing_overrides =
                InstrumentPricingOverrides::default().with_implied_volatility(sigma);
            let pv = option.value(&market, as_of()).expect("pv").amount();
            let expected = closed_form(&option.terms, &market, sigma);
            assert!(
                (pv - expected).abs() <= TOLERANCE * expected.abs().max(1.0),
                "{} {:?}: pv {pv} vs closed form {expected}",
                stringify!($ty),
                model
            );

            option.instrument_pricing_overrides = InstrumentPricingOverrides::default();
            let error = option
                .value(&market, as_of())
                .expect_err("a live option needs market_quotes.implied_volatility");
            assert!(
                error
                    .to_string()
                    .contains("instrument_pricing_overrides.market_quotes.implied_volatility"),
                "{}: {error}",
                stringify!($ty)
            );
        }
    }};
}

#[test]
fn every_future_option_prices_from_market_quote_implied_volatility() {
    check_wrapper!(InterestRateFutureOption);
    check_wrapper!(EquityFutureOption);
    check_wrapper!(FxFutureOption);
    check_wrapper!(CommodityFutureOption);
    check_wrapper!(VolatilityIndexFutureOption);
}
